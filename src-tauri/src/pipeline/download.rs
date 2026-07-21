use super::{douyin, logs};
use crate::error::{AppError, AppResult};
use crate::sidecar::ResolvedBinary;
use reqwest::blocking::Client;
use reqwest::header::{CONTENT_LENGTH, CONTENT_TYPE, REFERER, USER_AGENT};
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub type DownloadProgressCallback = Arc<Mutex<dyn FnMut(f32) + Send>>;

#[derive(Debug, Clone)]
pub struct DownloadResult {
    pub media_files: Vec<String>,
    pub tool_path: String,
    pub tool_version: Option<String>,
    /// Optional title resolved from a platform-specific path (e.g. Douyin desc).
    pub resolved_title: Option<String>,
}

/// Best-effort download entry: Douyin share text/links use the share-page
/// resolver; everything else falls back to yt-dlp.
pub fn run_download(
    job_dir: &Path,
    raw_input: &str,
    yt_dlp: &ResolvedBinary,
    on_progress: Option<DownloadProgressCallback>,
) -> AppResult<DownloadResult> {
    if douyin::looks_like_douyin_input(raw_input) {
        match run_douyin_download(job_dir, raw_input, on_progress.clone()) {
            Ok(result) => return Ok(result),
            Err(error) => {
                logs::append_log(
                    job_dir,
                    "download",
                    &format!("douyin resolver failed, falling back to yt-dlp: {error}\n"),
                )?;
                // Continue to yt-dlp with the extracted short/full URL when possible.
                let fallback_url = douyin::extract_douyin_url(raw_input)
                    .unwrap_or_else(|| raw_input.trim().to_string());
                return run_yt_dlp_download(job_dir, &fallback_url, yt_dlp, on_progress);
            }
        }
    }

    run_yt_dlp_download(job_dir, raw_input.trim(), yt_dlp, on_progress)
}

pub fn run_douyin_download(
    job_dir: &Path,
    raw_input: &str,
    on_progress: Option<DownloadProgressCallback>,
) -> AppResult<DownloadResult> {
    let media_dir = job_dir.join("media");
    fs::create_dir_all(&media_dir)?;
    logs::clear_log(job_dir, "download")?;

    logs::append_log(
        job_dir,
        "download",
        &format!("=== douyin share-page download ===\ninput: {raw_input}\n"),
    )?;

    let resolved = douyin::resolve_douyin_media(raw_input)?;
    logs::append_log(
        job_dir,
        "download",
        &format!(
            "resolved source_url: {}\nvideo_id: {}\nplay_url: {}\ntitle: {}\n",
            resolved.source_url,
            resolved.video_id,
            resolved.play_url,
            resolved.title.as_deref().unwrap_or("(none)")
        ),
    )?;

    report_progress(&on_progress, 5.0);

    let client = Client::builder()
        .timeout(Duration::from_secs(300))
        .redirect(reqwest::redirect::Policy::limited(10))
        .user_agent(douyin::DOUYIN_MOBILE_USER_AGENT)
        .build()
        .map_err(|error| AppError::message(format!("HTTP 客户端初始化失败: {error}")))?;

    let response = client
        .get(&resolved.play_url)
        .header(USER_AGENT, douyin::DOUYIN_MOBILE_USER_AGENT)
        .header(REFERER, "https://www.douyin.com/")
        .send()
        .map_err(|error| {
            AppError::message(format!(
                "下载抖音视频流失败（网络错误）: {error}。请查看 logs/download.log。"
            ))
        })?;

    let status = response.status();
    if !status.is_success() {
        return Err(AppError::message(format!(
            "下载抖音视频流失败（HTTP {status}）。请查看 logs/download.log。"
        )));
    }

    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(ToOwned::to_owned);
    let content_length = response
        .headers()
        .get(CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok());

    let extension = douyin::guess_media_extension(content_type.as_deref(), &resolved.play_url);
    let destination = media_dir.join(format!("original.{extension}"));
    let partial_path = media_dir.join(format!("original.{extension}.part"));

    logs::append_log(
        job_dir,
        "download",
        &format!(
            "content-type: {}\ncontent-length: {}\nwriting: {}\n",
            content_type.as_deref().unwrap_or("unknown"),
            content_length
                .map(|value| value.to_string())
                .unwrap_or_else(|| "unknown".to_string()),
            destination.display()
        ),
    )?;

    if partial_path.exists() {
        let _ = fs::remove_file(&partial_path);
    }

    let mut file = fs::File::create(&partial_path).map_err(|error| {
        AppError::message(format!(
            "无法创建临时文件 {}: {error}",
            partial_path.display()
        ))
    })?;

    let mut response_body = response;
    let mut buffer = [0_u8; 64 * 1024];
    let mut downloaded_bytes: u64 = 0;
    let mut last_logged_percent: i32 = -1;

    loop {
        let bytes_read = response_body.read(&mut buffer).map_err(|error| {
            AppError::message(format!(
                "读取视频流失败: {error}。已写入 {downloaded_bytes} 字节。"
            ))
        })?;
        if bytes_read == 0 {
            break;
        }
        file.write_all(&buffer[..bytes_read])
            .map_err(|error| AppError::message(format!("写入视频文件失败: {error}")))?;
        downloaded_bytes += bytes_read as u64;

        if let Some(total_bytes) = content_length.filter(|value| *value > 0) {
            let percent = ((downloaded_bytes as f64 / total_bytes as f64) * 95.0 + 5.0)
                .clamp(5.0, 99.0) as f32;
            let percent_bucket = (percent as i32 / 5) * 5;
            if percent_bucket != last_logged_percent {
                last_logged_percent = percent_bucket;
                let _ = logs::append_log(
                    job_dir,
                    "download",
                    &format!("[download] {percent:.1}% ({downloaded_bytes}/{total_bytes} bytes)"),
                );
            }
            report_progress(&on_progress, percent);
        } else if downloaded_bytes % (2 * 1024 * 1024) < buffer.len() as u64 {
            // Without Content-Length, nudge progress slowly based on MB received.
            let soft_percent = (5.0 + (downloaded_bytes as f32 / (20.0 * 1024.0 * 1024.0)) * 40.0)
                .clamp(5.0, 90.0);
            report_progress(&on_progress, soft_percent);
        }
    }

    file.flush()
        .map_err(|error| AppError::message(format!("刷新视频文件失败: {error}")))?;
    drop(file);

    if downloaded_bytes == 0 {
        let _ = fs::remove_file(&partial_path);
        return Err(AppError::message(
            "抖音视频流为空（0 字节）。可能是地区限制、链接失效或需要登录。",
        ));
    }

    if destination.exists() {
        let _ = fs::remove_file(&destination);
    }
    fs::rename(&partial_path, &destination).map_err(|error| {
        AppError::message(format!(
            "无法将临时文件重命名为 {}: {error}",
            destination.display()
        ))
    })?;

    let media_files = list_media_files(&media_dir)?;
    if media_files.is_empty() {
        return Err(AppError::message(
            "抖音下载完成但 media/ 中未找到文件。请查看 logs/download.log。",
        ));
    }

    report_progress(&on_progress, 100.0);
    logs::append_log(
        job_dir,
        "download",
        &format!(
            "download succeeded: {} ({downloaded_bytes} bytes)\n",
            media_files.join(", ")
        ),
    )?;

    Ok(DownloadResult {
        media_files,
        tool_path: "douyin-share-page".to_string(),
        tool_version: Some(format!("video_id={}", resolved.video_id)),
        resolved_title: resolved.title,
    })
}

pub fn run_yt_dlp_download(
    job_dir: &Path,
    url: &str,
    yt_dlp: &ResolvedBinary,
    on_progress: Option<DownloadProgressCallback>,
) -> AppResult<DownloadResult> {
    let binary_path = yt_dlp
        .path
        .as_ref()
        .ok_or_else(|| AppError::message("未找到 yt-dlp，请安装并加入 PATH，或在设置中配置路径"))?;

    let media_dir = job_dir.join("media");
    fs::create_dir_all(&media_dir)?;
    // Keep existing log when falling back from douyin; only clear for pure yt-dlp runs.
    if !logs::log_path(job_dir, "download").exists()
        || fs::metadata(logs::log_path(job_dir, "download"))
            .map(|metadata| metadata.len() == 0)
            .unwrap_or(true)
    {
        logs::clear_log(job_dir, "download")?;
    }

    logs::append_log(
        job_dir,
        "download",
        &format!(
            "=== yt-dlp download ===\nurl: {url}\nbinary: {binary_path}\nversion: {}\n",
            yt_dlp.version.as_deref().unwrap_or("unknown")
        ),
    )?;

    let output_template = media_dir
        .join("original.%(ext)s")
        .to_string_lossy()
        .replace('\\', "/");

    let mut command = Command::new(binary_path);
    command
        .args([
            "--no-playlist",
            "--newline",
            "--progress",
            "-o",
            &output_template,
            url,
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    hide_console_window(&mut command);

    let mut child = command.spawn().map_err(|error| {
        AppError::message(format!(
            "无法启动 yt-dlp（{binary_path}）: {error}。请确认工具可执行。"
        ))
    })?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| AppError::message("yt-dlp stdout 不可用"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| AppError::message("yt-dlp stderr 不可用"))?;

    let job_dir_for_stdout = job_dir.to_path_buf();
    let job_dir_for_stderr = job_dir.to_path_buf();
    let stdout_progress_callback = on_progress.clone();
    let stderr_progress_callback = on_progress;

    let stdout_handle = std::thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line in reader.lines().map_while(Result::ok) {
            let _ = logs::append_log(&job_dir_for_stdout, "download", &line);
            if let Some(percent) = parse_progress_percent(&line) {
                if let Some(callback) = &stdout_progress_callback {
                    if let Ok(mut guard) = callback.lock() {
                        guard(percent);
                    }
                }
            }
        }
    });

    let stderr_handle = std::thread::spawn(move || {
        let reader = BufReader::new(stderr);
        for line in reader.lines().map_while(Result::ok) {
            let _ = logs::append_log(&job_dir_for_stderr, "download", &line);
            if let Some(percent) = parse_progress_percent(&line) {
                if let Some(callback) = &stderr_progress_callback {
                    if let Ok(mut guard) = callback.lock() {
                        guard(percent);
                    }
                }
            }
        }
    });

    let status = child
        .wait()
        .map_err(|error| AppError::message(format!("等待 yt-dlp 退出失败: {error}")))?;

    let _ = stdout_handle.join();
    let _ = stderr_handle.join();

    if !status.success() {
        let code = status
            .code()
            .map(|value| value.to_string())
            .unwrap_or_else(|| "unknown".to_string());
        return Err(AppError::message(format!(
            "yt-dlp 下载失败（exit {code}）。这是最佳努力下载：请检查 URL、网络与 logs/download.log。工具: {binary_path}"
        )));
    }

    let media_files = list_media_files(&media_dir)?;
    if media_files.is_empty() {
        return Err(AppError::message(
            "yt-dlp 已退出但未在 media/ 找到文件。请查看 logs/download.log。",
        ));
    }

    logs::append_log(
        job_dir,
        "download",
        &format!("download succeeded: {}", media_files.join(", ")),
    )?;

    Ok(DownloadResult {
        media_files,
        tool_path: binary_path.clone(),
        tool_version: yt_dlp.version.clone(),
        resolved_title: None,
    })
}

fn report_progress(on_progress: &Option<DownloadProgressCallback>, percent: f32) {
    if let Some(callback) = on_progress {
        if let Ok(mut guard) = callback.lock() {
            guard(percent.clamp(0.0, 100.0));
        }
    }
}

fn list_media_files(media_dir: &Path) -> AppResult<Vec<String>> {
    let mut files = Vec::new();
    if !media_dir.exists() {
        return Ok(files);
    }
    for entry in fs::read_dir(media_dir)? {
        let entry = entry?;
        if entry.file_type()?.is_file() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.ends_with(".part") || name.ends_with(".ytdl") || name.ends_with(".temp") {
                continue;
            }
            files.push(name);
        }
    }
    files.sort();
    Ok(files)
}

fn parse_progress_percent(line: &str) -> Option<f32> {
    // yt-dlp lines may contain "[download]  12.3%"
    let marker = "[download]";
    let position = line.find(marker)?;
    let rest = &line[position + marker.len()..];
    let percent_position = rest.find('%')?;
    let before = rest[..percent_position].trim();
    let token = before.split_whitespace().last()?;
    token.parse::<f32>().ok()
}

fn hide_console_window(command: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let _ = command;
}

#[allow(dead_code)]
pub fn media_dir_relative_files(job_dir: &Path) -> AppResult<Vec<String>> {
    list_media_files(&job_dir.join("media"))
}

pub fn resolve_import_extension(path: &Path) -> String {
    path.extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_string())
        .unwrap_or_else(|| "bin".to_string())
}

pub fn copy_local_media(job_dir: &Path, local_path: &str) -> AppResult<Vec<String>> {
    let source = PathBuf::from(local_path);
    if !source.exists() {
        return Err(AppError::message(format!("本地文件不存在: {local_path}")));
    }
    if !source.is_file() {
        return Err(AppError::message(format!("本地路径不是文件: {local_path}")));
    }

    let media_dir = job_dir.join("media");
    fs::create_dir_all(&media_dir)?;
    logs::clear_log(job_dir, "download")?;
    logs::append_log(
        job_dir,
        "download",
        &format!("=== import local media ===\nsource: {local_path}\n"),
    )?;

    let extension = resolve_import_extension(&source);
    let destination = media_dir.join(format!("original.{extension}"));
    fs::copy(&source, &destination)?;

    let file_name = destination
        .file_name()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|| format!("original.{extension}"));

    logs::append_log(
        job_dir,
        "download",
        &format!("import succeeded: {file_name}"),
    )?;

    Ok(vec![file_name])
}

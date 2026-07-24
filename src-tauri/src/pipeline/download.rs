use super::{douyin, logs};
use crate::config::{validate_cookies_browser, AppConfig};
use crate::error::{AppError, AppResult};
use crate::models::{JobSource, MediaSaveMode};
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

/// Resolved cookie auth for yt-dlp only (paths/labels, never cookie body).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DownloadCookiesOptions {
    pub cookies_file: Option<String>,
    pub cookies_from_browser: Option<String>,
}

impl DownloadCookiesOptions {
    pub fn resolve(source: &JobSource, config: &AppConfig) -> AppResult<Self> {
        let mode = source
            .download_cookies_mode
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("inherit")
            .to_ascii_lowercase();

        match mode.as_str() {
            "none" | "off" | "disable" | "disabled" => Ok(Self::default()),
            "file" => {
                let path = source
                    .download_cookies_file
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .or_else(|| {
                        config
                            .download_cookies_file
                            .as_deref()
                            .map(str::trim)
                            .filter(|value| !value.is_empty())
                    })
                    .ok_or_else(|| {
                        AppError::message("Cookie 模式为文件，但未配置 cookies.txt 路径")
                    })?;
                Ok(Self {
                    cookies_file: Some(path.replace('\\', "/")),
                    cookies_from_browser: None,
                })
            }
            "browser" => {
                let browser = source
                    .download_cookies_from_browser
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .or_else(|| {
                        config
                            .download_cookies_from_browser
                            .as_deref()
                            .map(str::trim)
                            .filter(|value| !value.is_empty())
                    })
                    .ok_or_else(|| AppError::message("Cookie 模式为浏览器，但未配置浏览器标识"))?;
                validate_cookies_browser(browser)?;
                Ok(Self {
                    cookies_file: None,
                    cookies_from_browser: Some(browser.to_string()),
                })
            }
            "inherit" | "" => {
                // Prefer explicit file over browser when both are configured.
                if let Some(path) = config
                    .download_cookies_file
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                {
                    return Ok(Self {
                        cookies_file: Some(path.replace('\\', "/")),
                        cookies_from_browser: None,
                    });
                }
                if let Some(browser) = config
                    .download_cookies_from_browser
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                {
                    validate_cookies_browser(browser)?;
                    return Ok(Self {
                        cookies_file: None,
                        cookies_from_browser: Some(browser.to_string()),
                    });
                }
                Ok(Self::default())
            }
            other => Err(AppError::message(format!(
                "未知的 Cookie 模式: {other}（可用 inherit / none / file / browser）"
            ))),
        }
    }

    /// CLI fragments for yt-dlp (no binary, no URL). Safe to log.
    pub fn yt_dlp_args(&self) -> Vec<String> {
        let mut args = Vec::new();
        if let Some(path) = &self.cookies_file {
            args.push("--cookies".to_string());
            args.push(path.clone());
        } else if let Some(browser) = &self.cookies_from_browser {
            args.push("--cookies-from-browser".to_string());
            args.push(browser.clone());
        }
        args
    }

    pub fn describe_for_log(&self) -> String {
        if let Some(path) = &self.cookies_file {
            format!("cookies_file={path}")
        } else if let Some(browser) = &self.cookies_from_browser {
            format!("cookies_from_browser={browser}")
        } else {
            "cookies=none".to_string()
        }
    }
}

/// yt-dlp format / extract flags for exclusive video | audio save mode.
pub fn yt_dlp_format_args(mode: MediaSaveMode) -> Vec<String> {
    match mode {
        MediaSaveMode::Video => Vec::new(),
        MediaSaveMode::Audio => vec![
            "-f".to_string(),
            "ba/b".to_string(),
            "-x".to_string(),
            "--audio-format".to_string(),
            "m4a".to_string(),
            "--audio-quality".to_string(),
            "0".to_string(),
        ],
    }
}

/// ffmpeg args: Douyin video play URL in → audio-only file out (no full video artifact).
///
/// Output may use a non-standard extension such as `.m4a.part`, so the muxer is
/// forced with `-f ipod` (AAC-in-MP4 / m4a). Without this, ffmpeg fails with
/// "Unable to choose an output format".
pub fn douyin_audio_ffmpeg_args(play_url: &str, output_path: &str) -> Vec<String> {
    vec![
        "-hide_banner".to_string(),
        "-nostdin".to_string(),
        "-y".to_string(),
        "-user_agent".to_string(),
        douyin::DOUYIN_MOBILE_USER_AGENT.to_string(),
        "-headers".to_string(),
        "Referer: https://www.douyin.com/\r\n".to_string(),
        "-i".to_string(),
        play_url.to_string(),
        "-vn".to_string(),
        "-c:a".to_string(),
        "aac".to_string(),
        "-b:a".to_string(),
        "192k".to_string(),
        "-f".to_string(),
        "ipod".to_string(),
        output_path.to_string(),
    ]
}

/// Best-effort download entry: Douyin share text/links use the share-page
/// resolver; everything else falls back to yt-dlp.
pub fn run_download(
    job_dir: &Path,
    raw_input: &str,
    yt_dlp: &ResolvedBinary,
    ffmpeg_path: Option<&str>,
    media_save_mode: MediaSaveMode,
    cookies: &DownloadCookiesOptions,
    on_progress: Option<DownloadProgressCallback>,
) -> AppResult<DownloadResult> {
    if douyin::looks_like_douyin_input(raw_input) {
        match run_douyin_download(
            job_dir,
            raw_input,
            ffmpeg_path,
            media_save_mode,
            on_progress.clone(),
        ) {
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
                return run_yt_dlp_download(
                    job_dir,
                    &fallback_url,
                    yt_dlp,
                    media_save_mode,
                    cookies,
                    on_progress,
                );
            }
        }
    }

    run_yt_dlp_download(
        job_dir,
        raw_input.trim(),
        yt_dlp,
        media_save_mode,
        cookies,
        on_progress,
    )
}

pub fn run_douyin_download(
    job_dir: &Path,
    raw_input: &str,
    ffmpeg_path: Option<&str>,
    media_save_mode: MediaSaveMode,
    on_progress: Option<DownloadProgressCallback>,
) -> AppResult<DownloadResult> {
    let media_dir = job_dir.join("media");
    fs::create_dir_all(&media_dir)?;
    logs::clear_log(job_dir, "download")?;

    logs::append_log(
        job_dir,
        "download",
        &format!(
            "=== douyin share-page download ===\ninput: {raw_input}\nmedia_save_mode: {}\n",
            media_save_mode_label(media_save_mode)
        ),
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

    if media_save_mode == MediaSaveMode::Audio {
        return run_douyin_audio_via_ffmpeg(job_dir, &resolved, ffmpeg_path, on_progress);
    }

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

fn run_douyin_audio_via_ffmpeg(
    job_dir: &Path,
    resolved: &douyin::ResolvedDouyinMedia,
    ffmpeg_path: Option<&str>,
    on_progress: Option<DownloadProgressCallback>,
) -> AppResult<DownloadResult> {
    let binary_path = ffmpeg_path.ok_or_else(|| {
        AppError::message(
            "抖音「保存音频」需要 ffmpeg：请安装并加入 PATH，或在设置中配置 ffmpeg 路径。",
        )
    })?;

    let media_dir = job_dir.join("media");
    let destination = media_dir.join("original.m4a");
    let partial_path = media_dir.join("original.m4a.part");
    if partial_path.exists() {
        let _ = fs::remove_file(&partial_path);
    }
    if destination.exists() {
        let _ = fs::remove_file(&destination);
    }

    let partial_display = partial_path.to_string_lossy().replace('\\', "/");
    let ffmpeg_args = douyin_audio_ffmpeg_args(&resolved.play_url, &partial_display);
    logs::append_log(
        job_dir,
        "download",
        &format!(
            "=== douyin audio via ffmpeg ===\nffmpeg: {binary_path}\nargs: {}\n",
            ffmpeg_args.join(" ")
        ),
    )?;

    report_progress(&on_progress, 10.0);

    let mut command = Command::new(binary_path);
    command
        .args(&ffmpeg_args)
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    hide_console_window(&mut command);

    let mut child = command.spawn().map_err(|error| {
        AppError::message(format!(
            "无法启动 ffmpeg（{binary_path}）: {error}。请确认工具可执行。"
        ))
    })?;

    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| AppError::message("ffmpeg stderr 不可用"))?;
    let job_dir_for_stderr = job_dir.to_path_buf();
    let stderr_handle = std::thread::spawn(move || {
        let reader = BufReader::new(stderr);
        for line in reader.lines().map_while(Result::ok) {
            let _ = logs::append_log(&job_dir_for_stderr, "download", &line);
        }
    });

    let status = child
        .wait()
        .map_err(|error| AppError::message(format!("等待 ffmpeg 退出失败: {error}")))?;
    let _ = stderr_handle.join();

    if !status.success() {
        let _ = fs::remove_file(&partial_path);
        let code = status
            .code()
            .map(|value| value.to_string())
            .unwrap_or_else(|| "unknown".to_string());
        return Err(AppError::message(format!(
            "抖音音频提取失败（ffmpeg exit {code}）。请查看 logs/download.log。工具: {binary_path}"
        )));
    }

    let partial_size = fs::metadata(&partial_path)
        .map(|metadata| metadata.len())
        .unwrap_or(0);
    if partial_size == 0 {
        let _ = fs::remove_file(&partial_path);
        return Err(AppError::message(
            "抖音音频提取结果为空（0 字节）。可能是无音轨、链接失效或地区限制。",
        ));
    }

    fs::rename(&partial_path, &destination).map_err(|error| {
        AppError::message(format!(
            "无法将临时音频文件重命名为 {}: {error}",
            destination.display()
        ))
    })?;

    let media_files = list_media_files(&media_dir)?;
    if media_files.is_empty() {
        return Err(AppError::message(
            "抖音音频提取完成但 media/ 中未找到文件。请查看 logs/download.log。",
        ));
    }

    report_progress(&on_progress, 100.0);
    logs::append_log(
        job_dir,
        "download",
        &format!(
            "douyin audio succeeded: {} ({partial_size} bytes)\n",
            media_files.join(", ")
        ),
    )?;

    Ok(DownloadResult {
        media_files,
        tool_path: format!("douyin-ffmpeg:{binary_path}"),
        tool_version: Some(format!("video_id={}", resolved.video_id)),
        resolved_title: resolved.title.clone(),
    })
}

pub fn run_yt_dlp_download(
    job_dir: &Path,
    url: &str,
    yt_dlp: &ResolvedBinary,
    media_save_mode: MediaSaveMode,
    cookies: &DownloadCookiesOptions,
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

    let format_args = yt_dlp_format_args(media_save_mode);
    logs::append_log(
        job_dir,
        "download",
        &format!(
            "=== yt-dlp download ===\nurl: {url}\nbinary: {binary_path}\nversion: {}\nmedia_save_mode: {}\nformat_args: {}\n{}\n",
            yt_dlp.version.as_deref().unwrap_or("unknown"),
            media_save_mode_label(media_save_mode),
            if format_args.is_empty() {
                "(default)".to_string()
            } else {
                format_args.join(" ")
            },
            cookies.describe_for_log()
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
        ])
        .args(&format_args)
        .args(cookies.yt_dlp_args())
        .arg(url)
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

fn media_save_mode_label(mode: MediaSaveMode) -> &'static str {
    match mode {
        MediaSaveMode::Video => "video",
        MediaSaveMode::Audio => "audio",
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
    crate::sidecar::hide_console_window(command);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{JobKind, JobSource, MediaSaveMode};

    fn sample_source() -> JobSource {
        JobSource {
            kind: JobKind::Download,
            url: Some("https://example.com".into()),
            title: None,
            local_path: None,
            segment_minutes: None,
            download_cookies_mode: None,
            download_cookies_file: None,
            download_cookies_from_browser: None,
            media_save_mode: MediaSaveMode::default(),
        }
    }

    #[test]
    fn cookie_args_prefer_file_over_browser_when_inheriting() {
        let config = AppConfig {
            download_cookies_file: Some(r"C:\cookies\net.txt".into()),
            download_cookies_from_browser: Some("chrome".into()),
            ..AppConfig::default()
        };
        let source = sample_source();
        let options = DownloadCookiesOptions::resolve(&source, &config).expect("resolve");
        assert_eq!(
            options.yt_dlp_args(),
            vec!["--cookies".to_string(), "C:/cookies/net.txt".to_string()]
        );
    }

    #[test]
    fn cookie_mode_none_disables_global_cookies() {
        let config = AppConfig {
            download_cookies_from_browser: Some("edge".into()),
            ..AppConfig::default()
        };
        let mut source = sample_source();
        source.download_cookies_mode = Some("none".into());
        let options = DownloadCookiesOptions::resolve(&source, &config).expect("resolve");
        assert!(options.yt_dlp_args().is_empty());
        assert_eq!(options.describe_for_log(), "cookies=none");
    }

    #[test]
    fn cookie_mode_browser_builds_from_browser_arg() {
        let config = AppConfig::default();
        let mut source = sample_source();
        source.download_cookies_mode = Some("browser".into());
        source.download_cookies_from_browser = Some("firefox".into());
        let options = DownloadCookiesOptions::resolve(&source, &config).expect("resolve");
        assert_eq!(
            options.yt_dlp_args(),
            vec!["--cookies-from-browser".to_string(), "firefox".to_string()]
        );
    }

    #[test]
    fn yt_dlp_audio_mode_uses_direct_audio_extract_flags() {
        let args = yt_dlp_format_args(MediaSaveMode::Audio);
        assert!(args.iter().any(|value| value == "-x"));
        assert!(args.iter().any(|value| value == "ba/b"));
        assert!(args.iter().any(|value| value == "m4a"));
        assert!(!yt_dlp_format_args(MediaSaveMode::Video)
            .iter()
            .any(|value| value == "-x"));
    }

    #[test]
    fn douyin_audio_ffmpeg_args_map_play_url_to_audio_only() {
        let args = douyin_audio_ffmpeg_args(
            "https://example.com/play/video.mp4",
            "media/original.m4a.part",
        );
        assert!(args.iter().any(|value| value == "-vn"));
        assert!(args.iter().any(|value| value == "-i"));
        let input_index = args.iter().position(|value| value == "-i").expect("-i");
        assert_eq!(args[input_index + 1], "https://example.com/play/video.mp4");
        // .part is not a real muxer extension; force ipod/m4a container.
        let format_index = args.iter().position(|value| value == "-f").expect("-f");
        assert_eq!(args[format_index + 1], "ipod");
        assert_eq!(
            args.last().map(String::as_str),
            Some("media/original.m4a.part")
        );
        assert!(!args
            .iter()
            .any(|value| value.ends_with(".mp4") && *value != args[input_index + 1]));
    }
}

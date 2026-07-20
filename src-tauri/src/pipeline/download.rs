use super::logs;
use crate::error::{AppError, AppResult};
use crate::sidecar::ResolvedBinary;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};

pub type DownloadProgressCallback = Arc<Mutex<dyn FnMut(f32) + Send>>;

#[derive(Debug, Clone)]
pub struct DownloadResult {
    pub media_files: Vec<String>,
    pub tool_path: String,
    pub tool_version: Option<String>,
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
    logs::clear_log(job_dir, "download")?;

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
    })
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

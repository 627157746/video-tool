use super::{logs, paths};
use crate::error::{AppError, AppResult};
use crate::sidecar::{ResolvedBinary, SidecarStatus};
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct RecordResult {
    pub media_files: Vec<String>,
    pub tool_path: String,
    pub tool_version: Option<String>,
    pub termination: RecordTermination,
}

pub struct LiveRecordOptions<'a> {
    pub source_url: &'a str,
    pub segment_minutes: u32,
    pub minimum_free_disk_gb: u32,
    pub reconnect_attempts: u32,
    pub sidecars: &'a SidecarStatus,
    pub stop_requested: Arc<AtomicBool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecordTermination {
    EndedNormally,
    StoppedByUser,
    ReconnectExhausted { detail: String },
    DiskGuard { detail: String },
    Failed { detail: String },
}

impl RecordTermination {
    pub fn is_success(&self) -> bool {
        matches!(self, Self::EndedNormally | Self::StoppedByUser)
    }

    pub fn detail(&self) -> Option<&str> {
        match self {
            Self::EndedNormally | Self::StoppedByUser => None,
            Self::ReconnectExhausted { detail }
            | Self::DiskGuard { detail }
            | Self::Failed { detail } => Some(detail),
        }
    }
}

pub fn record_live_segments(
    job_dir: &Path,
    options: LiveRecordOptions<'_>,
    on_capture_ended: impl FnOnce(),
) -> AppResult<RecordResult> {
    let LiveRecordOptions {
        source_url,
        segment_minutes,
        minimum_free_disk_gb,
        reconnect_attempts,
        sidecars,
        stop_requested,
    } = options;
    let ffmpeg_path = require_binary(&sidecars.ffmpeg, "ffmpeg")?;
    let segment_seconds = segment_minutes
        .max(1)
        .checked_mul(60)
        .ok_or_else(|| AppError::message("直播分段时长过大"))?;
    paths::ensure_job_layout(job_dir)?;
    let _ = logs::clear_log(job_dir, "record");
    let _ = logs::append_log(
        job_dir,
        "record",
        &format!(
            "=== live record ===\nsource: {source_url}\nsegment_minutes: {}\nreconnect_attempts: {}\nminimum_free_disk_gb: {}\nffmpeg: {}\n",
            segment_minutes.max(1),
            reconnect_attempts,
            minimum_free_disk_gb,
            ffmpeg_path
        ),
    );

    let mut last_error = String::new();
    let mut termination = None;
    for attempt in 0..=reconnect_attempts {
        if stop_requested.load(Ordering::SeqCst) {
            termination = Some(RecordTermination::StoppedByUser);
            break;
        }

        if let Some(free_gb) = paths::free_disk_gb(&paths::media_dir(job_dir)) {
            if free_gb < minimum_free_disk_gb as u64 {
                termination = Some(RecordTermination::DiskGuard {
                    detail: format!(
                        "磁盘保护已停止录制：剩余 {free_gb} GB，低于阈值 {minimum_free_disk_gb} GB"
                    ),
                });
                break;
            }
        }

        let input_url =
            match resolve_stream_url(source_url, &sidecars.streamlink, job_dir, &stop_requested) {
                Ok(resolved_url) => resolved_url,
                Err(_) if stop_requested.load(Ordering::SeqCst) => {
                    termination = Some(RecordTermination::StoppedByUser);
                    break;
                }
                Err(error) => {
                    let _ = logs::append_log(
                        job_dir,
                        "record",
                        &format!("streamlink resolve failed, use original URL: {error}"),
                    );
                    source_url.to_string()
                }
            };
        if stop_requested.load(Ordering::SeqCst) {
            termination = Some(RecordTermination::StoppedByUser);
            break;
        }
        let start_number = existing_segment_count(job_dir)?;
        let _ = logs::append_log(
            job_dir,
            "record",
            &format!(
                "record attempt {}/{}; start segment {}",
                attempt + 1,
                reconnect_attempts + 1,
                start_number
            ),
        );

        let output_pattern = paths::media_dir(job_dir)
            .join("segment_%03d.ts")
            .to_string_lossy()
            .replace('\\', "/");
        let mut command = Command::new(&ffmpeg_path);
        command
            .args([
                "-hide_banner",
                "-nostdin",
                "-y",
                "-rw_timeout",
                "15000000",
                "-i",
                &input_url,
                "-map",
                "0",
                "-c",
                "copy",
                "-f",
                "segment",
                "-segment_time",
                &segment_seconds.to_string(),
                "-reset_timestamps",
                "1",
                "-segment_start_number",
                &start_number.to_string(),
                &output_pattern,
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::piped());
        hide_console_window(&mut command);

        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(error) => {
                last_error = format!("无法启动 ffmpeg（{ffmpeg_path}）: {error}");
                let _ = logs::append_log(job_dir, "record", &last_error);
                if attempt < reconnect_attempts {
                    std::thread::sleep(Duration::from_secs(3));
                    continue;
                }
                termination = Some(RecordTermination::ReconnectExhausted {
                    detail: last_error.clone(),
                });
                break;
            }
        };
        let Some(stderr) = child.stderr.take() else {
            let _ = child.kill();
            let _ = child.wait();
            last_error = "ffmpeg stderr 不可用".to_string();
            let _ = logs::append_log(job_dir, "record", &last_error);
            if attempt < reconnect_attempts {
                std::thread::sleep(Duration::from_secs(3));
                continue;
            }
            termination = Some(RecordTermination::ReconnectExhausted {
                detail: last_error.clone(),
            });
            break;
        };
        let log_dir = job_dir.to_path_buf();
        let stderr_thread = std::thread::spawn(move || {
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                let _ = logs::append_log(&log_dir, "record", &line);
            }
        });

        let mut last_heartbeat = Instant::now();
        let mut stopped_by_user = false;
        let mut disk_guard_detail = None;
        let exit_status = loop {
            if stop_requested.load(Ordering::SeqCst) {
                stopped_by_user = true;
                let _ = child.kill();
                break child.wait().ok();
            }
            if let Some(free_gb) = paths::free_disk_gb(&paths::media_dir(job_dir)) {
                if free_gb < minimum_free_disk_gb as u64 {
                    let _ = child.kill();
                    disk_guard_detail = Some(format!(
                        "磁盘保护已停止录制：剩余 {free_gb} GB，低于阈值 {minimum_free_disk_gb} GB"
                    ));
                    break child.wait().ok();
                }
            }
            if last_heartbeat.elapsed() >= Duration::from_secs(15) {
                let _ =
                    logs::append_log(job_dir, "record", "heartbeat: recording process is alive");
                last_heartbeat = Instant::now();
            }
            match child.try_wait() {
                Ok(Some(status)) => break Some(status),
                Ok(None) => std::thread::sleep(Duration::from_millis(750)),
                Err(error) => {
                    last_error = format!("检查 ffmpeg 状态失败: {error}");
                    let _ = child.kill();
                    break child.wait().ok();
                }
            }
        };
        let _ = stderr_thread.join();

        if stopped_by_user {
            let _ = logs::append_log(job_dir, "record", "record stopped by user");
            termination = Some(RecordTermination::StoppedByUser);
            break;
        }
        if let Some(detail) = disk_guard_detail {
            let _ = logs::append_log(job_dir, "record", &detail);
            termination = Some(RecordTermination::DiskGuard { detail });
            break;
        }
        if exit_status.as_ref().is_some_and(|status| status.success()) {
            let _ = logs::append_log(job_dir, "record", "record input ended normally");
            termination = Some(RecordTermination::EndedNormally);
            break;
        }

        let exit_code = exit_status
            .and_then(|status| status.code())
            .map(|code| code.to_string())
            .unwrap_or_else(|| "unknown".to_string());
        last_error = format!("ffmpeg 断流/退出（exit {exit_code}）");
        let _ = logs::append_log(job_dir, "record", &last_error);
        if attempt < reconnect_attempts {
            let _ = logs::append_log(job_dir, "record", "等待 3 秒后重连");
            std::thread::sleep(Duration::from_secs(3));
        } else {
            termination = Some(RecordTermination::ReconnectExhausted {
                detail: last_error.clone(),
            });
        }
    }

    on_capture_ended();
    let mut termination = termination.unwrap_or_else(|| RecordTermination::Failed {
        detail: if last_error.is_empty() {
            "录制在没有明确终止原因的情况下结束".to_string()
        } else {
            last_error.clone()
        },
    });
    let segment_files = live_segment_files(job_dir)?;
    if segment_files.is_empty() {
        let detail = termination
            .detail()
            .map(ToString::to_string)
            .unwrap_or_else(|| "录制已停止，但没有生成媒体分段".to_string());
        return Err(AppError::message(format!(
            "录制没有生成可用媒体分段：{detail}"
        )));
    }

    let mut media_files = segment_files;
    match merge_segments(job_dir, &sidecars.ffmpeg, &media_files) {
        Ok(Some(merged_file)) => media_files.push(merged_file),
        Ok(None) => {}
        Err(error) if termination.is_success() => {
            termination = RecordTermination::Failed {
                detail: error.to_string(),
            };
        }
        Err(error) => {
            let _ = logs::append_log(
                job_dir,
                "record",
                &format!("保留原始失败原因；额外的分段合并也失败：{error}"),
            );
        }
    }
    Ok(RecordResult {
        media_files,
        tool_path: ffmpeg_path,
        tool_version: sidecars.ffmpeg.version.clone(),
        termination,
    })
}

pub fn merge_segments(
    job_dir: &Path,
    ffmpeg: &ResolvedBinary,
    segment_files: &[String],
) -> AppResult<Option<String>> {
    if segment_files.len() < 2 {
        return Ok(None);
    }
    let ffmpeg_path = require_binary(ffmpeg, "ffmpeg")?;
    let media_dir = paths::media_dir(job_dir);
    let list_path = media_dir.join("concat_list.txt");
    let mut list_file = fs::File::create(&list_path)?;
    for file_name in segment_files {
        let escaped = file_name.replace('\'', "'\\''");
        writeln!(list_file, "file '{escaped}'")?;
    }
    let output_path = media_dir.join("merged.mkv");
    let mut command = Command::new(&ffmpeg_path);
    hide_console_window(&mut command);
    let output = command
        .current_dir(&media_dir)
        .args([
            "-hide_banner",
            "-y",
            "-f",
            "concat",
            "-safe",
            "0",
            "-i",
            "concat_list.txt",
            "-c",
            "copy",
            "merged.mkv",
        ])
        .output()?;
    let _ = fs::remove_file(list_path);
    logs::append_log(job_dir, "record", &String::from_utf8_lossy(&output.stderr))?;
    if !output.status.success() {
        return Err(AppError::message(format!(
            "媒体分段已保留，但合并失败（exit {:?}）；可重试录制步骤",
            output.status.code()
        )));
    }
    Ok(output_path
        .file_name()
        .map(|name| name.to_string_lossy().to_string()))
}

fn resolve_stream_url(
    source_url: &str,
    streamlink: &ResolvedBinary,
    job_dir: &Path,
    stop_requested: &AtomicBool,
) -> AppResult<String> {
    let Some(binary_path) = streamlink.path.as_deref() else {
        return Ok(source_url.to_string());
    };
    let mut command = Command::new(binary_path);
    command
        .args(["--stream-url", source_url, "best"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    hide_console_window(&mut command);
    let mut child = command.spawn()?;
    let started_at = Instant::now();
    let status = loop {
        if stop_requested.load(Ordering::SeqCst) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(AppError::message("用户在解析直播流地址时停止了录制"));
        }
        if started_at.elapsed() >= Duration::from_secs(30) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(AppError::message("streamlink 解析流地址超时（30 秒）"));
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => std::thread::sleep(Duration::from_millis(100)),
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error.into());
            }
        }
    };

    let mut stdout = Vec::new();
    if let Some(mut child_stdout) = child.stdout.take() {
        child_stdout.read_to_end(&mut stdout)?;
    }
    let mut stderr = Vec::new();
    if let Some(mut child_stderr) = child.stderr.take() {
        child_stderr.read_to_end(&mut stderr)?;
    }
    if !status.success() {
        return Err(AppError::message(String::from_utf8_lossy(&stderr).trim()));
    }
    let resolved = String::from_utf8_lossy(&stdout).trim().to_string();
    if resolved.is_empty() {
        return Err(AppError::message("streamlink 未返回流地址"));
    }
    let _ = logs::append_log(job_dir, "record", "streamlink resolved stream URL");
    Ok(resolved)
}

fn existing_segment_count(job_dir: &Path) -> AppResult<usize> {
    Ok(live_segment_files(job_dir)?.len())
}

fn live_segment_files(job_dir: &Path) -> AppResult<Vec<String>> {
    let mut files: Vec<String> = paths::list_media_files(job_dir)?
        .into_iter()
        .filter(|name| name.starts_with("segment_") && !name.ends_with(".part"))
        .collect();
    files.sort();
    Ok(files)
}

fn require_binary(binary: &ResolvedBinary, label: &str) -> AppResult<String> {
    binary.path.clone().ok_or_else(|| {
        AppError::message(format!(
            "未找到 {label}，请安装并加入 PATH，或在设置中配置路径"
        ))
    })
}

fn hide_console_window(command: &mut Command) {
    crate::sidecar::hide_console_window(command);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_normal_end_and_user_stop_are_successful_outcomes() {
        assert!(RecordTermination::EndedNormally.is_success());
        assert!(RecordTermination::StoppedByUser.is_success());
        assert!(!RecordTermination::ReconnectExhausted {
            detail: "network".into()
        }
        .is_success());
        assert!(!RecordTermination::DiskGuard {
            detail: "disk".into()
        }
        .is_success());
        assert!(!RecordTermination::Failed {
            detail: "merge".into()
        }
        .is_success());
    }
}

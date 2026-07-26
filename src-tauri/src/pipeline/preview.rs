//! In-app media preview support (v0.3).
//!
//! WebView cannot play `.ts` / often `.mkv` containers, so we offer a one-shot
//! remux (`ffmpeg -c copy`, no re-encode) into `media/preview.mp4`. The preview
//! copy is excluded from the pipeline media index (see `paths::list_media_files`)
//! and is deleted together with the rest of `media/` on purge.

use super::paths;
use crate::error::{AppError, AppResult};
use crate::sidecar::ResolvedBinary;
use serde::Serialize;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::Command;

pub const PREVIEW_FILE_NAME: &str = "preview.mp4";

/// Extensions the WebView `<video>`/`<audio>` tags can play directly.
const DIRECTLY_PLAYABLE_EXTENSIONS: &[&str] = &["mp4", "webm", "m4a", "mp3", "ogg", "wav"];
/// Extensions that may play depending on codecs; worth trying before remux.
const MAYBE_PLAYABLE_EXTENSIONS: &[&str] = &["mkv", "mov"];

#[derive(Debug, Clone, Serialize)]
pub struct JobMediaFile {
    pub file_name: String,
    /// Absolute path (forward slashes) for `convertFileSrc` on the frontend.
    pub absolute_path: String,
    pub size_bytes: u64,
    /// original | segment | merged | preview | other
    pub kind: String,
    /// direct | maybe | incompatible
    pub playability: String,
    pub is_audio: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct JobMediaOverview {
    pub files: Vec<JobMediaFile>,
    pub has_preview: bool,
    pub has_srt: bool,
    pub media_purged: bool,
}

pub fn build_media_overview(job_dir: &Path, media_purged: bool) -> AppResult<JobMediaOverview> {
    let media_dir = paths::media_dir(job_dir);
    let mut files = Vec::new();
    if media_dir.is_dir() {
        for entry in fs::read_dir(&media_dir)? {
            let entry = entry?;
            if !entry.file_type()?.is_file() {
                continue;
            }
            let file_name = entry.file_name().to_string_lossy().to_string();
            if is_transient_media_file(&file_name) {
                continue;
            }
            let size_bytes = entry.metadata().map(|meta| meta.len()).unwrap_or(0);
            let absolute_path = entry.path().to_string_lossy().replace('\\', "/");
            files.push(build_media_file(file_name, absolute_path, size_bytes));
        }
    }
    files.sort_by(|left, right| {
        media_kind_order(&left.kind)
            .cmp(&media_kind_order(&right.kind))
            .then_with(|| left.file_name.cmp(&right.file_name))
    });

    let has_preview = files.iter().any(|file| file.kind == "preview");
    let has_srt = paths::transcript_dir(job_dir).join("srt.srt").is_file();
    Ok(JobMediaOverview {
        files,
        has_preview,
        has_srt,
        media_purged,
    })
}

fn is_transient_media_file(file_name: &str) -> bool {
    file_name.ends_with(".part")
        || file_name.ends_with(".ytdl")
        || file_name.ends_with(".temp")
        || file_name.ends_with(".tmp")
        || file_name == "concat_list.txt"
        || file_name == "preview_concat_list.txt"
}

fn build_media_file(file_name: String, absolute_path: String, size_bytes: u64) -> JobMediaFile {
    let lower_name = file_name.to_ascii_lowercase();
    let extension = lower_name.rsplit('.').next().unwrap_or("").to_string();
    let kind = if file_name == PREVIEW_FILE_NAME {
        "preview"
    } else if lower_name.starts_with("original.") {
        "original"
    } else if lower_name.starts_with("segment_") {
        "segment"
    } else if lower_name.starts_with("merged.") {
        "merged"
    } else {
        "other"
    };
    let is_audio = matches!(extension.as_str(), "m4a" | "mp3" | "ogg" | "wav" | "aac");
    let playability = if DIRECTLY_PLAYABLE_EXTENSIONS.contains(&extension.as_str()) {
        "direct"
    } else if MAYBE_PLAYABLE_EXTENSIONS.contains(&extension.as_str()) {
        "maybe"
    } else {
        "incompatible"
    };
    JobMediaFile {
        file_name,
        absolute_path,
        size_bytes,
        kind: kind.to_string(),
        playability: playability.to_string(),
        is_audio,
    }
}

fn media_kind_order(kind: &str) -> u8 {
    match kind {
        "preview" => 0,
        "merged" => 1,
        "original" => 2,
        "segment" => 3,
        _ => 4,
    }
}

/// Remux job media into `media/preview.mp4` without re-encoding.
///
/// Source preference: `merged.*` video → single video file → concat of
/// `segment_*` files. Audio-only jobs are rejected (m4a plays directly).
pub fn generate_preview(job_dir: &Path, ffmpeg: &ResolvedBinary) -> AppResult<String> {
    let ffmpeg_path = ffmpeg
        .path
        .clone()
        .ok_or_else(|| AppError::message("未找到 ffmpeg，请安装并加入 PATH，或在设置中配置路径"))?;
    let media_dir = paths::media_dir(job_dir);
    let media_files = paths::list_media_files(job_dir)?;
    if media_files.is_empty() {
        return Err(AppError::message("该任务没有媒体文件（可能已清理）"));
    }

    let video_files: Vec<&String> = media_files
        .iter()
        .filter(|name| !is_audio_file_name(name))
        .collect();
    if video_files.is_empty() {
        return Err(AppError::message(
            "该任务只有音频文件，可直接播放，无需生成预览副本",
        ));
    }

    let merged_video = video_files
        .iter()
        .find(|name| name.to_ascii_lowercase().starts_with("merged."));
    let segment_videos: Vec<&&String> = video_files
        .iter()
        .filter(|name| name.to_ascii_lowercase().starts_with("segment_"))
        .collect();

    let output_path = media_dir.join(PREVIEW_FILE_NAME);
    let output = if let Some(source_name) = merged_video {
        run_single_input_remux(&ffmpeg_path, &media_dir, source_name)?
    } else if video_files.len() == 1 {
        run_single_input_remux(&ffmpeg_path, &media_dir, video_files[0])?
    } else if !segment_videos.is_empty() {
        run_concat_remux(
            &ffmpeg_path,
            &media_dir,
            &segment_videos
                .iter()
                .map(|name| name.as_str())
                .collect::<Vec<_>>(),
        )?
    } else {
        return Err(AppError::message(
            "存在多个非分段视频文件，无法自动选择预览来源；请先合并媒体",
        ));
    };

    super::logs::append_log(job_dir, "record", &String::from_utf8_lossy(&output.stderr))?;
    if !output.status.success() {
        let _ = fs::remove_file(&output_path);
        return Err(AppError::message(format!(
            "生成预览副本失败（exit {:?}）：源文件编码可能无法直接封装为 MP4，请使用外部播放器打开。详见 logs/record.log",
            output.status.code()
        )));
    }
    if !output_path.is_file() {
        return Err(AppError::message("ffmpeg 未产出 preview.mp4，请查看日志"));
    }
    Ok(PREVIEW_FILE_NAME.to_string())
}

fn is_audio_file_name(file_name: &str) -> bool {
    let lower = file_name.to_ascii_lowercase();
    lower.ends_with(".m4a")
        || lower.ends_with(".mp3")
        || lower.ends_with(".ogg")
        || lower.ends_with(".wav")
        || lower.ends_with(".aac")
}

/// `aac_adtstoasc` is required when copying AAC audio out of MPEG-TS/FLV.
fn needs_adts_to_asc_filter(file_name: &str) -> bool {
    let lower = file_name.to_ascii_lowercase();
    lower.ends_with(".ts") || lower.ends_with(".flv")
}

fn run_single_input_remux(
    ffmpeg_path: &str,
    media_dir: &Path,
    source_name: &str,
) -> AppResult<std::process::Output> {
    let mut command = Command::new(ffmpeg_path);
    crate::sidecar::hide_console_window(&mut command);
    command
        .current_dir(media_dir)
        .args(["-hide_banner", "-y", "-i", source_name, "-c", "copy"]);
    if needs_adts_to_asc_filter(source_name) {
        command.args(["-bsf:a", "aac_adtstoasc"]);
    }
    command.arg(PREVIEW_FILE_NAME);
    Ok(command.output()?)
}

fn run_concat_remux(
    ffmpeg_path: &str,
    media_dir: &Path,
    segment_names: &[&str],
) -> AppResult<std::process::Output> {
    let list_path = media_dir.join("preview_concat_list.txt");
    let mut list_file = fs::File::create(&list_path)?;
    for file_name in segment_names {
        let escaped = file_name.replace('\'', "'\\''");
        writeln!(list_file, "file '{escaped}'")?;
    }
    drop(list_file);

    let needs_adts_filter = segment_names
        .iter()
        .any(|name| needs_adts_to_asc_filter(name));
    let mut command = Command::new(ffmpeg_path);
    crate::sidecar::hide_console_window(&mut command);
    command.current_dir(media_dir).args([
        "-hide_banner",
        "-y",
        "-f",
        "concat",
        "-safe",
        "0",
        "-i",
        "preview_concat_list.txt",
        "-c",
        "copy",
    ]);
    if needs_adts_filter {
        command.args(["-bsf:a", "aac_adtstoasc"]);
    }
    command.arg(PREVIEW_FILE_NAME);
    let output = command.output();
    let _ = fs::remove_file(&list_path);
    Ok(output?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_media_files_by_kind_and_playability() {
        let preview = build_media_file(
            PREVIEW_FILE_NAME.to_string(),
            "c:/w/preview.mp4".to_string(),
            10,
        );
        assert_eq!(preview.kind, "preview");
        assert_eq!(preview.playability, "direct");

        let ts_segment = build_media_file(
            "segment_001.ts".to_string(),
            "c:/w/segment_001.ts".to_string(),
            10,
        );
        assert_eq!(ts_segment.kind, "segment");
        assert_eq!(ts_segment.playability, "incompatible");

        let merged_mkv =
            build_media_file("merged.mkv".to_string(), "c:/w/merged.mkv".to_string(), 10);
        assert_eq!(merged_mkv.kind, "merged");
        assert_eq!(merged_mkv.playability, "maybe");

        let audio = build_media_file(
            "original.m4a".to_string(),
            "c:/w/original.m4a".to_string(),
            10,
        );
        assert_eq!(audio.kind, "original");
        assert!(audio.is_audio);
        assert_eq!(audio.playability, "direct");
    }

    #[test]
    fn adts_filter_only_for_ts_and_flv() {
        assert!(needs_adts_to_asc_filter("segment_001.ts"));
        assert!(needs_adts_to_asc_filter("original.FLV"));
        assert!(!needs_adts_to_asc_filter("merged.mkv"));
        assert!(!needs_adts_to_asc_filter("original.mp4"));
    }
}

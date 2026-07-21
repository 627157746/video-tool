use super::{logs, paths};
use crate::config::AppConfig;
use crate::error::{AppError, AppResult};
use crate::models::{Job, SegmentStatus, TranscriptSegmentInfo};
use crate::sidecar::SidecarStatus;
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn transcribe_media_segments(
    job_dir: &Path,
    job: &mut Job,
    config: &AppConfig,
    sidecars: &SidecarStatus,
    target_segment_id: Option<&str>,
    mut on_segment: impl FnMut(&Job) -> AppResult<()>,
) -> AppResult<()> {
    let transcribe_path = sidecars.transcribe.path.as_deref().ok_or_else(|| {
        AppError::message("未找到 whisper.cpp whisper-cli，请在设置中配置转写可执行文件")
    })?;
    let ffmpeg_path = sidecars
        .ffmpeg
        .path
        .as_deref()
        .ok_or_else(|| AppError::message("本地转写需要 ffmpeg 提取 16kHz 单声道音频"))?;
    let model_path = config
        .resolve_transcribe_model_path()
        .ok_or_else(|| AppError::message("未配置 whisper.cpp 模型文件路径"))?;
    if !Path::new(&model_path).exists() {
        return Err(AppError::message(format!("转写模型不存在: {model_path}")));
    }
    let whisper_prompt = super::glossary::build_whisper_prompt(&config.glossary);
    job.glossary_hash = Some(super::glossary::glossary_content_hash(&config.glossary));

    paths::ensure_job_layout(job_dir)?;
    if target_segment_id.is_some() {
        logs::append_log(job_dir, "transcribe", "=== retry transcript segment ===")?;
    } else {
        logs::clear_log(job_dir, "transcribe")?;
    }
    let work_items: Vec<(usize, PathBuf)> = if let Some(segment_id) = target_segment_id {
        let index = job
            .transcript_segments
            .iter()
            .position(|segment| segment.id == segment_id)
            .ok_or_else(|| AppError::message(format!("转写分段不存在: {segment_id}")))?;
        let media_path = paths::media_dir(job_dir).join(&job.transcript_segments[index].media_file);
        if !media_path.exists() {
            let detail = format!("转写分段媒体不存在: {}", media_path.display());
            job.transcript_segments[index].status = SegmentStatus::Failed;
            job.transcript_segments[index].detail = Some(detail.clone());
            job.transcript_segments[index].plain_path = None;
            job.transcript_segments[index].srt_path = None;
            on_segment(job)?;
            return Err(AppError::message(detail));
        }
        vec![(index, media_path)]
    } else {
        let media_inputs = transcription_inputs(job_dir)?;
        if media_inputs.is_empty() {
            return Err(AppError::message("media/ 中没有可转写的媒体文件"));
        }
        job.transcript_segments = media_inputs
            .iter()
            .enumerate()
            .map(|(index, media_path)| TranscriptSegmentInfo {
                id: format!("seg-{:03}", index + 1),
                media_file: media_path
                    .file_name()
                    .map(|name| name.to_string_lossy().to_string())
                    .unwrap_or_default(),
                index: (index + 1) as u32,
                status: SegmentStatus::Pending,
                plain_path: None,
                srt_path: None,
                detail: None,
            })
            .collect();
        job.selected_segment_ids = job
            .transcript_segments
            .iter()
            .map(|segment| segment.id.clone())
            .collect();
        media_inputs.into_iter().enumerate().collect()
    };

    let work_item_count = work_items.len();
    for (completed_count, (index, media_path)) in work_items.into_iter().enumerate() {
        job.transcript_segments[index].status = SegmentStatus::Running;
        job.progress = (completed_count as f32 / work_item_count as f32) * 100.0;
        on_segment(job)?;

        let segment_name = format!("segment_{:03}", index + 1);
        let output_base = paths::transcript_segments_dir(job_dir).join(&segment_name);
        let wav_path = output_base.with_extension("wav");
        let plain_output_path = output_base.with_extension("txt");
        // Keep previous plain text for single-segment quality comparison (v0.2 P2).
        if plain_output_path.exists() {
            let previous_path = output_base.with_extension("prev.txt");
            let _ = fs::copy(&plain_output_path, &previous_path);
        }
        logs::append_log(
            job_dir,
            "transcribe",
            &format!("transcribe {} -> {segment_name}", media_path.display()),
        )?;

        let segment_result = (|| -> AppResult<()> {
            extract_audio(ffmpeg_path, &media_path, &wav_path, job_dir)?;
            let mut command = Command::new(transcribe_path);
            command.args([
                "-m",
                &model_path,
                "-f",
                &wav_path.to_string_lossy(),
                "-otxt",
                "-osrt",
                "-oj",
                "-of",
                &output_base.to_string_lossy(),
            ]);
            // merge_pipeline resolves `pipeline.transcribe_language` to the
            // effective value (job override or global config). Fall back to the
            // global config again so manual edits to old `source.json` files
            // that predate this field still transcribe with a sane language.
            let effective_language = job
                .pipeline
                .transcribe_language
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or(config.transcribe_language.trim());
            if effective_language != "auto" && !effective_language.is_empty() {
                command.args(["-l", effective_language]);
            }
            if let Some(prompt_text) = whisper_prompt.as_deref() {
                command.args(["--prompt", prompt_text]);
            }
            let output = command.output().map_err(|error| {
                AppError::message(format!(
                    "无法启动 whisper.cpp（{transcribe_path}）: {error}"
                ))
            })?;
            logs::append_log(
                job_dir,
                "transcribe",
                &String::from_utf8_lossy(&output.stdout),
            )?;
            logs::append_log(
                job_dir,
                "transcribe",
                &String::from_utf8_lossy(&output.stderr),
            )?;

            if !output.status.success() || !output_base.with_extension("txt").exists() {
                return Err(AppError::message(format!(
                    "whisper.cpp 失败（exit {:?}）",
                    output.status.code()
                )));
            }
            fs::read_to_string(output_base.with_extension("txt")).map_err(|error| {
                AppError::message(format!("whisper.cpp 文本输出不可读: {error}"))
            })?;
            if output_base.with_extension("srt").exists() {
                fs::read_to_string(output_base.with_extension("srt")).map_err(|error| {
                    AppError::message(format!("whisper.cpp SRT 输出不可读: {error}"))
                })?;
            }
            Ok(())
        })();
        let _ = fs::remove_file(&wav_path);

        if let Err(error) = segment_result {
            let detail = error.to_string();
            job.transcript_segments[index].status = SegmentStatus::Failed;
            job.transcript_segments[index].detail = Some(detail.clone());
            job.transcript_segments[index].plain_path = None;
            job.transcript_segments[index].srt_path = None;
            on_segment(job)?;
            return Err(AppError::message(detail));
        }

        job.transcript_segments[index].status = SegmentStatus::Succeeded;
        job.transcript_segments[index].plain_path =
            Some(format!("transcript/segments/{segment_name}.txt"));
        if output_base.with_extension("srt").exists() {
            job.transcript_segments[index].srt_path =
                Some(format!("transcript/segments/{segment_name}.srt"));
        }
        job.transcript_segments[index].detail = Some("转写完成".to_string());
        job.progress = ((completed_count + 1) as f32 / work_item_count as f32) * 100.0;
        on_segment(job)?;
    }
    Ok(())
}

pub fn merge_transcripts(
    job_dir: &Path,
    job: &mut Job,
    config: &AppConfig,
    ffprobe_path: Option<&str>,
) -> AppResult<String> {
    let mut segments = job.transcript_segments.clone();
    segments.sort_by_key(|segment| segment.index);
    let use_selection = !job.selected_segment_ids.is_empty();
    let selected_segments: Vec<TranscriptSegmentInfo> = segments
        .into_iter()
        .filter(|segment| !use_selection || job.selected_segment_ids.contains(&segment.id))
        .collect();
    if selected_segments.is_empty() {
        return Err(AppError::message("没有选择可合并的转写分段"));
    }
    if let Some(incomplete_segment) = selected_segments
        .iter()
        .find(|segment| segment.status != SegmentStatus::Succeeded)
    {
        return Err(AppError::message(format!(
            "选中的转写分段尚未成功: {}",
            incomplete_segment.id
        )));
    }
    let mut merged_parts = Vec::new();
    let mut raw_segments = Vec::new();
    let mut merged_srt = String::new();
    let mut srt_sequence = 1_u32;
    let mut srt_offset_milliseconds = 0_u64;
    for segment in selected_segments {
        let relative_path = segment
            .plain_path
            .as_deref()
            .ok_or_else(|| AppError::message(format!("{} 缺少转写文本路径", segment.id)))?;
        let text = fs::read_to_string(job_dir.join(relative_path))?;
        let trimmed = text.trim();
        if !trimmed.is_empty() {
            merged_parts.push(trimmed.to_string());
            raw_segments.push(json!({
                "id": &segment.id,
                "index": segment.index,
                "media_file": &segment.media_file,
                "text": trimmed,
            }));
        }
        let mut subtitle_duration_milliseconds = 0_u64;
        if let Some(srt_path) = segment.srt_path.as_deref() {
            let srt_text = fs::read_to_string(job_dir.join(srt_path))?;
            let (adjusted_srt, maximum_subtitle_end_milliseconds) =
                offset_srt(&srt_text, srt_offset_milliseconds, &mut srt_sequence);
            merged_srt.push_str(&adjusted_srt);
            subtitle_duration_milliseconds = maximum_subtitle_end_milliseconds;
        }
        let media_path = paths::media_dir(job_dir).join(&segment.media_file);
        let media_duration_milliseconds = ffprobe_path
            .and_then(|binary_path| probe_duration_milliseconds(binary_path, &media_path))
            .or_else(|| {
                job.media_segments
                    .iter()
                    .find(|media_segment| media_segment.id == segment.id)
                    .and_then(|media_segment| media_segment.duration_seconds)
                    .map(|duration_seconds| (duration_seconds * 1_000.0).round() as u64)
            })
            .unwrap_or(subtitle_duration_milliseconds);
        srt_offset_milliseconds =
            srt_offset_milliseconds.saturating_add(media_duration_milliseconds);
    }
    if merged_parts.is_empty() {
        return Err(AppError::message(
            "没有可合并的成功转写分段，请检查选段范围",
        ));
    }
    let merged_raw = merged_parts.join("\n\n");
    let merged = super::glossary::apply_post_replace(&merged_raw, &config.glossary);
    if merged != merged_raw {
        logs::append_log(
            job_dir,
            "merge_transcript",
            "applied glossary post-replace on merged plain text",
        )?;
    }
    job.glossary_hash = Some(super::glossary::glossary_content_hash(&config.glossary));
    let transcript_dir = paths::transcript_dir(job_dir);
    fs::create_dir_all(&transcript_dir)?;
    fs::write(transcript_dir.join("plain.txt"), &merged)?;
    if !merged_srt.trim().is_empty() {
        fs::write(transcript_dir.join("srt.srt"), merged_srt)?;
    } else {
        match fs::remove_file(transcript_dir.join("srt.srt")) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    fs::write(
        transcript_dir.join("raw.json"),
        serde_json::to_string_pretty(&json!({ "segments": raw_segments }))?,
    )?;
    logs::append_log(
        job_dir,
        "merge_transcript",
        &format!(
            "merged {} segments; {} chars",
            merged_parts.len(),
            merged.chars().count()
        ),
    )?;
    job.plain_transcript_path = Some("transcript/plain.txt".to_string());
    Ok(merged)
}

fn probe_duration_milliseconds(ffprobe_path: &str, media_path: &Path) -> Option<u64> {
    let output = Command::new(ffprobe_path)
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=duration",
            "-of",
            "default=noprint_wrappers=1:nokey=1",
            &media_path.to_string_lossy(),
        ])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let duration_seconds: f64 = String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse()
        .ok()?;
    if !duration_seconds.is_finite() || duration_seconds < 0.0 {
        return None;
    }
    Some((duration_seconds * 1_000.0).round() as u64)
}

fn offset_srt(input: &str, offset_milliseconds: u64, sequence: &mut u32) -> (String, u64) {
    let normalized = input.replace("\r\n", "\n");
    let mut output = String::new();
    let mut maximum_end_milliseconds = 0_u64;
    for block in normalized.split("\n\n") {
        let lines: Vec<&str> = block.lines().collect();
        let Some(timeline_index) = lines.iter().position(|line| line.contains(" --> ")) else {
            continue;
        };
        let Some((start, end)) = lines[timeline_index].split_once(" --> ") else {
            continue;
        };
        let (Some(start_milliseconds), Some(end_milliseconds)) =
            (parse_srt_timestamp(start), parse_srt_timestamp(end))
        else {
            continue;
        };
        maximum_end_milliseconds = maximum_end_milliseconds.max(end_milliseconds);
        output.push_str(&format!(
            "{}\n{} --> {}\n",
            *sequence,
            format_srt_timestamp(start_milliseconds.saturating_add(offset_milliseconds)),
            format_srt_timestamp(end_milliseconds.saturating_add(offset_milliseconds)),
        ));
        for text_line in lines.iter().skip(timeline_index + 1) {
            output.push_str(text_line);
            output.push('\n');
        }
        output.push('\n');
        *sequence = sequence.saturating_add(1);
    }
    (output, maximum_end_milliseconds)
}

fn parse_srt_timestamp(value: &str) -> Option<u64> {
    let (clock, milliseconds) = value.trim().split_once(',')?;
    let mut parts = clock.split(':');
    let hours: u64 = parts.next()?.parse().ok()?;
    let minutes: u64 = parts.next()?.parse().ok()?;
    let seconds: u64 = parts.next()?.parse().ok()?;
    let milliseconds: u64 = milliseconds.parse().ok()?;
    Some((((hours * 60) + minutes) * 60 + seconds) * 1_000 + milliseconds)
}

fn format_srt_timestamp(milliseconds: u64) -> String {
    let hours = milliseconds / 3_600_000;
    let minutes = (milliseconds % 3_600_000) / 60_000;
    let seconds = (milliseconds % 60_000) / 1_000;
    let remainder = milliseconds % 1_000;
    format!("{hours:02}:{minutes:02}:{seconds:02},{remainder:03}")
}

fn transcription_inputs(job_dir: &Path) -> AppResult<Vec<PathBuf>> {
    let all = paths::media_paths(job_dir)?;
    let mut live_segments: Vec<PathBuf> = all
        .iter()
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("segment_"))
        })
        .cloned()
        .collect();
    live_segments.sort();
    if !live_segments.is_empty() {
        return Ok(live_segments);
    }
    let original = all.iter().find(|path| {
        path.file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with("original."))
    });
    if let Some(path) = original {
        return Ok(vec![path.clone()]);
    }
    Ok(all.into_iter().take(1).collect())
}

fn extract_audio(
    ffmpeg_path: &str,
    media_path: &Path,
    wav_path: &Path,
    job_dir: &Path,
) -> AppResult<()> {
    let output = Command::new(ffmpeg_path)
        .args([
            "-hide_banner",
            "-y",
            "-i",
            &media_path.to_string_lossy(),
            "-vn",
            "-ar",
            "16000",
            "-ac",
            "1",
            "-c:a",
            "pcm_s16le",
            &wav_path.to_string_lossy(),
        ])
        .output()?;
    logs::append_log(
        job_dir,
        "transcribe",
        &String::from_utf8_lossy(&output.stderr),
    )?;
    if !output.status.success() {
        return Err(AppError::message(format!(
            "ffmpeg 音频提取失败（exit {:?}）: {}",
            output.status.code(),
            media_path.display()
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{JobKind, JobSource, PipelineOptions};

    #[test]
    fn merges_selected_transcripts_in_order() {
        let root = std::env::temp_dir().join(format!("video-tool-test-{}", uuid::Uuid::new_v4()));
        paths::ensure_job_layout(&root).unwrap();
        fs::write(
            paths::transcript_segments_dir(&root).join("segment_001.txt"),
            "first",
        )
        .unwrap();
        fs::write(
            paths::transcript_segments_dir(&root).join("segment_002.txt"),
            "second",
        )
        .unwrap();
        let mut job = Job::new(
            JobSource {
                kind: JobKind::ImportLocal,
                url: None,
                title: None,
                local_path: Some("x".into()),
                segment_minutes: None,
                download_cookies_mode: None,
                download_cookies_file: None,
                download_cookies_from_browser: None,
            },
            PipelineOptions::default(),
        );
        job.transcript_segments = vec![
            TranscriptSegmentInfo {
                id: "seg-002".into(),
                media_file: "b".into(),
                index: 2,
                status: SegmentStatus::Succeeded,
                plain_path: Some("transcript/segments/segment_002.txt".into()),
                srt_path: None,
                detail: None,
            },
            TranscriptSegmentInfo {
                id: "seg-001".into(),
                media_file: "a".into(),
                index: 1,
                status: SegmentStatus::Succeeded,
                plain_path: Some("transcript/segments/segment_001.txt".into()),
                srt_path: None,
                detail: None,
            },
        ];
        job.selected_segment_ids = vec!["seg-001".into(), "seg-002".into()];
        assert_eq!(
            merge_transcripts(&root, &mut job, &AppConfig::default(), None).unwrap(),
            "first\n\nsecond"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_selected_segments_that_have_not_succeeded() {
        let root = std::env::temp_dir().join(format!(
            "video-tool-incomplete-merge-test-{}",
            uuid::Uuid::new_v4()
        ));
        paths::ensure_job_layout(&root).expect("create job layout");
        let mut job = Job::new(
            JobSource {
                kind: JobKind::ImportLocal,
                url: None,
                title: None,
                local_path: Some("video.mp4".into()),
                segment_minutes: None,
                download_cookies_mode: None,
                download_cookies_file: None,
                download_cookies_from_browser: None,
            },
            PipelineOptions::default(),
        );
        job.transcript_segments = vec![TranscriptSegmentInfo {
            id: "seg-001".into(),
            media_file: "original.mp4".into(),
            index: 1,
            status: SegmentStatus::Failed,
            plain_path: None,
            srt_path: None,
            detail: Some("failed".into()),
        }];
        job.selected_segment_ids = vec!["seg-001".into()];

        let error = merge_transcripts(&root, &mut job, &AppConfig::default(), None)
            .expect_err("failed selected segment must block merge");
        assert!(error.to_string().contains("尚未成功"));

        fs::remove_dir_all(root).expect("remove test directory");
    }

    #[test]
    fn offsets_srt_timestamps_and_resequences_entries() {
        let mut sequence = 3;
        let (output, duration) = offset_srt(
            "1\n00:00:01,250 --> 00:00:03,500\nhello\n",
            10_000,
            &mut sequence,
        );
        assert!(output.contains("3\n00:00:11,250 --> 00:00:13,500"));
        assert_eq!(duration, 3_500);
        assert_eq!(sequence, 4);
    }
}

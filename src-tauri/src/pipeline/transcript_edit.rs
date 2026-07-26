//! Manual proofreading of the merged transcript (v0.3).
//!
//! The merged `transcript/srt.srt` is treated as the editing source of truth:
//! cue text edits are written back to `srt.srt` and `transcript/plain.txt`
//! together so preview subtitles and the summarize input stay consistent.
//! Timing lines are preserved verbatim; only text changes.

use super::paths;
use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize)]
pub struct TranscriptCue {
    /// 0-based position in the parsed cue list (stable edit key).
    pub index: u32,
    pub start_ms: u64,
    pub end_ms: u64,
    /// Original `HH:MM:SS,mmm --> HH:MM:SS,mmm` line, preserved on save.
    pub timing_line: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct TranscriptCueDocument {
    pub has_srt: bool,
    pub cues: Vec<TranscriptCue>,
    /// Current merged plain text (fallback editing target when no SRT).
    pub plain_text: String,
    pub plain_exists: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CueTextEdit {
    pub index: u32,
    pub text: String,
}

pub fn srt_path(job_dir: &Path) -> std::path::PathBuf {
    paths::transcript_dir(job_dir).join("srt.srt")
}

pub fn plain_path(job_dir: &Path) -> std::path::PathBuf {
    paths::transcript_dir(job_dir).join("plain.txt")
}

fn srt_backup_path(job_dir: &Path) -> std::path::PathBuf {
    paths::transcript_dir(job_dir).join("srt.prev.srt")
}

fn plain_backup_path(job_dir: &Path) -> std::path::PathBuf {
    paths::transcript_dir(job_dir).join("plain.prev.txt")
}

pub fn load_cue_document(job_dir: &Path) -> AppResult<TranscriptCueDocument> {
    let plain_file = plain_path(job_dir);
    let plain_exists = plain_file.is_file();
    let plain_text = if plain_exists {
        fs::read_to_string(&plain_file)?
    } else {
        String::new()
    };

    let srt_file = srt_path(job_dir);
    if !srt_file.is_file() {
        return Ok(TranscriptCueDocument {
            has_srt: false,
            cues: Vec::new(),
            plain_text,
            plain_exists,
        });
    }

    let srt_raw = fs::read_to_string(&srt_file)?;
    Ok(TranscriptCueDocument {
        has_srt: true,
        cues: parse_srt_cues(&srt_raw),
        plain_text,
        plain_exists,
    })
}

/// Apply cue text edits: back up current artifacts once (overwrite style),
/// then rewrite `srt.srt` and rebuild `plain.txt` from the edited cues.
pub fn save_cue_edits(job_dir: &Path, edits: &[CueTextEdit]) -> AppResult<()> {
    let srt_file = srt_path(job_dir);
    if !srt_file.is_file() {
        return Err(AppError::message(
            "该任务没有合并字幕（srt.srt），请使用整篇文本编辑",
        ));
    }
    let srt_raw = fs::read_to_string(&srt_file)?;
    let mut cues = parse_srt_cues(&srt_raw);
    if cues.is_empty() {
        return Err(AppError::message("合并字幕内容为空，无法按句校对"));
    }

    for edit in edits {
        let cue = cues
            .get_mut(edit.index as usize)
            .ok_or_else(|| AppError::message(format!("字幕行序号超出范围: {}", edit.index)))?;
        cue.text = edit.text.trim().to_string();
    }

    let next_srt = serialize_srt_cues(&cues);
    let next_plain = build_plain_from_cues(&cues);
    if next_plain.trim().is_empty() {
        return Err(AppError::message("校对后全文为空，已取消保存"));
    }

    backup_file_once(&srt_file, &srt_backup_path(job_dir))?;
    let plain_file = plain_path(job_dir);
    if plain_file.is_file() {
        backup_file_once(&plain_file, &plain_backup_path(job_dir))?;
    }

    fs::write(&srt_file, next_srt)?;
    fs::write(&plain_file, next_plain)?;
    Ok(())
}

/// Whole-text fallback editing when the job has no merged SRT.
pub fn save_plain_edit(job_dir: &Path, plain_text: &str) -> AppResult<()> {
    if plain_text.trim().is_empty() {
        return Err(AppError::message("校对后全文为空，已取消保存"));
    }
    let plain_file = plain_path(job_dir);
    if !plain_file.is_file() {
        return Err(AppError::message(
            "该任务还没有合并全文（plain.txt），请先完成合并文字步骤",
        ));
    }
    backup_file_once(&plain_file, &plain_backup_path(job_dir))?;
    fs::write(&plain_file, normalize_line_endings(plain_text))?;
    Ok(())
}

fn backup_file_once(source: &Path, backup: &Path) -> AppResult<()> {
    fs::copy(source, backup)?;
    Ok(())
}

fn normalize_line_endings(text: &str) -> String {
    text.replace("\r\n", "\n")
}

pub fn parse_srt_cues(srt: &str) -> Vec<TranscriptCue> {
    let normalized = srt.replace("\r\n", "\n");
    let mut cues = Vec::new();
    for block in normalized.split("\n\n").map(str::trim) {
        if block.is_empty() {
            continue;
        }
        let lines: Vec<&str> = block.lines().collect();
        if lines.len() < 2 {
            continue;
        }
        let (timing_line_index, timing_line) =
            if lines[0].chars().all(|character| character.is_ascii_digit()) && lines.len() >= 2 {
                (1usize, lines[1])
            } else {
                (0usize, lines[0])
            };
        let Some((start_ms, end_ms)) = parse_timing_line(timing_line) else {
            continue;
        };
        let text = lines
            .get(timing_line_index + 1..)
            .unwrap_or(&[])
            .join("\n")
            .trim()
            .to_string();
        cues.push(TranscriptCue {
            index: cues.len() as u32,
            start_ms,
            end_ms,
            timing_line: timing_line.trim().to_string(),
            text,
        });
    }
    cues
}

fn serialize_srt_cues(cues: &[TranscriptCue]) -> String {
    let mut output = String::new();
    let mut sequence_number = 0u32;
    for cue in cues {
        if cue.text.trim().is_empty() {
            continue;
        }
        sequence_number += 1;
        output.push_str(&format!(
            "{sequence_number}\n{}\n{}\n\n",
            cue.timing_line,
            cue.text.trim()
        ));
    }
    output
}

fn build_plain_from_cues(cues: &[TranscriptCue]) -> String {
    let mut text = cues
        .iter()
        .map(|cue| cue.text.trim())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    text.push('\n');
    text
}

fn parse_timing_line(line: &str) -> Option<(u64, u64)> {
    let parts: Vec<&str> = line.split("-->").map(str::trim).collect();
    if parts.len() != 2 {
        return None;
    }
    let start = parse_timestamp_ms(parts[0])?;
    let end = parse_timestamp_ms(parts[1].split_whitespace().next().unwrap_or(parts[1]))?;
    Some((start, end))
}

fn parse_timestamp_ms(value: &str) -> Option<u64> {
    // 00:01:02,345 or 00:01:02.345
    let normalized = value.trim().replace(',', ".");
    let sections: Vec<&str> = normalized.split(':').collect();
    if sections.len() != 3 {
        return None;
    }
    let hours: u64 = sections[0].parse().ok()?;
    let minutes: u64 = sections[1].parse().ok()?;
    let seconds_and_millis: f64 = sections[2].parse().ok()?;
    Some(hours * 3_600_000 + minutes * 60_000 + (seconds_and_millis * 1000.0).round() as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_SRT: &str =
        "1\n00:00:00,000 --> 00:00:02,500\n你好 世界\n\n2\n00:00:02,500 --> 00:00:05,000\n第二句\n";

    #[test]
    fn parses_srt_cues_with_timing_and_text() {
        let cues = parse_srt_cues(SAMPLE_SRT);
        assert_eq!(cues.len(), 2);
        assert_eq!(cues[0].start_ms, 0);
        assert_eq!(cues[0].end_ms, 2500);
        assert_eq!(cues[0].text, "你好 世界");
        assert_eq!(cues[1].timing_line, "00:00:02,500 --> 00:00:05,000");
    }

    #[test]
    fn save_cue_edits_rewrites_srt_and_plain_with_backup() {
        let job_dir =
            std::env::temp_dir().join(format!("vt-transcript-edit-{}", uuid::Uuid::new_v4()));
        let transcript_dir = job_dir.join("transcript");
        fs::create_dir_all(&transcript_dir).expect("create transcript dir");
        fs::write(transcript_dir.join("srt.srt"), SAMPLE_SRT).expect("write srt");
        fs::write(transcript_dir.join("plain.txt"), "你好 世界\n\n第二句\n").expect("write plain");

        save_cue_edits(
            &job_dir,
            &[CueTextEdit {
                index: 0,
                text: "你好，世界！".to_string(),
            }],
        )
        .expect("save cue edits");

        let next_srt = fs::read_to_string(transcript_dir.join("srt.srt")).expect("read srt");
        assert!(next_srt.contains("你好，世界！"));
        assert!(next_srt.contains("00:00:02,500 --> 00:00:05,000"));
        let next_plain = fs::read_to_string(transcript_dir.join("plain.txt")).expect("read plain");
        assert_eq!(next_plain, "你好，世界！\n第二句\n");
        assert!(transcript_dir.join("srt.prev.srt").is_file());
        assert!(transcript_dir.join("plain.prev.txt").is_file());
        let backup = fs::read_to_string(transcript_dir.join("srt.prev.srt")).expect("read backup");
        assert!(backup.contains("你好 世界"));

        fs::remove_dir_all(job_dir).expect("cleanup");
    }

    #[test]
    fn empty_cue_text_drops_cue_from_srt() {
        let cues = vec![
            TranscriptCue {
                index: 0,
                start_ms: 0,
                end_ms: 1000,
                timing_line: "00:00:00,000 --> 00:00:01,000".to_string(),
                text: String::new(),
            },
            TranscriptCue {
                index: 1,
                start_ms: 1000,
                end_ms: 2000,
                timing_line: "00:00:01,000 --> 00:00:02,000".to_string(),
                text: "保留".to_string(),
            },
        ];
        let serialized = serialize_srt_cues(&cues);
        assert!(!serialized.contains("00:00:00,000"));
        assert!(serialized.starts_with("1\n00:00:01,000"));
    }

    #[test]
    fn save_plain_edit_backs_up_previous_version() {
        let job_dir = std::env::temp_dir().join(format!("vt-plain-edit-{}", uuid::Uuid::new_v4()));
        let transcript_dir = job_dir.join("transcript");
        fs::create_dir_all(&transcript_dir).expect("create transcript dir");
        fs::write(transcript_dir.join("plain.txt"), "旧内容\n").expect("write plain");

        save_plain_edit(&job_dir, "新内容\r\n第二行\n").expect("save plain edit");

        let next_plain = fs::read_to_string(transcript_dir.join("plain.txt")).expect("read plain");
        assert_eq!(next_plain, "新内容\n第二行\n");
        let backup =
            fs::read_to_string(transcript_dir.join("plain.prev.txt")).expect("read backup");
        assert_eq!(backup, "旧内容\n");

        fs::remove_dir_all(job_dir).expect("cleanup");
    }
}

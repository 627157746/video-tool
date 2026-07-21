//! Heuristic chapter / outline generation after transcript merge.

use super::{logs, paths};
use crate::error::{AppError, AppResult};
use crate::models::Job;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChapterEntry {
    pub id: String,
    pub index: u32,
    pub title: String,
    /// Inclusive start time in seconds when known from SRT; otherwise null.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_seconds: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_seconds: Option<f64>,
    /// Short plain excerpt (not a full map-reduce summary).
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChaptersDocument {
    pub version: u32,
    pub algorithm: String,
    pub chapters: Vec<ChapterEntry>,
}

/// Build chapters from merged plain text and optional SRT, write artifacts.
pub fn chapterize_job(job_dir: &Path, job: &mut Job) -> AppResult<ChaptersDocument> {
    paths::ensure_job_layout(job_dir)?;
    logs::clear_log(job_dir, "chapterize")?;

    let plain_path = job_dir.join("transcript").join("plain.txt");
    if !plain_path.exists() {
        return Err(AppError::message(
            "缺少 transcript/plain.txt，请先完成合并字幕",
        ));
    }
    let plain = fs::read_to_string(&plain_path)?;
    if plain.trim().is_empty() {
        return Err(AppError::message("合并文本为空，无法生成章节大纲"));
    }

    let srt_path = job_dir.join("transcript").join("srt.srt");
    let srt_text = if srt_path.exists() {
        fs::read_to_string(&srt_path).ok()
    } else {
        None
    };

    let document = if let Some(srt) = srt_text.as_deref().filter(|value| !value.trim().is_empty()) {
        chapterize_from_srt(srt)?
    } else {
        chapterize_from_plain(&plain)
    };

    let transcript_dir = paths::transcript_dir(job_dir);
    fs::create_dir_all(&transcript_dir)?;
    let json_path = transcript_dir.join("chapters.json");
    fs::write(&json_path, serde_json::to_string_pretty(&document)?)?;
    let markdown = render_chapters_markdown(&document);
    fs::write(transcript_dir.join("chapters.md"), &markdown)?;

    logs::append_log(
        job_dir,
        "chapterize",
        &format!(
            "algorithm={} chapters={} written chapters.json / chapters.md",
            document.algorithm,
            document.chapters.len()
        ),
    )?;

    job.chapters_path = Some("transcript/chapters.json".to_string());
    Ok(document)
}

pub fn load_chapters_markdown(job_dir: &Path) -> Option<String> {
    let path = job_dir.join("transcript").join("chapters.md");
    fs::read_to_string(path).ok()
}

fn chapterize_from_plain(plain: &str) -> ChaptersDocument {
    let paragraphs: Vec<&str> = plain
        .split("\n\n")
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect();

    let mut chapters = Vec::new();
    if paragraphs.is_empty() {
        return ChaptersDocument {
            version: 1,
            algorithm: "plain_paragraphs".to_string(),
            chapters,
        };
    }

    // Group consecutive paragraphs into ~N target chapters by char budget.
    let total_chars: usize = paragraphs.iter().map(|part| part.chars().count()).sum();
    let target_chapter_count = (total_chars / 1_200).clamp(1, 24);
    let target_chars = (total_chars / target_chapter_count).max(200);

    let mut bucket: Vec<&str> = Vec::new();
    let mut bucket_chars = 0usize;
    let mut chapter_index = 1u32;

    let flush = |bucket: &mut Vec<&str>,
                 bucket_chars: &mut usize,
                 chapter_index: &mut u32,
                 chapters: &mut Vec<ChapterEntry>| {
        if bucket.is_empty() {
            return;
        }
        let body = bucket.join("\n\n");
        let title = derive_title(&body, *chapter_index);
        let summary = excerpt(&body, 160);
        chapters.push(ChapterEntry {
            id: format!("ch-{:03}", *chapter_index),
            index: *chapter_index,
            title,
            start_seconds: None,
            end_seconds: None,
            summary,
        });
        *chapter_index += 1;
        bucket.clear();
        *bucket_chars = 0;
    };

    for paragraph in paragraphs {
        let paragraph_chars = paragraph.chars().count();
        if !bucket.is_empty() && bucket_chars + paragraph_chars > target_chars {
            flush(
                &mut bucket,
                &mut bucket_chars,
                &mut chapter_index,
                &mut chapters,
            );
        }
        bucket.push(paragraph);
        bucket_chars += paragraph_chars;
    }
    flush(
        &mut bucket,
        &mut bucket_chars,
        &mut chapter_index,
        &mut chapters,
    );

    ChaptersDocument {
        version: 1,
        algorithm: "plain_paragraphs".to_string(),
        chapters,
    }
}

fn chapterize_from_srt(srt: &str) -> AppResult<ChaptersDocument> {
    let cues = parse_srt_cues(srt);
    if cues.is_empty() {
        return Ok(chapterize_from_plain(&strip_srt_to_plain(srt)));
    }

    // Split when gap between cues exceeds threshold (silence / scene change approx).
    const GAP_SECONDS: f64 = 4.0;
    let mut chapters = Vec::new();
    let mut chapter_index = 1u32;
    let mut bucket: Vec<&SrtCue> = Vec::new();

    let flush =
        |bucket: &mut Vec<&SrtCue>, chapter_index: &mut u32, chapters: &mut Vec<ChapterEntry>| {
            if bucket.is_empty() {
                return;
            }
            let body = bucket
                .iter()
                .map(|cue| cue.text.as_str())
                .collect::<Vec<_>>()
                .join(" ");
            let start_seconds = bucket.first().map(|cue| cue.start_seconds);
            let end_seconds = bucket.last().map(|cue| cue.end_seconds);
            chapters.push(ChapterEntry {
                id: format!("ch-{:03}", *chapter_index),
                index: *chapter_index,
                title: derive_title(&body, *chapter_index),
                start_seconds,
                end_seconds,
                summary: excerpt(&body, 160),
            });
            *chapter_index += 1;
            bucket.clear();
        };

    for (index, cue) in cues.iter().enumerate() {
        if let Some(previous) = cues.get(index.wrapping_sub(1)).filter(|_| index > 0) {
            let gap = cue.start_seconds - previous.end_seconds;
            if gap >= GAP_SECONDS && !bucket.is_empty() {
                flush(&mut bucket, &mut chapter_index, &mut chapters);
            }
        }
        bucket.push(cue);
        // Also cap chapter length by cue count.
        if bucket.len() >= 40 {
            flush(&mut bucket, &mut chapter_index, &mut chapters);
        }
    }
    flush(&mut bucket, &mut chapter_index, &mut chapters);

    if chapters.is_empty() {
        return Ok(chapterize_from_plain(&strip_srt_to_plain(srt)));
    }

    Ok(ChaptersDocument {
        version: 1,
        algorithm: "srt_gap".to_string(),
        chapters,
    })
}

struct SrtCue {
    start_seconds: f64,
    end_seconds: f64,
    text: String,
}

fn parse_srt_cues(srt: &str) -> Vec<SrtCue> {
    let mut cues = Vec::new();
    let normalized_srt = srt.replace("\r\n", "\n");
    let blocks = normalized_srt.split("\n\n").map(str::trim);
    for block in blocks {
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
        let Some((start_seconds, end_seconds)) = parse_srt_timing(timing_line) else {
            continue;
        };
        let text_start = timing_line_index + 1;
        let text = lines
            .get(text_start..)
            .unwrap_or(&[])
            .join(" ")
            .replace("<i>", "")
            .replace("</i>", "")
            .trim()
            .to_string();
        if text.is_empty() {
            continue;
        }
        cues.push(SrtCue {
            start_seconds,
            end_seconds,
            text,
        });
    }
    cues
}

fn parse_srt_timing(line: &str) -> Option<(f64, f64)> {
    let parts: Vec<&str> = line.split("-->").map(str::trim).collect();
    if parts.len() != 2 {
        return None;
    }
    let start = parse_srt_timestamp(parts[0])?;
    let end = parse_srt_timestamp(parts[1].split_whitespace().next().unwrap_or(parts[1]))?;
    Some((start, end))
}

fn parse_srt_timestamp(value: &str) -> Option<f64> {
    // 00:01:02,345 or 00:01:02.345
    let normalized = value.trim().replace(',', ".");
    let parts: Vec<&str> = normalized.split(':').collect();
    if parts.len() != 3 {
        return None;
    }
    let hours: f64 = parts[0].parse().ok()?;
    let minutes: f64 = parts[1].parse().ok()?;
    let seconds: f64 = parts[2].parse().ok()?;
    Some(hours * 3600.0 + minutes * 60.0 + seconds)
}

fn strip_srt_to_plain(srt: &str) -> String {
    parse_srt_cues(srt)
        .into_iter()
        .map(|cue| cue.text)
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn derive_title(body: &str, index: u32) -> String {
    let first_line = body
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("");
    let compact: String = first_line.chars().take(28).collect();
    if compact.is_empty() {
        format!("第 {index} 章")
    } else if first_line.chars().count() > 28 {
        format!("{compact}…")
    } else {
        compact
    }
}

fn excerpt(body: &str, max_chars: usize) -> String {
    let trimmed = body.split_whitespace().collect::<Vec<_>>().join(" ");
    if trimmed.chars().count() <= max_chars {
        return trimmed;
    }
    let short: String = trimmed.chars().take(max_chars).collect();
    format!("{short}…")
}

pub fn render_chapters_markdown(document: &ChaptersDocument) -> String {
    let mut lines = vec!["# 章节大纲".to_string(), String::new()];
    for chapter in &document.chapters {
        let time_label = match (chapter.start_seconds, chapter.end_seconds) {
            (Some(start), Some(end)) => {
                format!("（{} – {}）", format_clock(start), format_clock(end))
            }
            (Some(start), None) => format!("（自 {}）", format_clock(start)),
            _ => String::new(),
        };
        lines.push(format!(
            "## {}. {}{}",
            chapter.index, chapter.title, time_label
        ));
        lines.push(String::new());
        lines.push(chapter.summary.clone());
        lines.push(String::new());
    }
    lines.join("\n")
}

fn format_clock(seconds: f64) -> String {
    let total = seconds.max(0.0).floor() as u64;
    let hours = total / 3600;
    let minutes = (total % 3600) / 60;
    let secs = total % 60;
    if hours > 0 {
        format!("{hours:02}:{minutes:02}:{secs:02}")
    } else {
        format!("{minutes:02}:{secs:02}")
    }
}

/// Compact chapters text for `{{chapters}}` template injection.
pub fn chapters_template_text(job_dir: &Path) -> String {
    if let Some(markdown) = load_chapters_markdown(job_dir) {
        return markdown;
    }
    let json_path = job_dir.join("transcript").join("chapters.json");
    if let Ok(raw) = fs::read_to_string(json_path) {
        if let Ok(document) = serde_json::from_str::<ChaptersDocument>(&raw) {
            return render_chapters_markdown(&document);
        }
    }
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_splits_into_chapters() {
        let mut parts = Vec::new();
        // Need enough characters that target_chapter_count >= 2 (threshold 1200).
        for index in 0..20 {
            parts.push(format!("段落 {index}：{}", "内容".repeat(120)));
        }
        let plain = parts.join("\n\n");
        let document = chapterize_from_plain(&plain);
        assert!(
            document.chapters.len() >= 2,
            "expected multi-chapter split, got {}",
            document.chapters.len()
        );
        assert_eq!(document.algorithm, "plain_paragraphs");
    }

    #[test]
    fn srt_gap_splits() {
        let srt = "\
1
00:00:00,000 --> 00:00:02,000
开场白

2
00:00:02,500 --> 00:00:04,000
继续

3
00:00:12,000 --> 00:00:14,000
新章节开始
";
        let document = chapterize_from_srt(srt).expect("srt");
        assert!(document.chapters.len() >= 2);
        assert_eq!(document.algorithm, "srt_gap");
        assert!(document.chapters[0].start_seconds.is_some());
    }

    #[test]
    fn render_markdown_nonempty() {
        let document = ChaptersDocument {
            version: 1,
            algorithm: "test".into(),
            chapters: vec![ChapterEntry {
                id: "ch-001".into(),
                index: 1,
                title: "测试".into(),
                start_seconds: Some(0.0),
                end_seconds: Some(10.0),
                summary: "摘要".into(),
            }],
        };
        let markdown = render_chapters_markdown(&document);
        assert!(markdown.contains("测试"));
        assert!(markdown.contains("摘要"));
    }
}

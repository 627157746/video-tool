use crate::error::AppResult;
use crate::models::JobStep;
use std::fs;
use std::path::{Path, PathBuf};

#[allow(dead_code)]
pub fn job_dir(workspace_root: impl AsRef<Path>, job_id: &str) -> PathBuf {
    workspace_root.as_ref().join("jobs").join(job_id)
}

pub fn media_dir(job_dir: &Path) -> PathBuf {
    job_dir.join("media")
}

pub fn transcript_dir(job_dir: &Path) -> PathBuf {
    job_dir.join("transcript")
}

pub fn transcript_segments_dir(job_dir: &Path) -> PathBuf {
    transcript_dir(job_dir).join("segments")
}

pub fn summary_dir(job_dir: &Path) -> PathBuf {
    job_dir.join("summary")
}

pub fn ensure_job_layout(job_dir: &Path) -> AppResult<()> {
    for directory in [
        job_dir.to_path_buf(),
        media_dir(job_dir),
        transcript_segments_dir(job_dir),
        summary_dir(job_dir),
        job_dir.join("logs"),
    ] {
        fs::create_dir_all(directory)?;
    }
    Ok(())
}

pub fn list_media_files(job_dir: &Path) -> AppResult<Vec<String>> {
    let media = media_dir(job_dir);
    let mut files = Vec::new();
    if !media.exists() {
        return Ok(files);
    }
    for entry in fs::read_dir(media)? {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if name.ends_with(".part")
            || name.ends_with(".ytdl")
            || name.ends_with(".temp")
            || name.ends_with(".tmp")
            || name == "concat_list.txt"
            || name == "preview_concat_list.txt"
            // Preview remux copy is a playback aid, never pipeline input.
            || name == "preview.mp4"
        {
            continue;
        }
        files.push(name);
    }
    files.sort();
    Ok(files)
}

pub fn media_paths(job_dir: &Path) -> AppResult<Vec<PathBuf>> {
    let media = media_dir(job_dir);
    Ok(list_media_files(job_dir)?
        .into_iter()
        .map(|name| media.join(name))
        .collect())
}

/// Prefer original.* then segment_*.* then others, sorted.
#[allow(dead_code)]
pub fn ordered_media_paths(job_dir: &Path) -> AppResult<Vec<PathBuf>> {
    let mut paths = media_paths(job_dir)?;
    paths.sort_by(|left, right| {
        let left_name = left.file_name().and_then(|v| v.to_str()).unwrap_or("");
        let right_name = right.file_name().and_then(|v| v.to_str()).unwrap_or("");
        media_sort_key(left_name).cmp(&media_sort_key(right_name))
    });
    Ok(paths)
}

#[allow(dead_code)]
pub fn media_sort_key(file_name: &str) -> (u8, String) {
    let lower = file_name.to_ascii_lowercase();
    if lower.starts_with("original.") {
        return (0, lower);
    }
    if lower.starts_with("merged.") {
        return (2, lower);
    }
    if lower.starts_with("segment_") {
        return (1, lower);
    }
    (3, lower)
}

pub fn free_disk_gb(path: &Path) -> Option<u64> {
    let target = if path.exists() {
        path.to_path_buf()
    } else {
        path.parent()?.to_path_buf()
    };
    fs2::available_space(target)
        .ok()
        .map(|bytes| bytes / (1024 * 1024 * 1024))
}

pub fn remove_downstream_artifacts(job_dir: &Path, changed_step: &JobStep) -> AppResult<()> {
    match changed_step {
        JobStep::Ingest => {
            remove_directory_contents(&transcript_segments_dir(job_dir))?;
            remove_merged_transcript_artifacts(job_dir)?;
            remove_chapter_artifacts(job_dir)?;
            remove_summary_artifacts(job_dir)?;
        }
        JobStep::Transcribe => {
            remove_merged_transcript_artifacts(job_dir)?;
            remove_chapter_artifacts(job_dir)?;
            remove_summary_artifacts(job_dir)?;
        }
        JobStep::MergeTranscript => {
            remove_merged_transcript_artifacts(job_dir)?;
            remove_chapter_artifacts(job_dir)?;
            remove_summary_artifacts(job_dir)?;
        }
        JobStep::Chapterize => {
            remove_chapter_artifacts(job_dir)?;
            remove_summary_artifacts(job_dir)?;
        }
        JobStep::Summarize => remove_summary_artifacts(job_dir)?,
    }
    Ok(())
}

fn remove_merged_transcript_artifacts(job_dir: &Path) -> AppResult<()> {
    for file_name in ["plain.txt", "raw.json", "srt.srt"] {
        remove_file_if_exists(&transcript_dir(job_dir).join(file_name))?;
    }
    Ok(())
}

fn remove_chapter_artifacts(job_dir: &Path) -> AppResult<()> {
    for file_name in ["chapters.json", "chapters.md"] {
        remove_file_if_exists(&transcript_dir(job_dir).join(file_name))?;
    }
    Ok(())
}

fn remove_summary_artifacts(job_dir: &Path) -> AppResult<()> {
    for file_name in ["summary.md", "meta.json"] {
        remove_file_if_exists(&summary_dir(job_dir).join(file_name))?;
    }
    let by_template_dir = summary_dir(job_dir).join("by_template");
    if by_template_dir.exists() {
        fs::remove_dir_all(&by_template_dir)?;
    }
    Ok(())
}

pub fn summary_by_template_dir(job_dir: &Path) -> PathBuf {
    summary_dir(job_dir).join("by_template")
}

fn remove_directory_contents(directory: &Path) -> AppResult<()> {
    if !directory.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            fs::remove_dir_all(path)?;
        } else {
            fs::remove_file(path)?;
        }
    }
    Ok(())
}

fn remove_file_if_exists(path: &Path) -> AppResult<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn media_sort_orders_original_segments_merged() {
        let mut names = vec![
            "segment_002.ts",
            "merged.mp4",
            "original.mp4",
            "segment_001.ts",
            "other.bin",
        ];
        names.sort_by_key(|name| media_sort_key(name));
        assert_eq!(
            names,
            vec![
                "original.mp4",
                "segment_001.ts",
                "segment_002.ts",
                "merged.mp4",
                "other.bin",
            ]
        );
    }
}

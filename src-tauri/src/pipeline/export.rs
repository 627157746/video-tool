use super::logs;
use crate::error::{AppError, AppResult};
use crate::workspace;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use uuid::Uuid;
use zip::write::SimpleFileOptions;

pub fn export_job_package(
    workspace_root: &Path,
    job_id: &str,
    destination_dir: Option<&str>,
    secrets: &[String],
) -> AppResult<String> {
    let job_dir = workspace::validated_job_dir(workspace_root, job_id)?;
    if !job_dir.exists() {
        return Err(AppError::message(format!("任务不存在: {job_id}")));
    }
    let export_dir = destination_dir
        .map(PathBuf::from)
        .unwrap_or_else(|| workspace_root.join("exports"));
    fs::create_dir_all(&export_dir)?;
    let canonical_job_dir = fs::canonicalize(&job_dir)?;
    let canonical_export_dir = fs::canonicalize(&export_dir)?;
    if canonical_export_dir.starts_with(&canonical_job_dir) {
        return Err(AppError::message("导出目录不能位于任务目录内部"));
    }

    let stamp = chrono::Utc::now().format("%Y%m%d-%H%M%S");
    let unique_suffix = Uuid::new_v4().simple().to_string();
    let output_path = export_dir.join(format!("{job_id}-{stamp}-{}.zip", &unique_suffix[..8]));
    let temporary_path = export_dir.join(format!(".{job_id}-{unique_suffix}.zip.tmp"));

    let export_result = (|| -> AppResult<()> {
        let output_file = File::create(&temporary_path)?;
        let mut archive = zip::ZipWriter::new(output_file);
        let options = SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .unix_permissions(0o644);
        add_directory_to_zip(&mut archive, &job_dir, &job_dir, options, secrets)?;
        let completed_file = archive
            .finish()
            .map_err(|error| AppError::message(format!("完成导出包失败: {error}")))?;
        completed_file.sync_all()?;
        fs::rename(&temporary_path, &output_path)?;
        Ok(())
    })();
    if export_result.is_err() {
        let _ = fs::remove_file(&temporary_path);
    }
    export_result?;
    Ok(output_path.to_string_lossy().replace('\\', "/"))
}

fn add_directory_to_zip(
    archive: &mut zip::ZipWriter<File>,
    root: &Path,
    directory: &Path,
    options: SimpleFileOptions,
    secrets: &[String],
) -> AppResult<()> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            add_directory_to_zip(archive, root, &path, options, secrets)?;
            continue;
        }
        if !entry.file_type()?.is_file() {
            continue;
        }
        let relative = path
            .strip_prefix(root)
            .map_err(|error| AppError::message(format!("导出路径错误: {error}")))?
            .to_string_lossy()
            .replace('\\', "/");
        // Config/API keys live outside a job. Keep an explicit guard for future layout changes.
        if relative.eq_ignore_ascii_case("config.json")
            || relative.to_ascii_lowercase().contains("api_key")
        {
            continue;
        }
        archive
            .start_file(relative, options)
            .map_err(|error| AppError::message(format!("写入导出包失败: {error}")))?;
        if is_text_metadata(&path) {
            let mut input = String::new();
            File::open(&path)?.read_to_string(&mut input)?;
            let redacted = logs::redact_secrets(&input, secrets);
            archive
                .write_all(redacted.as_bytes())
                .map_err(|error| AppError::message(format!("写入导出文件失败: {error}")))?;
        } else {
            let mut input = File::open(&path)?;
            io::copy(&mut input, archive)
                .map_err(|error| AppError::message(format!("流式写入导出文件失败: {error}")))?;
        }
    }
    Ok(())
}

fn is_text_metadata(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "json" | "log" | "txt" | "md" | "srt"
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Job, JobKind, JobSource, PipelineOptions};
    use std::io::Read;

    #[test]
    fn streams_export_and_redacts_text_metadata() {
        let workspace_root =
            std::env::temp_dir().join(format!("video-tool-export-test-{}", Uuid::new_v4()));
        let export_directory = workspace_root.join("exports");
        let mut job = Job::new(
            JobSource {
                kind: JobKind::Download,
                url: Some("https://example.com/video?token=signed-secret".into()),
                title: None,
                local_path: None,
                segment_minutes: None,
            },
            PipelineOptions::default(),
        );
        job.error_message = Some("Authorization: Bearer provider-secret".into());
        let job_dir =
            workspace::create_job_directories(&workspace_root, &job).expect("create export job");
        fs::write(
            job_dir.join("media").join("original.bin"),
            vec![42_u8; 2_000_000],
        )
        .expect("write media fixture");

        let output = export_job_package(
            &workspace_root,
            &job.id,
            export_directory.to_str(),
            &["provider-secret".into()],
        )
        .expect("export job");

        let archive_file = File::open(output).expect("open export archive");
        let mut archive = zip::ZipArchive::new(archive_file).expect("read export archive");
        let mut source_json = String::new();
        archive
            .by_name("source.json")
            .expect("source metadata")
            .read_to_string(&mut source_json)
            .expect("read source metadata");
        assert!(!source_json.contains("signed-secret"));
        assert!(!source_json.contains("provider-secret"));
        assert_eq!(
            archive
                .by_name("media/original.bin")
                .expect("media file")
                .size(),
            2_000_000
        );

        fs::remove_dir_all(workspace_root).expect("remove export fixture");
    }

    #[test]
    fn rejects_export_destination_inside_job_directory() {
        let workspace_root =
            std::env::temp_dir().join(format!("video-tool-export-path-test-{}", Uuid::new_v4()));
        let job = Job::new(
            JobSource {
                kind: JobKind::ImportLocal,
                url: None,
                title: None,
                local_path: Some("video.mp4".into()),
                segment_minutes: None,
            },
            PipelineOptions::default(),
        );
        let job_dir =
            workspace::create_job_directories(&workspace_root, &job).expect("create export job");
        let nested_destination = job_dir.join("exports");

        let error = export_job_package(&workspace_root, &job.id, nested_destination.to_str(), &[])
            .expect_err("nested export destination must fail");
        assert!(error.to_string().contains("任务目录内部"));

        fs::remove_dir_all(workspace_root).expect("remove export fixture");
    }
}

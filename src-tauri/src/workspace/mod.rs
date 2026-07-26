use crate::error::{AppError, AppResult};
use crate::models::{Job, JobStatus, StepStatus};
use crate::storage;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspacePaths {
    pub root: PathBuf,
    pub jobs: PathBuf,
}

impl WorkspacePaths {
    pub fn from_root(root: impl AsRef<Path>) -> Self {
        let root = root.as_ref().to_path_buf();
        Self {
            jobs: root.join("jobs"),
            root,
        }
    }

    pub fn ensure(&self) -> AppResult<()> {
        fs::create_dir_all(&self.jobs)?;
        Ok(())
    }

    fn job_dir(&self, job_id: &str) -> PathBuf {
        self.jobs.join(job_id)
    }
}

pub fn validated_job_dir(workspace_root: impl AsRef<Path>, job_id: &str) -> AppResult<PathBuf> {
    let parsed_job_id = Uuid::parse_str(job_id)
        .map_err(|_| AppError::message(format!("任务 ID 无效: {job_id}")))?;
    Ok(WorkspacePaths::from_root(workspace_root).job_dir(&parsed_job_id.to_string()))
}

pub fn create_job_directories(workspace_root: impl AsRef<Path>, job: &Job) -> AppResult<PathBuf> {
    let paths = WorkspacePaths::from_root(workspace_root);
    paths.ensure()?;

    let job_dir = validated_job_dir(&paths.root, &job.id)?;
    let media_dir = job_dir.join("media");
    let transcript_dir = job_dir.join("transcript").join("segments");
    let summary_dir = job_dir.join("summary");
    let logs_dir = job_dir.join("logs");

    for directory in [
        &job_dir,
        &media_dir,
        &transcript_dir,
        &summary_dir,
        &logs_dir,
    ] {
        fs::create_dir_all(directory)?;
    }

    let source_path = job_dir.join("source.json");
    storage::write_json_atomically(&source_path, job)?;

    Ok(job_dir)
}

pub fn list_jobs(workspace_root: impl AsRef<Path>) -> AppResult<Vec<Job>> {
    let paths = WorkspacePaths::from_root(workspace_root);
    paths.ensure()?;

    let mut jobs = Vec::new();
    let read_dir = match fs::read_dir(&paths.jobs) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(jobs),
        Err(error) => return Err(error.into()),
    };

    for entry in read_dir {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let source_path = entry.path().join("source.json");
        if !source_path.exists() {
            continue;
        }
        let raw = fs::read_to_string(source_path)?;
        match serde_json::from_str::<Job>(&raw) {
            Ok(job) => jobs.push(job),
            Err(error) => {
                eprintln!("skip invalid job at {}: {error}", entry.path().display());
            }
        }
    }

    jobs.sort_by(|left, right| right.created_at.cmp(&left.created_at));
    Ok(jobs)
}

pub fn load_job(workspace_root: impl AsRef<Path>, job_id: &str) -> AppResult<Job> {
    let job_dir = validated_job_dir(workspace_root, job_id)?;
    let source_path = job_dir.join("source.json");
    if !source_path.exists() {
        return Err(AppError::message(format!("任务不存在: {job_id}")));
    }
    let raw = fs::read_to_string(source_path)?;
    Ok(serde_json::from_str(&raw)?)
}

pub fn save_job(workspace_root: impl AsRef<Path>, job: &Job) -> AppResult<()> {
    let job_dir = validated_job_dir(workspace_root, &job.id)?;
    fs::create_dir_all(&job_dir)?;
    let source_path = job_dir.join("source.json");
    storage::write_json_atomically(&source_path, job)?;
    Ok(())
}

pub fn delete_job(workspace_root: impl AsRef<Path>, job_id: &str) -> AppResult<()> {
    let workspace_root = workspace_root.as_ref();
    let job_dir = validated_job_dir(workspace_root, job_id)?;
    let _persisted_job = load_job(workspace_root, job_id)?;
    fs::remove_dir_all(job_dir)?;
    Ok(())
}

pub fn recover_interrupted_jobs(workspace_root: impl AsRef<Path>) -> AppResult<usize> {
    let workspace_root = workspace_root.as_ref();
    let mut recovered_count = 0;
    for mut job in list_jobs(workspace_root)? {
        match job.status {
            JobStatus::Running => {
                let detail =
                    "应用上次退出时任务仍在运行；后台进程已中断，请重试失败步骤".to_string();
                job.status = JobStatus::Failed;
                job.stop_requested = false;
                job.live_capture_active = false;
                job.error_message = Some(detail.clone());
                job.error_code = Some(crate::pipeline::error_code::INTERRUPTED.to_string());
                for step_progress in &mut job.step_statuses {
                    if step_progress.status == StepStatus::Running {
                        step_progress.status = StepStatus::Failed;
                        step_progress.detail = Some(detail.clone());
                    }
                }
                if let Some(current_step) = job.current_step.take() {
                    job.set_step_status(&current_step, StepStatus::Failed, Some(detail.clone()));
                }
                job.updated_at = chrono::Utc::now();
                save_job(workspace_root, &job)?;
                recovered_count += 1;
            }
            // In-memory FIFO is gone after restart; never leave permanent "queued".
            JobStatus::Queued => {
                job.status = JobStatus::Pending;
                job.error_message = None;
                job.error_code = None;
                job.updated_at = chrono::Utc::now();
                save_job(workspace_root, &job)?;
                recovered_count += 1;
            }
            _ => {}
        }
    }
    Ok(recovered_count)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthFinding {
    pub job_id_or_name: String,
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceHealthReport {
    pub workspace_dir: String,
    pub free_disk_gb: Option<u64>,
    pub min_free_disk_gb: u32,
    pub disk_below_threshold: bool,
    pub orphan_directories: Vec<HealthFinding>,
    pub corrupt_jobs: Vec<HealthFinding>,
    pub interrupted_running_jobs: Vec<HealthFinding>,
    pub stale_queued_jobs: Vec<HealthFinding>,
    pub empty_media_index_jobs: Vec<HealthFinding>,
    pub repaired: Vec<String>,
}

/// Scan workspace health without mutating jobs (except optional disk probe paths).
pub fn inspect_workspace_health(
    workspace_root: impl AsRef<Path>,
    min_free_disk_gb: u32,
    active_runner_job_ids: &HashSet<String>,
) -> AppResult<WorkspaceHealthReport> {
    use crate::pipeline::paths;

    let workspace_root = workspace_root.as_ref();
    let paths_layout = WorkspacePaths::from_root(workspace_root);
    paths_layout.ensure()?;

    let free_disk_gb = paths::free_disk_gb(workspace_root);
    let disk_below_threshold = free_disk_gb.is_some_and(|free| free < u64::from(min_free_disk_gb));

    let mut orphan_directories = Vec::new();
    let mut corrupt_jobs = Vec::new();
    let mut interrupted_running_jobs = Vec::new();
    let mut stale_queued_jobs = Vec::new();
    let mut empty_media_index_jobs = Vec::new();

    let read_dir = match fs::read_dir(&paths_layout.jobs) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(WorkspaceHealthReport {
                workspace_dir: workspace_root.to_string_lossy().replace('\\', "/"),
                free_disk_gb,
                min_free_disk_gb,
                disk_below_threshold,
                orphan_directories,
                corrupt_jobs,
                interrupted_running_jobs,
                stale_queued_jobs,
                empty_media_index_jobs,
                repaired: Vec::new(),
            });
        }
        Err(error) => return Err(error.into()),
    };

    for entry in read_dir {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let dir_name = entry.file_name().to_string_lossy().to_string();
        let dir_path = entry.path();
        let path_display = dir_path.to_string_lossy().replace('\\', "/");

        if Uuid::parse_str(&dir_name).is_err() {
            orphan_directories.push(HealthFinding {
                job_id_or_name: dir_name,
                path: path_display,
                message: "目录名不是有效任务 ID（UUID）".to_string(),
            });
            continue;
        }

        let source_path = dir_path.join("source.json");
        if !source_path.exists() {
            orphan_directories.push(HealthFinding {
                job_id_or_name: dir_name,
                path: path_display,
                message: "缺少 source.json".to_string(),
            });
            continue;
        }

        let raw = match fs::read_to_string(&source_path) {
            Ok(value) => value,
            Err(error) => {
                corrupt_jobs.push(HealthFinding {
                    job_id_or_name: dir_name,
                    path: path_display,
                    message: format!("无法读取 source.json: {error}"),
                });
                continue;
            }
        };

        let job = match serde_json::from_str::<Job>(&raw) {
            Ok(job) => job,
            Err(error) => {
                corrupt_jobs.push(HealthFinding {
                    job_id_or_name: dir_name,
                    path: path_display,
                    message: format!("source.json 解析失败: {error}"),
                });
                continue;
            }
        };

        if job.status == JobStatus::Running && !active_runner_job_ids.contains(&job.id) {
            interrupted_running_jobs.push(HealthFinding {
                job_id_or_name: job.id.clone(),
                path: path_display.clone(),
                message: "持久化为 running，但当前无活跃 runner".to_string(),
            });
        }

        if job.status == JobStatus::Queued && !active_runner_job_ids.contains(&job.id) {
            stale_queued_jobs.push(HealthFinding {
                job_id_or_name: job.id.clone(),
                path: path_display.clone(),
                message: "持久化为 queued（进程内队列可能已丢失）".to_string(),
            });
        }

        let media_files_on_disk = paths::list_media_files(&dir_path).unwrap_or_default();
        if !media_files_on_disk.is_empty() && job.media_segments.is_empty() {
            empty_media_index_jobs.push(HealthFinding {
                job_id_or_name: job.id.clone(),
                path: path_display,
                message: format!(
                    "media/ 有 {} 个文件，但 media_segments 索引为空",
                    media_files_on_disk.len()
                ),
            });
        }
    }

    Ok(WorkspaceHealthReport {
        workspace_dir: workspace_root.to_string_lossy().replace('\\', "/"),
        free_disk_gb,
        min_free_disk_gb,
        disk_below_threshold,
        orphan_directories,
        corrupt_jobs,
        interrupted_running_jobs,
        stale_queued_jobs,
        empty_media_index_jobs,
        repaired: Vec::new(),
    })
}

/// Repair interrupted running / stale queued jobs and rebuild empty media indexes.
/// Does **not** delete orphan directories or corrupt JSON (user must handle manually).
pub fn repair_workspace_health(
    workspace_root: impl AsRef<Path>,
    min_free_disk_gb: u32,
    active_runner_job_ids: &HashSet<String>,
) -> AppResult<WorkspaceHealthReport> {
    use crate::pipeline::paths;

    let workspace_root = workspace_root.as_ref();
    let mut report =
        inspect_workspace_health(workspace_root, min_free_disk_gb, active_runner_job_ids)?;
    let mut repaired = Vec::new();

    let recovered = recover_interrupted_jobs(workspace_root)?;
    if recovered > 0 {
        repaired.push(format!("已恢复 {recovered} 个 interrupted/queued 状态任务"));
    }

    for finding in &report.empty_media_index_jobs {
        let job_id = finding.job_id_or_name.as_str();
        if active_runner_job_ids.contains(job_id) {
            continue;
        }
        let Ok(mut job) = load_job(workspace_root, job_id) else {
            continue;
        };
        let job_dir = validated_job_dir(workspace_root, job_id)?;
        if let Ok(media_files) = paths::list_media_files(&job_dir) {
            if media_files.is_empty() {
                continue;
            }
            job.media_files = media_files;
            job.rebuild_media_segments_from_files();
            job.updated_at = chrono::Utc::now();
            save_job(workspace_root, &job)?;
            repaired.push(format!("已重建媒体分段索引: {job_id}"));
        }
    }

    report = inspect_workspace_health(workspace_root, min_free_disk_gb, active_runner_job_ids)?;
    report.repaired = repaired;
    Ok(report)
}

#[derive(Debug, Clone, Serialize)]
pub struct JobUsage {
    pub job_id: String,
    pub title: String,
    pub status: JobStatus,
    pub media_bytes: u64,
    /// transcript + summary + logs + source.json (long-lived text assets).
    pub text_bytes: u64,
    pub total_bytes: u64,
    pub media_purged: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkspaceUsageReport {
    pub workspace_dir: String,
    pub free_disk_gb: Option<u64>,
    pub total_bytes: u64,
    pub total_media_bytes: u64,
    /// Sorted by `media_bytes` descending.
    pub jobs: Vec<JobUsage>,
}

fn directory_size_bytes(path: &Path) -> u64 {
    let Ok(entries) = fs::read_dir(path) else {
        return 0;
    };
    let mut total = 0u64;
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            total += directory_size_bytes(&entry.path());
        } else if let Ok(metadata) = entry.metadata() {
            total += metadata.len();
        }
    }
    total
}

pub fn compute_workspace_usage(
    workspace_root: impl AsRef<Path>,
) -> AppResult<WorkspaceUsageReport> {
    let workspace_root = workspace_root.as_ref();
    let free_disk_gb = crate::pipeline::paths::free_disk_gb(workspace_root);

    let mut job_usages = Vec::new();
    let mut total_media_bytes = 0u64;
    let mut total_bytes = 0u64;

    for job in list_jobs(workspace_root)? {
        let Ok(job_dir) = validated_job_dir(workspace_root, &job.id) else {
            continue;
        };
        let media_bytes = directory_size_bytes(&job_dir.join("media"));
        let job_total_bytes = directory_size_bytes(&job_dir);
        let text_bytes = job_total_bytes.saturating_sub(media_bytes);
        total_media_bytes += media_bytes;
        total_bytes += job_total_bytes;
        job_usages.push(JobUsage {
            job_id: job.id.clone(),
            title: job.display_title(),
            status: job.status.clone(),
            media_bytes,
            text_bytes,
            total_bytes: job_total_bytes,
            media_purged: job.media_purged_at.is_some(),
        });
    }

    // Index directory (search FTS) also lives in the workspace.
    total_bytes += directory_size_bytes(&workspace_root.join("index"));

    job_usages.sort_by(|left, right| right.media_bytes.cmp(&left.media_bytes));

    Ok(WorkspaceUsageReport {
        workspace_dir: workspace_root.to_string_lossy().replace('\\', "/"),
        free_disk_gb,
        total_bytes,
        total_media_bytes,
        jobs: job_usages,
    })
}

/// Delete everything under `media/` (segments, merged, preview copy) while
/// keeping transcript / summary / logs / source.json. Marks the job as purged.
pub fn purge_job_media(workspace_root: impl AsRef<Path>, job_id: &str) -> AppResult<Job> {
    let workspace_root = workspace_root.as_ref();
    let job_dir = validated_job_dir(workspace_root, job_id)?;
    let mut job = load_job(workspace_root, job_id)?;

    let media_dir = job_dir.join("media");
    if media_dir.is_dir() {
        for entry in fs::read_dir(&media_dir)? {
            let entry = entry?;
            let path = entry.path();
            if entry.file_type()?.is_dir() {
                fs::remove_dir_all(path)?;
            } else {
                fs::remove_file(path)?;
            }
        }
    }

    job.media_purged_at = Some(chrono::Utc::now());
    job.live_capture_active = false;
    job.updated_at = chrono::Utc::now();
    save_job(workspace_root, &job)?;
    Ok(job)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{JobKind, JobSource, MediaSaveMode, PipelineOptions};

    fn temporary_workspace(test_name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "video-tool-workspace-{test_name}-{}",
            Uuid::new_v4()
        ))
    }

    #[test]
    fn rejects_non_uuid_job_paths() {
        let workspace_root = temporary_workspace("path-validation");
        let error = validated_job_dir(&workspace_root, "../outside")
            .expect_err("path traversal must be rejected");
        assert!(error.to_string().contains("任务 ID 无效"));
    }

    #[test]
    fn deletes_complete_job_directory() {
        let workspace_root = temporary_workspace("delete-job");
        let job = Job::new(
            JobSource {
                kind: JobKind::Download,
                url: Some("https://example.com/video".to_string()),
                title: Some("deletion test".to_string()),
                local_path: None,
                segment_minutes: None,
                download_cookies_mode: None,
                download_cookies_file: None,
                download_cookies_from_browser: None,
                media_save_mode: MediaSaveMode::default(),
            },
            PipelineOptions::default(),
        );
        let job_dir = create_job_directories(&workspace_root, &job).expect("create job");
        let media_path = job_dir.join("media").join("segment.mp4");
        fs::write(&media_path, b"test media").expect("write test media");

        delete_job(&workspace_root, &job.id).expect("delete job");

        assert!(!job_dir.exists());
        let load_error = load_job(&workspace_root, &job.id).expect_err("job must be absent");
        assert!(load_error.to_string().contains("任务不存在"));
        fs::remove_dir_all(workspace_root).expect("remove test workspace");
    }

    #[test]
    fn recovers_jobs_left_running() {
        let workspace_root = temporary_workspace("recovery");
        let mut job = Job::new(
            JobSource {
                kind: JobKind::Download,
                url: Some("https://example.com/video".to_string()),
                title: None,
                local_path: None,
                segment_minutes: None,
                download_cookies_mode: None,
                download_cookies_file: None,
                download_cookies_from_browser: None,
                media_save_mode: MediaSaveMode::default(),
            },
            PipelineOptions::default(),
        );
        job.status = JobStatus::Running;
        job.live_capture_active = true;
        create_job_directories(&workspace_root, &job).expect("create job");

        assert_eq!(
            recover_interrupted_jobs(&workspace_root).expect("recover"),
            1
        );
        let recovered = load_job(&workspace_root, &job.id).expect("load recovered job");
        assert_eq!(recovered.status, JobStatus::Failed);
        assert!(recovered.current_step.is_none());
        assert!(!recovered.live_capture_active);
        assert!(recovered
            .error_message
            .as_deref()
            .is_some_and(|message| message.contains("已中断")));

        fs::remove_dir_all(workspace_root).expect("remove test workspace");
    }

    #[test]
    fn recovers_queued_jobs_to_pending() {
        let workspace_root = temporary_workspace("queued-recovery");
        let mut job = Job::new(
            JobSource {
                kind: JobKind::Download,
                url: Some("https://example.com/video".to_string()),
                title: None,
                local_path: None,
                segment_minutes: None,
                download_cookies_mode: None,
                download_cookies_file: None,
                download_cookies_from_browser: None,
                media_save_mode: MediaSaveMode::default(),
            },
            PipelineOptions::default(),
        );
        job.status = JobStatus::Queued;
        create_job_directories(&workspace_root, &job).expect("create job");

        assert_eq!(
            recover_interrupted_jobs(&workspace_root).expect("recover"),
            1
        );
        let recovered = load_job(&workspace_root, &job.id).expect("load recovered job");
        assert_eq!(recovered.status, JobStatus::Pending);

        fs::remove_dir_all(workspace_root).expect("remove test workspace");
    }

    #[test]
    fn health_scan_finds_orphan_and_corrupt() {
        let workspace_root = temporary_workspace("health-scan");
        let paths = WorkspacePaths::from_root(&workspace_root);
        paths.ensure().expect("ensure");

        let orphan_dir = paths.jobs.join("not-a-uuid");
        fs::create_dir_all(&orphan_dir).expect("orphan dir");

        let corrupt_id = Uuid::new_v4().to_string();
        let corrupt_dir = paths.jobs.join(&corrupt_id);
        fs::create_dir_all(&corrupt_dir).expect("corrupt dir");
        fs::write(corrupt_dir.join("source.json"), "{not-json").expect("write corrupt");

        let report =
            inspect_workspace_health(&workspace_root, 5, &HashSet::new()).expect("inspect");
        assert_eq!(report.orphan_directories.len(), 1);
        assert_eq!(report.corrupt_jobs.len(), 1);

        fs::remove_dir_all(workspace_root).expect("remove test workspace");
    }
}

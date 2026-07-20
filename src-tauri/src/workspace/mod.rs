use crate::error::{AppError, AppResult};
use crate::models::{Job, JobStatus, StepStatus};
use crate::storage;
use serde::{Deserialize, Serialize};
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
        if job.status != JobStatus::Running {
            continue;
        }

        let detail = "应用上次退出时任务仍在运行；后台进程已中断，请重试失败步骤".to_string();
        job.status = JobStatus::Failed;
        job.stop_requested = false;
        job.live_capture_active = false;
        job.error_message = Some(detail.clone());
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
    Ok(recovered_count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{JobKind, JobSource, PipelineOptions};

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
}

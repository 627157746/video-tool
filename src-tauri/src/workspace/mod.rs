use crate::error::{AppError, AppResult};
use crate::models::Job;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

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

    pub fn job_dir(&self, job_id: &str) -> PathBuf {
        self.jobs.join(job_id)
    }
}

pub fn create_job_directories(workspace_root: impl AsRef<Path>, job: &Job) -> AppResult<PathBuf> {
    let paths = WorkspacePaths::from_root(workspace_root);
    paths.ensure()?;

    let job_dir = paths.job_dir(&job.id);
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
    let source_json = serde_json::to_string_pretty(job)?;
    fs::write(source_path, source_json)?;

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
                eprintln!(
                    "skip invalid job at {}: {error}",
                    entry.path().display()
                );
            }
        }
    }

    jobs.sort_by(|left, right| right.created_at.cmp(&left.created_at));
    Ok(jobs)
}

pub fn load_job(workspace_root: impl AsRef<Path>, job_id: &str) -> AppResult<Job> {
    let job_dir = WorkspacePaths::from_root(workspace_root).job_dir(job_id);
    let source_path = job_dir.join("source.json");
    if !source_path.exists() {
        return Err(AppError::message(format!("任务不存在: {job_id}")));
    }
    let raw = fs::read_to_string(source_path)?;
    Ok(serde_json::from_str(&raw)?)
}

pub fn save_job(workspace_root: impl AsRef<Path>, job: &Job) -> AppResult<()> {
    let job_dir = WorkspacePaths::from_root(workspace_root).job_dir(&job.id);
    fs::create_dir_all(&job_dir)?;
    let source_path = job_dir.join("source.json");
    fs::write(source_path, serde_json::to_string_pretty(job)?)?;
    Ok(())
}

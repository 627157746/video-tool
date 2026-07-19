use crate::config::{AppConfig, AppConfigPublic};
use crate::error::AppResult;
use crate::models::{
    CreateDownloadJobRequest, CreateImportJobRequest, CreateLiveRecordJobRequest, Job, JobKind,
    JobListItem, JobSource, PipelineOptions,
};
use crate::sidecar::{self, SidecarStatus};
use crate::workspace;
use chrono::Utc;
use std::sync::Mutex;
use tauri::State;

pub struct AppState {
    pub config: Mutex<AppConfig>,
}

#[tauri::command]
pub fn get_app_info() -> AppInfo {
    AppInfo {
        name: "video-tool".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        description: "下载 / 直播录制 / 本地转写 / AI 总结".to_string(),
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct AppInfo {
    pub name: String,
    pub version: String,
    pub description: String,
}

#[tauri::command]
pub fn get_config(state: State<'_, AppState>) -> AppResult<AppConfigPublic> {
    let config = state.config.lock().expect("config lock");
    Ok(config.public_view())
}

#[tauri::command]
pub fn reload_config(state: State<'_, AppState>) -> AppResult<AppConfigPublic> {
    let loaded = AppConfig::load_or_init()?;
    loaded.ensure_workspace()?;
    let public_view = loaded.public_view();
    let mut config = state.config.lock().expect("config lock");
    *config = loaded;
    Ok(public_view)
}

#[tauri::command]
pub fn list_jobs(state: State<'_, AppState>) -> AppResult<Vec<JobListItem>> {
    let config = state.config.lock().expect("config lock");
    config.ensure_workspace()?;
    let jobs = workspace::list_jobs(config.workspace_path())?;
    Ok(jobs.iter().map(Job::to_list_item).collect())
}

#[tauri::command]
pub fn get_job(state: State<'_, AppState>, job_id: String) -> AppResult<Job> {
    let config = state.config.lock().expect("config lock");
    workspace::load_job(config.workspace_path(), &job_id)
}

#[tauri::command]
pub fn create_download_job(
    state: State<'_, AppState>,
    request: CreateDownloadJobRequest,
) -> AppResult<Job> {
    let config = state.config.lock().expect("config lock");
    config.ensure_workspace()?;

    let url = request.url.trim().to_string();
    if url.is_empty() {
        return Err(crate::error::AppError::message("下载链接不能为空"));
    }

    let pipeline = merge_pipeline(&config, request.pipeline);
    let job = Job::new(
        JobSource {
            kind: JobKind::Download,
            url: Some(url),
            title: request.title,
            local_path: None,
            segment_minutes: None,
        },
        pipeline,
    );

    workspace::create_job_directories(config.workspace_path(), &job)?;
    Ok(job)
}

#[tauri::command]
pub fn create_live_record_job(
    state: State<'_, AppState>,
    request: CreateLiveRecordJobRequest,
) -> AppResult<Job> {
    let config = state.config.lock().expect("config lock");
    config.ensure_workspace()?;

    let url = request.url.trim().to_string();
    if url.is_empty() {
        return Err(crate::error::AppError::message("直播地址不能为空"));
    }

    let pipeline = merge_pipeline(&config, request.pipeline);
    let segment_minutes = request
        .segment_minutes
        .unwrap_or(config.default_segment_minutes)
        .max(1);

    let job = Job::new(
        JobSource {
            kind: JobKind::LiveRecord,
            url: Some(url),
            title: request.title,
            local_path: None,
            segment_minutes: Some(segment_minutes),
        },
        pipeline,
    );

    workspace::create_job_directories(config.workspace_path(), &job)?;
    Ok(job)
}

#[tauri::command]
pub fn create_import_job(
    state: State<'_, AppState>,
    request: CreateImportJobRequest,
) -> AppResult<Job> {
    let config = state.config.lock().expect("config lock");
    config.ensure_workspace()?;

    let local_path = request.local_path.trim().to_string();
    if local_path.is_empty() {
        return Err(crate::error::AppError::message("本地路径不能为空"));
    }

    let pipeline = merge_pipeline(&config, request.pipeline);
    let job = Job::new(
        JobSource {
            kind: JobKind::ImportLocal,
            url: None,
            title: request.title,
            local_path: Some(local_path),
            segment_minutes: None,
        },
        pipeline,
    );

    workspace::create_job_directories(config.workspace_path(), &job)?;
    Ok(job)
}

#[tauri::command]
pub fn open_job_directory(state: State<'_, AppState>, job_id: String) -> AppResult<String> {
    let config = state.config.lock().expect("config lock");
    let job_dir = config.workspace_path().join("jobs").join(&job_id);
    if !job_dir.exists() {
        return Err(crate::error::AppError::message(format!(
            "任务目录不存在: {}",
            job_dir.display()
        )));
    }
    Ok(job_dir.to_string_lossy().replace('\\', "/"))
}

#[tauri::command]
pub fn probe_sidecars(state: State<'_, AppState>) -> AppResult<SidecarStatus> {
    let config = state.config.lock().expect("config lock");
    Ok(sidecar::resolve_all(&config.sidecar_paths, None))
}

#[tauri::command]
pub fn mark_job_placeholder_failed(
    state: State<'_, AppState>,
    job_id: String,
    message: String,
) -> AppResult<Job> {
    let config = state.config.lock().expect("config lock");
    let mut job = workspace::load_job(config.workspace_path(), &job_id)?;
    job.status = crate::models::JobStatus::Failed;
    job.error_message = Some(message);
    job.updated_at = Utc::now();
    workspace::save_job(config.workspace_path(), &job)?;
    Ok(job)
}

fn merge_pipeline(config: &AppConfig, request: Option<PipelineOptions>) -> PipelineOptions {
    let mut pipeline = request.unwrap_or(PipelineOptions {
        auto_transcribe: config.default_auto_transcribe,
        auto_summarize: config.default_auto_summarize,
        provider_profile_id: config.default_provider_profile_id.clone(),
        template_id: config.default_template_id.clone(),
    });

    if pipeline.provider_profile_id.is_none() {
        pipeline.provider_profile_id = config.default_provider_profile_id.clone();
    }
    if pipeline.template_id.is_none() {
        pipeline.template_id = config.default_template_id.clone();
    }
    pipeline
}

use crate::config::{AppConfig, AppConfigPublic};
use crate::error::{AppError, AppResult};
use crate::models::{
    CreateDownloadJobRequest, CreateDownloadJobsBatchRequest, CreateDownloadJobsBatchResponse,
    CreateImportJobRequest, CreateLiveRecordJobRequest, ExportJobRequest, Job, JobKind,
    JobListItem, JobLogRequest, JobSource, PipelineOptions, RetryTranscriptSegmentRequest,
    RunJobRequest, SaveConfigRequest, SelectSegmentsRequest, TestProviderRequest,
    UpdateJobGroupRequest, UpdateJobPipelineRequest, UpdateJobTitleRequest,
};
use crate::pipeline::{self, RunnerState};
use crate::sidecar::SidecarStatus;
use crate::workspace;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, State};

pub struct AppState {
    pub config: Mutex<AppConfig>,
    pub operation_lock: Mutex<()>,
    pub runner: Arc<RunnerState>,
}

#[tauri::command]
pub fn get_app_info() -> AppInfo {
    AppInfo {
        name: "video-tool".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        description: "下载 / 直播录制 / 本地转写 / AI 总结".to_string(),
    }
}

#[derive(Debug, Clone, Serialize)]
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
    let _operation_guard = state.operation_lock.lock().expect("operation lock");
    let loaded = AppConfig::load_or_init()?;
    loaded.validate()?;
    let current_workspace = state
        .config
        .lock()
        .expect("config lock")
        .workspace_dir
        .clone();
    if state.runner.has_running_jobs() && loaded.workspace_dir != current_workspace {
        return Err(AppError::message("任务运行期间不能切换工作区"));
    }
    loaded.ensure_workspace()?;
    if loaded.workspace_dir != current_workspace {
        workspace::recover_interrupted_jobs(loaded.workspace_path())?;
    }
    let public_view = loaded.public_view();
    let mut config = state.config.lock().expect("config lock");
    *config = loaded;
    Ok(public_view)
}

#[tauri::command]
pub fn save_config(
    app: AppHandle,
    state: State<'_, AppState>,
    request: SaveConfigRequest,
) -> AppResult<AppConfigPublic> {
    let _operation_guard = state.operation_lock.lock().expect("operation lock");
    let current_config = state.config.lock().expect("config lock").clone();
    let candidate = current_config.candidate_with_update(request)?;
    let workspace_changed = candidate.workspace_dir != current_config.workspace_dir;
    if workspace_changed && state.runner.has_running_jobs() {
        return Err(AppError::message("任务运行期间不能切换工作区"));
    }

    candidate.ensure_workspace()?;
    if workspace_changed {
        workspace::recover_interrupted_jobs(candidate.workspace_path())?;
    }

    let removed_group_ids = collect_removed_job_group_ids(&current_config, &candidate);
    if !removed_group_ids.is_empty() {
        let workspace_path = candidate.workspace_path();
        let jobs = workspace::list_jobs(&workspace_path)?;
        for job in &jobs {
            let Some(group_id) = job
                .group
                .as_ref()
                .map(|value| value.trim())
                .filter(|value| !value.is_empty())
            else {
                continue;
            };
            if !removed_group_ids.contains(group_id) {
                continue;
            }
            if state.runner.is_job_running(&job.id) {
                return Err(AppError::message(format!(
                    "任务「{}」仍在运行且属于将被删除的分组，请等待结束后再保存",
                    job.display_title()
                )));
            }
        }
        for mut job in jobs {
            let Some(group_id) = job
                .group
                .as_ref()
                .map(|value| value.trim())
                .filter(|value| !value.is_empty())
                .map(|value| value.to_string())
            else {
                continue;
            };
            if !removed_group_ids.contains(group_id.as_str()) {
                continue;
            }
            job.group = None;
            job.updated_at = chrono::Utc::now();
            workspace::save_job(&workspace_path, &job)?;
            use tauri::Emitter;
            let _ = app.emit("job-updated", &job);
        }
    }

    candidate.save()?;
    let public_view = candidate.public_view();
    *state.config.lock().expect("config lock") = candidate;
    Ok(public_view)
}

#[tauri::command]
pub fn list_jobs(state: State<'_, AppState>) -> AppResult<Vec<JobListItem>> {
    let config = state.config.lock().expect("config lock");
    config.ensure_workspace()?;
    let jobs = workspace::list_jobs(config.workspace_path())?;
    Ok(jobs
        .iter()
        .map(|job| {
            let mut item = job.to_list_item();
            if item.status == crate::models::JobStatus::Queued {
                item.queue_position = state.runner.queue_position(&job.id);
            }
            item
        })
        .collect())
}

#[tauri::command]
pub fn get_job(state: State<'_, AppState>, job_id: String) -> AppResult<Job> {
    let config = state.config.lock().expect("config lock");
    workspace::load_job(config.workspace_path(), &job_id)
}

#[tauri::command]
pub fn delete_job(state: State<'_, AppState>, job_id: String) -> AppResult<()> {
    let _operation_guard = state.operation_lock.lock().expect("operation lock");
    if state.runner.is_job_running(&job_id) {
        return Err(AppError::message(
            "任务运行期间不能删除，请等待当前步骤结束",
        ));
    }

    let _ = state.runner.remove_from_queue(&job_id);
    let workspace_path = state.config.lock().expect("config lock").workspace_path();
    let _ = crate::search::remove_job(&workspace_path, &job_id);
    workspace::delete_job(&workspace_path, &job_id)
}

#[tauri::command]
pub fn create_download_job(
    app: AppHandle,
    state: State<'_, AppState>,
    request: CreateDownloadJobRequest,
) -> AppResult<Job> {
    let _operation_guard = state.operation_lock.lock().expect("operation lock");
    let (job, should_start, config_snapshot, runner) = {
        let mut config = state.config.lock().expect("config lock");
        config.ensure_workspace()?;

        let url = request.url.trim().to_string();
        if url.is_empty() {
            return Err(AppError::message("下载链接不能为空"));
        }
        // Persist the raw paste (including Douyin share text) so retries keep
        // the original input; the download step extracts the real URL.

        let pipeline = merge_pipeline(&config, request.pipeline);
        let mut job = Job::new(
            JobSource {
                kind: JobKind::Download,
                url: Some(url),
                title: request.title,
                local_path: None,
                segment_minutes: None,
                download_cookies_mode: None,
                download_cookies_file: None,
                download_cookies_from_browser: None,
            },
            pipeline,
        );
        let previous_group_count = config.job_groups.len();
        job.group = config.resolve_or_create_job_group(request.group)?;
        job.batch_id = request
            .batch_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| value.to_string());
        apply_download_cookies_to_source(
            &mut job.source,
            request.download_cookies_mode,
            request.download_cookies_file,
            request.download_cookies_from_browser,
        )?;
        if config.job_groups.len() != previous_group_count {
            config.save()?;
        }

        workspace::create_job_directories(config.workspace_path(), &job)?;
        (
            job,
            request.auto_start,
            config.clone(),
            Arc::clone(&state.runner),
        )
    };

    if should_start {
        pipeline::spawn_job_run(app, config_snapshot, runner, job.id.clone(), None)?;
    }

    Ok(job)
}

#[tauri::command]
pub fn create_download_jobs_batch(
    app: AppHandle,
    state: State<'_, AppState>,
    request: CreateDownloadJobsBatchRequest,
) -> AppResult<CreateDownloadJobsBatchResponse> {
    let _operation_guard = state.operation_lock.lock().expect("operation lock");
    let entries = Job::split_download_url_entries(&request.urls_text);
    if entries.is_empty() {
        return Err(AppError::message("请至少粘贴一个下载链接"));
    }

    let batch_id = if entries.len() > 1 {
        Some(uuid::Uuid::new_v4().to_string())
    } else {
        None
    };

    let title_prefix = request
        .title
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string());

    let (created_jobs, config_snapshot, runner, should_start) = {
        let mut config = state.config.lock().expect("config lock");
        config.ensure_workspace()?;
        let pipeline = merge_pipeline(&config, request.pipeline);
        let previous_group_count = config.job_groups.len();
        let resolved_group = config.resolve_or_create_job_group(request.group)?;
        if config.job_groups.len() != previous_group_count {
            config.save()?;
        }

        let mut created_jobs = Vec::with_capacity(entries.len());
        for (entry_index, entry_url) in entries.iter().enumerate() {
            let job_title = title_prefix.as_ref().map(|prefix| {
                if entries.len() == 1 {
                    prefix.clone()
                } else {
                    format!("{prefix} ({})", entry_index + 1)
                }
            });
            let mut job = Job::new(
                JobSource {
                    kind: JobKind::Download,
                    url: Some(entry_url.clone()),
                    title: job_title,
                    local_path: None,
                    segment_minutes: None,
                    download_cookies_mode: None,
                    download_cookies_file: None,
                    download_cookies_from_browser: None,
                },
                pipeline.clone(),
            );
            job.group = resolved_group.clone();
            job.batch_id = batch_id.clone();
            apply_download_cookies_to_source(
                &mut job.source,
                request.download_cookies_mode.clone(),
                request.download_cookies_file.clone(),
                request.download_cookies_from_browser.clone(),
            )?;
            workspace::create_job_directories(config.workspace_path(), &job)?;
            created_jobs.push(job);
        }

        (
            created_jobs,
            config.clone(),
            Arc::clone(&state.runner),
            request.auto_start,
        )
    };

    if should_start {
        for job in &created_jobs {
            // Each start is independent: a full queue simply marks later jobs as queued.
            // Failure to start one job must not roll back already-created siblings.
            if let Err(error) = pipeline::spawn_job_run(
                app.clone(),
                config_snapshot.clone(),
                Arc::clone(&runner),
                job.id.clone(),
                None,
            ) {
                eprintln!("failed to queue download job {}: {error}", job.id);
            }
        }
    }

    Ok(CreateDownloadJobsBatchResponse {
        batch_id,
        jobs: created_jobs,
        skipped: Vec::new(),
    })
}

#[tauri::command]
pub fn create_live_record_job(
    app: AppHandle,
    state: State<'_, AppState>,
    request: CreateLiveRecordJobRequest,
) -> AppResult<Job> {
    let _operation_guard = state.operation_lock.lock().expect("operation lock");
    let (job, should_start, config_snapshot, runner) = {
        let mut config = state.config.lock().expect("config lock");
        config.ensure_workspace()?;

        let url = request.url.trim().to_string();
        if url.is_empty() {
            return Err(AppError::message("直播地址不能为空"));
        }

        let pipeline = merge_pipeline(&config, request.pipeline);
        let segment_minutes = request
            .segment_minutes
            .unwrap_or(config.default_segment_minutes)
            .max(1);

        let mut job = Job::new(
            JobSource {
                kind: JobKind::LiveRecord,
                url: Some(url),
                title: request.title,
                local_path: None,
                segment_minutes: Some(segment_minutes),
                download_cookies_mode: None,
                download_cookies_file: None,
                download_cookies_from_browser: None,
            },
            pipeline,
        );
        let previous_group_count = config.job_groups.len();
        job.group = config.resolve_or_create_job_group(request.group)?;
        if config.job_groups.len() != previous_group_count {
            config.save()?;
        }

        workspace::create_job_directories(config.workspace_path(), &job)?;
        (
            job,
            request.auto_start,
            config.clone(),
            Arc::clone(&state.runner),
        )
    };

    if should_start {
        pipeline::spawn_job_run(app, config_snapshot, runner, job.id.clone(), None)?;
    }

    Ok(job)
}

#[tauri::command]
pub fn create_import_job(
    app: AppHandle,
    state: State<'_, AppState>,
    request: CreateImportJobRequest,
) -> AppResult<Job> {
    let _operation_guard = state.operation_lock.lock().expect("operation lock");
    let (job, should_start, config_snapshot, runner) = {
        let mut config = state.config.lock().expect("config lock");
        config.ensure_workspace()?;

        let local_path = request.local_path.trim().to_string();
        if local_path.is_empty() {
            return Err(AppError::message("本地路径不能为空"));
        }

        let pipeline = merge_pipeline(&config, request.pipeline);
        let mut job = Job::new(
            JobSource {
                kind: JobKind::ImportLocal,
                url: None,
                title: request.title,
                local_path: Some(local_path),
                segment_minutes: None,
                download_cookies_mode: None,
                download_cookies_file: None,
                download_cookies_from_browser: None,
            },
            pipeline,
        );
        let previous_group_count = config.job_groups.len();
        job.group = config.resolve_or_create_job_group(request.group)?;
        if config.job_groups.len() != previous_group_count {
            config.save()?;
        }

        workspace::create_job_directories(config.workspace_path(), &job)?;
        (
            job,
            request.auto_start,
            config.clone(),
            Arc::clone(&state.runner),
        )
    };

    if should_start {
        pipeline::spawn_job_run(app, config_snapshot, runner, job.id.clone(), None)?;
    }

    Ok(job)
}

#[tauri::command]
pub fn run_job(
    app: AppHandle,
    state: State<'_, AppState>,
    request: RunJobRequest,
) -> AppResult<()> {
    let _operation_guard = state.operation_lock.lock().expect("operation lock");
    let config = state.config.lock().expect("config lock").clone();
    config.ensure_workspace()?;
    let _ = workspace::load_job(config.workspace_path(), &request.job_id)?;
    pipeline::spawn_job_run(
        app,
        config,
        Arc::clone(&state.runner),
        request.job_id,
        request.step,
    )
}

#[tauri::command]
pub fn retry_job_step(
    app: AppHandle,
    state: State<'_, AppState>,
    request: RunJobRequest,
) -> AppResult<()> {
    run_job(app, state, request)
}

#[tauri::command]
pub fn retry_transcript_segment(
    app: AppHandle,
    state: State<'_, AppState>,
    request: RetryTranscriptSegmentRequest,
) -> AppResult<()> {
    let _operation_guard = state.operation_lock.lock().expect("operation lock");
    let config = state.config.lock().expect("config lock").clone();
    pipeline::spawn_transcript_segment_retry(
        app,
        config,
        Arc::clone(&state.runner),
        request.job_id,
        request.segment_id,
    )
}

#[tauri::command]
pub fn stop_recording(
    app: AppHandle,
    state: State<'_, AppState>,
    job_id: String,
) -> AppResult<Job> {
    let config = state.config.lock().expect("config lock");
    let mut job = workspace::load_job(config.workspace_path(), &job_id)?;
    if job.source.kind != JobKind::LiveRecord {
        return Err(AppError::message("只有直播录制任务支持停止"));
    }
    if !state.runner.request_stop(&job_id) {
        return Err(AppError::message("该任务当前没有可停止的运行进程"));
    }
    // The runner owns the persisted Job snapshot. Avoid writing this stale
    // command-side copy over final media and step state while ffmpeg exits.
    job.stop_requested = true;
    use tauri::Emitter;
    let _ = app.emit("job-updated", &job);
    Ok(job)
}

#[tauri::command]
pub fn select_job_segments(
    app: AppHandle,
    state: State<'_, AppState>,
    request: SelectSegmentsRequest,
) -> AppResult<Job> {
    let _operation_guard = state.operation_lock.lock().expect("operation lock");
    if request.segment_ids.is_empty() {
        return Err(AppError::message("总结范围至少选择一个转写分段"));
    }
    let config = state.config.lock().expect("config lock");
    if state.runner.is_job_running(&request.job_id) {
        return Err(AppError::message("任务运行期间不能修改总结选段"));
    }
    let mut job = workspace::load_job(config.workspace_path(), &request.job_id)?;
    let known_ids: std::collections::HashSet<&str> = job
        .transcript_segments
        .iter()
        .map(|segment| segment.id.as_str())
        .collect();
    let unknown: Vec<&str> = request
        .segment_ids
        .iter()
        .map(String::as_str)
        .filter(|id| !known_ids.contains(id))
        .collect();
    if !unknown.is_empty() {
        return Err(AppError::message(format!(
            "包含未知转写分段: {}",
            unknown.join(", ")
        )));
    }
    job.selected_segment_ids = request.segment_ids;
    for segment in &mut job.media_segments {
        segment.selected_for_summary = job.selected_segment_ids.contains(&segment.id);
    }
    job.invalidate_after_step(&crate::models::JobStep::Transcribe);
    let job_dir = workspace::validated_job_dir(config.workspace_path(), &job.id)?;
    job.refresh_derived_status();
    job.updated_at = chrono::Utc::now();
    workspace::save_job(config.workspace_path(), &job)?;
    use tauri::Emitter;
    let _ = app.emit("job-updated", &job);
    pipeline::paths::remove_downstream_artifacts(&job_dir, &crate::models::JobStep::Transcribe)?;
    Ok(job)
}

#[tauri::command]
pub fn update_job_title(
    app: AppHandle,
    state: State<'_, AppState>,
    request: UpdateJobTitleRequest,
) -> AppResult<Job> {
    let _operation_guard = state.operation_lock.lock().expect("operation lock");
    // Runner owns the live Job snapshot; concurrent disk writes can clobber a
    // rename that lands between progress updates.
    if state.runner.is_job_running(&request.job_id) {
        return Err(AppError::message("任务运行期间不能修改标题"));
    }

    let config = state.config.lock().expect("config lock");
    let mut job = workspace::load_job(config.workspace_path(), &request.job_id)?;

    let next_title = normalize_optional_id(request.title);
    let current_title = job
        .source
        .title
        .as_ref()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string());
    if current_title == next_title {
        return Ok(job);
    }

    job.source.title = next_title;
    job.updated_at = chrono::Utc::now();
    workspace::save_job(config.workspace_path(), &job)?;
    use tauri::Emitter;
    let _ = app.emit("job-updated", &job);
    Ok(job)
}

#[tauri::command]
pub fn update_job_group(
    app: AppHandle,
    state: State<'_, AppState>,
    request: UpdateJobGroupRequest,
) -> AppResult<Job> {
    let _operation_guard = state.operation_lock.lock().expect("operation lock");
    // Runner owns the live Job snapshot; concurrent disk writes can clobber a
    // group change that lands between progress updates.
    if state.runner.is_job_running(&request.job_id) {
        return Err(AppError::message("任务运行期间不能修改分组"));
    }

    let mut config = state.config.lock().expect("config lock");
    let mut job = workspace::load_job(config.workspace_path(), &request.job_id)?;

    let previous_group_count = config.job_groups.len();
    let next_group = config.resolve_or_create_job_group(request.group)?;
    if config.job_groups.len() != previous_group_count {
        config.save()?;
    }
    let current_group = job
        .group
        .as_ref()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string());
    if current_group == next_group {
        return Ok(job);
    }

    job.group = next_group;
    job.updated_at = chrono::Utc::now();
    workspace::save_job(config.workspace_path(), &job)?;
    use tauri::Emitter;
    let _ = app.emit("job-updated", &job);
    Ok(job)
}

#[tauri::command]
pub fn update_job_pipeline(
    app: AppHandle,
    state: State<'_, AppState>,
    request: UpdateJobPipelineRequest,
) -> AppResult<Job> {
    let _operation_guard = state.operation_lock.lock().expect("operation lock");
    if state.runner.is_job_running(&request.job_id) {
        return Err(AppError::message("任务运行期间不能修改总结配置"));
    }

    let config = state.config.lock().expect("config lock");
    let mut job = workspace::load_job(config.workspace_path(), &request.job_id)?;

    let provider_profile_id = normalize_optional_id(request.provider_profile_id);
    let template_id = normalize_optional_id(request.template_id);
    let model = normalize_optional_id(request.model);
    let template_ids = request.template_ids.map(|ids| {
        let mut ordered: Vec<String> = ids
            .into_iter()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .collect();
        let mut seen = std::collections::HashSet::new();
        ordered.retain(|id| seen.insert(id.clone()));
        ordered
    });

    if let Some(provider_id) = provider_profile_id.as_deref() {
        if !config
            .providers
            .iter()
            .any(|provider| provider.id == provider_id)
        {
            return Err(AppError::message(format!(
                "Provider 档案不存在: {provider_id}"
            )));
        }
    }
    if let Some(template_id_value) = template_id.as_deref() {
        if !config
            .templates
            .iter()
            .any(|template| template.id == template_id_value)
        {
            return Err(AppError::message(format!(
                "总结模板不存在: {template_id_value}"
            )));
        }
    }
    if let Some(ids) = template_ids.as_ref() {
        for template_id_value in ids {
            if !config
                .templates
                .iter()
                .any(|template| &template.id == template_id_value)
            {
                return Err(AppError::message(format!(
                    "总结模板不存在: {template_id_value}"
                )));
            }
        }
    }

    let provider_changed = job.pipeline.provider_profile_id != provider_profile_id;
    let template_changed = job.pipeline.template_id != template_id;
    let template_ids_changed = template_ids
        .as_ref()
        .is_some_and(|ids| ids != &job.pipeline.template_ids);
    let model_changed = job.pipeline.model != model;
    if !provider_changed && !template_changed && !template_ids_changed && !model_changed {
        return Ok(job);
    }

    job.pipeline.provider_profile_id = provider_profile_id;
    if let Some(ids) = template_ids {
        job.pipeline.template_ids = ids.clone();
        job.pipeline.template_id = ids.first().cloned();
    } else {
        job.pipeline.template_id = template_id;
        if let Some(primary) = job.pipeline.template_id.clone() {
            if job.pipeline.template_ids.is_empty() {
                job.pipeline.template_ids = vec![primary];
            } else if let Some(first) = job.pipeline.template_ids.first_mut() {
                *first = primary;
            }
        }
    }
    job.pipeline.model = model;
    // Changing summarize inputs only invalidates the summarize step / artifact.
    job.invalidate_after_step(&crate::models::JobStep::MergeTranscript);
    let job_dir = workspace::validated_job_dir(config.workspace_path(), &job.id)?;
    pipeline::paths::remove_downstream_artifacts(
        &job_dir,
        &crate::models::JobStep::MergeTranscript,
    )?;
    job.refresh_derived_status();
    job.updated_at = chrono::Utc::now();
    workspace::save_job(config.workspace_path(), &job)?;
    use tauri::Emitter;
    let _ = app.emit("job-updated", &job);
    Ok(job)
}

fn normalize_optional_id(value: Option<String>) -> Option<String> {
    value
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string())
}

fn collect_removed_job_group_ids(
    previous_config: &AppConfig,
    next_config: &AppConfig,
) -> std::collections::HashSet<String> {
    let next_group_ids: std::collections::HashSet<&str> = next_config
        .job_groups
        .iter()
        .map(|group| group.id.as_str())
        .collect();
    previous_config
        .job_groups
        .iter()
        .map(|group| group.id.clone())
        .filter(|group_id| !next_group_ids.contains(group_id.as_str()))
        .collect()
}

#[tauri::command]
pub fn export_job(state: State<'_, AppState>, request: ExportJobRequest) -> AppResult<String> {
    let _operation_guard = state.operation_lock.lock().expect("operation lock");
    if state.runner.is_job_running(&request.job_id) {
        return Err(AppError::message(
            "任务运行期间不能导出，请等待当前步骤结束",
        ));
    }
    let (workspace_path, secrets) = {
        let config = state.config.lock().expect("config lock");
        (config.workspace_path(), config.secret_values())
    };
    pipeline::export::export_job_package(
        &workspace_path,
        &request.job_id,
        request.destination_dir.as_deref(),
        &secrets,
    )
}

#[tauri::command]
pub fn test_provider(
    state: State<'_, AppState>,
    request: TestProviderRequest,
) -> AppResult<String> {
    let config = state.config.lock().expect("config lock").clone();
    pipeline::summarize::test_provider(&config, &request.provider_profile_id)
}

#[tauri::command]
pub fn get_job_transcript(state: State<'_, AppState>, job_id: String) -> AppResult<String> {
    let config = state.config.lock().expect("config lock");
    let job_dir = workspace::validated_job_dir(config.workspace_path(), &job_id)?;
    let path = job_dir.join("transcript").join("plain.txt");
    if !path.exists() {
        return Ok(String::new());
    }
    Ok(std::fs::read_to_string(path)?)
}

#[tauri::command]
pub fn get_job_summary(state: State<'_, AppState>, job_id: String) -> AppResult<String> {
    let config = state.config.lock().expect("config lock");
    let job_dir = workspace::validated_job_dir(config.workspace_path(), &job_id)?;
    let path = job_dir.join("summary").join("summary.md");
    if !path.exists() {
        return Ok(String::new());
    }
    Ok(std::fs::read_to_string(path)?)
}

#[derive(Debug, Clone, Serialize)]
pub struct SummaryTemplateArtifact {
    pub template_id: String,
    pub path: String,
    pub content: String,
    pub primary: bool,
}

#[tauri::command]
pub fn get_job_summaries(
    state: State<'_, AppState>,
    job_id: String,
) -> AppResult<Vec<SummaryTemplateArtifact>> {
    let config = state.config.lock().expect("config lock");
    let job_dir = workspace::validated_job_dir(config.workspace_path(), &job_id)?;
    let mut artifacts = Vec::new();
    let primary_path = job_dir.join("summary").join("summary.md");
    if primary_path.exists() {
        let content = std::fs::read_to_string(&primary_path).unwrap_or_default();
        let template_id = std::fs::read_to_string(job_dir.join("summary").join("meta.json"))
            .ok()
            .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok())
            .and_then(|value| {
                value
                    .get("template_id")
                    .and_then(|item| item.as_str())
                    .map(|value| value.to_string())
            })
            .unwrap_or_else(|| "primary".to_string());
        artifacts.push(SummaryTemplateArtifact {
            template_id,
            path: "summary/summary.md".to_string(),
            content,
            primary: true,
        });
    }
    let by_template = job_dir.join("summary").join("by_template");
    if by_template.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&by_template) {
            let mut files: Vec<_> = entries.flatten().map(|entry| entry.path()).collect();
            files.sort();
            for path in files {
                if path.extension().and_then(|value| value.to_str()) != Some("md") {
                    continue;
                }
                let file_name = path
                    .file_stem()
                    .map(|value| value.to_string_lossy().to_string())
                    .unwrap_or_else(|| "template".to_string());
                let content = std::fs::read_to_string(&path).unwrap_or_default();
                artifacts.push(SummaryTemplateArtifact {
                    template_id: file_name.clone(),
                    path: format!("summary/by_template/{file_name}.md"),
                    content,
                    primary: false,
                });
            }
        }
    }
    Ok(artifacts)
}

#[tauri::command]
pub fn search_workspace(
    state: State<'_, AppState>,
    query: String,
    limit: Option<u32>,
) -> AppResult<Vec<crate::search::SearchHit>> {
    let config = state.config.lock().expect("config lock");
    config.ensure_workspace()?;
    crate::search::search(&config.workspace_path(), &query, limit.unwrap_or(30))
}

#[tauri::command]
pub fn rebuild_search_index(state: State<'_, AppState>) -> AppResult<u32> {
    let config = state.config.lock().expect("config lock");
    config.ensure_workspace()?;
    crate::search::rebuild_all(&config.workspace_path())
}

#[tauri::command]
pub fn get_job_chapters(state: State<'_, AppState>, job_id: String) -> AppResult<String> {
    let config = state.config.lock().expect("config lock");
    let job_dir = workspace::validated_job_dir(config.workspace_path(), &job_id)?;
    let markdown_path = job_dir.join("transcript").join("chapters.md");
    if markdown_path.exists() {
        return Ok(std::fs::read_to_string(markdown_path)?);
    }
    let json_path = job_dir.join("transcript").join("chapters.json");
    if json_path.exists() {
        return Ok(std::fs::read_to_string(json_path)?);
    }
    Ok(String::new())
}

/// Load current + previous plain text for a transcript segment (quality diff).
#[tauri::command]
pub fn get_transcript_segment_texts(
    state: State<'_, AppState>,
    job_id: String,
    segment_id: String,
) -> AppResult<TranscriptSegmentTexts> {
    let config = state.config.lock().expect("config lock");
    let job = workspace::load_job(config.workspace_path(), &job_id)?;
    let segment = job
        .transcript_segments
        .iter()
        .find(|entry| entry.id == segment_id)
        .ok_or_else(|| AppError::message(format!("转写分段不存在: {segment_id}")))?;
    let job_dir = workspace::validated_job_dir(config.workspace_path(), &job_id)?;
    let current = segment
        .plain_path
        .as_ref()
        .and_then(|relative| std::fs::read_to_string(job_dir.join(relative)).ok())
        .unwrap_or_default();
    let previous = segment
        .plain_path
        .as_ref()
        .map(|relative| {
            let path = PathBuf::from(relative);
            let stem = path
                .file_stem()
                .and_then(|value| value.to_str())
                .unwrap_or("segment");
            let parent = path
                .parent()
                .map(|value| value.to_path_buf())
                .unwrap_or_else(|| PathBuf::from("transcript/segments"));
            job_dir.join(parent).join(format!("{stem}.prev.txt"))
        })
        .and_then(|path| std::fs::read_to_string(path).ok());
    Ok(TranscriptSegmentTexts {
        segment_id,
        current,
        previous,
    })
}

#[derive(Debug, Clone, Serialize)]
pub struct TranscriptSegmentTexts {
    pub segment_id: String,
    pub current: String,
    pub previous: Option<String>,
}

#[tauri::command]
pub fn get_job_log(state: State<'_, AppState>, request: JobLogRequest) -> AppResult<String> {
    let config = state.config.lock().expect("config lock");
    let job_dir = workspace::validated_job_dir(config.workspace_path(), &request.job_id)?;
    if !job_dir.exists() {
        return Err(AppError::message(format!(
            "任务目录不存在: {}",
            job_dir.display()
        )));
    }

    let log_name = sanitize_log_name(&request.log_name)?;
    pipeline::logs::read_log(&job_dir, &log_name, 120_000)
}

#[tauri::command]
pub fn open_job_directory(state: State<'_, AppState>, job_id: String) -> AppResult<String> {
    let config = state.config.lock().expect("config lock");
    let job_dir = workspace::validated_job_dir(config.workspace_path(), &job_id)?;
    if !job_dir.exists() {
        return Err(AppError::message(format!(
            "任务目录不存在: {}",
            job_dir.display()
        )));
    }
    Ok(job_dir.to_string_lossy().replace('\\', "/"))
}

#[tauri::command]
pub fn probe_sidecars(state: State<'_, AppState>) -> AppResult<SidecarStatus> {
    let configured_paths = state
        .config
        .lock()
        .expect("config lock")
        .sidecar_paths
        .clone();
    Ok(state.runner.resolve_sidecars(&configured_paths))
}

#[tauri::command]
pub fn get_dependency_report(
    state: State<'_, AppState>,
) -> AppResult<crate::distribution::DependencyReport> {
    let configured_paths = state
        .config
        .lock()
        .expect("config lock")
        .sidecar_paths
        .clone();
    let status = state.runner.resolve_sidecars(&configured_paths);
    Ok(crate::distribution::build_dependency_report(&status))
}

#[tauri::command]
pub fn list_transcribe_models(
    state: State<'_, AppState>,
) -> AppResult<crate::distribution::ModelInventory> {
    let config = state.config.lock().expect("config lock");
    Ok(crate::distribution::scan_models(&config))
}

#[tauri::command]
pub fn open_transcribe_model_directory(state: State<'_, AppState>) -> AppResult<String> {
    let config = state.config.lock().expect("config lock");
    let inventory = crate::distribution::scan_models(&config);
    if let Some(selected) = inventory.selected_path.as_ref() {
        let path = PathBuf::from(selected);
        let directory = if path.is_file() {
            path.parent()
                .map(|parent| parent.to_path_buf())
                .unwrap_or(path)
        } else {
            path
        };
        if directory.exists() {
            return Ok(directory.to_string_lossy().replace('\\', "/"));
        }
    }
    if let Some(first_dir) = inventory.scan_directories.first() {
        return Ok(first_dir.clone());
    }
    Err(AppError::message(
        "尚未配置转写模型路径；请在流水线设置中指定 GGML 模型文件",
    ))
}

#[tauri::command]
pub fn export_app_config(
    state: State<'_, AppState>,
    include_secrets: Option<bool>,
) -> AppResult<crate::distribution::ConfigExportPackage> {
    let config = state.config.lock().expect("config lock");
    Ok(crate::distribution::export_config_package(
        &config,
        include_secrets.unwrap_or(false),
    ))
}

#[tauri::command]
pub fn import_app_config(
    state: State<'_, AppState>,
    package: crate::distribution::ConfigExportPackage,
    import_secrets: Option<bool>,
) -> AppResult<crate::distribution::ConfigImportResult> {
    let _operation_guard = state.operation_lock.lock().expect("operation lock");
    if state.runner.has_running_jobs() {
        return Err(AppError::message("任务运行期间不能导入配置"));
    }
    let current = state.config.lock().expect("config lock").clone();
    let (next, result) = crate::distribution::apply_import_package(
        &current,
        package,
        import_secrets.unwrap_or(false),
    )?;
    next.ensure_workspace()?;
    next.save()?;
    *state.config.lock().expect("config lock") = next;
    Ok(result)
}

#[tauri::command]
pub fn check_app_update() -> AppResult<crate::distribution::UpdateCheckResult> {
    crate::distribution::check_app_update()
}

#[tauri::command]
pub fn install_app_update(
    app: tauri::AppHandle,
) -> AppResult<crate::distribution::AppUpdateInstallResult> {
    use tauri::Emitter;
    crate::distribution::install_app_update(&mut |progress| {
        let _ = app.emit("app-update-progress", &progress);
    })
}

#[tauri::command]
pub fn get_system_diagnostics(
    state: State<'_, AppState>,
) -> AppResult<crate::distribution::SystemDiagnostics> {
    let config = state.config.lock().expect("config lock").clone();
    config.ensure_workspace()?;
    let status = state.runner.resolve_sidecars(&config.sidecar_paths);
    let active = state.runner.active_job_ids();
    crate::distribution::build_system_diagnostics(&config, status, &active)
}

#[tauri::command]
pub fn check_yt_dlp_update(state: State<'_, AppState>) -> AppResult<String> {
    let configured_paths = state
        .config
        .lock()
        .expect("config lock")
        .sidecar_paths
        .clone();
    let status = state.runner.resolve_sidecars(&configured_paths);
    let binary = status
        .yt_dlp
        .path
        .ok_or_else(|| AppError::message("未找到 yt-dlp，无法检查更新"))?;

    let mut command = std::process::Command::new(&binary);
    crate::sidecar::hide_console_window(&mut command);
    let output = command
        .args(["-U"])
        .output()
        .map_err(|error| AppError::message(format!("执行 yt-dlp -U 失败: {error}")))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}{stderr}").trim().to_string();
    if !output.status.success() {
        return Err(AppError::message(if combined.is_empty() {
            format!("yt-dlp 更新失败，exit {:?}", output.status.code())
        } else {
            format!("yt-dlp 更新失败：{combined}")
        }));
    }
    if combined.is_empty() {
        Ok("yt-dlp 检查并更新完成（无额外输出）".to_string())
    } else {
        Ok(combined)
    }
}

#[tauri::command]
pub fn inspect_workspace_health(
    state: State<'_, AppState>,
) -> AppResult<workspace::WorkspaceHealthReport> {
    let config = state.config.lock().expect("config lock");
    config.ensure_workspace()?;
    let active_ids = state.runner.active_job_ids();
    workspace::inspect_workspace_health(
        config.workspace_path(),
        config.min_free_disk_gb,
        &active_ids,
    )
}

#[tauri::command]
pub fn repair_workspace_health(
    state: State<'_, AppState>,
) -> AppResult<workspace::WorkspaceHealthReport> {
    let _operation_guard = state.operation_lock.lock().expect("operation lock");
    let config = state.config.lock().expect("config lock");
    config.ensure_workspace()?;
    let active_ids = state.runner.active_job_ids();
    workspace::repair_workspace_health(
        config.workspace_path(),
        config.min_free_disk_gb,
        &active_ids,
    )
}

/// Apply download cookie override fields onto a JobSource.
/// Mode `inherit`/empty clears override fields so runtime uses global config.
fn apply_download_cookies_to_source(
    source: &mut JobSource,
    mode: Option<String>,
    cookies_file: Option<String>,
    cookies_from_browser: Option<String>,
) -> AppResult<()> {
    let normalized_mode = mode
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("inherit")
        .to_ascii_lowercase();

    match normalized_mode.as_str() {
        "inherit" | "" => {
            source.download_cookies_mode = None;
            source.download_cookies_file = None;
            source.download_cookies_from_browser = None;
        }
        "none" | "off" | "disable" | "disabled" => {
            source.download_cookies_mode = Some("none".to_string());
            source.download_cookies_file = None;
            source.download_cookies_from_browser = None;
        }
        "file" => {
            let path = cookies_file
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| AppError::message("Cookie 模式为文件时必须提供 cookies.txt 路径"))?;
            source.download_cookies_mode = Some("file".to_string());
            source.download_cookies_file = Some(path.replace('\\', "/"));
            source.download_cookies_from_browser = None;
        }
        "browser" => {
            let browser = cookies_from_browser
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| AppError::message("Cookie 模式为浏览器时必须选择浏览器"))?;
            crate::config::validate_cookies_browser(browser)?;
            source.download_cookies_mode = Some("browser".to_string());
            source.download_cookies_file = None;
            source.download_cookies_from_browser = Some(browser.to_string());
        }
        other => {
            return Err(AppError::message(format!(
                "未知的 Cookie 模式: {other}（可用 inherit / none / file / browser）"
            )));
        }
    }
    Ok(())
}

fn merge_pipeline(config: &AppConfig, request: Option<PipelineOptions>) -> PipelineOptions {
    let request_was_none = request.is_none();
    let mut pipeline = request.unwrap_or(PipelineOptions {
        auto_transcribe: config.default_auto_transcribe,
        auto_summarize: config.default_auto_summarize,
        auto_chapterize: config.default_auto_chapterize && config.default_auto_summarize,
        // Leave provider/template/model as None so summarize resolves the
        // *current* global defaults at run time (not a stale snapshot).
        provider_profile_id: None,
        template_id: None,
        template_ids: Vec::new(),
        model: None,
        transcribe_language: None,
    });

    // Empty/whitespace means "follow current global / provider default".
    // Do not bake default_provider_profile_id / default_template_id here:
    // resolve_provider / resolve_template read those when the job field is None.
    pipeline.provider_profile_id = pipeline
        .provider_profile_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string());
    pipeline.template_id = pipeline
        .template_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string());
    pipeline.template_ids = {
        let mut ordered: Vec<String> = pipeline
            .template_ids
            .iter()
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
            .map(|value| value.to_string())
            .collect();
        let mut seen = std::collections::HashSet::new();
        ordered.retain(|id| seen.insert(id.clone()));
        if ordered.is_empty() {
            if let Some(primary) = pipeline.template_id.clone() {
                ordered.push(primary);
            }
        } else if pipeline.template_id.is_none() {
            pipeline.template_id = ordered.first().cloned();
        }
        ordered
    };
    pipeline.model = pipeline
        .model
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string());
    // `None` (or empty/whitespace) means "follow the global config"; resolve it
    // eagerly so the persisted job snapshot records the effective language and
    // transcribe.rs can read a single source of truth at run time.
    let resolved_transcribe_language = pipeline
        .transcribe_language
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string())
        .unwrap_or_else(|| config.transcribe_language.trim().to_string());
    pipeline.transcribe_language = Some(resolved_transcribe_language);
    if pipeline.auto_summarize {
        pipeline.auto_transcribe = true;
        // Default: chapterize before summarize when global flag is on and
        // the create form did not explicitly set auto_chapterize.
        if request_was_none && config.default_auto_chapterize {
            pipeline.auto_chapterize = true;
        }
    }
    if pipeline.auto_chapterize {
        pipeline.auto_transcribe = true;
    }
    pipeline
}

fn sanitize_log_name(name: &str) -> AppResult<String> {
    // Keep in sync with frontend `LOG_NAMES` / `LogName` in src/constants.ts.
    let allowed = [
        "download",
        "record",
        "transcribe",
        "merge_transcript",
        "chapterize",
        "summarize",
    ];
    if allowed.contains(&name) {
        Ok(name.to_string())
    } else {
        Err(AppError::message(format!("不支持的日志类型: {name}")))
    }
}

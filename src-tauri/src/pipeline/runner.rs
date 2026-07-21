use super::{chapterize, download, error_code, logs, paths, record, summarize, transcribe};
use crate::config::AppConfig;
use crate::error::{AppError, AppResult};
use crate::models::{Job, JobKind, JobStatus, JobStep, StepStatus};
use crate::sidecar::{self, SidecarStatus};
use crate::workspace;
use chrono::Utc;
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};

#[derive(Debug, Clone)]
enum QueuedWorkKind {
    FullRun { step: Option<JobStep> },
    SegmentRetry { segment_id: String },
}

#[derive(Debug, Clone)]
struct QueuedWork {
    job_id: String,
    work: QueuedWorkKind,
    /// True when this work may start a live-record ingest (needs a live slot).
    requires_live_slot: bool,
}

pub struct RunnerState {
    running_job_ids: Mutex<HashSet<String>>,
    stop_flags: Mutex<HashMap<String, Arc<AtomicBool>>>,
    live_recording_ids: Mutex<HashSet<String>>,
    /// Jobs that claimed a live concurrency slot for the whole run (not only
    /// the active ffmpeg capture window).
    live_slot_holders: Mutex<HashSet<String>>,
    /// FIFO of accepted work waiting for a free concurrency slot.
    queue: Mutex<VecDeque<QueuedWork>>,
    bundled_sidecar_root: Mutex<Option<PathBuf>>,
}

impl Default for RunnerState {
    fn default() -> Self {
        Self {
            running_job_ids: Mutex::new(HashSet::new()),
            stop_flags: Mutex::new(HashMap::new()),
            live_recording_ids: Mutex::new(HashSet::new()),
            live_slot_holders: Mutex::new(HashSet::new()),
            queue: Mutex::new(VecDeque::new()),
            bundled_sidecar_root: Mutex::new(None),
        }
    }
}

impl RunnerState {
    fn try_begin(&self, job_id: &str) -> Option<Arc<AtomicBool>> {
        let mut running = self.running_job_ids.lock().expect("runner lock");
        if running.contains(job_id) {
            return None;
        }
        running.insert(job_id.to_string());
        drop(running);

        let stop_flag = Arc::new(AtomicBool::new(false));
        self.stop_flags
            .lock()
            .expect("stop flags lock")
            .insert(job_id.to_string(), Arc::clone(&stop_flag));
        Some(stop_flag)
    }

    fn end(&self, job_id: &str) {
        self.running_job_ids
            .lock()
            .expect("runner lock")
            .remove(job_id);
        self.stop_flags
            .lock()
            .expect("stop flags lock")
            .remove(job_id);
        self.live_recording_ids
            .lock()
            .expect("live recording lock")
            .remove(job_id);
        self.live_slot_holders
            .lock()
            .expect("live slot holders lock")
            .remove(job_id);
    }

    pub fn request_stop(&self, job_id: &str) -> bool {
        if !self
            .live_recording_ids
            .lock()
            .expect("live recording lock")
            .contains(job_id)
        {
            return false;
        }
        let flags = self.stop_flags.lock().expect("stop flags lock");
        if let Some(flag) = flags.get(job_id) {
            flag.store(true, Ordering::SeqCst);
            return true;
        }
        false
    }

    pub fn has_live_recording(&self) -> bool {
        !self
            .live_recording_ids
            .lock()
            .expect("live recording lock")
            .is_empty()
    }

    pub fn has_running_jobs(&self) -> bool {
        !self.running_job_ids.lock().expect("runner lock").is_empty()
    }

    pub fn is_job_running(&self, job_id: &str) -> bool {
        self.running_job_ids
            .lock()
            .expect("runner lock")
            .contains(job_id)
    }

    pub fn is_job_queued(&self, job_id: &str) -> bool {
        self.queue
            .lock()
            .expect("queue lock")
            .iter()
            .any(|entry| entry.job_id == job_id)
    }

    /// 1-based queue position when the job is waiting in the in-memory FIFO.
    pub fn queue_position(&self, job_id: &str) -> Option<u32> {
        self.queue
            .lock()
            .expect("queue lock")
            .iter()
            .position(|entry| entry.job_id == job_id)
            .map(|index| (index + 1) as u32)
    }

    /// Remove a job from the wait queue (e.g. user deleted a queued job).
    pub fn remove_from_queue(&self, job_id: &str) -> bool {
        let mut queue = self.queue.lock().expect("queue lock");
        let before_len = queue.len();
        queue.retain(|entry| entry.job_id != job_id);
        before_len != queue.len()
    }

    /// Job IDs currently running or waiting in the in-memory queue.
    pub fn active_job_ids(&self) -> HashSet<String> {
        let mut ids = self.running_job_ids.lock().expect("runner lock").clone();
        for entry in self.queue.lock().expect("queue lock").iter() {
            ids.insert(entry.job_id.clone());
        }
        ids
    }

    pub fn set_bundled_sidecar_root(&self, root: Option<PathBuf>) {
        *self
            .bundled_sidecar_root
            .lock()
            .expect("bundled sidecar root lock") = root;
    }

    pub fn resolve_sidecars(&self, configured: &crate::config::SidecarPaths) -> SidecarStatus {
        let bundled_root = self
            .bundled_sidecar_root
            .lock()
            .expect("bundled sidecar root lock")
            .clone();
        sidecar::resolve_all(configured, bundled_root.as_deref())
    }

    fn mark_live_recording_started(&self, job_id: &str) {
        self.live_recording_ids
            .lock()
            .expect("live recording lock")
            .insert(job_id.to_string());
    }

    fn mark_live_recording_ended(&self, job_id: &str) {
        self.live_recording_ids
            .lock()
            .expect("live recording lock")
            .remove(job_id);
    }

    fn running_count(&self) -> usize {
        self.running_job_ids.lock().expect("runner lock").len()
    }

    fn live_slot_holder_count(&self) -> usize {
        self.live_slot_holders
            .lock()
            .expect("live slot holders lock")
            .len()
    }

    fn can_start_now(&self, requires_live_slot: bool, config: &AppConfig) -> bool {
        let max_jobs = config.max_concurrent_jobs.max(1) as usize;
        if self.running_count() >= max_jobs {
            return false;
        }
        if requires_live_slot {
            let max_live = config.max_live_records.max(1) as usize;
            if self.live_slot_holder_count() >= max_live {
                return false;
            }
        }
        true
    }

    fn reserve_live_slot(&self, job_id: &str) {
        self.live_slot_holders
            .lock()
            .expect("live slot holders lock")
            .insert(job_id.to_string());
    }

    fn push_queue(&self, work: QueuedWork) {
        self.queue.lock().expect("queue lock").push_back(work);
    }

    fn pop_next_runnable(&self, config: &AppConfig) -> Option<QueuedWork> {
        let mut queue = self.queue.lock().expect("queue lock");
        let mut skipped = VecDeque::new();
        let mut selected = None;
        while let Some(entry) = queue.pop_front() {
            if self.can_start_now(entry.requires_live_slot, config)
                && !self.is_job_running(&entry.job_id)
            {
                selected = Some(entry);
                break;
            }
            skipped.push_back(entry);
        }
        while let Some(entry) = skipped.pop_back() {
            queue.push_front(entry);
        }
        selected
    }
}

struct LiveRecordingGuard<'a> {
    runner: &'a RunnerState,
    job_id: String,
}

impl<'a> LiveRecordingGuard<'a> {
    fn new(runner: &'a RunnerState, job_id: &str) -> Self {
        runner.mark_live_recording_started(job_id);
        Self {
            runner,
            job_id: job_id.to_string(),
        }
    }
}

impl Drop for LiveRecordingGuard<'_> {
    fn drop(&mut self) {
        self.runner.mark_live_recording_ended(&self.job_id);
    }
}

pub fn spawn_job_run(
    app: AppHandle,
    config: AppConfig,
    runner: Arc<RunnerState>,
    job_id: String,
    step: Option<JobStep>,
) -> AppResult<()> {
    let job = workspace::load_job(config.workspace_path(), &job_id)?;
    if runner.is_job_running(&job_id) {
        return Err(AppError::message("该任务已在执行中"));
    }
    if runner.is_job_queued(&job_id) {
        return Err(AppError::message("该任务已在排队中"));
    }

    let requires_live_slot = job_requires_live_slot(&job, step.as_ref());
    let work = QueuedWork {
        job_id: job_id.clone(),
        work: QueuedWorkKind::FullRun { step },
        requires_live_slot,
    };
    admit_or_queue(app, config, runner, work)
}

pub fn spawn_transcript_segment_retry(
    app: AppHandle,
    config: AppConfig,
    runner: Arc<RunnerState>,
    job_id: String,
    segment_id: String,
) -> AppResult<()> {
    let job = workspace::load_job(config.workspace_path(), &job_id)?;
    if !job
        .transcript_segments
        .iter()
        .any(|segment| segment.id == segment_id)
    {
        return Err(AppError::message(format!("转写分段不存在: {segment_id}")));
    }
    if runner.is_job_running(&job_id) {
        return Err(AppError::message("该任务已在执行中"));
    }
    if runner.is_job_queued(&job_id) {
        return Err(AppError::message("该任务已在排队中"));
    }

    let work = QueuedWork {
        job_id,
        work: QueuedWorkKind::SegmentRetry { segment_id },
        requires_live_slot: false,
    };
    admit_or_queue(app, config, runner, work)
}

fn job_requires_live_slot(job: &Job, only_step: Option<&JobStep>) -> bool {
    if job.source.kind != JobKind::LiveRecord {
        return false;
    }
    match only_step {
        None => true,
        Some(JobStep::Ingest) => true,
        Some(_) => false,
    }
}

fn admit_or_queue(
    app: AppHandle,
    config: AppConfig,
    runner: Arc<RunnerState>,
    work: QueuedWork,
) -> AppResult<()> {
    if runner.can_start_now(work.requires_live_slot, &config) {
        start_queued_work(app, config, runner, work)?;
    } else {
        mark_job_queued(&app, &config.workspace_path(), &work.job_id)?;
        runner.push_queue(work);
    }
    Ok(())
}

fn mark_job_queued(app: &AppHandle, workspace_root: &Path, job_id: &str) -> AppResult<()> {
    let mut job = workspace::load_job(workspace_root, job_id)?;
    job.status = JobStatus::Queued;
    job.error_message = None;
    job.error_code = None;
    job.progress = 0.0;
    job.updated_at = Utc::now();
    workspace::save_job(workspace_root, &job)?;
    emit_job_updated(app, &job)
}

fn apply_job_failure(job: &mut Job, safe_error: String) {
    let code = error_code::classify_error(
        &safe_error,
        job.current_step.as_ref(),
        Some(&job.source.kind),
    );
    job.error_message = Some(safe_error);
    job.error_code = Some(code);
}

fn start_queued_work(
    app: AppHandle,
    config: AppConfig,
    runner: Arc<RunnerState>,
    work: QueuedWork,
) -> AppResult<()> {
    let job_id = work.job_id.clone();
    let stop_flag = runner
        .try_begin(&job_id)
        .ok_or_else(|| AppError::message("该任务已在执行中"))?;
    if work.requires_live_slot {
        runner.reserve_live_slot(&job_id);
    }

    std::thread::spawn(move || {
        match &work.work {
            QueuedWorkKind::FullRun { step } => {
                if let Err(error) =
                    run_job_steps(&app, &config, &runner, &job_id, step.clone(), stop_flag)
                {
                    if let Ok(mut failed_job) =
                        workspace::load_job(config.workspace_path(), &job_id)
                    {
                        let safe_error = redact_error(&config, &error);
                        if let Some(current_step) = failed_job.current_step.clone() {
                            failed_job.set_step_status(
                                &current_step,
                                StepStatus::Failed,
                                Some(safe_error.clone()),
                            );
                        }
                        apply_job_failure(&mut failed_job, safe_error);
                        failed_job.refresh_derived_status();
                        if failed_job.status != JobStatus::Failed {
                            failed_job.status = JobStatus::Failed;
                        }
                        failed_job.updated_at = Utc::now();
                        let _ = workspace::save_job(config.workspace_path(), &failed_job);
                        let _ = emit_job_updated(&app, &failed_job);
                    }
                }
            }
            QueuedWorkKind::SegmentRetry { segment_id } => {
                let result =
                    run_transcript_segment_retry(&app, &config, &runner, &job_id, segment_id);
                if let Err(error) = result {
                    if let Ok(mut failed_job) =
                        workspace::load_job(config.workspace_path(), &job_id)
                    {
                        let safe_error = redact_error(&config, &error);
                        if let Some(segment) = failed_job
                            .transcript_segments
                            .iter_mut()
                            .find(|segment| segment.id == *segment_id)
                        {
                            segment.status = crate::models::SegmentStatus::Failed;
                            segment.detail = Some(safe_error.clone());
                            segment.plain_path = None;
                            segment.srt_path = None;
                        }
                        failed_job.set_step_status(
                            &JobStep::Transcribe,
                            StepStatus::Failed,
                            Some(safe_error.clone()),
                        );
                        apply_job_failure(&mut failed_job, safe_error);
                        failed_job.refresh_derived_status();
                        let _ = persist(&app, &config.workspace_path(), &mut failed_job);
                    }
                }
            }
        }
        runner.end(&job_id);
        pump_queue(app, config, runner);
    });

    Ok(())
}

/// After a job finishes, start as many FIFO-queued jobs as free slots allow.
fn pump_queue(app: AppHandle, previous_config: AppConfig, runner: Arc<RunnerState>) {
    loop {
        let config = AppConfig::load_or_init().unwrap_or_else(|_| previous_config.clone());
        let Some(next_work) = runner.pop_next_runnable(&config) else {
            break;
        };
        let job_id = next_work.job_id.clone();
        if workspace::load_job(config.workspace_path(), &job_id).is_err() {
            continue;
        }
        if let Err(error) =
            start_queued_work(app.clone(), config.clone(), Arc::clone(&runner), next_work)
        {
            if let Ok(mut failed_job) = workspace::load_job(config.workspace_path(), &job_id) {
                let safe_error = redact_error(&config, &error);
                failed_job.status = JobStatus::Failed;
                apply_job_failure(&mut failed_job, safe_error);
                failed_job.updated_at = Utc::now();
                let _ = workspace::save_job(config.workspace_path(), &failed_job);
                let _ = emit_job_updated(&app, &failed_job);
            }
            continue;
        }
        // Only start one job per free slot evaluation; re-check after begin.
        if !runner.can_start_now(false, &config) {
            // Still may have free live-constrained slots for non-live work; keep looping.
            if runner.running_count() >= config.max_concurrent_jobs.max(1) as usize {
                break;
            }
        }
    }
}

fn run_transcript_segment_retry(
    app: &AppHandle,
    config: &AppConfig,
    runner: &RunnerState,
    job_id: &str,
    segment_id: &str,
) -> AppResult<()> {
    let workspace_root = config.workspace_path();
    let job_dir = workspace::validated_job_dir(&workspace_root, job_id)?;
    let mut job = workspace::load_job(&workspace_root, job_id)?;
    job.status = JobStatus::Running;
    job.error_message = None;
    job.error_code = None;
    job.invalidate_after_step(&JobStep::Transcribe);
    begin_step(app, &workspace_root, &mut job, JobStep::Transcribe)?;
    paths::remove_downstream_artifacts(&job_dir, &JobStep::Transcribe)?;
    let sidecars = runner.resolve_sidecars(&config.sidecar_paths);
    transcribe::transcribe_media_segments(
        &job_dir,
        &mut job,
        config,
        &sidecars,
        Some(segment_id),
        |current| {
            let mut snapshot = current.clone();
            persist(app, &workspace_root, &mut snapshot)
        },
    )?;
    let all_segments_succeeded = job
        .transcript_segments
        .iter()
        .all(|segment| segment.status == crate::models::SegmentStatus::Succeeded);
    let detail = if all_segments_succeeded {
        "全部转写分段已成功".to_string()
    } else {
        format!("分段 {segment_id} 重试成功，仍有其他未成功分段")
    };
    job.set_step_status(
        &JobStep::Transcribe,
        if all_segments_succeeded {
            StepStatus::Succeeded
        } else {
            StepStatus::Failed
        },
        Some(detail),
    );
    job.current_step = None;
    job.progress = 100.0;
    job.refresh_derived_status();
    if all_segments_succeeded {
        job.error_message = None;
        job.error_code = None;
    } else {
        apply_job_failure(
            &mut job,
            format!("分段 {segment_id} 重试后仍有其他转写分段未成功"),
        );
    }
    persist(app, &workspace_root, &mut job)
}

fn run_job_steps(
    app: &AppHandle,
    config: &AppConfig,
    runner: &RunnerState,
    job_id: &str,
    only_step: Option<JobStep>,
    stop_flag: Arc<AtomicBool>,
) -> AppResult<()> {
    let workspace_root = config.workspace_path();
    let job_dir = workspace::validated_job_dir(&workspace_root, job_id)?;
    let mut job = workspace::load_job(&workspace_root, job_id)?;
    job.status = JobStatus::Running;
    job.error_message = None;
    job.error_code = None;
    job.stop_requested = false;
    persist(app, &workspace_root, &mut job)?;

    let requested_steps = if let Some(step) = only_step {
        vec![step]
    } else {
        let mut steps = vec![JobStep::Ingest];
        if job.pipeline.auto_transcribe
            || job.pipeline.auto_summarize
            || job.pipeline.auto_chapterize
        {
            steps.push(JobStep::Transcribe);
            steps.push(JobStep::MergeTranscript);
        }
        if job.pipeline.auto_chapterize {
            steps.push(JobStep::Chapterize);
        }
        if job.pipeline.auto_summarize {
            steps.push(JobStep::Summarize);
        }
        steps
    };

    let requested_step_count = requested_steps.len();
    for (step_index, step) in requested_steps.into_iter().enumerate() {
        job.invalidate_after_step(&step);
        begin_step(app, &workspace_root, &mut job, step.clone())?;
        paths::remove_downstream_artifacts(&job_dir, &step)?;
        let result = match step {
            JobStep::Ingest => run_ingest(
                app,
                config,
                &mut job,
                &job_dir,
                &workspace_root,
                runner,
                Arc::clone(&stop_flag),
            ),
            JobStep::Transcribe => {
                run_transcribe(app, config, &mut job, &job_dir, &workspace_root, runner)
            }
            JobStep::MergeTranscript => {
                let sidecars = runner.resolve_sidecars(&config.sidecar_paths);
                transcribe::merge_transcripts(
                    &job_dir,
                    &mut job,
                    config,
                    sidecars.ffprobe.path.as_deref(),
                )
                .map(|_| ())
            }
            JobStep::Chapterize => chapterize::chapterize_job(&job_dir, &mut job).map(|_| ()),
            JobStep::Summarize => summarize::summarize_job(&job_dir, &mut job, config).map(|_| ()),
        };

        match result {
            Ok(()) => {
                let detail = step_success_detail(&step, &job);
                job.set_step_status(&step, StepStatus::Succeeded, Some(detail));
                job.progress = 100.0;
                job.current_step = None;
                if step_index + 1 < requested_step_count {
                    job.status = JobStatus::Running;
                } else {
                    job.refresh_derived_status();
                }
                if job.status != JobStatus::Failed {
                    job.error_message = None;
                }
                persist(app, &workspace_root, &mut job)?;
            }
            Err(error) => {
                let safe_error = redact_error(config, &error);
                job.set_step_status(&step, StepStatus::Failed, Some(safe_error.clone()));
                job.current_step = None;
                job.refresh_derived_status();
                job.error_message = Some(safe_error);
                persist(app, &workspace_root, &mut job)?;
                return Err(error);
            }
        }
    }

    job.current_step = None;
    job.progress = 100.0;
    job.refresh_derived_status();
    if job.status != JobStatus::Failed {
        job.error_message = None;
    }
    persist(app, &workspace_root, &mut job)?;
    Ok(())
}

fn run_ingest(
    app: &AppHandle,
    config: &AppConfig,
    job: &mut Job,
    job_dir: &Path,
    workspace_root: &Path,
    runner: &RunnerState,
    stop_flag: Arc<AtomicBool>,
) -> AppResult<()> {
    match job.source.kind {
        JobKind::Download => {
            let sidecars = runner.resolve_sidecars(&config.sidecar_paths);
            let url = job
                .source
                .url
                .clone()
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| AppError::message("下载任务缺少 URL"))?;
            let progress_job_id = job.id.clone();
            let progress_workspace = workspace_root.to_path_buf();
            let progress_app = app.clone();
            let callback = Arc::new(Mutex::new(move |percent: f32| {
                if let Ok(mut current) = workspace::load_job(&progress_workspace, &progress_job_id)
                {
                    current.progress = percent.clamp(0.0, 100.0);
                    let _ = persist(&progress_app, &progress_workspace, &mut current);
                }
            }));
            let cookies = download::DownloadCookiesOptions::resolve(&job.source, config)?;
            let result =
                download::run_download(job_dir, &url, &sidecars.yt_dlp, &cookies, Some(callback))?;
            job.media_files = result.media_files;
            job.tool_path = Some(result.tool_path);
            job.tool_version = result.tool_version;
            let should_fill_title = job
                .source
                .title
                .as_ref()
                .is_none_or(|value| value.trim().is_empty());
            if should_fill_title {
                if let Some(resolved_title) = result.resolved_title {
                    let trimmed = resolved_title.trim();
                    if !trimmed.is_empty() {
                        // Prefer a short, single-line title for the task list.
                        let shortened: String = trimmed.chars().take(80).collect();
                        job.source.title = Some(shortened);
                    }
                }
            }
        }
        JobKind::ImportLocal => {
            let local_path = job
                .source
                .local_path
                .clone()
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| AppError::message("导入任务缺少本地路径"))?;
            job.media_files = download::copy_local_media(job_dir, &local_path)?;
        }
        JobKind::LiveRecord => {
            let _live_recording_guard = LiveRecordingGuard::new(runner, &job.id);
            job.live_capture_active = true;
            persist(app, workspace_root, job)?;
            let sidecars = runner.resolve_sidecars(&config.sidecar_paths);
            let url = job
                .source
                .url
                .clone()
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| AppError::message("直播任务缺少 URL"))?;
            let capture_job_id = job.id.clone();
            let capture_workspace_root = workspace_root.to_path_buf();
            let capture_app = app.clone();
            let capture_result = record::record_live_segments(
                job_dir,
                record::LiveRecordOptions {
                    source_url: &url,
                    segment_minutes: job
                        .source
                        .segment_minutes
                        .unwrap_or(config.default_segment_minutes),
                    minimum_free_disk_gb: config.min_free_disk_gb,
                    reconnect_attempts: config.live_reconnect_attempts,
                    sidecars: &sidecars,
                    stop_requested: stop_flag,
                },
                || {
                    runner.mark_live_recording_ended(&capture_job_id);
                    if let Ok(mut current_job) =
                        workspace::load_job(&capture_workspace_root, &capture_job_id)
                    {
                        current_job.live_capture_active = false;
                        let _ = persist(&capture_app, &capture_workspace_root, &mut current_job);
                    }
                },
            );
            job.live_capture_active = false;
            let result = capture_result?;
            job.media_files = result.media_files;
            job.tool_path = Some(result.tool_path);
            job.tool_version = result.tool_version;
            job.stop_requested = result.termination == record::RecordTermination::StoppedByUser;
            job.rebuild_media_segments_from_files();
            if let Some(detail) = result.termination.detail() {
                return Err(AppError::message(detail));
            }
        }
    }
    job.rebuild_media_segments_from_files();
    Ok(())
}

fn run_transcribe(
    app: &AppHandle,
    config: &AppConfig,
    job: &mut Job,
    job_dir: &Path,
    workspace_root: &Path,
    runner: &RunnerState,
) -> AppResult<()> {
    let sidecars = runner.resolve_sidecars(&config.sidecar_paths);
    transcribe::transcribe_media_segments(job_dir, job, config, &sidecars, None, |current| {
        let mut snapshot = current.clone();
        persist(app, workspace_root, &mut snapshot)
    })
}

fn begin_step(
    app: &AppHandle,
    workspace_root: &Path,
    job: &mut Job,
    step: JobStep,
) -> AppResult<()> {
    job.ensure_step(step.clone());
    job.current_step = Some(step.clone());
    job.set_step_status(&step, StepStatus::Running, None);
    job.progress = 0.0;
    job.error_message = None;
    persist(app, workspace_root, job)
}

fn step_success_detail(step: &JobStep, job: &Job) -> String {
    match step {
        JobStep::Ingest => format!("媒体: {}", job.media_files.join(", ")),
        JobStep::Transcribe => format!("完成 {} 个分段", job.transcript_segments.len()),
        JobStep::MergeTranscript => "已生成 transcript/plain.txt".to_string(),
        JobStep::Chapterize => job
            .chapters_path
            .clone()
            .unwrap_or_else(|| "已生成 transcript/chapters.json".to_string()),
        JobStep::Summarize => "已生成 summary/summary.md".to_string(),
    }
}

fn persist(app: &AppHandle, workspace_root: &Path, job: &mut Job) -> AppResult<()> {
    job.updated_at = Utc::now();
    workspace::save_job(workspace_root, job)?;
    // Best-effort FTS index refresh (never fail the job pipeline on index errors).
    if let Err(error) = crate::search::upsert_job(workspace_root, job) {
        eprintln!("search index upsert skipped for {}: {error}", job.id);
    }
    emit_job_updated(app, job)
}

fn emit_job_updated(app: &AppHandle, job: &Job) -> AppResult<()> {
    app.emit("job-updated", job)
        .map_err(|error| AppError::message(format!("emit job-updated failed: {error}")))
}

fn redact_error(config: &AppConfig, error: &impl ToString) -> String {
    logs::redact_secrets(&error.to_string(), &config.secret_values())
}

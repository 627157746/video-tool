use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum JobKind {
    Download,
    LiveRecord,
    ImportLocal,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    Pending,
    /// Accepted by the global scheduler but not yet running (in-memory FIFO).
    Queued,
    Running,
    Succeeded,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StepStatus {
    Pending,
    Running,
    Succeeded,
    Failed,
    Skipped,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum JobStep {
    Ingest,
    Transcribe,
    MergeTranscript,
    /// Heuristic / outline chapters after merge (v0.2 P2).
    Chapterize,
    Summarize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SegmentStatus {
    Pending,
    Running,
    Succeeded,
    Failed,
    Skipped,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineOptions {
    pub auto_transcribe: bool,
    pub auto_summarize: bool,
    /// When true, run Chapterize after merge (typically before summarize).
    /// Omitted on older jobs → false unless auto_summarize implies it at create.
    #[serde(default)]
    pub auto_chapterize: bool,
    /// Job-level Provider override. `None` means use
    /// `AppConfig.default_provider_profile_id` **at summarize run time**
    /// (not a snapshot taken at job creation).
    pub provider_profile_id: Option<String>,
    /// Job-level primary template override. `None` means use
    /// `AppConfig.default_template_id` at summarize run time (when
    /// `template_ids` is also empty).
    pub template_id: Option<String>,
    /// Ordered multi-template list for one Summarize run (v0.2 P3).
    /// When non-empty, overrides single `template_id` for resolution order;
    /// first entry is the primary summary (`summary/summary.md`).
    #[serde(default)]
    pub template_ids: Vec<String>,
    /// Override the selected provider's `default_model` for this job.
    /// `None` / empty means use the provider default at summarize run time.
    #[serde(default)]
    pub model: Option<String>,
    /// Override the global `transcribe_language` for this job. `None` means
    /// follow the global config; `Some("auto")` forces auto-detection.
    /// Unlike provider/template, language is eagerly resolved at create time
    /// so the job snapshot records the effective value for transcribe.
    #[serde(default)]
    pub transcribe_language: Option<String>,
}

impl Default for PipelineOptions {
    fn default() -> Self {
        Self {
            auto_transcribe: true,
            auto_summarize: false,
            auto_chapterize: false,
            provider_profile_id: None,
            template_id: None,
            template_ids: Vec::new(),
            model: None,
            transcribe_language: None,
        }
    }
}

impl PipelineOptions {
    /// Effective ordered template ids for summarize (deduped, non-empty entries).
    pub fn effective_template_ids(
        &self,
        default_template_id: Option<&str>,
        available_template_ids: &[String],
    ) -> Vec<String> {
        let mut ordered: Vec<String> = if !self.template_ids.is_empty() {
            self.template_ids
                .iter()
                .map(|value| value.trim())
                .filter(|value| !value.is_empty())
                .map(|value| value.to_string())
                .collect()
        } else if let Some(single) = self
            .template_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            vec![single.to_string()]
        } else if let Some(default_id) = default_template_id
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            vec![default_id.to_string()]
        } else if let Some(first) = available_template_ids
            .iter()
            .map(|value| value.trim())
            .find(|value| !value.is_empty())
        {
            vec![first.to_string()]
        } else {
            Vec::new()
        };

        let mut seen = std::collections::HashSet::new();
        ordered.retain(|id| seen.insert(id.clone()));
        ordered
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobSource {
    pub kind: JobKind,
    pub url: Option<String>,
    pub title: Option<String>,
    pub local_path: Option<String>,
    pub segment_minutes: Option<u32>,
    /// Download auth override for yt-dlp. `None` / `"inherit"` follows global config.
    /// `"none"` disables cookies for this job. `"file"` / `"browser"` use the fields below.
    /// Never stores cookie file contents — only path or browser name.
    #[serde(default)]
    pub download_cookies_mode: Option<String>,
    /// Netscape cookies.txt path when mode is `file` (or explicit job path override).
    #[serde(default)]
    pub download_cookies_file: Option<String>,
    /// Browser id for yt-dlp `--cookies-from-browser` when mode is `browser`.
    #[serde(default)]
    pub download_cookies_from_browser: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaSegment {
    pub id: String,
    pub file_name: String,
    pub index: u32,
    #[serde(default)]
    pub duration_seconds: Option<f64>,
    #[serde(default)]
    pub selected_for_summary: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptSegmentInfo {
    pub id: String,
    pub media_file: String,
    pub index: u32,
    pub status: SegmentStatus,
    #[serde(default)]
    pub plain_path: Option<String>,
    #[serde(default)]
    pub srt_path: Option<String>,
    #[serde(default)]
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    pub id: String,
    pub status: JobStatus,
    pub source: JobSource,
    pub pipeline: PipelineOptions,
    /// User-defined grouping label for list filtering. `None` means ungrouped.
    /// Omitted on older `source.json` files (Serde default).
    #[serde(default)]
    pub group: Option<String>,
    /// Optional batch identifier shared by Jobs created together (multi-URL).
    /// Omitted on older `source.json` files.
    #[serde(default)]
    pub batch_id: Option<String>,
    pub current_step: Option<JobStep>,
    pub step_statuses: Vec<StepProgress>,
    #[serde(default)]
    pub progress: f32,
    #[serde(default)]
    pub media_files: Vec<String>,
    #[serde(default)]
    pub media_segments: Vec<MediaSegment>,
    #[serde(default)]
    pub transcript_segments: Vec<TranscriptSegmentInfo>,
    #[serde(default)]
    pub selected_segment_ids: Vec<String>,
    #[serde(default)]
    pub duration_label: Option<String>,
    #[serde(default)]
    pub tool_path: Option<String>,
    #[serde(default)]
    pub tool_version: Option<String>,
    #[serde(default)]
    pub summary_path: Option<String>,
    #[serde(default)]
    pub plain_transcript_path: Option<String>,
    /// Relative path to `transcript/chapters.json` when Chapterize succeeded.
    #[serde(default)]
    pub chapters_path: Option<String>,
    /// Hash of glossary used for the last transcribe/merge (reproducibility).
    #[serde(default)]
    pub glossary_hash: Option<String>,
    #[serde(default)]
    pub stop_requested: bool,
    #[serde(default)]
    pub live_capture_active: bool,
    pub error_message: Option<String>,
    /// Stable machine-readable failure code for recovery UI.
    /// Omitted on older `source.json` files.
    #[serde(default)]
    pub error_code: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepProgress {
    pub step: JobStep,
    pub status: StepStatus,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobListItem {
    pub id: String,
    pub status: JobStatus,
    pub kind: JobKind,
    pub title: String,
    pub source_reference: String,
    #[serde(default)]
    pub group: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub batch_id: Option<String>,
    pub current_step: Option<JobStep>,
    pub progress: f32,
    pub error_message: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
    /// 1-based position in the in-memory global queue; only set when status is queued.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub queue_position: Option<u32>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateDownloadJobRequest {
    pub url: String,
    pub title: Option<String>,
    #[serde(default)]
    pub group: Option<String>,
    /// Optional batch id when this job is part of a multi-URL batch.
    #[serde(default)]
    pub batch_id: Option<String>,
    pub pipeline: Option<PipelineOptions>,
    #[serde(default)]
    pub auto_start: bool,
    /// `inherit` (default) | `none` | `file` | `browser`
    #[serde(default)]
    pub download_cookies_mode: Option<String>,
    #[serde(default)]
    pub download_cookies_file: Option<String>,
    #[serde(default)]
    pub download_cookies_from_browser: Option<String>,
}

/// Multi-line download create: one Job per non-empty entry / URL-like line.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateDownloadJobsBatchRequest {
    /// Raw multi-line paste. Split on the server into individual URL entries.
    pub urls_text: String,
    pub title: Option<String>,
    #[serde(default)]
    pub group: Option<String>,
    pub pipeline: Option<PipelineOptions>,
    #[serde(default)]
    pub auto_start: bool,
    #[serde(default)]
    pub download_cookies_mode: Option<String>,
    #[serde(default)]
    pub download_cookies_file: Option<String>,
    #[serde(default)]
    pub download_cookies_from_browser: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateDownloadJobsBatchResponse {
    pub batch_id: Option<String>,
    pub jobs: Vec<Job>,
    /// Entries that could not be turned into jobs (empty after trim, etc.).
    #[serde(default)]
    pub skipped: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateLiveRecordJobRequest {
    pub url: String,
    pub title: Option<String>,
    #[serde(default)]
    pub group: Option<String>,
    pub segment_minutes: Option<u32>,
    pub pipeline: Option<PipelineOptions>,
    #[serde(default)]
    pub auto_start: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateImportJobRequest {
    pub local_path: String,
    pub title: Option<String>,
    #[serde(default)]
    pub group: Option<String>,
    pub pipeline: Option<PipelineOptions>,
    #[serde(default)]
    pub auto_start: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunJobRequest {
    pub job_id: String,
    pub step: Option<JobStep>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobLogRequest {
    pub job_id: String,
    pub log_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectSegmentsRequest {
    pub job_id: String,
    pub segment_ids: Vec<String>,
}

/// Update summarize-related pipeline overrides on an existing Job.
/// Empty strings are treated as `None` (follow global / provider defaults at run time).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateJobPipelineRequest {
    pub job_id: String,
    pub provider_profile_id: Option<String>,
    /// Primary template when `template_ids` is omitted / empty.
    pub template_id: Option<String>,
    /// Full ordered multi-template list. When `Some`, replaces `template_ids`.
    #[serde(default)]
    pub template_ids: Option<Vec<String>>,
    pub model: Option<String>,
}

/// Rename a Job display title.
/// Empty / whitespace-only titles clear the custom title so list/detail fall
/// back to URL, local path, or Job id.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateJobTitleRequest {
    pub job_id: String,
    pub title: Option<String>,
}

/// Assign or clear a Job grouping label.
/// Empty / whitespace-only values clear the group (ungrouped).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateJobGroupRequest {
    pub job_id: String,
    pub group: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportJobRequest {
    pub job_id: String,
    pub destination_dir: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestProviderRequest {
    pub provider_profile_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryTranscriptSegmentRequest {
    pub job_id: String,
    pub segment_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveConfigRequest {
    pub workspace_dir: Option<String>,
    pub default_segment_minutes: Option<u32>,
    pub default_auto_transcribe: Option<bool>,
    pub default_auto_summarize: Option<bool>,
    pub default_provider_profile_id: Option<String>,
    pub default_template_id: Option<String>,
    pub proxy_url: Option<String>,
    pub min_free_disk_gb: Option<u32>,
    pub live_reconnect_attempts: Option<u32>,
    pub max_context_chars: Option<usize>,
    /// Global max concurrent running jobs (queued work waits).
    #[serde(default)]
    pub max_concurrent_jobs: Option<u32>,
    /// Max concurrent live-record slot holders (also counts against global max).
    #[serde(default)]
    pub max_live_records: Option<u32>,
    /// Default Netscape cookies.txt path for yt-dlp downloads.
    #[serde(default)]
    pub download_cookies_file: Option<String>,
    /// Default browser for yt-dlp `--cookies-from-browser` (used when file empty).
    #[serde(default)]
    pub download_cookies_from_browser: Option<String>,
    pub transcribe_model: Option<String>,
    pub transcribe_language: Option<String>,
    #[serde(default)]
    pub transcribe_model_preset: Option<String>,
    #[serde(default)]
    pub transcribe_model_presets: Option<crate::config::TranscribeModelPresets>,
    #[serde(default)]
    pub glossary: Option<crate::config::GlossaryConfig>,
    #[serde(default)]
    pub default_auto_chapterize: Option<bool>,
    pub sidecar_paths: Option<crate::config::SidecarPaths>,
    pub providers: Option<Vec<crate::config::ProviderProfile>>,
    pub templates: Option<Vec<crate::config::SummaryTemplate>>,
    #[serde(default)]
    pub job_groups: Option<Vec<crate::config::JobGroupDefinition>>,
}

impl Job {
    pub fn new(source: JobSource, pipeline: PipelineOptions) -> Self {
        let now = Utc::now();
        let mut step_statuses = vec![StepProgress {
            step: JobStep::Ingest,
            status: StepStatus::Pending,
            detail: None,
        }];

        if pipeline.auto_transcribe {
            step_statuses.push(StepProgress {
                step: JobStep::Transcribe,
                status: StepStatus::Pending,
                detail: None,
            });
            step_statuses.push(StepProgress {
                step: JobStep::MergeTranscript,
                status: StepStatus::Pending,
                detail: None,
            });
        }

        if pipeline.auto_summarize {
            if !pipeline.auto_transcribe {
                // summarize needs merge path; ensure steps exist
                step_statuses.push(StepProgress {
                    step: JobStep::Transcribe,
                    status: StepStatus::Pending,
                    detail: None,
                });
                step_statuses.push(StepProgress {
                    step: JobStep::MergeTranscript,
                    status: StepStatus::Pending,
                    detail: None,
                });
            }
            if pipeline.auto_chapterize {
                step_statuses.push(StepProgress {
                    step: JobStep::Chapterize,
                    status: StepStatus::Pending,
                    detail: None,
                });
            }
            step_statuses.push(StepProgress {
                step: JobStep::Summarize,
                status: StepStatus::Pending,
                detail: None,
            });
        } else if pipeline.auto_chapterize {
            if !pipeline.auto_transcribe {
                step_statuses.push(StepProgress {
                    step: JobStep::Transcribe,
                    status: StepStatus::Pending,
                    detail: None,
                });
                step_statuses.push(StepProgress {
                    step: JobStep::MergeTranscript,
                    status: StepStatus::Pending,
                    detail: None,
                });
            }
            step_statuses.push(StepProgress {
                step: JobStep::Chapterize,
                status: StepStatus::Pending,
                detail: None,
            });
        }

        Self {
            id: Uuid::new_v4().to_string(),
            status: JobStatus::Pending,
            source,
            pipeline,
            group: None,
            batch_id: None,
            current_step: Some(JobStep::Ingest),
            step_statuses,
            progress: 0.0,
            media_files: Vec::new(),
            media_segments: Vec::new(),
            transcript_segments: Vec::new(),
            selected_segment_ids: Vec::new(),
            duration_label: None,
            tool_path: None,
            tool_version: None,
            summary_path: None,
            plain_transcript_path: None,
            chapters_path: None,
            glossary_hash: None,
            stop_requested: false,
            live_capture_active: false,
            error_message: None,
            error_code: None,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn ensure_step(&mut self, step: JobStep) {
        if self
            .step_statuses
            .iter()
            .any(|item| std::mem::discriminant(&item.step) == std::mem::discriminant(&step))
        {
            return;
        }
        self.step_statuses.push(StepProgress {
            step,
            status: StepStatus::Pending,
            detail: None,
        });
    }

    pub fn set_step_status(&mut self, step: &JobStep, status: StepStatus, detail: Option<String>) {
        if let Some(existing) = self
            .step_statuses
            .iter_mut()
            .find(|item| std::mem::discriminant(&item.step) == std::mem::discriminant(step))
        {
            existing.status = status;
            existing.detail = detail;
            return;
        }
        self.step_statuses.push(StepProgress {
            step: step.clone(),
            status,
            detail,
        });
    }

    pub fn invalidate_after_step(&mut self, step: &JobStep) {
        let dependent_steps: &[JobStep] = match step {
            JobStep::Ingest => &[
                JobStep::Transcribe,
                JobStep::MergeTranscript,
                JobStep::Chapterize,
                JobStep::Summarize,
            ],
            JobStep::Transcribe => &[
                JobStep::MergeTranscript,
                JobStep::Chapterize,
                JobStep::Summarize,
            ],
            JobStep::MergeTranscript => &[JobStep::Chapterize, JobStep::Summarize],
            JobStep::Chapterize => &[JobStep::Summarize],
            JobStep::Summarize => &[],
        };

        for dependent_step in dependent_steps {
            if let Some(progress) = self
                .step_statuses
                .iter_mut()
                .find(|progress| progress.step == *dependent_step)
            {
                progress.status = StepStatus::Pending;
                progress.detail = Some("上游数据已变化，需要重新执行".to_string());
            }
        }

        match step {
            JobStep::Ingest => {
                self.media_files.clear();
                self.media_segments.clear();
                self.transcript_segments.clear();
                self.selected_segment_ids.clear();
                self.tool_path = None;
                self.tool_version = None;
                self.plain_transcript_path = None;
                self.chapters_path = None;
                self.glossary_hash = None;
                self.summary_path = None;
            }
            JobStep::Transcribe => {
                self.plain_transcript_path = None;
                self.chapters_path = None;
                self.summary_path = None;
            }
            JobStep::MergeTranscript => {
                self.plain_transcript_path = None;
                self.chapters_path = None;
                self.summary_path = None;
            }
            JobStep::Chapterize => {
                self.chapters_path = None;
                self.summary_path = None;
            }
            JobStep::Summarize => {
                self.summary_path = None;
            }
        }
    }

    pub fn derived_status(&self) -> JobStatus {
        let required_steps = self.required_steps();
        let required_progress: Vec<Option<&StepProgress>> = required_steps
            .iter()
            .map(|step| {
                self.step_statuses
                    .iter()
                    .find(|progress| progress.step == *step)
            })
            .collect();
        if required_progress
            .iter()
            .flatten()
            .any(|progress| progress.status == StepStatus::Running)
        {
            return JobStatus::Running;
        }
        if required_progress
            .iter()
            .flatten()
            .any(|progress| progress.status == StepStatus::Failed)
        {
            return JobStatus::Failed;
        }
        if required_progress.iter().all(|progress| {
            progress.is_some_and(|progress| {
                matches!(progress.status, StepStatus::Succeeded | StepStatus::Skipped)
            })
        }) {
            return JobStatus::Succeeded;
        }
        JobStatus::Pending
    }

    pub fn required_steps(&self) -> Vec<JobStep> {
        let mut required_steps = vec![JobStep::Ingest];
        if self.pipeline.auto_transcribe
            || self.pipeline.auto_summarize
            || self.pipeline.auto_chapterize
        {
            required_steps.push(JobStep::Transcribe);
            required_steps.push(JobStep::MergeTranscript);
        }
        if self.pipeline.auto_chapterize || self.pipeline.auto_summarize {
            // Summarize benefits from chapters when auto_chapterize is on.
            if self.pipeline.auto_chapterize {
                required_steps.push(JobStep::Chapterize);
            }
        }
        if self.pipeline.auto_summarize {
            required_steps.push(JobStep::Summarize);
        }
        required_steps
    }

    pub fn refresh_derived_status(&mut self) {
        self.status = self.derived_status();
        let required_steps = self.required_steps();
        self.current_step = self
            .step_statuses
            .iter()
            .find(|progress| {
                progress.status == StepStatus::Running && required_steps.contains(&progress.step)
            })
            .map(|progress| progress.step.clone());
    }

    pub fn display_title(&self) -> String {
        if let Some(title) = &self.source.title {
            if !title.trim().is_empty() {
                return title.clone();
            }
        }
        if let Some(url) = &self.source.url {
            return url.clone();
        }
        if let Some(local_path) = &self.source.local_path {
            return local_path.clone();
        }
        self.id.clone()
    }

    pub fn to_list_item(&self) -> JobListItem {
        JobListItem {
            id: self.id.clone(),
            status: self.status.clone(),
            kind: self.source.kind.clone(),
            title: self.display_title(),
            source_reference: self
                .source
                .url
                .as_ref()
                .or(self.source.local_path.as_ref())
                .cloned()
                .unwrap_or_default(),
            group: self
                .group
                .as_ref()
                .map(|value| value.trim())
                .filter(|value| !value.is_empty())
                .map(|value| value.to_string()),
            batch_id: self
                .batch_id
                .as_ref()
                .map(|value| value.trim())
                .filter(|value| !value.is_empty())
                .map(|value| value.to_string()),
            current_step: self.current_step.clone(),
            progress: self.progress,
            error_message: self.error_message.clone(),
            error_code: self
                .error_code
                .as_ref()
                .map(|value| value.trim())
                .filter(|value| !value.is_empty())
                .map(|value| value.to_string()),
            queue_position: None,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }

    /// Split multi-line paste into download entries.
    ///
    /// - If **more than one** line looks like a URL (`http://` / `https://` or
    ///   a bare host-like path), each non-empty line becomes one entry.
    /// - Otherwise the entire trimmed paste is a single entry (preserves Douyin
    ///   multi-line share text with one short link).
    pub fn split_download_url_entries(urls_text: &str) -> Vec<String> {
        let trimmed_all = urls_text.trim();
        if trimmed_all.is_empty() {
            return Vec::new();
        }

        let lines: Vec<String> = urls_text
            .lines()
            .map(|line| line.trim().to_string())
            .filter(|line| !line.is_empty())
            .collect();
        if lines.is_empty() {
            return Vec::new();
        }

        let url_like_count = lines
            .iter()
            .filter(|line| line_looks_like_url(line))
            .count();
        if url_like_count > 1 {
            // Prefer URL-like lines when the paste mixes commentary + links.
            let url_lines: Vec<String> = lines
                .into_iter()
                .filter(|line| line_looks_like_url(line))
                .collect();
            return url_lines;
        }

        vec![trimmed_all.to_string()]
    }

    pub fn rebuild_media_segments_from_files(&mut self) {
        let segment_candidates: Vec<&String> = self
            .media_files
            .iter()
            .filter(|file_name| {
                file_name.starts_with("segment_") || file_name.starts_with("original.")
            })
            .collect();
        let selected_files: Vec<&String> = if segment_candidates.is_empty() {
            self.media_files
                .iter()
                .filter(|file_name| !file_name.starts_with("merged."))
                .collect()
        } else {
            segment_candidates
        };
        let mut segments = Vec::new();
        for (index, file_name) in selected_files.into_iter().enumerate() {
            let id = format!("seg-{:03}", index + 1);
            segments.push(MediaSegment {
                id: id.clone(),
                file_name: file_name.to_string(),
                index: (index + 1) as u32,
                duration_seconds: None,
                selected_for_summary: true,
            });
        }
        self.media_segments = segments;
        if self.selected_segment_ids.is_empty() {
            self.selected_segment_ids = self
                .media_segments
                .iter()
                .map(|segment| segment.id.clone())
                .collect();
        }
    }
}

fn line_looks_like_url(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") {
        return true;
    }
    // Bare short hosts often appear without scheme in share dumps.
    if lower.contains("v.douyin.com/")
        || lower.contains("www.douyin.com/")
        || lower.contains("www.bilibili.com/")
        || lower.contains("b23.tv/")
        || lower.contains("youtu.be/")
        || lower.contains("youtube.com/")
    {
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn job_steps_follow_pipeline_flags() {
        let job = Job::new(
            JobSource {
                kind: JobKind::Download,
                url: Some("https://example.com".into()),
                title: Some("t".into()),
                local_path: None,
                segment_minutes: None,
                download_cookies_mode: None,
                download_cookies_file: None,
                download_cookies_from_browser: None,
            },
            PipelineOptions {
                auto_transcribe: true,
                auto_summarize: true,
                auto_chapterize: false,
                provider_profile_id: None,
                template_id: None,
                template_ids: Vec::new(),
                model: None,
                transcribe_language: None,
            },
        );
        assert_eq!(job.step_statuses.len(), 4);
        assert_eq!(job.display_title(), "t");
        assert_eq!(job.group, None);
    }

    #[test]
    fn deserializes_legacy_job_without_group_field() {
        let job = Job::new(
            JobSource {
                kind: JobKind::Download,
                url: Some("https://example.com".into()),
                title: Some("legacy".into()),
                local_path: None,
                segment_minutes: None,
                download_cookies_mode: None,
                download_cookies_file: None,
                download_cookies_from_browser: None,
            },
            PipelineOptions::default(),
        );
        let mut value = serde_json::to_value(&job).expect("serialize job");
        value.as_object_mut().expect("job object").remove("group");
        let restored: Job = serde_json::from_value(value).expect("deserialize without group");
        assert_eq!(restored.group, None);
    }

    #[test]
    fn list_item_includes_trimmed_group() {
        let mut job = Job::new(
            JobSource {
                kind: JobKind::Download,
                url: Some("https://example.com".into()),
                title: Some("t".into()),
                local_path: None,
                segment_minutes: None,
                download_cookies_mode: None,
                download_cookies_file: None,
                download_cookies_from_browser: None,
            },
            PipelineOptions::default(),
        );
        job.group = Some("  学习笔记  ".into());
        let list_item = job.to_list_item();
        assert_eq!(list_item.group.as_deref(), Some("学习笔记"));
    }

    #[test]
    fn split_keeps_douyin_share_text_as_single_entry() {
        let paste = "5.23 abc:/ 复制打开抖音\nhttps://v.douyin.com/abc123/\n# 生活";
        let entries = Job::split_download_url_entries(paste);
        assert_eq!(entries.len(), 1);
        assert!(entries[0].contains("v.douyin.com"));
    }

    #[test]
    fn split_multi_url_lines_into_batch_entries() {
        let paste = "https://example.com/a\nhttps://example.com/b\n\nhttps://example.com/c\n";
        let entries = Job::split_download_url_entries(paste);
        assert_eq!(
            entries,
            vec![
                "https://example.com/a".to_string(),
                "https://example.com/b".to_string(),
                "https://example.com/c".to_string(),
            ]
        );
    }

    #[test]
    fn list_item_includes_batch_id() {
        let mut job = Job::new(
            JobSource {
                kind: JobKind::Download,
                url: Some("https://example.com".into()),
                title: Some("t".into()),
                local_path: None,
                segment_minutes: None,
                download_cookies_mode: None,
                download_cookies_file: None,
                download_cookies_from_browser: None,
            },
            PipelineOptions::default(),
        );
        job.batch_id = Some("  batch-1  ".into());
        let list_item = job.to_list_item();
        assert_eq!(list_item.batch_id.as_deref(), Some("batch-1"));
    }

    #[test]
    fn derives_overall_status_from_every_required_step() {
        let mut job = Job::new(
            JobSource {
                kind: JobKind::Download,
                url: Some("https://example.com".into()),
                title: None,
                local_path: None,
                segment_minutes: None,
                download_cookies_mode: None,
                download_cookies_file: None,
                download_cookies_from_browser: None,
            },
            PipelineOptions {
                auto_transcribe: true,
                auto_summarize: true,
                auto_chapterize: false,
                provider_profile_id: None,
                template_id: None,
                template_ids: Vec::new(),
                model: None,
                transcribe_language: None,
            },
        );

        for progress in &mut job.step_statuses {
            progress.status = StepStatus::Succeeded;
        }
        assert_eq!(job.derived_status(), JobStatus::Succeeded);

        job.set_step_status(
            &JobStep::Summarize,
            StepStatus::Failed,
            Some("provider failed".into()),
        );
        assert_eq!(job.derived_status(), JobStatus::Failed);

        job.set_step_status(&JobStep::Transcribe, StepStatus::Running, None);
        assert_eq!(job.derived_status(), JobStatus::Running);
    }

    #[test]
    fn invalidating_transcription_resets_only_downstream_steps() {
        let mut job = Job::new(
            JobSource {
                kind: JobKind::ImportLocal,
                url: None,
                title: None,
                local_path: Some("video.mp4".into()),
                segment_minutes: None,
                download_cookies_mode: None,
                download_cookies_file: None,
                download_cookies_from_browser: None,
            },
            PipelineOptions {
                auto_transcribe: true,
                auto_summarize: true,
                auto_chapterize: false,
                provider_profile_id: None,
                template_id: None,
                template_ids: Vec::new(),
                model: None,
                transcribe_language: None,
            },
        );
        for progress in &mut job.step_statuses {
            progress.status = StepStatus::Succeeded;
        }
        job.plain_transcript_path = Some("transcript/plain.txt".into());
        job.summary_path = Some("summary/summary.md".into());

        job.invalidate_after_step(&JobStep::Transcribe);

        assert_eq!(
            job.step_statuses
                .iter()
                .find(|progress| progress.step == JobStep::Transcribe)
                .expect("transcribe step")
                .status,
            StepStatus::Succeeded
        );
        assert_eq!(
            job.step_statuses
                .iter()
                .find(|progress| progress.step == JobStep::MergeTranscript)
                .expect("merge step")
                .status,
            StepStatus::Pending
        );
        assert_eq!(
            job.step_statuses
                .iter()
                .find(|progress| progress.step == JobStep::Summarize)
                .expect("summary step")
                .status,
            StepStatus::Pending
        );
        assert!(job.plain_transcript_path.is_none());
        assert!(job.summary_path.is_none());
    }

    #[test]
    fn missing_required_step_cannot_be_reported_as_success() {
        let mut job = Job::new(
            JobSource {
                kind: JobKind::Download,
                url: Some("https://example.com".into()),
                title: None,
                local_path: None,
                segment_minutes: None,
                download_cookies_mode: None,
                download_cookies_file: None,
                download_cookies_from_browser: None,
            },
            PipelineOptions {
                auto_transcribe: true,
                auto_summarize: false,
                auto_chapterize: false,
                provider_profile_id: None,
                template_id: None,
                template_ids: Vec::new(),
                model: None,
                transcribe_language: None,
            },
        );
        job.step_statuses
            .retain(|progress| progress.step == JobStep::Ingest);
        job.step_statuses[0].status = StepStatus::Succeeded;

        assert_eq!(job.derived_status(), JobStatus::Pending);
    }

    #[test]
    fn effective_template_ids_prefers_list_then_single_then_default() {
        let available = vec!["a".into(), "b".into(), "c".into()];
        let with_list = PipelineOptions {
            template_ids: vec!["b".into(), "a".into(), "b".into()],
            template_id: Some("c".into()),
            ..PipelineOptions::default()
        };
        assert_eq!(
            with_list.effective_template_ids(Some("a"), &available),
            vec!["b".to_string(), "a".to_string()]
        );

        let with_single = PipelineOptions {
            template_id: Some("c".into()),
            ..PipelineOptions::default()
        };
        assert_eq!(
            with_single.effective_template_ids(Some("a"), &available),
            vec!["c".to_string()]
        );

        let with_default = PipelineOptions::default();
        assert_eq!(
            with_default.effective_template_ids(Some("a"), &available),
            vec!["a".to_string()]
        );
    }
}

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
    /// Job-level Provider override. `None` means use
    /// `AppConfig.default_provider_profile_id` **at summarize run time**
    /// (not a snapshot taken at job creation).
    pub provider_profile_id: Option<String>,
    /// Job-level template override. `None` means use
    /// `AppConfig.default_template_id` at summarize run time.
    pub template_id: Option<String>,
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
            provider_profile_id: None,
            template_id: None,
            model: None,
            transcribe_language: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobSource {
    pub kind: JobKind,
    pub url: Option<String>,
    pub title: Option<String>,
    pub local_path: Option<String>,
    pub segment_minutes: Option<u32>,
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
    #[serde(default)]
    pub stop_requested: bool,
    #[serde(default)]
    pub live_capture_active: bool,
    pub error_message: Option<String>,
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
    pub current_step: Option<JobStep>,
    pub progress: f32,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateDownloadJobRequest {
    pub url: String,
    pub title: Option<String>,
    #[serde(default)]
    pub group: Option<String>,
    pub pipeline: Option<PipelineOptions>,
    #[serde(default)]
    pub auto_start: bool,
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
    pub template_id: Option<String>,
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
    pub transcribe_model: Option<String>,
    pub transcribe_language: Option<String>,
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
            step_statuses.push(StepProgress {
                step: JobStep::Summarize,
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
            stop_requested: false,
            live_capture_active: false,
            error_message: None,
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
                JobStep::Summarize,
            ],
            JobStep::Transcribe => &[JobStep::MergeTranscript, JobStep::Summarize],
            JobStep::MergeTranscript => &[JobStep::Summarize],
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
                self.summary_path = None;
            }
            JobStep::Transcribe => {
                self.plain_transcript_path = None;
                self.summary_path = None;
            }
            JobStep::MergeTranscript => {
                self.plain_transcript_path = None;
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
        if self.pipeline.auto_transcribe || self.pipeline.auto_summarize {
            required_steps.push(JobStep::Transcribe);
            required_steps.push(JobStep::MergeTranscript);
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
            current_step: self.current_step.clone(),
            progress: self.progress,
            error_message: self.error_message.clone(),
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
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
            },
            PipelineOptions {
                auto_transcribe: true,
                auto_summarize: true,
                provider_profile_id: None,
                template_id: None,
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
            },
            PipelineOptions::default(),
        );
        job.group = Some("  学习笔记  ".into());
        let list_item = job.to_list_item();
        assert_eq!(list_item.group.as_deref(), Some("学习笔记"));
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
            },
            PipelineOptions {
                auto_transcribe: true,
                auto_summarize: true,
                provider_profile_id: None,
                template_id: None,
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
            },
            PipelineOptions {
                auto_transcribe: true,
                auto_summarize: true,
                provider_profile_id: None,
                template_id: None,
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
            },
            PipelineOptions {
                auto_transcribe: true,
                auto_summarize: false,
                provider_profile_id: None,
                template_id: None,
                model: None,
                transcribe_language: None,
            },
        );
        job.step_statuses
            .retain(|progress| progress.step == JobStep::Ingest);
        job.step_statuses[0].status = StepStatus::Succeeded;

        assert_eq!(job.derived_status(), JobStatus::Pending);
    }
}

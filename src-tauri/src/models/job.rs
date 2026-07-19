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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobStep {
    Ingest,
    Transcribe,
    MergeTranscript,
    Summarize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineOptions {
    pub auto_transcribe: bool,
    pub auto_summarize: bool,
    pub provider_profile_id: Option<String>,
    pub template_id: Option<String>,
}

impl Default for PipelineOptions {
    fn default() -> Self {
        Self {
            auto_transcribe: true,
            auto_summarize: false,
            provider_profile_id: None,
            template_id: None,
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
pub struct Job {
    pub id: String,
    pub status: JobStatus,
    pub source: JobSource,
    pub pipeline: PipelineOptions,
    pub current_step: Option<JobStep>,
    pub step_statuses: Vec<StepProgress>,
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
    pub current_step: Option<JobStep>,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateDownloadJobRequest {
    pub url: String,
    pub title: Option<String>,
    pub pipeline: Option<PipelineOptions>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateLiveRecordJobRequest {
    pub url: String,
    pub title: Option<String>,
    pub segment_minutes: Option<u32>,
    pub pipeline: Option<PipelineOptions>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateImportJobRequest {
    pub local_path: String,
    pub title: Option<String>,
    pub pipeline: Option<PipelineOptions>,
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
            current_step: Some(JobStep::Ingest),
            step_statuses,
            error_message: None,
            created_at: now,
            updated_at: now,
        }
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
            current_step: self.current_step.clone(),
            error_message: self.error_message.clone(),
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

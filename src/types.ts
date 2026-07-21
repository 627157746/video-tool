export type JobKind = "download" | "live_record" | "import_local";
export type JobStatus =
  | "pending"
  | "running"
  | "succeeded"
  | "failed"
  | "cancelled";
export type JobStep =
  | "ingest"
  | "transcribe"
  | "merge_transcript"
  | "summarize";
export type StepStatus =
  | "pending"
  | "running"
  | "succeeded"
  | "failed"
  | "skipped";
export type SegmentStatus = StepStatus;
export type BinarySource = "bundled" | "configured" | "path" | "missing";

export interface PipelineOptions {
  auto_transcribe: boolean;
  auto_summarize: boolean;
  /**
   * Job-level Provider override.
   * `null` / omitted means use global default at summarize run time.
   */
  provider_profile_id?: string | null;
  /**
   * Job-level template override.
   * `null` / omitted means use global default at summarize run time.
   */
  template_id?: string | null;
  /**
   * Job-level model override for the selected (or default) Provider.
   * `null` / omitted means use that Provider's default_model at run time.
   */
  model?: string | null;
  transcribe_language?: string | null;
}

export interface UpdateJobPipelineRequest {
  job_id: string;
  provider_profile_id?: string | null;
  template_id?: string | null;
  model?: string | null;
}

export interface UpdateJobTitleRequest {
  job_id: string;
  /** Empty / null clears the custom title and falls back to URL / path / id. */
  title?: string | null;
}

export interface JobListItem {
  id: string;
  status: JobStatus;
  kind: JobKind;
  title: string;
  source_reference: string;
  current_step?: JobStep | null;
  progress: number;
  error_message?: string | null;
  created_at: string;
  updated_at: string;
}

export interface StepProgress {
  step: JobStep;
  status: StepStatus;
  detail?: string | null;
}

export interface Job {
  id: string;
  status: JobStatus;
  source: {
    kind: JobKind;
    url?: string | null;
    title?: string | null;
    local_path?: string | null;
    segment_minutes?: number | null;
  };
  pipeline: PipelineOptions;
  current_step?: JobStep | null;
  step_statuses: StepProgress[];
  progress: number;
  media_files: string[];
  media_segments: Array<{
    id: string;
    file_name: string;
    index: number;
    duration_seconds?: number | null;
    selected_for_summary: boolean;
  }>;
  transcript_segments: Array<{
    id: string;
    media_file: string;
    index: number;
    status: SegmentStatus;
    plain_path?: string | null;
    srt_path?: string | null;
    detail?: string | null;
  }>;
  selected_segment_ids: string[];
  duration_label?: string | null;
  tool_path?: string | null;
  tool_version?: string | null;
  summary_path?: string | null;
  plain_transcript_path?: string | null;
  stop_requested: boolean;
  live_capture_active: boolean;
  error_message?: string | null;
  created_at: string;
  updated_at: string;
}

export interface ProviderProfilePublic {
  id: string;
  name: string;
  protocol: string;
  base_url: string;
  api_key_env?: string | null;
  has_api_key: boolean;
  default_model: string;
  models: string[];
  extra_headers: Array<[string, string]>;
}

export interface ProviderProfileInput {
  id: string;
  name: string;
  protocol: "openai" | "anthropic";
  base_url: string;
  api_key?: string | null;
  api_key_env?: string | null;
  default_model: string;
  models: string[];
  extra_headers: Array<[string, string]>;
}

export interface SummaryTemplate {
  id: string;
  name: string;
  system_prompt: string;
  user_template: string;
}

export interface SidecarPaths {
  ffmpeg?: string | null;
  ffprobe?: string | null;
  yt_dlp?: string | null;
  streamlink?: string | null;
  transcribe?: string | null;
}

export interface AppConfigPublic {
  workspace_dir: string;
  default_segment_minutes: number;
  default_auto_transcribe: boolean;
  default_auto_summarize: boolean;
  default_provider_profile_id?: string | null;
  default_template_id?: string | null;
  proxy_url?: string | null;
  min_free_disk_gb: number;
  live_reconnect_attempts: number;
  max_context_chars: number;
  transcribe_model?: string | null;
  transcribe_language: string;
  sidecar_paths: SidecarPaths;
  providers: ProviderProfilePublic[];
  templates: SummaryTemplate[];
  config_path: string;
}

export interface SaveConfigRequest {
  workspace_dir?: string | null;
  default_segment_minutes?: number | null;
  default_auto_transcribe?: boolean | null;
  default_auto_summarize?: boolean | null;
  default_provider_profile_id?: string | null;
  default_template_id?: string | null;
  proxy_url?: string | null;
  min_free_disk_gb?: number | null;
  live_reconnect_attempts?: number | null;
  max_context_chars?: number | null;
  transcribe_model?: string | null;
  transcribe_language?: string | null;
  sidecar_paths?: SidecarPaths | null;
  providers?: ProviderProfileInput[] | null;
  templates?: SummaryTemplate[] | null;
}

export interface ResolvedBinary {
  name: string;
  path?: string | null;
  version?: string | null;
  source: BinarySource;
}

export interface SidecarStatus {
  ffmpeg: ResolvedBinary;
  ffprobe: ResolvedBinary;
  yt_dlp: ResolvedBinary;
  streamlink: ResolvedBinary;
  transcribe: ResolvedBinary;
}

export interface AppInfo {
  name: string;
  version: string;
  description: string;
}

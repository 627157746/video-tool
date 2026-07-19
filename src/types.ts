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
export type BinarySource = "bundled" | "configured" | "path" | "missing";

export interface PipelineOptions {
  auto_transcribe: boolean;
  auto_summarize: boolean;
  provider_profile_id?: string | null;
  template_id?: string | null;
}

export interface JobListItem {
  id: string;
  status: JobStatus;
  kind: JobKind;
  title: string;
  current_step?: JobStep | null;
  error_message?: string | null;
  created_at: string;
  updated_at: string;
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
  step_statuses: Array<{
    step: JobStep;
    status: StepStatus;
    detail?: string | null;
  }>;
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
}

export interface SummaryTemplate {
  id: string;
  name: string;
  system_prompt: string;
  user_template: string;
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
  sidecar_paths: {
    ffmpeg?: string | null;
    ffprobe?: string | null;
    yt_dlp?: string | null;
    streamlink?: string | null;
    transcribe?: string | null;
  };
  providers: ProviderProfilePublic[];
  templates: SummaryTemplate[];
  config_path: string;
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

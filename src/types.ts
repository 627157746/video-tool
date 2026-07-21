export type JobKind = "download" | "live_record" | "import_local";
export type JobStatus =
  | "pending"
  | "queued"
  | "running"
  | "succeeded"
  | "failed"
  | "cancelled";
export type JobStep =
  | "ingest"
  | "transcribe"
  | "merge_transcript"
  | "chapterize"
  | "summarize";
export type StepStatus =
  | "pending"
  | "running"
  | "succeeded"
  | "failed"
  | "skipped";
export type SegmentStatus = StepStatus;
export type BinarySource = "bundled" | "configured" | "path" | "missing";

export interface GlossaryReplacement {
  from: string;
  to: string;
}

export interface GlossaryConfig {
  hotwords: string[];
  replacements: GlossaryReplacement[];
  apply_as_whisper_prompt: boolean;
  apply_post_replace: boolean;
}

export interface TranscribeModelPresets {
  speed?: string | null;
  balanced?: string | null;
  quality?: string | null;
}

export interface PipelineOptions {
  auto_transcribe: boolean;
  auto_summarize: boolean;
  /** Run Chapterize after merge (before summarize when auto). */
  auto_chapterize?: boolean;
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
  /** Ordered multi-template list for one summarize run (v0.2 P3). */
  template_ids?: string[];
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
  template_ids?: string[] | null;
  model?: string | null;
}

export interface SearchHit {
  job_id: string;
  title: string;
  kind: string;
  field: string;
  snippet: string;
  path: string;
}

export interface SummaryTemplateArtifact {
  template_id: string;
  path: string;
  content: string;
  primary: boolean;
}

export interface UpdateJobTitleRequest {
  job_id: string;
  /** Empty / null clears the custom title and falls back to URL / path / id. */
  title?: string | null;
}

export interface UpdateJobGroupRequest {
  job_id: string;
  /** Empty / null clears the group (ungrouped). */
  group?: string | null;
}

export interface JobListItem {
  id: string;
  status: JobStatus;
  kind: JobKind;
  title: string;
  source_reference: string;
  /** Custom grouping label; null / omitted means ungrouped. */
  group?: string | null;
  /** Shared id for Jobs created in one multi-URL batch. */
  batch_id?: string | null;
  current_step?: JobStep | null;
  progress: number;
  error_message?: string | null;
  /** Stable machine-readable failure code for recovery UI. */
  error_code?: string | null;
  /** 1-based position in the in-memory global queue when status is queued. */
  queue_position?: number | null;
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
    /** inherit (default) | none | file | browser */
    download_cookies_mode?: string | null;
    download_cookies_file?: string | null;
    download_cookies_from_browser?: string | null;
  };
  pipeline: PipelineOptions;
  /** Custom grouping label; null / omitted means ungrouped. */
  group?: string | null;
  /** Shared id for Jobs created in one multi-URL batch. */
  batch_id?: string | null;
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
  chapters_path?: string | null;
  glossary_hash?: string | null;
  stop_requested: boolean;
  live_capture_active: boolean;
  error_message?: string | null;
  /** Stable machine-readable failure code for recovery UI. */
  error_code?: string | null;
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

/** Managed job group catalog entry stored in app config. */
export interface JobGroupDefinition {
  id: string;
  name: string;
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
  max_concurrent_jobs: number;
  max_live_records: number;
  /** Default Netscape cookies.txt path for yt-dlp (path only). */
  download_cookies_file?: string | null;
  /** Default browser for yt-dlp --cookies-from-browser. */
  download_cookies_from_browser?: string | null;
  transcribe_model?: string | null;
  transcribe_language: string;
  /** speed | balanced | quality | custom */
  transcribe_model_preset?: string;
  transcribe_model_presets?: TranscribeModelPresets;
  glossary?: GlossaryConfig;
  default_auto_chapterize?: boolean;
  sidecar_paths: SidecarPaths;
  providers: ProviderProfilePublic[];
  templates: SummaryTemplate[];
  /** Ordered catalog of custom job groups; empty/omitted on older configs. */
  job_groups?: JobGroupDefinition[];
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
  max_concurrent_jobs?: number | null;
  max_live_records?: number | null;
  download_cookies_file?: string | null;
  download_cookies_from_browser?: string | null;
  transcribe_model?: string | null;
  transcribe_language?: string | null;
  transcribe_model_preset?: string | null;
  transcribe_model_presets?: TranscribeModelPresets | null;
  glossary?: GlossaryConfig | null;
  default_auto_chapterize?: boolean | null;
  sidecar_paths?: SidecarPaths | null;
  providers?: ProviderProfileInput[] | null;
  templates?: SummaryTemplate[] | null;
  job_groups?: JobGroupDefinition[] | null;
}

export interface HealthFinding {
  job_id_or_name: string;
  path: string;
  message: string;
}

export interface WorkspaceHealthReport {
  workspace_dir: string;
  free_disk_gb?: number | null;
  min_free_disk_gb: number;
  disk_below_threshold: boolean;
  orphan_directories: HealthFinding[];
  corrupt_jobs: HealthFinding[];
  interrupted_running_jobs: HealthFinding[];
  stale_queued_jobs: HealthFinding[];
  empty_media_index_jobs: HealthFinding[];
  repaired: string[];
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

export interface DependencyHint {
  name: string;
  display_name: string;
  required: boolean;
  status: ResolvedBinary;
  guidance: string;
  install_hint: string;
}

export interface DependencyReport {
  items: DependencyHint[];
  all_required_ready: boolean;
  missing_required: string[];
}

export interface ModelFileInfo {
  path: string;
  file_name: string;
  size_bytes: number;
  exists: boolean;
  is_selected: boolean;
  kind: string;
}

export interface ModelInventory {
  selected_path?: string | null;
  selected_exists: boolean;
  scan_directories: string[];
  models: ModelFileInfo[];
}

export interface ConfigExportPackage {
  format_version: number;
  exported_at: string;
  app_version: string;
  include_secrets: boolean;
  workspace_dir?: string | null;
  default_segment_minutes: number;
  default_auto_transcribe: boolean;
  default_auto_summarize: boolean;
  default_provider_profile_id?: string | null;
  default_template_id?: string | null;
  proxy_url?: string | null;
  min_free_disk_gb: number;
  live_reconnect_attempts: number;
  max_context_chars: number;
  max_concurrent_jobs: number;
  max_live_records: number;
  download_cookies_file?: string | null;
  download_cookies_from_browser?: string | null;
  transcribe_model?: string | null;
  transcribe_language: string;
  transcribe_model_preset: string;
  transcribe_model_presets: TranscribeModelPresets;
  glossary: GlossaryConfig;
  default_auto_chapterize: boolean;
  sidecar_paths: SidecarPaths;
  providers: Array<{
    id: string;
    name: string;
    protocol: string;
    base_url: string;
    api_key?: string | null;
    api_key_env?: string | null;
    default_model: string;
    models: string[];
    extra_headers: Array<[string, string]>;
  }>;
  templates: SummaryTemplate[];
  job_groups: JobGroupDefinition[];
}

export interface ConfigImportResult {
  providers: number;
  templates: number;
  job_groups: number;
  message: string;
}

export interface UpdateCheckResult {
  current_version: string;
  latest_version?: string | null;
  update_available: boolean;
  release_page_url: string;
  release_notes?: string | null;
  message: string;
}

export interface SystemDiagnostics {
  app_name: string;
  app_version: string;
  config_path: string;
  workspace_dir: string;
  free_disk_gb?: number | null;
  min_free_disk_gb: number;
  disk_below_threshold: boolean;
  sidecars: SidecarStatus;
  dependency: DependencyReport;
  models: ModelInventory;
  workspace_health: WorkspaceHealthReport;
}

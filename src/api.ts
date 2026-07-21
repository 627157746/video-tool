import { invoke } from "@tauri-apps/api/core";
import type {
  AppConfigPublic,
  AppInfo,
  Job,
  JobListItem,
  JobStep,
  PipelineOptions,
  SaveConfigRequest,
  SidecarStatus,
  UpdateJobGroupRequest,
  UpdateJobPipelineRequest,
  UpdateJobTitleRequest,
  WorkspaceHealthReport,
} from "./types";

export async function getAppInfo(): Promise<AppInfo> {
  return invoke<AppInfo>("get_app_info");
}

export async function getConfig(): Promise<AppConfigPublic> {
  return invoke<AppConfigPublic>("get_config");
}

export async function reloadConfig(): Promise<AppConfigPublic> {
  return invoke<AppConfigPublic>("reload_config");
}

export async function saveConfig(
  request: SaveConfigRequest,
): Promise<AppConfigPublic> {
  return invoke<AppConfigPublic>("save_config", { request });
}

export async function listJobs(): Promise<JobListItem[]> {
  return invoke<JobListItem[]>("list_jobs");
}

export async function getJob(jobId: string): Promise<Job> {
  return invoke<Job>("get_job", { jobId });
}

export async function deleteJob(jobId: string): Promise<void> {
  return invoke<void>("delete_job", { jobId });
}

export async function createDownloadJob(input: {
  url: string;
  title?: string;
  group?: string | null;
  batch_id?: string | null;
  pipeline?: PipelineOptions;
  auto_start?: boolean;
}): Promise<Job> {
  return invoke<Job>("create_download_job", { request: input });
}

export interface CreateDownloadJobsBatchResult {
  batch_id?: string | null;
  jobs: Job[];
  skipped: string[];
}

export async function createDownloadJobsBatch(input: {
  urls_text: string;
  title?: string;
  group?: string | null;
  pipeline?: PipelineOptions;
  auto_start?: boolean;
  download_cookies_mode?: string | null;
  download_cookies_file?: string | null;
  download_cookies_from_browser?: string | null;
}): Promise<CreateDownloadJobsBatchResult> {
  return invoke<CreateDownloadJobsBatchResult>("create_download_jobs_batch", {
    request: input,
  });
}

export async function createLiveRecordJob(input: {
  url: string;
  title?: string;
  group?: string | null;
  segment_minutes?: number;
  pipeline?: PipelineOptions;
  auto_start?: boolean;
}): Promise<Job> {
  return invoke<Job>("create_live_record_job", { request: input });
}

export async function createImportJob(input: {
  local_path: string;
  title?: string;
  group?: string | null;
  pipeline?: PipelineOptions;
  auto_start?: boolean;
}): Promise<Job> {
  return invoke<Job>("create_import_job", { request: input });
}

export async function runJob(jobId: string, step?: JobStep | null): Promise<void> {
  return invoke<void>("run_job", {
    request: { job_id: jobId, step: step ?? null },
  });
}

export async function retryJobStep(
  jobId: string,
  step?: JobStep | null,
): Promise<void> {
  return invoke<void>("retry_job_step", {
    request: { job_id: jobId, step: step ?? null },
  });
}

export async function retryTranscriptSegment(
  jobId: string,
  segmentId: string,
): Promise<void> {
  return invoke<void>("retry_transcript_segment", {
    request: { job_id: jobId, segment_id: segmentId },
  });
}

export async function getJobLog(
  jobId: string,
  logName: string,
): Promise<string> {
  return invoke<string>("get_job_log", {
    request: { job_id: jobId, log_name: logName },
  });
}

export async function openJobDirectory(jobId: string): Promise<string> {
  return invoke<string>("open_job_directory", { jobId });
}

export async function probeSidecars(): Promise<SidecarStatus> {
  return invoke<SidecarStatus>("probe_sidecars");
}

export async function checkYtDlpUpdate(): Promise<string> {
  return invoke<string>("check_yt_dlp_update");
}

export async function stopRecording(jobId: string): Promise<Job> {
  return invoke<Job>("stop_recording", { jobId });
}

export async function selectJobSegments(
  jobId: string,
  segmentIds: string[],
): Promise<Job> {
  return invoke<Job>("select_job_segments", {
    request: { job_id: jobId, segment_ids: segmentIds },
  });
}

export async function updateJobPipeline(
  request: UpdateJobPipelineRequest,
): Promise<Job> {
  return invoke<Job>("update_job_pipeline", { request });
}

export async function updateJobTitle(
  request: UpdateJobTitleRequest,
): Promise<Job> {
  return invoke<Job>("update_job_title", { request });
}

export async function updateJobGroup(
  request: UpdateJobGroupRequest,
): Promise<Job> {
  return invoke<Job>("update_job_group", { request });
}

export async function exportJob(
  jobId: string,
  destinationDir?: string | null,
): Promise<string> {
  return invoke<string>("export_job", {
    request: { job_id: jobId, destination_dir: destinationDir ?? null },
  });
}

export async function testProvider(providerProfileId: string): Promise<string> {
  return invoke<string>("test_provider", {
    request: { provider_profile_id: providerProfileId },
  });
}

export async function getJobTranscript(jobId: string): Promise<string> {
  return invoke<string>("get_job_transcript", { jobId });
}

export async function getJobSummary(jobId: string): Promise<string> {
  return invoke<string>("get_job_summary", { jobId });
}

export async function getJobSummaries(
  jobId: string,
): Promise<
  Array<{
    template_id: string;
    path: string;
    content: string;
    primary: boolean;
  }>
> {
  return invoke("get_job_summaries", { jobId });
}

export async function searchWorkspace(
  query: string,
  limit?: number,
): Promise<
  Array<{
    job_id: string;
    title: string;
    kind: string;
    field: string;
    snippet: string;
    path: string;
  }>
> {
  return invoke("search_workspace", { query, limit: limit ?? 30 });
}

export async function rebuildSearchIndex(): Promise<number> {
  return invoke<number>("rebuild_search_index");
}

export async function getJobChapters(jobId: string): Promise<string> {
  return invoke<string>("get_job_chapters", { jobId });
}

export async function getTranscriptSegmentTexts(
  jobId: string,
  segmentId: string,
): Promise<{
  segment_id: string;
  current: string;
  previous?: string | null;
}> {
  return invoke("get_transcript_segment_texts", { jobId, segmentId });
}

export async function inspectWorkspaceHealth(): Promise<WorkspaceHealthReport> {
  return invoke<WorkspaceHealthReport>("inspect_workspace_health");
}

export async function repairWorkspaceHealth(): Promise<WorkspaceHealthReport> {
  return invoke<WorkspaceHealthReport>("repair_workspace_health");
}

export async function getDependencyReport(): Promise<
  import("./types").DependencyReport
> {
  return invoke("get_dependency_report");
}

export async function listTranscribeModels(): Promise<
  import("./types").ModelInventory
> {
  return invoke("list_transcribe_models");
}

export async function openTranscribeModelDirectory(): Promise<string> {
  return invoke<string>("open_transcribe_model_directory");
}

export async function exportAppConfig(
  includeSecrets = false,
): Promise<import("./types").ConfigExportPackage> {
  return invoke("export_app_config", { includeSecrets });
}

export async function importAppConfig(
  packagePayload: import("./types").ConfigExportPackage,
  importSecrets = false,
): Promise<import("./types").ConfigImportResult> {
  return invoke("import_app_config", {
    package: packagePayload,
    importSecrets,
  });
}

export async function checkAppUpdate(): Promise<
  import("./types").UpdateCheckResult
> {
  return invoke("check_app_update");
}

export async function getSystemDiagnostics(): Promise<
  import("./types").SystemDiagnostics
> {
  return invoke("get_system_diagnostics");
}

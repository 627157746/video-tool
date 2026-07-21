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
  pipeline?: PipelineOptions;
  auto_start?: boolean;
}): Promise<Job> {
  return invoke<Job>("create_download_job", { request: input });
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

import { invoke } from "@tauri-apps/api/core";
import type {
  AppConfigPublic,
  AppInfo,
  Job,
  JobListItem,
  PipelineOptions,
  SidecarStatus,
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

export async function listJobs(): Promise<JobListItem[]> {
  return invoke<JobListItem[]>("list_jobs");
}

export async function getJob(jobId: string): Promise<Job> {
  return invoke<Job>("get_job", { job_id: jobId });
}

export async function createDownloadJob(input: {
  url: string;
  title?: string;
  pipeline?: PipelineOptions;
}): Promise<Job> {
  return invoke<Job>("create_download_job", { request: input });
}

export async function createLiveRecordJob(input: {
  url: string;
  title?: string;
  segment_minutes?: number;
  pipeline?: PipelineOptions;
}): Promise<Job> {
  return invoke<Job>("create_live_record_job", { request: input });
}

export async function createImportJob(input: {
  local_path: string;
  title?: string;
  pipeline?: PipelineOptions;
}): Promise<Job> {
  return invoke<Job>("create_import_job", { request: input });
}

export async function openJobDirectory(jobId: string): Promise<string> {
  return invoke<string>("open_job_directory", { job_id: jobId });
}

export async function probeSidecars(): Promise<SidecarStatus> {
  return invoke<SidecarStatus>("probe_sidecars");
}

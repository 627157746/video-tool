import type { JobKind, JobStatus, JobStep, StepStatus } from "./types";

export const KIND_LABEL: Record<JobKind, string> = {
  download: "链接下载",
  live_record: "直播录制",
  import_local: "本地导入",
};

export const STATUS_LABEL: Record<JobStatus, string> = {
  pending: "等待中",
  queued: "排队中",
  running: "运行中",
  succeeded: "成功",
  failed: "失败",
  cancelled: "已取消",
};

export const STEP_LABEL: Record<JobStep, string> = {
  ingest: "获取媒体",
  transcribe: "转写",
  merge_transcript: "合并字幕",
  chapterize: "章节大纲",
  summarize: "AI 总结",
};

export const STEP_STATUS_LABEL: Record<StepStatus, string> = {
  pending: "等待",
  running: "进行中",
  succeeded: "完成",
  failed: "失败",
  skipped: "跳过",
};

export const PIPELINE_STEPS: JobStep[] = [
  "ingest",
  "transcribe",
  "merge_transcript",
  "chapterize",
  "summarize",
];

export function getStepActionLabel(status: StepStatus): string {
  if (status === "running") {
    return "运行中";
  }
  if (status === "pending" || status === "skipped") {
    return "运行";
  }
  return "重试";
}

export function formatTime(value: string): string {
  try {
    return new Date(value).toLocaleString();
  } catch {
    return value;
  }
}

export function formatProgress(value: number | undefined): string {
  if (value == null || Number.isNaN(value)) {
    return "0%";
  }
  return `${Math.max(0, Math.min(100, value)).toFixed(0)}%`;
}

export function formatQueueStatusLabel(
  status: JobStatus,
  queuePosition?: number | null,
): string {
  if (status === "queued") {
    if (queuePosition != null && queuePosition > 0) {
      return `排队中 · 第 ${queuePosition} 位`;
    }
    return "排队中";
  }
  return STATUS_LABEL[status];
}

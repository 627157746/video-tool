import type { Job, JobStep } from "./types";
import type { JobDetailSection, SettingsSection } from "./constants";
import { STEP_LABEL } from "./labels";

export type RecoveryActionId =
  | "retry_step"
  | "retry_pipeline"
  | "open_logs"
  | "open_directory"
  | "open_settings_sidecars"
  | "open_settings_pipeline"
  | "open_settings_providers"
  | "open_segments"
  | "open_pipeline";

export interface RecoveryAction {
  id: RecoveryActionId;
  label: string;
  primary?: boolean;
}

export interface RecoverySuggestion {
  code: string;
  title: string;
  summary: string;
  hints: string[];
  actions: RecoveryAction[];
  retryStep: JobStep | null;
  settingsSection?: SettingsSection;
  detailSection?: JobDetailSection;
}

function resolveRetryStep(job: Job): JobStep | null {
  if (job.current_step) {
    return job.current_step;
  }
  const failedStep = [...job.step_statuses]
    .reverse()
    .find((stepProgress) => stepProgress.status === "failed");
  return failedStep?.step ?? null;
}

function baseActionsForStep(retryStep: JobStep | null): RecoveryAction[] {
  const actions: RecoveryAction[] = [];
  if (retryStep) {
    actions.push({
      id: "retry_step",
      label: `重试本步（${STEP_LABEL[retryStep]}）`,
      primary: true,
    });
  } else {
    actions.push({
      id: "retry_pipeline",
      label: "重新运行流水线",
      primary: true,
    });
  }
  actions.push({ id: "open_logs", label: "查看日志" });
  actions.push({ id: "open_directory", label: "打开目录" });
  return actions;
}

/**
 * Map stable `error_code` + step context to recovery copy and one-click actions.
 * Falls back to UNKNOWN guidance when code is missing (older jobs).
 */
export function buildRecoverySuggestion(job: Job): RecoverySuggestion | null {
  if (job.status !== "failed" && !job.error_message && !job.error_code) {
    return null;
  }
  if (job.status !== "failed" && job.status !== "cancelled") {
    // Still show when error fields are present on a non-running job.
    if (!job.error_message && !job.error_code) {
      return null;
    }
  }
  if (job.status === "running" || job.status === "queued") {
    return null;
  }

  const code = (job.error_code ?? "").trim() || "UNKNOWN";
  const retryStep = resolveRetryStep(job);
  const baseActions = baseActionsForStep(retryStep);

  switch (code) {
    case "SIDECAR_MISSING":
      return {
        code,
        title: "外部工具缺失或未配置",
        summary:
          "流水线依赖的 yt-dlp / streamlink / ffmpeg / whisper-cli 等未找到或路径无效。",
        hints: [
          "在设置 → 外部工具 中检查可执行文件路径，或使用「探测 sidecar」。",
          "确认模型文件（whisper）路径存在且可读。",
          "配置完成后重试失败步骤。",
        ],
        actions: [
          {
            id: "open_settings_sidecars",
            label: "打开外部工具设置",
            primary: true,
          },
          ...baseActions.filter((action) => action.id !== "retry_step" && action.id !== "retry_pipeline"),
          ...baseActions.filter(
            (action) => action.id === "retry_step" || action.id === "retry_pipeline",
          ),
        ],
        retryStep,
        settingsSection: "sidecars",
      };
    case "AUTH_REQUIRED":
      return {
        code,
        title: "需要登录或 Cookie",
        summary: "下载/录制可能因未登录、会员内容或 Cookie 无效而失败。",
        hints: [
          "在设置中配置 cookies.txt 路径，或启用从浏览器读取 Cookie。",
          "也可在新建下载时为本任务单独覆盖 Cookie 模式。",
          "勿将 Cookie 文件内容提交到版本库或导出包。",
        ],
        actions: [
          {
            id: "open_settings_pipeline",
            label: "打开 Cookie / 工作区设置",
            primary: true,
          },
          ...baseActions,
        ],
        retryStep,
        settingsSection: "pipeline",
      };
    case "CONTEXT_TOO_LONG":
      return {
        code,
        title: "上下文过长",
        summary: "送入总结的文本超过模型上下文上限，或触发了本地上下文保护。",
        hints: [
          "在「分段」中缩小选中的媒体/转写范围后再总结。",
          "可换用更大上下文的模型，或在设置中调整最大上下文字符。",
          "避免一次合并过多分段全文。",
        ],
        actions: [
          { id: "open_segments", label: "打开分段选择", primary: true },
          { id: "open_pipeline", label: "调整 Provider/模型" },
          { id: "open_settings_providers", label: "打开 Provider 设置" },
          ...baseActions.filter((action) => !action.primary),
        ],
        retryStep: "summarize",
        settingsSection: "providers",
        detailSection: "segments",
      };
    case "DISK_GUARD":
      return {
        code,
        title: "磁盘空间不足",
        summary: "工作区可用空间低于安全阈值，已阻止继续写入。",
        hints: [
          "清理工作区中不需要的任务目录，或更换更大的磁盘路径。",
          "可在设置中调整「最小剩余磁盘」阈值（请谨慎降低）。",
        ],
        actions: [
          {
            id: "open_settings_pipeline",
            label: "打开工作区/磁盘设置",
            primary: true,
          },
          { id: "open_directory", label: "打开任务目录" },
          ...baseActions.filter((action) => action.id !== "open_directory"),
        ],
        retryStep,
        settingsSection: "pipeline",
      };
    case "NETWORK":
      return {
        code,
        title: "网络或代理问题",
        summary: "连接超时、DNS/TLS 失败，或代理配置不可用。",
        hints: [
          "检查本机网络，以及设置中的代理 URL 是否正确。",
          "部分站点可能需要 Cookie（见 AUTH_REQUIRED）。",
          "稍后重试；若持续失败请查看任务日志。",
        ],
        actions: [
          ...baseActions,
          { id: "open_settings_pipeline", label: "打开代理 / 流水线设置" },
        ],
        retryStep,
        settingsSection: "pipeline",
      };
    case "DOWNLOAD_FAILED":
      return {
        code,
        title: "获取媒体失败",
        summary: "下载或直播录制步骤未成功完成。",
        hints: [
          "核对 URL 是否仍有效、是否需要 Cookie。",
          "查看 ingest 日志中的 yt-dlp / streamlink / ffmpeg 输出。",
          "直播可在开播后重试；点播可检查格式与地区限制。",
        ],
        actions: baseActions,
        retryStep: retryStep ?? "ingest",
      };
    case "TRANSCRIBE_FAILED":
      return {
        code,
        title: "转写失败",
        summary: "本地 whisper 转写未成功，或合并文本失败。",
        hints: [
          "确认 whisper-cli、模型与 ffmpeg 已配置。",
          "可对失败分段单独重试（见转写分段列表）。",
          "语言设置错误时也可在设置中调整后重试。",
        ],
        actions: [
          ...baseActions,
          { id: "open_settings_sidecars", label: "检查转写工具" },
        ],
        retryStep: retryStep ?? "transcribe",
        settingsSection: "sidecars",
      };
    case "SUMMARIZE_FAILED":
      return {
        code,
        title: "总结失败",
        summary: "调用 LLM Provider 或生成总结产物时失败。",
        hints: [
          "检查 Provider API Key、Base URL 与模型名。",
          "可在任务「配置」中切换 Provider / 模板 / 模型后重试。",
          "若报上下文过长，请缩小分段选择。",
        ],
        actions: [
          { id: "open_pipeline", label: "打开任务配置", primary: true },
          {
            id: "retry_step",
            label: `重试本步（${STEP_LABEL.summarize}）`,
          },
          { id: "open_settings_providers", label: "打开 Provider 设置" },
          { id: "open_logs", label: "查看日志" },
        ],
        retryStep: "summarize",
        settingsSection: "providers",
        detailSection: "summarize",
      };
    case "INTERRUPTED":
      return {
        code,
        title: "任务被中断",
        summary: "用户停止、应用退出或进程中断导致步骤未完成。",
        hints: [
          "可直接重试失败步骤；媒体若已部分落盘，通常无需重新下载全部内容。",
          "直播停止后可对已有分段继续转写/总结。",
        ],
        actions: baseActions,
        retryStep,
      };
    default:
      return {
        code,
        title: "任务失败",
        summary: job.error_message?.trim()
          ? "请结合错误信息与日志排查；下方提供常用恢复操作。"
          : "未识别具体错误码，请查看日志后重试或调整配置。",
        hints: [
          "优先打开日志确认失败步骤与工具输出。",
          "确认外部工具、网络与磁盘空间正常后重试本步。",
        ],
        actions: baseActions,
        retryStep,
      };
  }
}

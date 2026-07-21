import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { openPath } from "@tauri-apps/plugin-opener";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import ReactMarkdown from "react-markdown";
import {
  checkYtDlpUpdate,
  createDownloadJob,
  createImportJob,
  createLiveRecordJob,
  deleteJob,
  exportJob,
  getAppInfo,
  getConfig,
  getJob,
  getJobLog,
  getJobSummary,
  getJobTranscript,
  listJobs,
  openJobDirectory,
  probeSidecars,
  runJob,
  retryTranscriptSegment,
  saveConfig,
  selectJobSegments,
  stopRecording,
  testProvider,
  updateJobPipeline,
} from "./api";
import type {
  AppConfigPublic,
  AppInfo,
  Job,
  JobKind,
  JobListItem,
  JobStatus,
  JobStep,
  ProviderProfileInput,
  SidecarStatus,
  StepStatus,
  SummaryTemplate,
} from "./types";
import {
  ACCENT_COLOR_OPTIONS,
  applyThemePreferences,
  loadThemePreferences,
  resolveThemeMode,
  saveThemePreferences,
  THEME_MODE_OPTIONS,
  type AccentColor,
  type ResolvedTheme,
  type ThemeMode,
  type ThemePreferences,
} from "./theme";
import "./App.css";

type CreateMode = "download" | "live" | "import" | null;
type MainView = "jobs" | "settings";
type SettingsSection =
  | "appearance"
  | "pipeline"
  | "providers"
  | "templates"
  | "sidecars";
type SettingsPathPickerId =
  | "workspace"
  | "transcribe-model"
  | "yt-dlp"
  | "ffmpeg"
  | "ffprobe"
  | "streamlink"
  | "transcribe";
type LogName =
  | "download"
  | "record"
  | "transcribe"
  | "merge_transcript"
  | "summarize";

const SETTINGS_SECTIONS: ReadonlyArray<{
  id: SettingsSection;
  label: string;
  description: string;
}> = [
  {
    id: "appearance",
    label: "外观与主题",
    description: "深浅色模式与强调色，仅影响本机界面。",
  },
  {
    id: "pipeline",
    label: "工作区与流水线",
    description: "工作区、默认 Provider/模板、转写与磁盘保护等全局默认。",
  },
  {
    id: "providers",
    label: "Provider 档案",
    description: "管理 AI 接口档案；左侧列表选择，右侧编辑详情。",
  },
  {
    id: "templates",
    label: "总结模板",
    description: "管理总结提示词模板；左侧列表选择，右侧编辑内容。",
  },
  {
    id: "sidecars",
    label: "Sidecar 工具",
    description: "可选覆盖可执行路径，并查看当前解析结果。解析顺序：内置 → 配置路径 → PATH。",
  },
];

/** whisper.cpp `-l` codes; `auto` means omit `-l` and let whisper detect. */
const TRANSCRIBE_LANGUAGE_OPTIONS: ReadonlyArray<{ value: string; label: string }> = [
  { value: "auto", label: "自动检测" },
  { value: "zh", label: "中文" },
  { value: "en", label: "英语" },
  { value: "ja", label: "日语" },
  { value: "ko", label: "韩语" },
  { value: "yue", label: "粤语" },
  { value: "fr", label: "法语" },
  { value: "de", label: "德语" },
  { value: "es", label: "西班牙语" },
  { value: "ru", label: "俄语" },
  { value: "pt", label: "葡萄牙语" },
  { value: "it", label: "意大利语" },
  { value: "th", label: "泰语" },
  { value: "vi", label: "越南语" },
  { value: "id", label: "印尼语" },
  { value: "ms", label: "马来语" },
  { value: "ar", label: "阿拉伯语" },
  { value: "hi", label: "印地语" },
];

interface PathPickerFieldProps {
  label: string;
  value: string;
  emptyValueLabel: string;
  selectButtonLabel: string;
  isSelecting: boolean;
  isDisabled: boolean;
  onSelect: () => void;
  onClear?: () => void;
}

interface SettingsPathSelectionOptions {
  pickerId: SettingsPathPickerId;
  title: string;
  currentPath: string;
  selectionKind: "file" | "directory";
  filters?: Array<{
    name: string;
    extensions: string[];
  }>;
  updatePath: (selectedPath: string) => void;
}

const LOG_NAMES: LogName[] = [
  "download",
  "record",
  "transcribe",
  "merge_transcript",
  "summarize",
];

const KIND_LABEL: Record<JobKind, string> = {
  download: "链接下载",
  live_record: "直播录制",
  import_local: "本地导入",
};

const STATUS_LABEL: Record<JobStatus, string> = {
  pending: "等待中",
  running: "运行中",
  succeeded: "成功",
  failed: "失败",
  cancelled: "已取消",
};

const STEP_LABEL: Record<JobStep, string> = {
  ingest: "获取媒体",
  transcribe: "转写",
  merge_transcript: "合并字幕",
  summarize: "AI 总结",
};

const STEP_STATUS_LABEL: Record<StepStatus, string> = {
  pending: "等待",
  running: "进行中",
  succeeded: "完成",
  failed: "失败",
  skipped: "跳过",
};

const PIPELINE_STEPS: JobStep[] = [
  "ingest",
  "transcribe",
  "merge_transcript",
  "summarize",
];

function getPipelineStepProgress(job: Job, step: JobStep) {
  return (
    job.step_statuses.find((progress) => progress.step === step) ?? {
      step,
      status: "pending" as const,
      detail: "尚未运行，可随时手动执行",
    }
  );
}

function getStepActionLabel(status: StepStatus): string {
  if (status === "running") {
    return "运行中";
  }
  if (status === "pending" || status === "skipped") {
    return "运行";
  }
  return "重试";
}

function formatTime(value: string): string {
  try {
    return new Date(value).toLocaleString();
  } catch {
    return value;
  }
}

function formatProgress(value: number | undefined): string {
  if (value == null || Number.isNaN(value)) {
    return "0%";
  }
  return `${Math.max(0, Math.min(100, value)).toFixed(0)}%`;
}

function jobToListItem(job: Job): JobListItem {
  return {
    id: job.id,
    status: job.status,
    kind: job.source.kind,
    title: job.source.title?.trim()
      ? job.source.title
      : job.source.url || job.source.local_path || job.id,
    source_reference: job.source.url || job.source.local_path || "",
    current_step: job.current_step,
    progress: job.progress ?? 0,
    error_message: job.error_message,
    created_at: job.created_at,
    updated_at: job.updated_at,
  };
}

function mergeJobListSnapshots(
  currentJobs: JobListItem[],
  refreshedJobs: JobListItem[],
): JobListItem[] {
  const refreshedJobIds = new Set(refreshedJobs.map((job) => job.id));
  const currentJobsById = new Map(currentJobs.map((job) => [job.id, job]));
  const mergedJobs = refreshedJobs.map((refreshedJob) => {
    const currentJob = currentJobsById.get(refreshedJob.id);
    if (currentJob && currentJob.updated_at >= refreshedJob.updated_at) {
      return currentJob;
    }
    return refreshedJob;
  });
  for (const currentJob of currentJobs) {
    if (!refreshedJobIds.has(currentJob.id)) {
      mergedJobs.push(currentJob);
    }
  }
  return mergedJobs.sort((left, right) =>
    right.created_at.localeCompare(left.created_at),
  );
}

function resolveExistingDefaultId(
  preferredId: string,
  availableIds: string[],
): string {
  const normalizedPreferredId = preferredId.trim();
  if (availableIds.includes(normalizedPreferredId)) {
    return normalizedPreferredId;
  }
  return availableIds.find((availableId) => availableId.trim())?.trim() ?? "";
}

/** Dedupe/trim model names and ensure the default model is present. */
function normalizeProviderModels(
  models: string[],
  defaultModel: string,
): { models: string[]; default_model: string } {
  const seenModelNames = new Set<string>();
  const normalizedModels: string[] = [];
  for (const modelName of models) {
    const trimmedModelName = modelName.trim();
    if (!trimmedModelName || seenModelNames.has(trimmedModelName)) {
      continue;
    }
    seenModelNames.add(trimmedModelName);
    normalizedModels.push(trimmedModelName);
  }
  const trimmedDefaultModel = defaultModel.trim();
  if (trimmedDefaultModel && !seenModelNames.has(trimmedDefaultModel)) {
    normalizedModels.unshift(trimmedDefaultModel);
  }
  const resolvedDefaultModel =
    trimmedDefaultModel || normalizedModels[0] || "";
  return {
    models: normalizedModels.length > 0 ? normalizedModels : [resolvedDefaultModel].filter(Boolean),
    default_model: resolvedDefaultModel || normalizedModels[0] || "",
  };
}

function providerModelsListText(models: string[]): string {
  return models.join("\n");
}

function parseProviderModelsListText(text: string): string[] {
  return text
    .split(/[\n,]+/)
    .map((modelName) => modelName.trim())
    .filter(Boolean);
}

function resolveProviderModelOptions(
  provider:
    | { default_model: string; models?: string[] | null }
    | undefined,
): string[] {
  if (!provider) {
    return [];
  }
  return normalizeProviderModels(
    provider.models ?? [],
    provider.default_model,
  ).models;
}

function PathPickerField({
  label,
  value,
  emptyValueLabel,
  selectButtonLabel,
  isSelecting,
  isDisabled,
  onSelect,
  onClear,
}: PathPickerFieldProps) {
  const hasSelectedPath = value.trim().length > 0;

  return (
    <div className="file-picker-field">
      <span>{label}</span>
      <div className="file-picker-row">
        <button
          className="btn secondary"
          type="button"
          disabled={isDisabled}
          aria-label={`${selectButtonLabel}：${label}`}
          onClick={onSelect}
        >
          {isSelecting
            ? "正在选择…"
            : hasSelectedPath
              ? "重新选择"
              : selectButtonLabel}
        </button>
        {onClear && (
          <button
            className="btn ghost"
            type="button"
            disabled={isDisabled || !hasSelectedPath}
            aria-label={`清空${label}`}
            onClick={onClear}
          >
            清空
          </button>
        )}
        <div
          className={`file-picker-value${hasSelectedPath ? "" : " muted"}`}
          title={hasSelectedPath ? value : emptyValueLabel}
        >
          {hasSelectedPath ? value : emptyValueLabel}
        </div>
      </div>
    </div>
  );
}

function App() {
  const [view, setView] = useState<MainView>("jobs");
  const [appInfo, setAppInfo] = useState<AppInfo | null>(null);
  const [config, setConfig] = useState<AppConfigPublic | null>(null);
  const [jobs, setJobs] = useState<JobListItem[]>([]);
  const [sidecars, setSidecars] = useState<SidecarStatus | null>(null);
  const [selectedJobId, setSelectedJobId] = useState<string | null>(null);
  const [selectedJob, setSelectedJob] = useState<Job | null>(null);
  const [logName, setLogName] = useState<LogName>("download");
  const [logText, setLogText] = useState("");
  const [transcriptText, setTranscriptText] = useState("");
  const [summaryText, setSummaryText] = useState("");
  const [searchQuery, setSearchQuery] = useState("");
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [statusMessage, setStatusMessage] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [isBusy, setIsBusy] = useState(false);
  const [isSelectingLocalFile, setIsSelectingLocalFile] = useState(false);
  const [activeSettingsPathPicker, setActiveSettingsPathPicker] =
    useState<SettingsPathPickerId | null>(null);
  const [deletingJobIds, setDeletingJobIds] = useState<Set<string>>(new Set());
  const [stoppingRecordingJobIds, setStoppingRecordingJobIds] = useState<
    Set<string>
  >(new Set());
  const [isUpdatingSegmentSelection, setIsUpdatingSegmentSelection] =
    useState(false);
  const [createMode, setCreateMode] = useState<CreateMode>(null);

  const selectedJobIdRef = useRef<string | null>(null);
  const selectedJobRef = useRef<Job | null>(null);
  const logNameRef = useRef<LogName>("download");
  const detailRequestVersionRef = useRef(0);
  const logRequestVersionRef = useRef(0);
  const refreshRequestVersionRef = useRef(0);
  const deletedJobIdsRef = useRef<Set<string>>(new Set());
  const segmentSelectionInFlightRef = useRef(false);
  const providerDraftsRef = useRef<ProviderProfileInput[]>([]);
  const settingsProxyRef = useRef("");
  const downloadUrlInputRef = useRef<HTMLTextAreaElement | null>(null);
  const liveUrlInputRef = useRef<HTMLInputElement | null>(null);
  const localFilePickerButtonRef = useRef<HTMLButtonElement | null>(null);
  const createTriggerRef = useRef<HTMLElement | null>(null);

  const [formUrl, setFormUrl] = useState("");
  const [formTitle, setFormTitle] = useState("");
  const [formLocalPath, setFormLocalPath] = useState("");
  const [formSegmentMinutes, setFormSegmentMinutes] = useState(30);
  const [autoTranscribe, setAutoTranscribe] = useState(true);
  const [autoSummarize, setAutoSummarize] = useState(false);
  const [autoStart, setAutoStart] = useState(true);
  /** Empty string means follow global default at summarize run time. */
  const [formProviderId, setFormProviderId] = useState("");
  /** Empty string means use the selected/default Provider's default_model. */
  const [formModel, setFormModel] = useState("");
  /** Empty string means follow global default template at summarize run time. */
  const [formTemplateId, setFormTemplateId] = useState("");
  const [formTranscribeLanguage, setFormTranscribeLanguage] = useState("auto");
  const [jobProviderId, setJobProviderId] = useState("");
  const [jobModel, setJobModel] = useState("");
  const [jobTemplateId, setJobTemplateId] = useState("");
  const [isSavingJobPipeline, setIsSavingJobPipeline] = useState(false);

  const [settingsWorkspace, setSettingsWorkspace] = useState("");
  const [settingsSegmentMinutes, setSettingsSegmentMinutes] = useState(30);
  const [settingsAutoTranscribe, setSettingsAutoTranscribe] = useState(true);
  const [settingsAutoSummarize, setSettingsAutoSummarize] = useState(false);
  const [settingsProxy, setSettingsProxy] = useState("");
  const [settingsMinDisk, setSettingsMinDisk] = useState(5);
  const [settingsReconnect, setSettingsReconnect] = useState(3);
  const [settingsMaxContextChars, setSettingsMaxContextChars] = useState(400000);
  const [settingsTranscribeModel, setSettingsTranscribeModel] = useState("");
  const [settingsTranscribeLanguage, setSettingsTranscribeLanguage] = useState("auto");
  const [settingsDefaultProviderId, setSettingsDefaultProviderId] = useState("");
  const [settingsDefaultTemplateId, setSettingsDefaultTemplateId] = useState("");
  const [settingsYtDlp, setSettingsYtDlp] = useState("");
  const [settingsFfmpeg, setSettingsFfmpeg] = useState("");
  const [settingsFfprobe, setSettingsFfprobe] = useState("");
  const [settingsStreamlink, setSettingsStreamlink] = useState("");
  const [settingsTranscribe, setSettingsTranscribe] = useState("");
  const [providerDrafts, setProviderDrafts] = useState<ProviderProfileInput[]>([]);
  const [templateDrafts, setTemplateDrafts] = useState<SummaryTemplate[]>([]);
  const [selectedProviderIndex, setSelectedProviderIndex] = useState(0);
  const [selectedTemplateIndex, setSelectedTemplateIndex] = useState(0);
  const [settingsSection, setSettingsSection] =
    useState<SettingsSection>("pipeline");
  const [themePreferences, setThemePreferences] = useState<ThemePreferences>(() =>
    loadThemePreferences(),
  );
  const [resolvedTheme, setResolvedTheme] = useState<ResolvedTheme>(() =>
    resolveThemeMode(loadThemePreferences().mode),
  );

  providerDraftsRef.current = providerDrafts;
  settingsProxyRef.current = settingsProxy;

  const updateThemePreferences = useCallback(
    (partialPreferences: Partial<ThemePreferences>) => {
      setThemePreferences((currentPreferences) => {
        const nextPreferences: ThemePreferences = {
          ...currentPreferences,
          ...partialPreferences,
        };
        saveThemePreferences(nextPreferences);
        setResolvedTheme(applyThemePreferences(nextPreferences));
        return nextPreferences;
      });
    },
    [],
  );

  const handleThemeModeChange = useCallback(
    (mode: ThemeMode) => {
      updateThemePreferences({ mode });
    },
    [updateThemePreferences],
  );

  const handleAccentColorChange = useCallback(
    (accent: AccentColor) => {
      updateThemePreferences({ accent });
    },
    [updateThemePreferences],
  );

  const applyConfigToSettings = useCallback((nextConfig: AppConfigPublic) => {
    setConfig(nextConfig);
    setSettingsWorkspace(nextConfig.workspace_dir);
    setSettingsSegmentMinutes(nextConfig.default_segment_minutes);
    setSettingsAutoTranscribe(nextConfig.default_auto_transcribe);
    setSettingsAutoSummarize(nextConfig.default_auto_summarize);
    setSettingsProxy(nextConfig.proxy_url ?? "");
    setSettingsMinDisk(nextConfig.min_free_disk_gb);
    setSettingsReconnect(nextConfig.live_reconnect_attempts);
    setSettingsMaxContextChars(nextConfig.max_context_chars);
    setSettingsTranscribeModel(nextConfig.transcribe_model ?? "");
    setSettingsTranscribeLanguage(nextConfig.transcribe_language);
    setSettingsDefaultProviderId(nextConfig.default_provider_profile_id ?? "");
    setSettingsDefaultTemplateId(nextConfig.default_template_id ?? "");
    setSettingsYtDlp(nextConfig.sidecar_paths.yt_dlp ?? "");
    setSettingsFfmpeg(nextConfig.sidecar_paths.ffmpeg ?? "");
    setSettingsFfprobe(nextConfig.sidecar_paths.ffprobe ?? "");
    setSettingsStreamlink(nextConfig.sidecar_paths.streamlink ?? "");
    setSettingsTranscribe(nextConfig.sidecar_paths.transcribe ?? "");
    const nextProviderDrafts = nextConfig.providers.map((provider) => {
      const normalizedModels = normalizeProviderModels(
        provider.models ?? [],
        provider.default_model,
      );
      return {
        id: provider.id,
        name: provider.name,
        protocol: provider.protocol === "anthropic" ? "anthropic" : "openai",
        base_url: provider.base_url,
        api_key: null,
        api_key_env: provider.api_key_env ?? null,
        default_model: normalizedModels.default_model,
        models: normalizedModels.models,
        extra_headers: provider.extra_headers,
      };
    });
    setProviderDrafts(nextProviderDrafts);
    setSelectedProviderIndex((currentIndex) => {
      const previousProviderId =
        providerDraftsRef.current[currentIndex]?.id ??
        nextProviderDrafts[currentIndex]?.id;
      if (previousProviderId) {
        const matchedIndex = nextProviderDrafts.findIndex(
          (provider) => provider.id === previousProviderId,
        );
        if (matchedIndex >= 0) {
          return matchedIndex;
        }
      }
      return nextProviderDrafts.length > 0
        ? Math.min(currentIndex, nextProviderDrafts.length - 1)
        : 0;
    });
    setTemplateDrafts(nextConfig.templates);
    setSelectedTemplateIndex((currentIndex) => {
      const nextTemplates = nextConfig.templates;
      if (nextTemplates.length === 0) {
        return 0;
      }
      return Math.min(currentIndex, nextTemplates.length - 1);
    });
    setFormSegmentMinutes(nextConfig.default_segment_minutes);
    setAutoTranscribe(nextConfig.default_auto_transcribe);
    setAutoSummarize(nextConfig.default_auto_summarize);
    // Create form starts on "follow global defaults" rather than pinning IDs.
    setFormProviderId("");
    setFormModel("");
    setFormTemplateId("");
    setFormTranscribeLanguage(nextConfig.transcribe_language);
  }, []);

  const clearSelectedJobState = useCallback(() => {
    detailRequestVersionRef.current += 1;
    logRequestVersionRef.current += 1;
    selectedJobIdRef.current = null;
    selectedJobRef.current = null;
    setSelectedJobId(null);
    setSelectedJob(null);
    setLogText("");
    setTranscriptText("");
    setSummaryText("");
  }, []);

  const loadJobDetail = useCallback(
    async (
      jobId: string,
      requestedLogName: LogName | null = null,
      resetDisplay = true,
    ) => {
      const requestVersion = ++detailRequestVersionRef.current;
      logRequestVersionRef.current += 1;
      if (resetDisplay) {
        selectedJobIdRef.current = jobId;
        setSelectedJobId(jobId);
        selectedJobRef.current = null;
        setSelectedJob(null);
        setLogText("正在加载…");
        setTranscriptText("");
        setSummaryText("");
      }

      try {
        const job = await getJob(jobId);
        if (
          requestVersion !== detailRequestVersionRef.current ||
          selectedJobIdRef.current !== jobId
        ) {
          return;
        }

        const preferredLog = resetDisplay
          ? requestedLogName ??
            (job.source.kind === "live_record" ? "record" : "download")
          : logNameRef.current;
        if (resetDisplay) {
          logNameRef.current = preferredLog;
          setLogName(preferredLog);
        }
        const [logResult, transcriptResult, summaryResult] =
          await Promise.allSettled([
            getJobLog(jobId, preferredLog),
            getJobTranscript(jobId),
            getJobSummary(jobId),
          ]);
        if (
          requestVersion !== detailRequestVersionRef.current ||
          selectedJobIdRef.current !== jobId ||
          logNameRef.current !== preferredLog
        ) {
          return;
        }

        selectedJobRef.current = job;
        setSelectedJob(job);
        setLogText(
          logResult.status === "fulfilled"
            ? logResult.value || "（暂无日志）"
            : "（日志读取失败）",
        );
        setTranscriptText(
          transcriptResult.status === "fulfilled" ? transcriptResult.value : "",
        );
        setSummaryText(
          summaryResult.status === "fulfilled" ? summaryResult.value : "",
        );

        const failedResult = [logResult, transcriptResult, summaryResult].find(
          (result) => result.status === "rejected",
        );
        if (failedResult?.status === "rejected") {
          setErrorMessage(
            failedResult.reason instanceof Error
              ? failedResult.reason.message
              : String(failedResult.reason),
          );
        }
      } catch (error) {
        if (
          requestVersion === detailRequestVersionRef.current &&
          selectedJobIdRef.current === jobId
        ) {
          setSelectedJob(null);
          selectedJobRef.current = null;
          setLogText("");
          setTranscriptText("");
          setSummaryText("");
          setErrorMessage(error instanceof Error ? error.message : String(error));
        }
      }
    },
    [],
  );

  const reloadLog = useCallback(async (jobId: string, name: LogName) => {
    detailRequestVersionRef.current += 1;
    const requestVersion = ++logRequestVersionRef.current;
    logNameRef.current = name;
    setLogName(name);
    try {
      const log = await getJobLog(jobId, name);
      if (
        requestVersion === logRequestVersionRef.current &&
        selectedJobIdRef.current === jobId &&
        logNameRef.current === name
      ) {
        setLogText(log || "（暂无日志）");
      }
    } catch (error) {
      if (
        requestVersion === logRequestVersionRef.current &&
        selectedJobIdRef.current === jobId
      ) {
        setLogText("（日志读取失败）");
        setErrorMessage(error instanceof Error ? error.message : String(error));
      }
    }
  }, []);

  const refresh = useCallback(
    async (preserveSettingsDrafts = false) => {
      const refreshRequestVersion = ++refreshRequestVersionRef.current;
      setErrorMessage(null);
      try {
        const [nextInfo, nextConfig, nextJobs, nextSidecars] = await Promise.all([
          getAppInfo(),
          getConfig(),
          listJobs(),
          probeSidecars(),
        ]);
        if (refreshRequestVersion !== refreshRequestVersionRef.current) {
          return;
        }
        const visibleNextJobs = nextJobs.filter(
          (job) => !deletedJobIdsRef.current.has(job.id),
        );
        setAppInfo(nextInfo);
        if (preserveSettingsDrafts) {
          setConfig(nextConfig);
        } else {
          applyConfigToSettings(nextConfig);
        }
        setJobs((currentJobs) =>
          mergeJobListSnapshots(currentJobs, visibleNextJobs),
        );
        setSidecars(nextSidecars);
        const currentJobId = selectedJobIdRef.current;
        if (
          currentJobId &&
          visibleNextJobs.some((job) => job.id === currentJobId)
        ) {
          await loadJobDetail(currentJobId, logNameRef.current, false);
        } else if (currentJobId) {
          clearSelectedJobState();
        }
      } catch (error) {
        if (refreshRequestVersion === refreshRequestVersionRef.current) {
          setErrorMessage(error instanceof Error ? error.message : String(error));
        }
      } finally {
        if (refreshRequestVersion === refreshRequestVersionRef.current) {
          setIsLoading(false);
        }
      }
    },
    [applyConfigToSettings, clearSelectedJobState, loadJobDetail],
  );

  useEffect(() => {
    void refresh(false);
  }, [refresh]);

  useEffect(() => {
    setResolvedTheme(applyThemePreferences(themePreferences));

    if (themePreferences.mode !== "system") {
      return;
    }

    const mediaQueryList = window.matchMedia("(prefers-color-scheme: dark)");
    const handleSystemThemeChange = () => {
      setResolvedTheme(applyThemePreferences(themePreferences));
    };

    mediaQueryList.addEventListener("change", handleSystemThemeChange);
    return () => {
      mediaQueryList.removeEventListener("change", handleSystemThemeChange);
    };
  }, [themePreferences]);

  useEffect(() => {
    let cancelled = false;
    let disposeListener: (() => void) | undefined;
    void listen<Job>("job-updated", (event) => {
      const job = event.payload;
      if (deletedJobIdsRef.current.has(job.id)) {
        return;
      }
      setJobs((previous) => {
        const item = jobToListItem(job);
        const exists = previous.some((entry) => entry.id === job.id);
        if (!exists) {
          return [item, ...previous];
        }
        return previous.map((entry) => (entry.id === job.id ? item : entry));
      });
      if (selectedJobIdRef.current === job.id) {
        detailRequestVersionRef.current += 1;
        selectedJobRef.current = job;
        setSelectedJob(job);
        void reloadLog(job.id, logNameRef.current);
        if (job.status !== "running") {
          void loadJobDetail(job.id, logNameRef.current, false);
        }
      }
    })
      .then((dispose) => {
        if (cancelled) {
          dispose();
        } else {
          disposeListener = dispose;
        }
      })
      .catch((error) => {
        if (!cancelled) {
          setErrorMessage(
            error instanceof Error ? error.message : String(error),
          );
        }
      });
    return () => {
      cancelled = true;
      disposeListener?.();
    };
  }, [loadJobDetail, reloadLog]);

  useEffect(() => {
    if (selectedJob?.status !== "running" || !selectedJobId) {
      return;
    }
    const interval = window.setInterval(() => {
      void loadJobDetail(selectedJobId, logNameRef.current, false);
    }, 3_000);
    return () => window.clearInterval(interval);
  }, [loadJobDetail, selectedJob?.status, selectedJobId]);

  // Success toasts auto-dismiss so fixed overlays do not linger after save.
  useEffect(() => {
    if (!statusMessage) {
      return;
    }
    const dismissTimer = window.setTimeout(() => {
      setStatusMessage(null);
    }, 4_000);
    return () => window.clearTimeout(dismissTimer);
  }, [statusMessage]);

  const filteredJobs = useMemo(() => {
    const query = searchQuery.trim().toLowerCase();
    if (!query) {
      return jobs;
    }
    return jobs.filter((job) => {
      const haystack = [
        job.id,
        job.title,
        job.source_reference,
        job.status,
        STATUS_LABEL[job.status],
        job.kind,
        KIND_LABEL[job.kind],
        job.created_at,
        job.updated_at,
        formatTime(job.created_at),
        formatTime(job.updated_at),
        job.error_message ?? "",
      ]
        .join(" ")
        .toLowerCase();
      return haystack.includes(query);
    });
  }, [jobs, searchQuery]);

  const stats = useMemo(() => {
    return {
      total: jobs.length,
      running: jobs.filter((job) => job.status === "running").length,
      failed: jobs.filter((job) => job.status === "failed").length,
      succeeded: jobs.filter((job) => job.status === "succeeded").length,
    };
  }, [jobs]);

  const providerDraftsAreDirty = useMemo(() => {
    if (!config) {
      return false;
    }
    const persistedProfiles = config.providers.map((provider) => {
      const normalizedModels = normalizeProviderModels(
        provider.models ?? [],
        provider.default_model,
      );
      return {
        id: provider.id,
        name: provider.name,
        protocol: provider.protocol === "anthropic" ? "anthropic" : "openai",
        base_url: provider.base_url,
        api_key_env: provider.api_key_env ?? null,
        default_model: normalizedModels.default_model,
        models: normalizedModels.models,
        extra_headers: provider.extra_headers,
        has_new_api_key: false,
      };
    });
    const draftProfiles = providerDrafts.map((provider) => {
      const normalizedModels = normalizeProviderModels(
        provider.models,
        provider.default_model,
      );
      return {
        id: provider.id,
        name: provider.name,
        protocol: provider.protocol,
        base_url: provider.base_url,
        api_key_env: provider.api_key_env ?? null,
        default_model: normalizedModels.default_model,
        models: normalizedModels.models,
        extra_headers: provider.extra_headers,
        has_new_api_key: Boolean(provider.api_key?.trim()),
      };
    });
    return (
      settingsProxy !== (config.proxy_url ?? "") ||
      JSON.stringify(persistedProfiles) !== JSON.stringify(draftProfiles)
    );
  }, [config, providerDrafts, settingsProxy]);

  const selectedProviderDraft = useMemo(() => {
    if (providerDrafts.length === 0) {
      return null;
    }
    const clampedIndex = Math.min(
      Math.max(selectedProviderIndex, 0),
      providerDrafts.length - 1,
    );
    return {
      index: clampedIndex,
      provider: providerDrafts[clampedIndex],
    };
  }, [providerDrafts, selectedProviderIndex]);

  const selectedTemplateDraft = useMemo(() => {
    if (templateDrafts.length === 0) {
      return null;
    }
    const clampedIndex = Math.min(
      Math.max(selectedTemplateIndex, 0),
      templateDrafts.length - 1,
    );
    return {
      index: clampedIndex,
      template: templateDrafts[clampedIndex],
    };
  }, [templateDrafts, selectedTemplateIndex]);

  const selectedCreateProvider = useMemo(() => {
    if (!config) {
      return undefined;
    }
    if (formProviderId.trim()) {
      return config.providers.find(
        (provider) => provider.id === formProviderId,
      );
    }
    return (
      config.providers.find(
        (provider) => provider.id === config.default_provider_profile_id,
      ) ?? config.providers[0]
    );
  }, [config, formProviderId]);

  const createProviderModelOptions = useMemo(
    () => resolveProviderModelOptions(selectedCreateProvider),
    [selectedCreateProvider],
  );

  const selectedJobProvider = useMemo(() => {
    if (!config) {
      return undefined;
    }
    if (jobProviderId.trim()) {
      return config.providers.find((provider) => provider.id === jobProviderId);
    }
    return (
      config.providers.find(
        (provider) => provider.id === config.default_provider_profile_id,
      ) ?? config.providers[0]
    );
  }, [config, jobProviderId]);

  const jobProviderModelOptions = useMemo(
    () => resolveProviderModelOptions(selectedJobProvider),
    [selectedJobProvider],
  );

  const jobPipelineIsDirty = useMemo(() => {
    if (!selectedJob) {
      return false;
    }
    const currentProviderId = selectedJob.pipeline.provider_profile_id ?? "";
    const currentTemplateId = selectedJob.pipeline.template_id ?? "";
    const currentModel = selectedJob.pipeline.model ?? "";
    return (
      jobProviderId !== currentProviderId ||
      jobTemplateId !== currentTemplateId ||
      jobModel !== currentModel
    );
  }, [jobModel, jobProviderId, jobTemplateId, selectedJob]);

  useEffect(() => {
    if (!selectedJob) {
      setJobProviderId("");
      setJobModel("");
      setJobTemplateId("");
      return;
    }
    setJobProviderId(selectedJob.pipeline.provider_profile_id ?? "");
    setJobModel(selectedJob.pipeline.model ?? "");
    setJobTemplateId(selectedJob.pipeline.template_id ?? "");
  }, [
    selectedJob?.id,
    selectedJob?.pipeline.provider_profile_id,
    selectedJob?.pipeline.model,
    selectedJob?.pipeline.template_id,
  ]);

  useEffect(() => {
    if (!createMode) {
      return;
    }
    if (createMode === "import") {
      localFilePickerButtonRef.current?.focus();
    } else if (createMode === "download") {
      downloadUrlInputRef.current?.focus();
    } else {
      liveUrlInputRef.current?.focus();
    }
    const backgroundElementStates = new Map<HTMLElement, boolean>();
    const makeBackgroundInert = () => {
      const backgroundElements = document.querySelectorAll<HTMLElement>(
        ".topbar, .content",
      );
      backgroundElements.forEach((element) => {
        if (!backgroundElementStates.has(element)) {
          backgroundElementStates.set(element, element.inert);
        }
        element.inert = true;
      });
    };
    makeBackgroundInert();
    const backgroundObserver = new MutationObserver(makeBackgroundInert);
    backgroundObserver.observe(document.body, {
      childList: true,
      subtree: true,
    });
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        closeCreate();
        return;
      }
      if (event.key !== "Tab") {
        return;
      }
      const modal = document.querySelector<HTMLElement>(".modal");
      const focusableElements = Array.from(
        modal?.querySelectorAll<HTMLElement>(
          'button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])',
        ) ?? [],
      );
      if (focusableElements.length === 0) {
        return;
      }
      const firstElement = focusableElements[0];
      const lastElement = focusableElements[focusableElements.length - 1];
      if (event.shiftKey && document.activeElement === firstElement) {
        event.preventDefault();
        lastElement.focus();
      } else if (!event.shiftKey && document.activeElement === lastElement) {
        event.preventDefault();
        firstElement.focus();
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => {
      window.removeEventListener("keydown", handleKeyDown);
      backgroundObserver.disconnect();
      backgroundElementStates.forEach((wasInert, element) => {
        element.inert = wasInert;
      });
    };
  }, [createMode]);

  function resetCreateForm() {
    setFormUrl("");
    setFormTitle("");
    setFormLocalPath("");
    setFormSegmentMinutes(config?.default_segment_minutes ?? 30);
    setAutoTranscribe(config?.default_auto_transcribe ?? true);
    setAutoSummarize(config?.default_auto_summarize ?? false);
    setAutoStart(true);
    setFormProviderId("");
    setFormModel("");
    setFormTemplateId("");
    setFormTranscribeLanguage(config?.transcribe_language ?? "auto");
  }

  function handleCreateProviderChange(nextProviderId: string) {
    setFormProviderId(nextProviderId);
    // Switching Provider always resets to that Provider's default model.
    setFormModel("");
  }

  function handleJobProviderChange(nextProviderId: string) {
    setJobProviderId(nextProviderId);
    setJobModel("");
  }

  function openCreate(mode: CreateMode) {
    createTriggerRef.current = document.activeElement as HTMLElement | null;
    resetCreateForm();
    setCreateMode(mode);
    setStatusMessage(null);
    setErrorMessage(null);
  }

  function closeCreate() {
    setCreateMode(null);
    window.setTimeout(() => createTriggerRef.current?.focus(), 0);
  }

  async function handleSelectLocalFile() {
    setErrorMessage(null);
    setIsSelectingLocalFile(true);

    try {
      const selectedFilePath = await open({
        title: "选择本地视频",
        multiple: false,
        directory: false,
        filters: [
          {
            name: "视频文件",
            extensions: [
              "mp4",
              "mkv",
              "mov",
              "avi",
              "webm",
              "flv",
              "m4v",
              "wmv",
              "mpeg",
              "mpg",
              "ts",
              "m2ts",
            ],
          },
        ],
      });

      if (selectedFilePath) {
        setFormLocalPath(selectedFilePath);
      }
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setIsSelectingLocalFile(false);
    }
  }

  async function handleSelectSettingsPath({
    pickerId,
    title,
    currentPath,
    selectionKind,
    filters,
    updatePath,
  }: SettingsPathSelectionOptions) {
    setErrorMessage(null);
    setActiveSettingsPathPicker(pickerId);

    try {
      const defaultPath = currentPath.trim() || undefined;
      const selectedPath =
        selectionKind === "directory"
          ? await open({
              title,
              defaultPath,
              multiple: false,
              directory: true,
            })
          : await open({
              title,
              defaultPath,
              multiple: false,
              directory: false,
              filters,
            });

      if (selectedPath) {
        updatePath(selectedPath);
      }
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setActiveSettingsPathPicker(null);
    }
  }

  async function submitCreate() {
    if (!createMode) {
      return;
    }

    setErrorMessage(null);
    setStatusMessage(null);
    setIsBusy(true);

    // Empty selector values mean "follow defaults at summarize run time".
    // Non-empty values pin an explicit override onto the Job.
    const pipeline = {
      auto_transcribe: autoTranscribe,
      auto_summarize: autoSummarize,
      provider_profile_id: formProviderId.trim() || null,
      template_id: formTemplateId.trim() || null,
      model: formModel.trim() || null,
      transcribe_language: formTranscribeLanguage,
    };

    try {
      let created: Job;
      if (createMode === "download") {
        created = await createDownloadJob({
          url: formUrl,
          title: formTitle || undefined,
          pipeline,
          auto_start: autoStart,
        });
      } else if (createMode === "live") {
        created = await createLiveRecordJob({
          url: formUrl,
          title: formTitle || undefined,
          segment_minutes: formSegmentMinutes,
          pipeline,
          auto_start: autoStart,
        });
      } else {
        created = await createImportJob({
          local_path: formLocalPath,
          title: formTitle || undefined,
          pipeline,
          auto_start: autoStart,
        });
      }

      closeCreate();
      setStatusMessage(
        autoStart
          ? "任务已创建并开始执行（下载为最佳努力，失败请看日志）"
          : "任务已创建，可在详情中手动运行",
      );
      await loadJobDetail(created.id);
      await refresh();
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setIsBusy(false);
    }
  }

  async function handleOpenDirectory(jobId: string) {
    try {
      const path = await openJobDirectory(jobId);
      await openPath(path);
      setStatusMessage(`已打开任务目录：${path}`);
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : String(error));
    }
  }

  async function handleSaveJobPipeline() {
    if (!selectedJob) {
      return;
    }
    setIsSavingJobPipeline(true);
    setErrorMessage(null);
    setStatusMessage(null);
    try {
      const updatedJob = await updateJobPipeline({
        job_id: selectedJob.id,
        provider_profile_id: jobProviderId.trim() || null,
        template_id: jobTemplateId.trim() || null,
        model: jobModel.trim() || null,
      });
      selectedJobRef.current = updatedJob;
      setSelectedJob(updatedJob);
      setJobs((previousJobs) =>
        previousJobs.map((entry) =>
          entry.id === updatedJob.id ? jobToListItem(updatedJob) : entry,
        ),
      );
      setStatusMessage(
        "总结配置已更新（将使用新 Provider/模型；如已有总结请重跑「AI 总结」）",
      );
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setIsSavingJobPipeline(false);
    }
  }

  async function handleDeleteJob(job: JobListItem) {
    const confirmed = window.confirm(
      `确定永久删除任务“${job.title}”吗？\n\n任务目录中的媒体、字幕、总结和日志都会被删除，且无法恢复。`,
    );
    if (!confirmed) {
      return;
    }

    refreshRequestVersionRef.current += 1;
    setDeletingJobIds((currentJobIds) => {
      const nextJobIds = new Set(currentJobIds);
      nextJobIds.add(job.id);
      return nextJobIds;
    });
    setErrorMessage(null);
    setStatusMessage(null);

    try {
      await deleteJob(job.id);
      deletedJobIdsRef.current.add(job.id);
      setJobs((currentJobs) =>
        currentJobs.filter((currentJob) => currentJob.id !== job.id),
      );
      if (selectedJobIdRef.current === job.id) {
        clearSelectedJobState();
      }
      setStatusMessage(`任务已删除：${job.title}`);
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setDeletingJobIds((currentJobIds) => {
        const nextJobIds = new Set(currentJobIds);
        nextJobIds.delete(job.id);
        return nextJobIds;
      });
    }
  }

  async function handleStopRecording(jobId: string) {
    setStoppingRecordingJobIds((currentJobIds) => {
      const nextJobIds = new Set(currentJobIds);
      nextJobIds.add(jobId);
      return nextJobIds;
    });
    try {
      await stopRecording(jobId);
      setStatusMessage("已发送停止录制请求；正在收尾并合并已有分段");
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setStoppingRecordingJobIds((currentJobIds) => {
        const nextJobIds = new Set(currentJobIds);
        nextJobIds.delete(jobId);
        return nextJobIds;
      });
    }
  }

  async function handleExport(jobId: string) {
    setIsBusy(true);
    try {
      const path = await exportJob(jobId);
      setStatusMessage(`任务包已导出：${path}`);
      await openPath(path);
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setIsBusy(false);
    }
  }

  async function handleToggleSegment(job: Job, segmentId: string) {
    if (job.status === "running" || segmentSelectionInFlightRef.current) {
      return;
    }
    const selectedIds = job.selected_segment_ids.includes(segmentId)
      ? job.selected_segment_ids.filter((id) => id !== segmentId)
      : [...job.selected_segment_ids, segmentId];
    segmentSelectionInFlightRef.current = true;
    setIsUpdatingSegmentSelection(true);
    try {
      const updated = await selectJobSegments(job.id, selectedIds);
      if (selectedJobIdRef.current === job.id) {
        selectedJobRef.current = updated;
        setSelectedJob(updated);
        setTranscriptText("");
        setSummaryText("");
        setStatusMessage("选段范围已更新；请重跑“合并字幕”后再总结");
      }
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : String(error));
    } finally {
      segmentSelectionInFlightRef.current = false;
      setIsUpdatingSegmentSelection(false);
    }
  }

  async function handleRetrySegment(jobId: string, segmentId: string) {
    setIsBusy(true);
    try {
      setTranscriptText("");
      setSummaryText("");
      await retryTranscriptSegment(jobId, segmentId);
      setStatusMessage(`已开始重试转写分段：${segmentId}`);
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setIsBusy(false);
    }
  }

  async function handleRun(jobId: string, step?: JobStep | null) {
    setIsBusy(true);
    setErrorMessage(null);
    try {
      if (!step || step === "ingest" || step === "transcribe") {
        setTranscriptText("");
        setSummaryText("");
      } else if (step === "merge_transcript" || step === "summarize") {
        setSummaryText("");
      }
      await runJob(jobId, step ?? null);
      setStatusMessage(
        step ? `已开始执行步骤：${STEP_LABEL[step]}` : "任务已开始运行",
      );
      if (selectedJobIdRef.current === jobId) {
        await loadJobDetail(jobId, logNameRef.current, false);
      }
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setIsBusy(false);
    }
  }

  async function handleSaveSettings() {
    refreshRequestVersionRef.current += 1;
    setIsBusy(true);
    setErrorMessage(null);
    try {
      const previousWorkspace = config?.workspace_dir;
      const normalizedProviderDrafts = providerDrafts.map((provider) => {
        const normalizedModels = normalizeProviderModels(
          provider.models,
          provider.default_model,
        );
        return {
          ...provider,
          id: provider.id.trim(),
          default_model: normalizedModels.default_model,
          models: normalizedModels.models,
        };
      });
      const normalizedTemplateDrafts = templateDrafts.map((template) => ({
        ...template,
        id: template.id.trim(),
      }));
      const defaultProviderProfileId = resolveExistingDefaultId(
        settingsDefaultProviderId,
        normalizedProviderDrafts.map((provider) => provider.id),
      );
      const defaultTemplateId = resolveExistingDefaultId(
        settingsDefaultTemplateId,
        normalizedTemplateDrafts.map((template) => template.id),
      );
      setSettingsDefaultProviderId(defaultProviderProfileId);
      setSettingsDefaultTemplateId(defaultTemplateId);
      const next = await saveConfig({
        workspace_dir: settingsWorkspace,
        default_segment_minutes: settingsSegmentMinutes,
        default_auto_transcribe: settingsAutoTranscribe,
        default_auto_summarize: settingsAutoSummarize,
        proxy_url: settingsProxy,
        min_free_disk_gb: settingsMinDisk,
        live_reconnect_attempts: settingsReconnect,
        max_context_chars: settingsMaxContextChars,
        transcribe_model: settingsTranscribeModel,
        transcribe_language: settingsTranscribeLanguage,
        default_provider_profile_id: defaultProviderProfileId,
        default_template_id: defaultTemplateId,
        sidecar_paths: {
          yt_dlp: settingsYtDlp || null,
          ffmpeg: settingsFfmpeg || null,
          ffprobe: settingsFfprobe || null,
          streamlink: settingsStreamlink || null,
          transcribe: settingsTranscribe || null,
        },
        providers: normalizedProviderDrafts,
        templates: normalizedTemplateDrafts,
      });
      applyConfigToSettings(next);
      if (previousWorkspace && previousWorkspace !== next.workspace_dir) {
        setJobs([]);
        clearSelectedJobState();
        const nextJobs = await listJobs();
        setJobs(nextJobs);
      }
      const nextSidecars = await probeSidecars();
      setSidecars(nextSidecars);
      setStatusMessage("设置已保存");
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setIsBusy(false);
    }
  }

  function handleProviderIdChange(
    providerIndex: number,
    previousProviderId: string,
    nextProviderId: string,
  ) {
    setProviderDrafts((providers) =>
      providers.map((provider, currentIndex) =>
        currentIndex === providerIndex
          ? { ...provider, id: nextProviderId }
          : provider,
      ),
    );
    if (settingsDefaultProviderId === previousProviderId) {
      setSettingsDefaultProviderId(nextProviderId);
    }
  }

  function updateProviderDraft(
    providerIndex: number,
    updater: (provider: ProviderProfileInput) => ProviderProfileInput,
  ) {
    setProviderDrafts((providers) =>
      providers.map((provider, currentIndex) =>
        currentIndex === providerIndex ? updater(provider) : provider,
      ),
    );
  }

  function handleProviderModelsTextChange(
    providerIndex: number,
    modelsText: string,
  ) {
    updateProviderDraft(providerIndex, (provider) => {
      const parsedModels = parseProviderModelsListText(modelsText);
      const nextDefaultModel =
        provider.default_model.trim() &&
        parsedModels.includes(provider.default_model.trim())
          ? provider.default_model.trim()
          : (parsedModels[0] ?? "");
      return {
        ...provider,
        models: parsedModels,
        default_model: nextDefaultModel,
      };
    });
  }

  function handleProviderDefaultModelChange(
    providerIndex: number,
    nextDefaultModel: string,
  ) {
    updateProviderDraft(providerIndex, (provider) => {
      const trimmedDefaultModel = nextDefaultModel.trim();
      const nextModels = [...provider.models];
      if (
        trimmedDefaultModel &&
        !nextModels.some((modelName) => modelName === trimmedDefaultModel)
      ) {
        nextModels.unshift(trimmedDefaultModel);
      }
      return {
        ...provider,
        default_model: nextDefaultModel,
        models: nextModels,
      };
    });
  }

  function handleDeleteProvider(providerIndex: number) {
    const remainingProviders = providerDrafts.filter(
      (_, currentIndex) => currentIndex !== providerIndex,
    );
    setProviderDrafts(remainingProviders);
    setSelectedProviderIndex((currentIndex) => {
      if (remainingProviders.length === 0) {
        return 0;
      }
      if (currentIndex > providerIndex) {
        return currentIndex - 1;
      }
      if (currentIndex >= remainingProviders.length) {
        return remainingProviders.length - 1;
      }
      return currentIndex;
    });
    setSettingsDefaultProviderId((currentDefaultId) =>
      resolveExistingDefaultId(
        currentDefaultId,
        remainingProviders.map((provider) => provider.id),
      ),
    );
  }

  function handleAddProvider() {
    const nextProviderIndex = providerDrafts.length;
    setProviderDrafts((items) => [
      ...items,
      {
        id: `provider-${items.length + 1}`,
        name: "新 Provider",
        protocol: "openai",
        base_url: "https://api.openai.com/v1",
        api_key: null,
        api_key_env: "OPENAI_API_KEY",
        default_model: "gpt-4o-mini",
        models: ["gpt-4o-mini", "gpt-4o"],
        extra_headers: [],
      },
    ]);
    setSelectedProviderIndex(nextProviderIndex);
  }

  function handleAddTemplate() {
    const nextTemplateIndex = templateDrafts.length;
    setTemplateDrafts((items) => [
      ...items,
      {
        id: `template-${items.length + 1}`,
        name: "新模板",
        system_prompt: "你是一个严谨的中文内容助理。",
        user_template: "请总结以下内容：\n\n{{transcript}}",
      },
    ]);
    setSelectedTemplateIndex(nextTemplateIndex);
  }

  function handleTemplateIdChange(
    templateIndex: number,
    previousTemplateId: string,
    nextTemplateId: string,
  ) {
    setTemplateDrafts((templates) =>
      templates.map((template, currentIndex) =>
        currentIndex === templateIndex
          ? { ...template, id: nextTemplateId }
          : template,
      ),
    );
    if (settingsDefaultTemplateId === previousTemplateId) {
      setSettingsDefaultTemplateId(nextTemplateId);
    }
  }

  function handleDeleteTemplate(templateIndex: number) {
    const remainingTemplates = templateDrafts.filter(
      (_, currentIndex) => currentIndex !== templateIndex,
    );
    setTemplateDrafts(remainingTemplates);
    setSelectedTemplateIndex((currentIndex) => {
      if (remainingTemplates.length === 0) {
        return 0;
      }
      if (currentIndex > templateIndex) {
        return currentIndex - 1;
      }
      if (currentIndex >= remainingTemplates.length) {
        return remainingTemplates.length - 1;
      }
      return currentIndex;
    });
    setSettingsDefaultTemplateId((currentDefaultId) =>
      resolveExistingDefaultId(
        currentDefaultId,
        remainingTemplates.map((template) => template.id),
      ),
    );
  }

  async function handleTestProvider(providerId: string) {
    if (providerDraftsAreDirty) {
      setErrorMessage("Provider 设置有未保存修改，请先保存后再测试连通");
      return;
    }
    const getCurrentDraftRevision = () =>
      JSON.stringify({
        providers: providerDraftsRef.current,
        proxyUrl: settingsProxyRef.current,
      });
    const testedDraftRevision = getCurrentDraftRevision();
    setIsBusy(true);
    try {
      const message = await testProvider(providerId);
      const currentDraftRevision = getCurrentDraftRevision();
      if (testedDraftRevision === currentDraftRevision) {
        setStatusMessage(message);
      } else {
        setStatusMessage("Provider 设置在测试期间已修改，已忽略过期测试结果");
      }
    } catch (error) {
      if (testedDraftRevision === getCurrentDraftRevision()) {
        setErrorMessage(error instanceof Error ? error.message : String(error));
      } else {
        setStatusMessage("Provider 设置在测试期间已修改，已忽略过期测试结果");
      }
    } finally {
      setIsBusy(false);
    }
  }

  async function handleCheckYtDlp() {
    setIsBusy(true);
    try {
      const message = await checkYtDlpUpdate();
      setStatusMessage(message);
      const nextSidecars = await probeSidecars();
      setSidecars(nextSidecars);
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setIsBusy(false);
    }
  }

  function handleLogTabNavigation(
    jobId: string,
    currentLogName: LogName,
    pressedKey: string,
  ): boolean {
    const currentIndex = LOG_NAMES.indexOf(currentLogName);
    let nextIndex: number;
    switch (pressedKey) {
      case "ArrowLeft":
        nextIndex = (currentIndex - 1 + LOG_NAMES.length) % LOG_NAMES.length;
        break;
      case "ArrowRight":
        nextIndex = (currentIndex + 1) % LOG_NAMES.length;
        break;
      case "Home":
        nextIndex = 0;
        break;
      case "End":
        nextIndex = LOG_NAMES.length - 1;
        break;
      default:
        return false;
    }

    const nextLogName = LOG_NAMES[nextIndex];
    document.getElementById(`log-tab-${nextLogName}`)?.focus();
    void reloadLog(jobId, nextLogName);
    return true;
  }

  function handleSettingsSectionNavigation(
    currentSection: SettingsSection,
    pressedKey: string,
  ): boolean {
    const currentIndex = SETTINGS_SECTIONS.findIndex(
      (section) => section.id === currentSection,
    );
    if (currentIndex < 0) {
      return false;
    }
    let nextIndex: number;
    switch (pressedKey) {
      case "ArrowUp":
      case "ArrowLeft":
        nextIndex =
          (currentIndex - 1 + SETTINGS_SECTIONS.length) %
          SETTINGS_SECTIONS.length;
        break;
      case "ArrowDown":
      case "ArrowRight":
        nextIndex = (currentIndex + 1) % SETTINGS_SECTIONS.length;
        break;
      case "Home":
        nextIndex = 0;
        break;
      case "End":
        nextIndex = SETTINGS_SECTIONS.length - 1;
        break;
      default:
        return false;
    }
    const nextSection = SETTINGS_SECTIONS[nextIndex];
    setSettingsSection(nextSection.id);
    document.getElementById(`settings-nav-${nextSection.id}`)?.focus();
    return true;
  }

  const activeSettingsSectionMeta =
    SETTINGS_SECTIONS.find((section) => section.id === settingsSection) ??
    SETTINGS_SECTIONS[0];

  const settingsPathSelectionIsActive = activeSettingsPathPicker !== null;

  return (
    <div className="app-shell">
      <div className="ambient ambient-a" />
      <div className="ambient ambient-b" />

      <header className="topbar">
        <div className="brand">
          <div className="brand-mark" aria-hidden="true">
            VT
          </div>
          <div>
            <div className="brand-title">{appInfo?.name ?? "video-tool"}</div>
            <div className="brand-subtitle">
              {appInfo?.description ?? "本地视频工作台"} · v
              {appInfo?.version ?? "0.1.0"}
            </div>
          </div>
        </div>

        <nav className="nav" aria-label="主导航">
          <button
            className={view === "jobs" ? "nav-btn active" : "nav-btn"}
            onClick={() => setView("jobs")}
            type="button"
            aria-current={view === "jobs" ? "page" : undefined}
          >
            任务中心
          </button>
          <button
            className={view === "settings" ? "nav-btn active" : "nav-btn"}
            onClick={() => setView("settings")}
            type="button"
            aria-current={view === "settings" ? "page" : undefined}
          >
            设置
          </button>
        </nav>

        <div className="top-actions">
          <div className="theme-toolbar" role="group" aria-label="外观主题">
            <div className="theme-mode-group" role="group" aria-label="深浅色模式">
              {THEME_MODE_OPTIONS.map((option) => (
                <button
                  key={option.value}
                  type="button"
                  className={
                    themePreferences.mode === option.value
                      ? "theme-mode-btn active"
                      : "theme-mode-btn"
                  }
                  aria-pressed={themePreferences.mode === option.value}
                  title={option.description}
                  onClick={() => handleThemeModeChange(option.value)}
                >
                  {option.label}
                </button>
              ))}
            </div>
          </div>
          <button
            className="btn secondary"
            onClick={() => void refresh(true)}
            type="button"
            disabled={isBusy}
          >
            刷新
          </button>
          <button className="btn ghost" onClick={() => openCreate("download")} type="button">
            下载链接
          </button>
          <button className="btn ghost" onClick={() => openCreate("live")} type="button">
            录制直播
          </button>
          <button className="btn" onClick={() => openCreate("import")} type="button">
            本地导入
          </button>
        </div>
      </header>

      {(errorMessage || statusMessage) && (
        <div className="toast-stack" aria-live="polite">
          {errorMessage && (
            <div className="banner toast error" role="alert">
              <span>{errorMessage}</span>
              <button
                type="button"
                className="banner-close"
                onClick={() => setErrorMessage(null)}
              >
                关闭
              </button>
            </div>
          )}
          {statusMessage && (
            <div className="banner toast ok" role="status">
              <span>{statusMessage}</span>
              <button
                type="button"
                className="banner-close"
                onClick={() => setStatusMessage(null)}
              >
                关闭
              </button>
            </div>
          )}
        </div>
      )}

      <main className="content">
        {view === "jobs" ? (
          <>
            <section className="hero-strip">
              <div>
                <h1>任务中心</h1>
                <p className="muted">
                  下载、直播、转写与 AI 总结统一管理。媒体留在本机，仅总结文本出网。
                  下载为<strong>最佳努力</strong>，失败请查看任务日志。
                </p>
              </div>
              <div className="stat-grid" aria-label="任务统计">
                <div className="stat-card">
                  <span className="stat-label">全部</span>
                  <strong>{stats.total}</strong>
                </div>
                <div className="stat-card running">
                  <span className="stat-label">运行中</span>
                  <strong>{stats.running}</strong>
                </div>
                <div className="stat-card ok">
                  <span className="stat-label">成功</span>
                  <strong>{stats.succeeded}</strong>
                </div>
                <div className="stat-card bad">
                  <span className="stat-label">失败</span>
                  <strong>{stats.failed}</strong>
                </div>
              </div>
            </section>

            <div className="jobs-layout">
              <section className="panel list-panel">
                <div className="panel-header">
                  <div>
                    <h2>任务列表</h2>
                    <p className="muted small">点击任务查看步骤、日志与重试</p>
                  </div>
                  <input
                    className="search"
                    aria-label="搜索任务"
                    placeholder="搜索标题 / URL / 状态 / ID"
                    value={searchQuery}
                    onChange={(event) => setSearchQuery(event.target.value)}
                  />
                </div>

                {isLoading ? (
                  <div className="empty">加载中…</div>
                ) : filteredJobs.length === 0 ? (
                  <div className="empty empty-card">
                    <div className="empty-icon" aria-hidden="true">
                      +
                    </div>
                    <h3>还没有任务</h3>
                    <p>
                      从右上角创建下载、直播或本地导入。媒体会写入工作区 jobs
                      目录，任务状态与日志可在此追踪。
                    </p>
                    <div className="empty-actions">
                      <button className="btn" type="button" onClick={() => openCreate("download")}>
                        新建下载
                      </button>
                      <button
                        className="btn secondary"
                        type="button"
                        onClick={() => openCreate("import")}
                      >
                        导入本地
                      </button>
                    </div>
                  </div>
                ) : (
                  <div className="job-list">
                    {filteredJobs.map((job) => (
                      <article
                        key={job.id}
                        className={
                          selectedJobId === job.id
                            ? "job-card selected"
                            : "job-card"
                        }
                      >
                        <button
                          className="job-card-select"
                          type="button"
                          onClick={() => void loadJobDetail(job.id)}
                        >
                          <div className="job-card-top">
                            <span className={`pill kind-${job.kind}`}>
                              {KIND_LABEL[job.kind]}
                            </span>
                            <span className={`pill status-${job.status}`}>
                              {STATUS_LABEL[job.status]}
                            </span>
                          </div>
                          <div className="job-title">{job.title}</div>
                          <div
                            className="progress-track"
                            role="progressbar"
                            aria-label={`${job.title} 进度`}
                            aria-valuemin={0}
                            aria-valuemax={100}
                            aria-valuenow={Math.max(
                              0,
                              Math.min(100, job.progress ?? 0),
                            )}
                          >
                            <div
                              className="progress-fill"
                              style={{ width: formatProgress(job.progress) }}
                            />
                          </div>
                          <div className="job-card-meta">
                            <span>{formatProgress(job.progress)}</span>
                            <span>
                              {job.current_step
                                ? STEP_LABEL[job.current_step]
                                : "—"}
                            </span>
                            <span>{formatTime(job.created_at)}</span>
                          </div>
                          {job.error_message && (
                            <div className="error-text clamp">{job.error_message}</div>
                          )}
                        </button>
                        <div className="job-card-actions">
                          <button
                            className="btn danger small"
                            type="button"
                            disabled={
                              job.status === "running" ||
                              deletingJobIds.has(job.id)
                            }
                            aria-label={`删除任务：${job.title}`}
                            title={
                              job.status === "running"
                                ? "运行中的任务不能删除"
                                : "永久删除任务及全部产物"
                            }
                            onClick={() => void handleDeleteJob(job)}
                          >
                            {deletingJobIds.has(job.id) ? "删除中…" : "删除"}
                          </button>
                        </div>
                      </article>
                    ))}
                  </div>
                )}
              </section>

              <section className="panel detail-panel">
                {!selectedJob ? (
                  <div className="empty">
                    <div className="empty-icon" aria-hidden="true">
                      →
                    </div>
                    <h3>选择一个任务</h3>
                    <p className="muted">
                      在左侧列表点选任务，查看流水线步骤、媒体产物与完整日志
                    </p>
                  </div>
                ) : (
                  <>
                    <div className="detail-header">
                      <div>
                        <div className="detail-kicker">
                          {KIND_LABEL[selectedJob.source.kind]} ·{" "}
                          <span className={`pill status-${selectedJob.status}`}>
                            {STATUS_LABEL[selectedJob.status]}
                          </span>
                        </div>
                        <h2>{selectedJob.source.title || selectedJob.source.url || selectedJob.source.local_path || selectedJob.id}</h2>
                        <div className="mono muted small">{selectedJob.id}</div>
                      </div>
                      <div className="detail-actions">
                        {selectedJob.source.kind === "live_record" &&
                          selectedJob.live_capture_active && (
                            <button
                              className="btn danger"
                              type="button"
                              disabled={
                                stoppingRecordingJobIds.has(selectedJob.id) ||
                                selectedJob.stop_requested
                              }
                              onClick={() => void handleStopRecording(selectedJob.id)}
                            >
                              {selectedJob.stop_requested ||
                              stoppingRecordingJobIds.has(selectedJob.id)
                                ? "正在停止"
                                : "停止录制"}
                            </button>
                          )}
                        <button
                          className="btn"
                          type="button"
                          disabled={isBusy || selectedJob.status === "running"}
                          onClick={() => void handleRun(selectedJob.id)}
                        >
                          {selectedJob.status === "pending" ? "开始运行" : "重新运行"}
                        </button>
                        <button
                          className="btn secondary"
                          type="button"
                          disabled={isBusy || selectedJob.status === "running"}
                          onClick={() => void handleExport(selectedJob.id)}
                        >
                          导出任务包
                        </button>
                        <button
                          className="btn secondary"
                          type="button"
                          onClick={() => void handleOpenDirectory(selectedJob.id)}
                        >
                          打开目录
                        </button>
                      </div>
                    </div>

                    <div className="detail-grid">
                      <article className="card soft">
                        <h3>来源</h3>
                        <dl className="meta-list">
                          <div>
                            <dt>URL</dt>
                            <dd className="mono">
                              {selectedJob.source.url || "—"}
                            </dd>
                          </div>
                          <div>
                            <dt>本地路径</dt>
                            <dd className="mono">
                              {selectedJob.source.local_path || "—"}
                            </dd>
                          </div>
                          <div>
                            <dt>进度</dt>
                            <dd>{formatProgress(selectedJob.progress)}</dd>
                          </div>
                          <div>
                            <dt>媒体文件</dt>
                            <dd>
                              {selectedJob.media_files?.length
                                ? selectedJob.media_files.join(", ")
                                : "—"}
                            </dd>
                          </div>
                          <div>
                            <dt>工具</dt>
                            <dd className="mono small">
                              {selectedJob.tool_path || "—"}
                              {selectedJob.tool_version
                                ? ` (${selectedJob.tool_version})`
                                : ""}
                            </dd>
                          </div>
                        </dl>
                      </article>

                      <article className="card soft">
                        <h3>流水线</h3>
                        <div className="step-list">
                          {PIPELINE_STEPS.map((stepName) => {
                            const step = getPipelineStepProgress(
                              selectedJob,
                              stepName,
                            );
                            return (
                              <div key={step.step} className="step-row">
                                <div>
                                  <strong>{STEP_LABEL[step.step]}</strong>
                                  <div className="muted small">
                                    {step.detail || "—"}
                                  </div>
                                </div>
                                <div className="step-actions">
                                  <span className={`pill step-${step.status}`}>
                                    {STEP_STATUS_LABEL[step.status]}
                                  </span>
                                  <button
                                    type="button"
                                    className="chip"
                                    disabled={
                                      isBusy || selectedJob.status === "running"
                                    }
                                    onClick={() =>
                                      void handleRun(selectedJob.id, step.step)
                                    }
                                  >
                                    {getStepActionLabel(step.status)}
                                  </button>
                                </div>
                              </div>
                            );
                          })}
                        </div>
                        <div className="pipeline-flags muted small">
                          自动转写：
                          {selectedJob.pipeline.auto_transcribe ? "开" : "关"} ·
                          自动总结：
                          {selectedJob.pipeline.auto_summarize ? "开" : "关"}
                        </div>
                      </article>

                      <article className="card soft summarize-config-card">
                        <div className="log-header">
                          <div>
                            <h3>总结配置</h3>
                            <p className="muted small">
                              选「使用全局默认 / Provider 默认」则跟随设置；指定后固化到本任务。保存后请重跑「AI 总结」。
                            </p>
                          </div>
                        </div>
                        <div className="form-grid summarize-config-form">
                          <div className="two-col">
                            <label>
                              <span>Provider</span>
                              <select
                                value={jobProviderId}
                                disabled={
                                  selectedJob.status === "running" ||
                                  isSavingJobPipeline
                                }
                                onChange={(event) =>
                                  handleJobProviderChange(event.target.value)
                                }
                              >
                                <option value="">使用全局默认</option>
                                {config?.providers.map((provider) => (
                                  <option key={provider.id} value={provider.id}>
                                    {provider.name}
                                    {provider.id ===
                                    config.default_provider_profile_id
                                      ? "（全局默认）"
                                      : ""}
                                  </option>
                                ))}
                              </select>
                            </label>
                            <label>
                              <span>模型</span>
                              <select
                                value={jobModel}
                                disabled={
                                  selectedJob.status === "running" ||
                                  isSavingJobPipeline
                                }
                                onChange={(event) =>
                                  setJobModel(event.target.value)
                                }
                              >
                                <option value="">使用 Provider 默认</option>
                                {jobProviderModelOptions.map((modelName) => (
                                  <option key={modelName} value={modelName}>
                                    {modelName}
                                    {modelName ===
                                    selectedJobProvider?.default_model
                                      ? "（档案默认）"
                                      : ""}
                                  </option>
                                ))}
                              </select>
                            </label>
                          </div>
                          <label>
                            <span>总结模板</span>
                            <select
                              value={jobTemplateId}
                              disabled={
                                selectedJob.status === "running" ||
                                isSavingJobPipeline
                              }
                              onChange={(event) =>
                                setJobTemplateId(event.target.value)
                              }
                            >
                              <option value="">使用全局默认</option>
                              {config?.templates.map((template) => (
                                <option key={template.id} value={template.id}>
                                  {template.name}
                                  {template.id === config.default_template_id
                                    ? "（全局默认）"
                                    : ""}
                                </option>
                              ))}
                            </select>
                          </label>
                          <div className="detail-actions summarize-config-actions">
                            <button
                              type="button"
                              className="btn secondary small"
                              disabled={
                                selectedJob.status === "running" ||
                                isSavingJobPipeline ||
                                !jobPipelineIsDirty
                              }
                              onClick={() => void handleSaveJobPipeline()}
                            >
                              {isSavingJobPipeline ? "保存中…" : "保存总结配置"}
                            </button>
                            {jobPipelineIsDirty ? (
                              <span className="muted small">有未保存的修改</span>
                            ) : (
                              <span className="muted small">
                                当前与任务已保存配置一致
                              </span>
                            )}
                          </div>
                        </div>
                      </article>
                    </div>

                    {selectedJob.transcript_segments.length > 0 && (
                      <article className="card soft segment-card">
                        <div className="log-header">
                          <div>
                            <h3>总结选段</h3>
                            <p className="muted small">
                              取消不需要的分段后，依次重试“合并字幕”和“AI 总结”。
                            </p>
                          </div>
                          <span className="pill">
                            已选 {selectedJob.selected_segment_ids.length} / {selectedJob.transcript_segments.length}
                          </span>
                        </div>
                        <div className="segment-list">
                          {selectedJob.transcript_segments.map((segment) => (
                            <div key={segment.id} className="segment-row">
                              <input
                                type="checkbox"
                                aria-label={`选择转写分段 ${segment.id}`}
                                checked={selectedJob.selected_segment_ids.includes(segment.id)}
                                disabled={
                                  selectedJob.status === "running" ||
                                  isUpdatingSegmentSelection
                                }
                                onChange={() => void handleToggleSegment(selectedJob, segment.id)}
                              />
                              <span>
                                <strong>{segment.id}</strong>
                                <span className="muted small">{segment.media_file}</span>
                              </span>
                              <div className="step-actions">
                                <span className={`pill step-${segment.status}`}>
                                  {STEP_STATUS_LABEL[segment.status]}
                                </span>
                                <button
                                  type="button"
                                  className="chip"
                                  disabled={isBusy || selectedJob.status === "running"}
                                  onClick={() => {
                                    void handleRetrySegment(selectedJob.id, segment.id);
                                  }}
                                >
                                  重试转写
                                </button>
                              </div>
                            </div>
                          ))}
                        </div>
                      </article>
                    )}

                    {selectedJob.error_message && (
                      <div className="banner error inline">
                        {selectedJob.error_message}
                      </div>
                    )}

                    <article className="card soft log-card">
                      <div className="log-header">
                        <h3>日志</h3>
                        <div className="log-tabs" role="tablist" aria-label="任务日志">
                          {LOG_NAMES.map((name) => (
                            <button
                              key={name}
                              id={`log-tab-${name}`}
                              type="button"
                              role="tab"
                              aria-selected={logName === name}
                              aria-controls="job-log-panel"
                              tabIndex={logName === name ? 0 : -1}
                              className={
                                logName === name ? "chip active" : "chip"
                              }
                              onClick={() => {
                                setLogName(name);
                                void reloadLog(selectedJob.id, name);
                              }}
                              onKeyDown={(event) => {
                                if (
                                  handleLogTabNavigation(
                                    selectedJob.id,
                                    name,
                                    event.key,
                                  )
                                ) {
                                  event.preventDefault();
                                }
                              }}
                            >
                              {name}
                            </button>
                          ))}
                        </div>
                      </div>
                      <pre
                        id="job-log-panel"
                        className="log-view"
                        role="tabpanel"
                        aria-labelledby={`log-tab-${logName}`}
                        tabIndex={0}
                      >
                        {logText}
                      </pre>
                    </article>

                    {(transcriptText || summaryText) && (
                      <div className="artifact-stack">
                        {summaryText && (
                          <article className="card soft summary-card">
                            <div className="artifact-card-header">
                              <h3>Markdown 总结</h3>
                              <span className="muted small">可读文档视图</span>
                            </div>
                            <div className="markdown-view">
                              <ReactMarkdown>{summaryText}</ReactMarkdown>
                            </div>
                          </article>
                        )}
                        {transcriptText && (
                          <article className="card soft transcript-card">
                            <div className="artifact-card-header">
                              <h3>合并字幕</h3>
                              <span className="muted small">原文对照</span>
                            </div>
                            <pre className="artifact-view transcript-view">
                              {transcriptText}
                            </pre>
                          </article>
                        )}
                      </div>
                    )}
                  </>
                )}
              </section>
            </div>
          </>
        ) : (
          <section className="panel settings">
            <div className="panel-header">
              <div>
                <h1>设置</h1>
                <p className="muted">
                  工作区与 API Key 分离存放。按左侧分区管理，修改后点右上角保存。
                </p>
              </div>
              <div className="detail-actions">
                {settingsSection === "sidecars" && (
                  <button
                    className="btn secondary"
                    type="button"
                    disabled={isBusy}
                    onClick={() => void handleCheckYtDlp()}
                  >
                    检查并更新 yt-dlp
                  </button>
                )}
                <button
                  className="btn"
                  type="button"
                  disabled={isBusy}
                  onClick={() => void handleSaveSettings()}
                >
                  保存设置
                </button>
              </div>
            </div>

            <div className="settings-layout">
              <nav
                className="settings-nav"
                role="tablist"
                aria-label="设置分区"
                aria-orientation="vertical"
              >
                {SETTINGS_SECTIONS.map((section) => {
                  const isActive = settingsSection === section.id;
                  const sectionCountLabel =
                    section.id === "providers"
                      ? String(providerDrafts.length)
                      : section.id === "templates"
                        ? String(templateDrafts.length)
                        : null;
                  return (
                    <button
                      key={section.id}
                      id={`settings-nav-${section.id}`}
                      type="button"
                      role="tab"
                      aria-selected={isActive}
                      aria-controls={`settings-panel-${section.id}`}
                      tabIndex={isActive ? 0 : -1}
                      className={
                        isActive
                          ? "settings-nav-item active"
                          : "settings-nav-item"
                      }
                      onClick={() => setSettingsSection(section.id)}
                      onKeyDown={(event) => {
                        if (
                          handleSettingsSectionNavigation(
                            settingsSection,
                            event.key,
                          )
                        ) {
                          event.preventDefault();
                        }
                      }}
                    >
                      <span className="settings-nav-item-label">
                        {section.label}
                      </span>
                      {sectionCountLabel != null && (
                        <span className="settings-nav-count">
                          {sectionCountLabel}
                        </span>
                      )}
                    </button>
                  );
                })}
              </nav>

              <div className="settings-main">
                <div className="settings-section-intro">
                  <h2 id={`settings-panel-title-${settingsSection}`}>
                    {activeSettingsSectionMeta.label}
                  </h2>
                  <p className="muted small">
                    {activeSettingsSectionMeta.description}
                  </p>
                </div>

                {settingsSection === "appearance" && (
              <div
                id="settings-panel-appearance"
                className="settings-section-panel"
                role="tabpanel"
                aria-labelledby="settings-nav-appearance"
              >
              <article className="card settings-wide theme-card">
                <h2 className="visually-hidden">外观与主题</h2>
                <div>
                  <div className="theme-section-label">深浅色模式</div>
                  <div className="theme-mode-options" role="group" aria-label="深浅色模式">
                    {THEME_MODE_OPTIONS.map((option) => (
                      <button
                        key={option.value}
                        type="button"
                        className={
                          themePreferences.mode === option.value
                            ? "theme-mode-option active"
                            : "theme-mode-option"
                        }
                        aria-pressed={themePreferences.mode === option.value}
                        onClick={() => handleThemeModeChange(option.value)}
                      >
                        <strong>{option.label}</strong>
                        <span>{option.description}</span>
                      </button>
                    ))}
                  </div>
                  <p className="theme-resolved-hint">
                    当前生效：
                    {resolvedTheme === "light" ? "浅色" : "深色"}
                    {themePreferences.mode === "system" ? "（跟随系统）" : ""}
                  </p>
                </div>
                <div>
                  <div className="theme-section-label">主题强调色</div>
                  <div className="accent-swatch-list" role="group" aria-label="主题强调色">
                    {ACCENT_COLOR_OPTIONS.map((option) => (
                      <button
                        key={option.value}
                        type="button"
                        className={
                          themePreferences.accent === option.value
                            ? "accent-swatch active"
                            : "accent-swatch"
                        }
                        aria-pressed={themePreferences.accent === option.value}
                        onClick={() => handleAccentColorChange(option.value)}
                      >
                        <span
                          className="accent-swatch-dot"
                          style={{ background: option.swatch, color: option.swatch }}
                          aria-hidden="true"
                        />
                        {option.label}
                      </button>
                    ))}
                  </div>
                </div>
              </article>
              </div>
                )}

                {settingsSection === "pipeline" && (
              <div
                id="settings-panel-pipeline"
                className="settings-section-panel"
                role="tabpanel"
                aria-labelledby="settings-nav-pipeline"
              >
              <article className="card settings-wide">
                <h2 className="visually-hidden">工作区与默认流水线</h2>
                <div className="form-grid">
                  <PathPickerField
                    label="工作区路径"
                    value={settingsWorkspace}
                    emptyValueLabel="尚未选择工作区"
                    selectButtonLabel="选择目录"
                    isSelecting={activeSettingsPathPicker === "workspace"}
                    isDisabled={isBusy || settingsPathSelectionIsActive}
                    onSelect={() =>
                      void handleSelectSettingsPath({
                        pickerId: "workspace",
                        title: "选择工作区目录",
                        currentPath: settingsWorkspace,
                        selectionKind: "directory",
                        updatePath: setSettingsWorkspace,
                      })
                    }
                  />
                  <label>
                    <span>默认直播分段（分钟）</span>
                    <input
                      type="number"
                      min={1}
                      value={settingsSegmentMinutes}
                      onChange={(event) =>
                        setSettingsSegmentMinutes(Number(event.target.value) || 30)
                      }
                    />
                  </label>
                  <label>
                    <span>代理 URL（总结出网，可选）</span>
                    <input
                      value={settingsProxy}
                      onChange={(event) => setSettingsProxy(event.target.value)}
                      placeholder="http://127.0.0.1:7890"
                    />
                  </label>
                  <label>
                    <span>默认 Provider</span>
                    <select
                      value={settingsDefaultProviderId}
                      onChange={(event) => setSettingsDefaultProviderId(event.target.value)}
                    >
                      {providerDrafts.map((provider) => (
                        <option key={provider.id} value={provider.id}>{provider.name}</option>
                      ))}
                    </select>
                  </label>
                  <label>
                    <span>默认总结模板</span>
                    <select
                      value={settingsDefaultTemplateId}
                      onChange={(event) => setSettingsDefaultTemplateId(event.target.value)}
                    >
                      {templateDrafts.map((template) => (
                        <option key={template.id} value={template.id}>{template.name}</option>
                      ))}
                    </select>
                  </label>
                  <div className="checkbox-row">
                    <label className="checkbox">
                      <input
                        type="checkbox"
                        checked={settingsAutoTranscribe}
                        disabled={settingsAutoSummarize}
                        onChange={(event) =>
                          setSettingsAutoTranscribe(event.target.checked)
                        }
                      />
                      默认自动转写
                    </label>
                    <label className="checkbox">
                      <input
                        type="checkbox"
                        checked={settingsAutoSummarize}
                        onChange={(event) => {
                          const checked = event.target.checked;
                          setSettingsAutoSummarize(checked);
                          if (checked) {
                            setSettingsAutoTranscribe(true);
                          }
                        }}
                      />
                      默认自动总结
                    </label>
                  </div>
                  <div className="two-col">
                    <label>
                      <span>磁盘保护阈值（GB）</span>
                      <input
                        type="number"
                        min={1}
                        value={settingsMinDisk}
                        onChange={(event) =>
                          setSettingsMinDisk(Number(event.target.value) || 5)
                        }
                      />
                    </label>
                    <label>
                      <span>直播重连次数</span>
                      <input
                        type="number"
                        min={0}
                        value={settingsReconnect}
                        onChange={(event) =>
                          setSettingsReconnect(Number(event.target.value) || 0)
                        }
                      />
                    </label>
                  </div>
                  <div className="two-col">
                    <label>
                      <span>总结最大输入字符数</span>
                      <input
                        type="number"
                        min={1000}
                        value={settingsMaxContextChars}
                        onChange={(event) => setSettingsMaxContextChars(Number(event.target.value) || 400000)}
                      />
                    </label>
                    <label>
                      <span>转写语言</span>
                      <select
                        value={settingsTranscribeLanguage}
                        onChange={(event) => setSettingsTranscribeLanguage(event.target.value)}
                      >
                        {!TRANSCRIBE_LANGUAGE_OPTIONS.some(
                          (option) => option.value === settingsTranscribeLanguage,
                        ) && (
                          <option value={settingsTranscribeLanguage}>
                            {settingsTranscribeLanguage || "未知"}（当前配置）
                          </option>
                        )}
                        {TRANSCRIBE_LANGUAGE_OPTIONS.map((option) => (
                          <option key={option.value} value={option.value}>
                            {option.label}
                          </option>
                        ))}
                      </select>
                    </label>
                  </div>
                  <PathPickerField
                    label="whisper.cpp 模型文件"
                    value={settingsTranscribeModel}
                    emptyValueLabel="尚未选择 GGML 模型文件"
                    selectButtonLabel="选择文件"
                    isSelecting={activeSettingsPathPicker === "transcribe-model"}
                    isDisabled={isBusy || settingsPathSelectionIsActive}
                    onSelect={() =>
                      void handleSelectSettingsPath({
                        pickerId: "transcribe-model",
                        title: "选择 whisper.cpp GGML 模型文件",
                        currentPath: settingsTranscribeModel,
                        selectionKind: "file",
                        filters: [{ name: "GGML 模型", extensions: ["bin"] }],
                        updatePath: setSettingsTranscribeModel,
                      })
                    }
                    onClear={() => setSettingsTranscribeModel("")}
                  />
                  <p className="muted small mono">
                    配置文件：{config?.config_path ?? "—"}
                  </p>
                </div>
              </article>
              </div>
                )}

                {settingsSection === "sidecars" && (
              <div
                id="settings-panel-sidecars"
                className="settings-section-panel"
                role="tabpanel"
                aria-labelledby="settings-nav-sidecars"
              >
              <article className="card settings-wide">
                <h2>Sidecar 路径（可选覆盖）</h2>
                <div className="form-grid">
                  <PathPickerField
                    label="yt-dlp"
                    value={settingsYtDlp}
                    emptyValueLabel="未覆盖，使用内置版本或 PATH"
                    selectButtonLabel="选择文件"
                    isSelecting={activeSettingsPathPicker === "yt-dlp"}
                    isDisabled={isBusy || settingsPathSelectionIsActive}
                    onSelect={() =>
                      void handleSelectSettingsPath({
                        pickerId: "yt-dlp",
                        title: "选择 yt-dlp 可执行文件",
                        currentPath: settingsYtDlp,
                        selectionKind: "file",
                        updatePath: setSettingsYtDlp,
                      })
                    }
                    onClear={() => setSettingsYtDlp("")}
                  />
                  <PathPickerField
                    label="ffmpeg"
                    value={settingsFfmpeg}
                    emptyValueLabel="未覆盖，使用内置版本或 PATH"
                    selectButtonLabel="选择文件"
                    isSelecting={activeSettingsPathPicker === "ffmpeg"}
                    isDisabled={isBusy || settingsPathSelectionIsActive}
                    onSelect={() =>
                      void handleSelectSettingsPath({
                        pickerId: "ffmpeg",
                        title: "选择 ffmpeg 可执行文件",
                        currentPath: settingsFfmpeg,
                        selectionKind: "file",
                        updatePath: setSettingsFfmpeg,
                      })
                    }
                    onClear={() => setSettingsFfmpeg("")}
                  />
                  <PathPickerField
                    label="ffprobe"
                    value={settingsFfprobe}
                    emptyValueLabel="未覆盖，使用内置版本或 PATH"
                    selectButtonLabel="选择文件"
                    isSelecting={activeSettingsPathPicker === "ffprobe"}
                    isDisabled={isBusy || settingsPathSelectionIsActive}
                    onSelect={() =>
                      void handleSelectSettingsPath({
                        pickerId: "ffprobe",
                        title: "选择 ffprobe 可执行文件",
                        currentPath: settingsFfprobe,
                        selectionKind: "file",
                        updatePath: setSettingsFfprobe,
                      })
                    }
                    onClear={() => setSettingsFfprobe("")}
                  />
                  <PathPickerField
                    label="streamlink"
                    value={settingsStreamlink}
                    emptyValueLabel="未覆盖，使用内置版本或 PATH"
                    selectButtonLabel="选择文件"
                    isSelecting={activeSettingsPathPicker === "streamlink"}
                    isDisabled={isBusy || settingsPathSelectionIsActive}
                    onSelect={() =>
                      void handleSelectSettingsPath({
                        pickerId: "streamlink",
                        title: "选择 streamlink 可执行文件",
                        currentPath: settingsStreamlink,
                        selectionKind: "file",
                        updatePath: setSettingsStreamlink,
                      })
                    }
                    onClear={() => setSettingsStreamlink("")}
                  />
                  <PathPickerField
                    label="whisper.cpp / whisper-cli"
                    value={settingsTranscribe}
                    emptyValueLabel="未覆盖，使用内置版本或 PATH"
                    selectButtonLabel="选择文件"
                    isSelecting={activeSettingsPathPicker === "transcribe"}
                    isDisabled={isBusy || settingsPathSelectionIsActive}
                    onSelect={() =>
                      void handleSelectSettingsPath({
                        pickerId: "transcribe",
                        title: "选择 whisper-cli 可执行文件",
                        currentPath: settingsTranscribe,
                        selectionKind: "file",
                        updatePath: setSettingsTranscribe,
                      })
                    }
                    onClear={() => setSettingsTranscribe("")}
                  />
                </div>
              </article>

              <article className="card settings-wide">
                <h2>Sidecar 探测结果</h2>
                <div className="sidecar-list">
                  {sidecars &&
                    Object.values(sidecars).map((binary) => (
                      <div key={binary.name} className="sidecar-row">
                        <div>
                          <strong>{binary.name}</strong>
                          <span className={`pill source-${binary.source}`}>
                            {binary.source}
                          </span>
                        </div>
                        <div className="mono muted">
                          {binary.path ?? "未找到"}
                        </div>
                        <div className="muted small">
                          {binary.version ?? "无版本信息"}
                        </div>
                      </div>
                    ))}
                </div>
              </article>
              </div>
                )}

                {settingsSection === "providers" && (
              <div
                id="settings-panel-providers"
                className="settings-section-panel"
                role="tabpanel"
                aria-labelledby="settings-nav-providers"
              >
              <article className="card settings-wide">
                <div className="log-header">
                  <div>
                    <h2 className="visually-hidden">Provider 档案</h2>
                    <p className="muted small settings-collection-hint">
                      共 {providerDrafts.length} 个档案
                    </p>
                  </div>
                  <button
                    type="button"
                    className="btn secondary small"
                    onClick={handleAddProvider}
                  >
                    添加档案
                  </button>
                </div>
                <div className="settings-split">
                  <div
                    className="settings-item-list"
                    role="listbox"
                    aria-label="Provider 档案列表"
                  >
                    {providerDrafts.length === 0 ? (
                      <div className="settings-item-empty muted">
                        还没有 Provider，点击「添加档案」开始配置。
                      </div>
                    ) : (
                      providerDrafts.map((provider, index) => {
                        const isSelected =
                          selectedProviderDraft?.index === index;
                        const isDefault =
                          provider.id === settingsDefaultProviderId;
                        const protocolLabel =
                          provider.protocol === "anthropic"
                            ? "Anthropic"
                            : "OpenAI";
                        return (
                          <button
                            key={`${provider.id}-${index}`}
                            type="button"
                            role="option"
                            aria-selected={isSelected}
                            className={
                              isSelected
                                ? "settings-item selected"
                                : "settings-item"
                            }
                            onClick={() => setSelectedProviderIndex(index)}
                          >
                            <div className="settings-item-top">
                              <strong>
                                {provider.name.trim() || "未命名 Provider"}
                              </strong>
                              {isDefault && (
                                <span className="settings-item-badge">默认</span>
                              )}
                            </div>
                            <div className="settings-item-meta muted small">
                              <span>{protocolLabel}</span>
                              <span className="mono">
                                {provider.default_model.trim() || "未设模型"}
                              </span>
                            </div>
                            <div
                              className="settings-item-sub muted small mono"
                              title={provider.id}
                            >
                              {provider.id.trim() || "未设 ID"}
                            </div>
                          </button>
                        );
                      })
                    )}
                  </div>
                  <div className="settings-item-detail">
                    {selectedProviderDraft ? (
                      <div className="profile-editor settings-detail-editor">
                        <div className="settings-detail-title">
                          <strong>
                            {selectedProviderDraft.provider.name.trim() ||
                              "未命名 Provider"}
                          </strong>
                          <span className="muted small mono">
                            {selectedProviderDraft.provider.id.trim() ||
                              "未设 ID"}
                          </span>
                        </div>
                        <div className="two-col">
                          <label>
                            <span>ID</span>
                            <input
                              value={selectedProviderDraft.provider.id}
                              onChange={(event) =>
                                handleProviderIdChange(
                                  selectedProviderDraft.index,
                                  selectedProviderDraft.provider.id,
                                  event.target.value,
                                )
                              }
                            />
                          </label>
                          <label>
                            <span>名称</span>
                            <input
                              value={selectedProviderDraft.provider.name}
                              onChange={(event) =>
                                updateProviderDraft(
                                  selectedProviderDraft.index,
                                  (item) => ({
                                    ...item,
                                    name: event.target.value,
                                  }),
                                )
                              }
                            />
                          </label>
                        </div>
                        <div className="two-col">
                          <label>
                            <span>协议</span>
                            <select
                              value={selectedProviderDraft.provider.protocol}
                              onChange={(event) =>
                                updateProviderDraft(
                                  selectedProviderDraft.index,
                                  (item) => ({
                                    ...item,
                                    protocol: event.target.value as
                                      | "openai"
                                      | "anthropic",
                                  }),
                                )
                              }
                            >
                              <option value="openai">OpenAI</option>
                              <option value="anthropic">Anthropic</option>
                            </select>
                          </label>
                          <label>
                            <span>默认模型</span>
                            {selectedProviderDraft.provider.models.length >
                            0 ? (
                              <select
                                value={
                                  selectedProviderDraft.provider.models.includes(
                                    selectedProviderDraft.provider.default_model,
                                  )
                                    ? selectedProviderDraft.provider
                                        .default_model
                                    : selectedProviderDraft.provider.models[0]
                                }
                                onChange={(event) =>
                                  handleProviderDefaultModelChange(
                                    selectedProviderDraft.index,
                                    event.target.value,
                                  )
                                }
                              >
                                {selectedProviderDraft.provider.models.map(
                                  (modelName) => (
                                    <option key={modelName} value={modelName}>
                                      {modelName}
                                    </option>
                                  ),
                                )}
                              </select>
                            ) : (
                              <input
                                value={
                                  selectedProviderDraft.provider.default_model
                                }
                                onChange={(event) =>
                                  handleProviderDefaultModelChange(
                                    selectedProviderDraft.index,
                                    event.target.value,
                                  )
                                }
                                placeholder="gpt-4o-mini"
                              />
                            )}
                          </label>
                        </div>
                        <label>
                          <span>可用模型（每行一个，或用逗号分隔）</span>
                          <textarea
                            rows={4}
                            value={providerModelsListText(
                              selectedProviderDraft.provider.models,
                            )}
                            onChange={(event) =>
                              handleProviderModelsTextChange(
                                selectedProviderDraft.index,
                                event.target.value,
                              )
                            }
                            placeholder={"gpt-4o-mini\ngpt-4o\no3-mini"}
                          />
                        </label>
                        <div className="muted small">
                          同一 Provider 共用 Base URL 与 API Key；创建任务时可在模型列表中切换。
                        </div>
                        <label>
                          <span>Base URL</span>
                          <input
                            value={selectedProviderDraft.provider.base_url}
                            onChange={(event) =>
                              updateProviderDraft(
                                selectedProviderDraft.index,
                                (item) => ({
                                  ...item,
                                  base_url: event.target.value,
                                }),
                              )
                            }
                          />
                        </label>
                        <div className="two-col">
                          <label>
                            <span>API Key 环境变量</span>
                            <input
                              value={
                                selectedProviderDraft.provider.api_key_env ?? ""
                              }
                              onChange={(event) =>
                                updateProviderDraft(
                                  selectedProviderDraft.index,
                                  (item) => ({
                                    ...item,
                                    api_key_env: event.target.value || null,
                                  }),
                                )
                              }
                            />
                          </label>
                          <label>
                            <span>API Key（留空保留原值）</span>
                            <input
                              type="password"
                              value={
                                selectedProviderDraft.provider.api_key ?? ""
                              }
                              onChange={(event) =>
                                updateProviderDraft(
                                  selectedProviderDraft.index,
                                  (item) => ({
                                    ...item,
                                    api_key: event.target.value || null,
                                  }),
                                )
                              }
                            />
                          </label>
                        </div>
                        <div className="detail-actions">
                          <button
                            type="button"
                            className="btn secondary small"
                            disabled={isBusy || providerDraftsAreDirty}
                            title={
                              providerDraftsAreDirty
                                ? "请先保存 Provider 修改"
                                : undefined
                            }
                            onClick={() =>
                              void handleTestProvider(
                                selectedProviderDraft.provider.id,
                              )
                            }
                          >
                            测试连通
                          </button>
                          {providerDrafts.length > 1 && (
                            <button
                              type="button"
                              className="btn danger small"
                              onClick={() =>
                                handleDeleteProvider(selectedProviderDraft.index)
                              }
                            >
                              删除
                            </button>
                          )}
                        </div>
                      </div>
                    ) : (
                      <div className="settings-item-empty muted">
                        选择或添加一个 Provider 档案以开始编辑。
                      </div>
                    )}
                  </div>
                </div>
              </article>
              </div>
                )}

                {settingsSection === "templates" && (
              <div
                id="settings-panel-templates"
                className="settings-section-panel"
                role="tabpanel"
                aria-labelledby="settings-nav-templates"
              >
              <article className="card settings-wide">
                <div className="log-header">
                  <div>
                    <h2 className="visually-hidden">总结模板</h2>
                    <p className="muted small settings-collection-hint">
                      共 {templateDrafts.length} 个模板
                    </p>
                  </div>
                  <button
                    type="button"
                    className="btn secondary small"
                    onClick={handleAddTemplate}
                  >
                    添加模板
                  </button>
                </div>
                <div className="settings-split">
                  <div
                    className="settings-item-list"
                    role="listbox"
                    aria-label="总结模板列表"
                  >
                    {templateDrafts.length === 0 ? (
                      <div className="settings-item-empty muted">
                        还没有模板，点击「添加模板」开始配置。
                      </div>
                    ) : (
                      templateDrafts.map((template, index) => {
                        const isSelected =
                          selectedTemplateDraft?.index === index;
                        const isDefault =
                          template.id === settingsDefaultTemplateId;
                        const promptPreview =
                          template.system_prompt.trim() ||
                          template.user_template.trim() ||
                          "空提示词";
                        return (
                          <button
                            key={`${template.id}-${index}`}
                            type="button"
                            role="option"
                            aria-selected={isSelected}
                            className={
                              isSelected
                                ? "settings-item selected"
                                : "settings-item"
                            }
                            onClick={() => setSelectedTemplateIndex(index)}
                          >
                            <div className="settings-item-top">
                              <strong>
                                {template.name.trim() || "未命名模板"}
                              </strong>
                              {isDefault && (
                                <span className="settings-item-badge">默认</span>
                              )}
                            </div>
                            <div
                              className="settings-item-sub muted small mono"
                              title={template.id}
                            >
                              {template.id.trim() || "未设 ID"}
                            </div>
                            <div
                              className="settings-item-preview muted small"
                              title={promptPreview}
                            >
                              {promptPreview}
                            </div>
                          </button>
                        );
                      })
                    )}
                  </div>
                  <div className="settings-item-detail">
                    {selectedTemplateDraft ? (
                      <div className="profile-editor settings-detail-editor">
                        <div className="settings-detail-title">
                          <strong>
                            {selectedTemplateDraft.template.name.trim() ||
                              "未命名模板"}
                          </strong>
                          <span className="muted small mono">
                            {selectedTemplateDraft.template.id.trim() ||
                              "未设 ID"}
                          </span>
                        </div>
                        <div className="two-col">
                          <label>
                            <span>ID</span>
                            <input
                              value={selectedTemplateDraft.template.id}
                              onChange={(event) =>
                                handleTemplateIdChange(
                                  selectedTemplateDraft.index,
                                  selectedTemplateDraft.template.id,
                                  event.target.value,
                                )
                              }
                            />
                          </label>
                          <label>
                            <span>名称</span>
                            <input
                              value={selectedTemplateDraft.template.name}
                              onChange={(event) =>
                                setTemplateDrafts((items) =>
                                  items.map((item, itemIndex) =>
                                    itemIndex === selectedTemplateDraft.index
                                      ? {
                                          ...item,
                                          name: event.target.value,
                                        }
                                      : item,
                                  ),
                                )
                              }
                            />
                          </label>
                        </div>
                        <label>
                          <span>System Prompt</span>
                          <textarea
                            rows={5}
                            value={selectedTemplateDraft.template.system_prompt}
                            onChange={(event) =>
                              setTemplateDrafts((items) =>
                                items.map((item, itemIndex) =>
                                  itemIndex === selectedTemplateDraft.index
                                    ? {
                                        ...item,
                                        system_prompt: event.target.value,
                                      }
                                    : item,
                                ),
                              )
                            }
                          />
                        </label>
                        <label>
                          <span>用户模板</span>
                          <textarea
                            rows={8}
                            value={selectedTemplateDraft.template.user_template}
                            onChange={(event) =>
                              setTemplateDrafts((items) =>
                                items.map((item, itemIndex) =>
                                  itemIndex === selectedTemplateDraft.index
                                    ? {
                                        ...item,
                                        user_template: event.target.value,
                                      }
                                    : item,
                                ),
                              )
                            }
                          />
                        </label>
                        <div className="muted small">
                          变量：
                          {`{{title}} {{source_url}} {{duration}} {{transcript}}`}
                        </div>
                        {templateDrafts.length > 1 && (
                          <div className="detail-actions">
                            <button
                              type="button"
                              className="btn danger small"
                              onClick={() =>
                                handleDeleteTemplate(
                                  selectedTemplateDraft.index,
                                )
                              }
                            >
                              删除模板
                            </button>
                          </div>
                        )}
                      </div>
                    ) : (
                      <div className="settings-item-empty muted">
                        选择或添加一个总结模板以开始编辑。
                      </div>
                    )}
                  </div>
                </div>
              </article>
              </div>
                )}
              </div>
            </div>
          </section>
        )}
      </main>

      {createMode && (
        <div className="modal-backdrop" role="presentation">
          <div
            className="modal"
            role="dialog"
            aria-modal="true"
            aria-labelledby="create-dialog-title"
          >
            <div className="modal-header">
              <div>
                <div className="detail-kicker">新建任务</div>
                <h2 id="create-dialog-title">
                  {createMode === "download" && "下载链接"}
                  {createMode === "live" && "录制直播"}
                  {createMode === "import" && "导入本地视频"}
                </h2>
              </div>
              <button
                className="btn secondary small"
                type="button"
                onClick={closeCreate}
              >
                关闭
              </button>
            </div>

            <div className="form-grid">
              {(createMode === "download" || createMode === "live") && (
                <label>
                  <span>
                    {createMode === "download"
                      ? "URL / 抖音分享文案"
                      : "URL / 流地址"}
                  </span>
                  {createMode === "download" ? (
                    <textarea
                      ref={downloadUrlInputRef}
                      value={formUrl}
                      onChange={(event) => setFormUrl(event.target.value)}
                      placeholder="粘贴视频链接，或抖音分享文案（含 v.douyin.com 短链）。最佳努力下载。"
                      rows={4}
                    />
                  ) : (
                    <input
                      ref={liveUrlInputRef}
                      value={formUrl}
                      onChange={(event) => setFormUrl(event.target.value)}
                      placeholder="https://... 或 m3u8/flv（最佳努力）"
                    />
                  )}
                </label>
              )}

              {createMode === "import" && (
                <div className="file-picker-field">
                  <span>本地视频文件</span>
                  <div className="file-picker-row">
                    <button
                      ref={localFilePickerButtonRef}
                      className="btn secondary"
                      type="button"
                      disabled={isBusy || isSelectingLocalFile}
                      onClick={() => void handleSelectLocalFile()}
                    >
                      {isSelectingLocalFile
                        ? "正在选择…"
                        : formLocalPath
                          ? "重新选择"
                          : "选择文件"}
                    </button>
                    <div
                      className={`file-picker-value${formLocalPath ? "" : " muted"}`}
                      title={formLocalPath || "尚未选择文件"}
                    >
                      {formLocalPath || "尚未选择文件"}
                    </div>
                  </div>
                </div>
              )}

              <label>
                <span>标题（可选）</span>
                <input
                  value={formTitle}
                  onChange={(event) => setFormTitle(event.target.value)}
                  placeholder="便于列表识别"
                />
              </label>

              {createMode === "live" && (
                <label>
                  <span>分段时长（分钟）</span>
                  <input
                    type="number"
                    min={1}
                    value={formSegmentMinutes}
                    onChange={(event) =>
                      setFormSegmentMinutes(Number(event.target.value) || 30)
                    }
                  />
                </label>
              )}

              <div className="checkbox-row">
                <label className="checkbox">
                  <input
                    type="checkbox"
                    checked={autoTranscribe}
                    disabled={autoSummarize}
                    onChange={(event) => setAutoTranscribe(event.target.checked)}
                  />
                  完成后自动转写
                </label>
                <label className="checkbox">
                  <input
                    type="checkbox"
                    checked={autoSummarize}
                    onChange={(event) => {
                      const checked = event.target.checked;
                      setAutoSummarize(checked);
                      if (checked) {
                        setAutoTranscribe(true);
                      }
                    }}
                  />
                  转写后自动总结
                </label>
                <label className="checkbox">
                  <input
                    type="checkbox"
                    checked={autoStart}
                    onChange={(event) => setAutoStart(event.target.checked)}
                  />
                  创建后立即运行
                </label>
              </div>

              {autoTranscribe && (
                <label>
                  <span>转写语言</span>
                  <select
                    value={formTranscribeLanguage}
                    onChange={(event) => setFormTranscribeLanguage(event.target.value)}
                  >
                    {!TRANSCRIBE_LANGUAGE_OPTIONS.some(
                      (option) => option.value === formTranscribeLanguage,
                    ) && (
                      <option value={formTranscribeLanguage}>
                        {formTranscribeLanguage || "未知"}（当前配置）
                      </option>
                    )}
                    {TRANSCRIBE_LANGUAGE_OPTIONS.map((option) => (
                      <option key={option.value} value={option.value}>
                        {option.label}
                      </option>
                    ))}
                  </select>
                </label>
              )}

              {autoSummarize && (
                <>
                  <div className="two-col">
                    <label>
                      <span>Provider</span>
                      <select
                        value={formProviderId}
                        onChange={(event) =>
                          handleCreateProviderChange(event.target.value)
                        }
                      >
                        <option value="">使用全局默认</option>
                        {config?.providers.map((provider) => (
                          <option key={provider.id} value={provider.id}>
                            {provider.name}
                            {provider.id === config.default_provider_profile_id
                              ? "（全局默认）"
                              : ""}
                          </option>
                        ))}
                      </select>
                    </label>
                    <label>
                      <span>模型</span>
                      <select
                        value={formModel}
                        onChange={(event) => setFormModel(event.target.value)}
                      >
                        <option value="">使用 Provider 默认</option>
                        {createProviderModelOptions.map((modelName) => (
                          <option key={modelName} value={modelName}>
                            {modelName}
                            {modelName === selectedCreateProvider?.default_model
                              ? "（档案默认）"
                              : ""}
                          </option>
                        ))}
                      </select>
                    </label>
                  </div>
                  <label>
                    <span>总结模板</span>
                    <select
                      value={formTemplateId}
                      onChange={(event) => setFormTemplateId(event.target.value)}
                    >
                      <option value="">使用全局默认</option>
                      {config?.templates.map((template) => (
                        <option key={template.id} value={template.id}>
                          {template.name}
                          {template.id === config.default_template_id
                            ? "（全局默认）"
                            : ""}
                        </option>
                      ))}
                    </select>
                  </label>
                  <p className="muted small">
                    选「使用全局默认」时，任务不写死 Provider；之后改设置里的默认档案再总结会跟新默认。指定档案/模型后会固化到本任务。
                  </p>
                </>
              )}

              {createMode === "live" && (
                <p className="muted small">
                  录制中关闭窗口会隐藏到托盘；点击托盘可恢复。停止后会保留分段并尝试合并。
                </p>
              )}
            </div>

            <div className="modal-actions">
              <button
                className="btn secondary"
                type="button"
                onClick={closeCreate}
              >
                取消
              </button>
              <button
                className="btn"
                type="button"
                disabled={
                  isBusy ||
                  isSelectingLocalFile ||
                  (createMode === "import" && !formLocalPath)
                }
                onClick={() => void submitCreate()}
              >
                创建任务
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

export default App;

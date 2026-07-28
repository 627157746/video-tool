import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { openPath } from "@tauri-apps/plugin-opener";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import {
  checkAppUpdate,
  installAppUpdate,
  checkYtDlpUpdate,
  createDownloadJobsBatch,
  createImportJob,
  createLiveRecordJob,
  deleteJob,
  exportAppConfig,
  exportJob,
  getAppInfo,
  getConfig,
  getDependencyReport,
  getJob,
  getJobLog,
  getJobChapters,
  getJobSummaries,
  getJobSummary,
  getJobTranscript,
  getSystemDiagnostics,
  getTranscriptSegmentTexts,
  importAppConfig,
  inspectWorkspaceHealth,
  listJobs,
  listTranscribeModels,
  openJobDirectory,
  openTranscribeModelDirectory,
  probeSidecars,
  rebuildSearchIndex,
  repairWorkspaceHealth,
  runJob,
  retryTranscriptSegment,
  saveConfig,
  searchWorkspace,
  selectJobSegments,
  stopRecording,
  testProvider,
  updateJobGroup,
  updateJobMediaSaveMode,
  updateJobPipeline,
  updateJobTitle,
} from "./api";
import type {
  AppConfigPublic,
  AppInfo,
  ConfigExportPackage,
  DependencyReport,
  GlossaryConfig,
  Job,
  JobGroupDefinition,
  JobListItem,
  JobStatus,
  JobStep,
  MediaSaveMode,
  ModelInventory,
  ProviderProfileInput,
  SearchHit,
  SidecarStatus,
  SummaryTemplate,
  SummaryTemplateArtifact,
  SystemDiagnostics,
  TranscribeModelPresets,
  AppUpdateProgress,
  UpdateCheckResult,
  WorkspaceHealthReport,
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
import {
  COOKIES_BROWSER_OPTIONS,
  JOB_DETAIL_SECTIONS,
  LOG_NAMES,
  SETTINGS_SECTIONS,
  TRANSCRIBE_LANGUAGE_OPTIONS,
  type CreateMode,
  type JobDetailSection,
  type LogName,
  type MainView,
  type SettingsPathPickerId,
  type SettingsSection,
} from "./constants";
import {
  buildRecoverySuggestion,
  type RecoveryAction,
  type RecoverySuggestion,
} from "./recoveryUtils";
import { confirmAction } from "./confirmAction";
import {
  formatProgress,
  formatQueueStatusLabel,
  formatTime,
  getStepActionLabel,
  KIND_LABEL,
  PIPELINE_STEPS,
  STEP_LABEL,
  STEP_STATUS_LABEL,
} from "./labels";
import {
  createClientGroupId,
  getPipelineStepProgress,
  jobToListItem,
  mergeJobListSnapshots,
  normalizeJobGroup,
  normalizeProviderModels,
  parseProviderModelsListText,
  providerModelsListText,
  resolveExistingDefaultId,
  resolveJobGroupFilterKey,
  resolveJobGroupLabel,
  resolveJobGroupSelectValue,
  resolveProviderModelOptions,
  unwrapOuterMarkdownFence,
} from "./jobUtils";
import { PathPickerField } from "./components/PathPickerField";
import { CapacityPanel } from "./components/CapacityPanel";
import { ConfirmDialogHost } from "./components/ConfirmDialogHost";
import { MediaPreviewPanel } from "./components/MediaPreviewPanel";
import { TranscriptProofreadPanel } from "./components/TranscriptProofreadPanel";
import "./App.css";

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
  const [summaryArtifacts, setSummaryArtifacts] = useState<
    SummaryTemplateArtifact[]
  >([]);
  const [activeSummaryTemplateId, setActiveSummaryTemplateId] = useState("");
  const [searchQuery, setSearchQuery] = useState("");
  /** Full-text search over transcripts/summaries (workspace FTS). */
  const [fullTextQuery, setFullTextQuery] = useState("");
  const [fullTextHits, setFullTextHits] = useState<SearchHit[]>([]);
  const [isFullTextSearching, setIsFullTextSearching] = useState(false);
  /** True after the user has run at least one full-text search (for empty-state UI). */
  const [fullTextHasSearched, setFullTextHasSearched] = useState(false);
  /** `"all"` | JobStatus for filtering the job list via hero stat cards. */
  const [statusFilter, setStatusFilter] = useState<"all" | JobStatus>("all");
  /** `"all"` | `"ungrouped"` | exact group name for filtering the job list. */
  const [groupFilter, setGroupFilter] = useState<string>("all");
  /** `"all"` | batch UUID for filtering jobs from one multi-URL create. */
  const [batchFilter, setBatchFilter] = useState<string>("all");
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
  const [formGroup, setFormGroup] = useState("");
  const [formLocalPath, setFormLocalPath] = useState("");
  const [formSegmentMinutes, setFormSegmentMinutes] = useState(30);
  const [autoTranscribe, setAutoTranscribe] = useState(true);
  const [autoSummarize, setAutoSummarize] = useState(false);
  const [autoChapterize, setAutoChapterize] = useState(false);
  const [autoStart, setAutoStart] = useState(true);
  /** Empty string means follow global default at summarize run time. */
  const [formProviderId, setFormProviderId] = useState("");
  /** Empty string means use the selected/default Provider's default_model. */
  const [formModel, setFormModel] = useState("");
  /** Empty string means follow global default template at summarize run time. */
  const [formTemplateId, setFormTemplateId] = useState("");
  /** Ordered multi-template ids for create form (v0.2 P3). */
  const [formTemplateIds, setFormTemplateIds] = useState<string[]>([]);
  const [formTranscribeLanguage, setFormTranscribeLanguage] = useState("auto");
  const [jobProviderId, setJobProviderId] = useState("");
  const [jobModel, setJobModel] = useState("");
  const [jobTemplateId, setJobTemplateId] = useState("");
  const [jobTemplateIds, setJobTemplateIds] = useState<string[]>([]);
  const [isSavingJobPipeline, setIsSavingJobPipeline] = useState(false);
  const [jobTitleDraft, setJobTitleDraft] = useState("");
  const [isSavingJobTitle, setIsSavingJobTitle] = useState(false);
  const jobTitleDraftRef = useRef("");
  /** Selected group id (or legacy free-text value); empty string means ungrouped. */
  const [jobGroupDraft, setJobGroupDraft] = useState("");
  const [isSavingJobGroup, setIsSavingJobGroup] = useState(false);
  const [isSavingJobMediaSaveMode, setIsSavingJobMediaSaveMode] =
    useState(false);

  const [settingsWorkspace, setSettingsWorkspace] = useState("");
  const [settingsSegmentMinutes, setSettingsSegmentMinutes] = useState(30);
  const [settingsAutoTranscribe, setSettingsAutoTranscribe] = useState(true);
  const [settingsAutoSummarize, setSettingsAutoSummarize] = useState(false);
  const [settingsProxy, setSettingsProxy] = useState("");
  const [settingsMinDisk, setSettingsMinDisk] = useState(5);
  const [settingsReconnect, setSettingsReconnect] = useState(3);
  const [settingsMaxContextChars, setSettingsMaxContextChars] = useState(400000);
  const [settingsMaxConcurrentJobs, setSettingsMaxConcurrentJobs] = useState(2);
  const [settingsMaxLiveRecords, setSettingsMaxLiveRecords] = useState(1);
  const [settingsCookiesFile, setSettingsCookiesFile] = useState("");
  const [settingsCookiesBrowser, setSettingsCookiesBrowser] = useState("");
  const [formCookiesMode, setFormCookiesMode] = useState("inherit");
  const [formCookiesFile, setFormCookiesFile] = useState("");
  const [formCookiesBrowser, setFormCookiesBrowser] = useState("chrome");
  /** Exclusive save mode for download / live create forms. */
  const [formMediaSaveMode, setFormMediaSaveMode] =
    useState<MediaSaveMode>("video");
  const [workspaceHealth, setWorkspaceHealth] =
    useState<WorkspaceHealthReport | null>(null);
  const [isInspectingHealth, setIsInspectingHealth] = useState(false);
  const [isRepairingHealth, setIsRepairingHealth] = useState(false);
  const [dependencyReport, setDependencyReport] =
    useState<DependencyReport | null>(null);
  const [modelInventory, setModelInventory] = useState<ModelInventory | null>(
    null,
  );
  const [systemDiagnostics, setSystemDiagnostics] =
    useState<SystemDiagnostics | null>(null);
  const [updateCheckResult, setUpdateCheckResult] =
    useState<UpdateCheckResult | null>(null);
  const [updateProgress, setUpdateProgress] =
    useState<AppUpdateProgress | null>(null);
  const [isInstallingUpdate, setIsInstallingUpdate] = useState(false);
  const [isLoadingP4Tools, setIsLoadingP4Tools] = useState(false);
  const [settingsTranscribeModel, setSettingsTranscribeModel] = useState("");
  const [settingsTranscribeLanguage, setSettingsTranscribeLanguage] = useState("auto");
  const [settingsTranscribeModelPreset, setSettingsTranscribeModelPreset] =
    useState("custom");
  const [settingsModelPresetSpeed, setSettingsModelPresetSpeed] = useState("");
  const [settingsModelPresetBalanced, setSettingsModelPresetBalanced] =
    useState("");
  const [settingsModelPresetQuality, setSettingsModelPresetQuality] =
    useState("");
  const [settingsGlossaryHotwords, setSettingsGlossaryHotwords] = useState("");
  const [settingsGlossaryReplacements, setSettingsGlossaryReplacements] =
    useState("");
  const [settingsGlossaryWhisperPrompt, setSettingsGlossaryWhisperPrompt] =
    useState(true);
  const [settingsGlossaryPostReplace, setSettingsGlossaryPostReplace] =
    useState(true);
  const [settingsAutoChapterize, setSettingsAutoChapterize] = useState(true);
  const [settingsNotifyOnJobFinish, setSettingsNotifyOnJobFinish] =
    useState(true);
  const [chaptersText, setChaptersText] = useState("");
  const [segmentDiffText, setSegmentDiffText] = useState<string | null>(null);
  const [settingsDefaultProviderId, setSettingsDefaultProviderId] = useState("");
  const [settingsDefaultTemplateId, setSettingsDefaultTemplateId] = useState("");
  const [settingsYtDlp, setSettingsYtDlp] = useState("");
  const [settingsFfmpeg, setSettingsFfmpeg] = useState("");
  const [settingsFfprobe, setSettingsFfprobe] = useState("");
  const [settingsStreamlink, setSettingsStreamlink] = useState("");
  const [settingsTranscribe, setSettingsTranscribe] = useState("");
  const [providerDrafts, setProviderDrafts] = useState<ProviderProfileInput[]>([]);
  const [templateDrafts, setTemplateDrafts] = useState<SummaryTemplate[]>([]);
  const [groupDrafts, setGroupDrafts] = useState<JobGroupDefinition[]>([]);
  const [selectedProviderIndex, setSelectedProviderIndex] = useState(0);
  const [selectedTemplateIndex, setSelectedTemplateIndex] = useState(0);
  const [selectedGroupIndex, setSelectedGroupIndex] = useState(0);
  const [settingsSection, setSettingsSection] =
    useState<SettingsSection>("pipeline");
  const [jobDetailSection, setJobDetailSection] =
    useState<JobDetailSection>("overview");
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
    setSettingsAutoChapterize(nextConfig.default_auto_chapterize ?? true);
    setSettingsNotifyOnJobFinish(nextConfig.notify_on_job_finish ?? true);
    setSettingsProxy(nextConfig.proxy_url ?? "");
    setSettingsMinDisk(nextConfig.min_free_disk_gb);
    setSettingsReconnect(nextConfig.live_reconnect_attempts);
    setSettingsMaxContextChars(nextConfig.max_context_chars);
    setSettingsMaxConcurrentJobs(nextConfig.max_concurrent_jobs ?? 2);
    setSettingsMaxLiveRecords(nextConfig.max_live_records ?? 1);
    setSettingsCookiesFile(nextConfig.download_cookies_file ?? "");
    setSettingsCookiesBrowser(nextConfig.download_cookies_from_browser ?? "");
    setSettingsTranscribeModel(nextConfig.transcribe_model ?? "");
    setSettingsTranscribeLanguage(nextConfig.transcribe_language);
    setSettingsTranscribeModelPreset(
      nextConfig.transcribe_model_preset ?? "custom",
    );
    setSettingsModelPresetSpeed(
      nextConfig.transcribe_model_presets?.speed ?? "",
    );
    setSettingsModelPresetBalanced(
      nextConfig.transcribe_model_presets?.balanced ?? "",
    );
    setSettingsModelPresetQuality(
      nextConfig.transcribe_model_presets?.quality ?? "",
    );
    const glossary = nextConfig.glossary;
    setSettingsGlossaryHotwords((glossary?.hotwords ?? []).join("\n"));
    setSettingsGlossaryReplacements(
      (glossary?.replacements ?? [])
        .map((pair) => `${pair.from} => ${pair.to}`)
        .join("\n"),
    );
    setSettingsGlossaryWhisperPrompt(
      glossary?.apply_as_whisper_prompt ?? true,
    );
    setSettingsGlossaryPostReplace(glossary?.apply_post_replace ?? true);
    setSettingsDefaultProviderId(nextConfig.default_provider_profile_id ?? "");
    setSettingsDefaultTemplateId(nextConfig.default_template_id ?? "");
    setSettingsYtDlp(nextConfig.sidecar_paths.yt_dlp ?? "");
    setSettingsFfmpeg(nextConfig.sidecar_paths.ffmpeg ?? "");
    setSettingsFfprobe(nextConfig.sidecar_paths.ffprobe ?? "");
    setSettingsStreamlink(nextConfig.sidecar_paths.streamlink ?? "");
    setSettingsTranscribe(nextConfig.sidecar_paths.transcribe ?? "");
    const nextProviderDrafts: ProviderProfileInput[] = nextConfig.providers.map(
      (provider) => {
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
      },
    );
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
    const nextGroupDrafts = nextConfig.job_groups ?? [];
    setGroupDrafts(nextGroupDrafts);
    setSelectedGroupIndex((currentIndex) => {
      if (nextGroupDrafts.length === 0) {
        return 0;
      }
      return Math.min(currentIndex, nextGroupDrafts.length - 1);
    });
    setFormSegmentMinutes(nextConfig.default_segment_minutes);
    setAutoTranscribe(nextConfig.default_auto_transcribe);
    setAutoSummarize(nextConfig.default_auto_summarize);
    // Create form starts on "follow global defaults" rather than pinning IDs.
    setFormProviderId("");
    setFormModel("");
    setFormTemplateId("");
    setFormTemplateIds([]);
    setFormTranscribeLanguage(nextConfig.transcribe_language);
  }, []);

  const clearSelectedJobState = useCallback(() => {
    detailRequestVersionRef.current += 1;
    logRequestVersionRef.current += 1;
    selectedJobIdRef.current = null;
    selectedJobRef.current = null;
    setSelectedJobId(null);
    setSelectedJob(null);
    setJobDetailSection("overview");
    setLogText("");
    setTranscriptText("");
    setSummaryText("");
    setSummaryArtifacts([]);
    setActiveSummaryTemplateId("");
    setChaptersText("");
    setSegmentDiffText(null);
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
        setJobDetailSection("overview");
        setLogText("正在加载…");
        setTranscriptText("");
        setSummaryText("");
        setSummaryArtifacts([]);
        setActiveSummaryTemplateId("");
        setChaptersText("");
        setSegmentDiffText(null);
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
        const [
          logResult,
          transcriptResult,
          summaryResult,
          summariesResult,
          chaptersResult,
        ] = await Promise.allSettled([
          getJobLog(jobId, preferredLog),
          getJobTranscript(jobId),
          getJobSummary(jobId),
          getJobSummaries(jobId),
          getJobChapters(jobId),
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
        const nextArtifacts =
          summariesResult.status === "fulfilled" ? summariesResult.value : [];
        setSummaryArtifacts(nextArtifacts);
        setActiveSummaryTemplateId((previous) => {
          if (
            previous &&
            nextArtifacts.some((item) => item.template_id === previous)
          ) {
            return previous;
          }
          return nextArtifacts[0]?.template_id ?? "";
        });
        setChaptersText(
          chaptersResult.status === "fulfilled" ? chaptersResult.value : "",
        );

        const failedResult = [
          logResult,
          transcriptResult,
          summaryResult,
          chaptersResult,
        ].find((result) => result.status === "rejected");
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
          const message =
            error instanceof Error ? error.message : String(error);
          const isMissingJobError = message.includes("任务不存在");
          const liveSnapshot = selectedJobRef.current;
          const hasLiveRunningSnapshot =
            liveSnapshot?.id === jobId &&
            (liveSnapshot.status === "running" ||
              liveSnapshot.status === "queued");
          // Background poll/event refresh can race atomic source.json writes
          // during download progress. Keep the last good snapshot instead of
          // blanking the detail pane with a false "任务不存在" toast.
          if (!resetDisplay && isMissingJobError && hasLiveRunningSnapshot) {
            return;
          }
          if (resetDisplay || !hasLiveRunningSnapshot) {
            setSelectedJob(null);
            selectedJobRef.current = null;
            setLogText("");
            setTranscriptText("");
            setSummaryText("");
          }
          if (resetDisplay || !isMissingJobError) {
            setErrorMessage(message);
          }
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
    void listen<AppUpdateProgress>("app-update-progress", (event) => {
      if (cancelled) {
        return;
      }
      setUpdateProgress(event.payload);
    })
      .then((dispose) => {
        if (cancelled) {
          dispose();
          return;
        }
        disposeListener = dispose;
      })
      .catch(() => {
        // Event bridge unavailable outside Tauri shell.
      });
    return () => {
      cancelled = true;
      disposeListener?.();
    };
  }, []);

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
        const previousSelectedJob = selectedJobRef.current;
        selectedJobRef.current = job;
        setSelectedJob(job);
        // Progress-only ticks already carry the full Job; avoid reloading logs
        // (and bumping detail generations) on every percent update while running.
        const stepChanged =
          previousSelectedJob?.current_step !== job.current_step;
        const statusChanged = previousSelectedJob?.status !== job.status;
        if (job.status !== "running" || stepChanged || statusChanged) {
          detailRequestVersionRef.current += 1;
          void reloadLog(job.id, logNameRef.current);
        }
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

  const managedJobGroups = useMemo(
    () => config?.job_groups ?? groupDrafts,
    [config?.job_groups, groupDrafts],
  );

  const orphanGroupFilterOptions = useMemo(() => {
    const orphanOptions = new Map<string, string>();
    for (const job of jobs) {
      const filterKey = resolveJobGroupFilterKey(job.group, managedJobGroups);
      if (!filterKey || !filterKey.startsWith("legacy:")) {
        continue;
      }
      const label = resolveJobGroupLabel(job.group, managedJobGroups);
      if (label) {
        orphanOptions.set(filterKey, label);
      }
    }
    return Array.from(orphanOptions.entries())
      .map(([filterKey, label]) => ({ filterKey, label }))
      .sort((left, right) => left.label.localeCompare(right.label, "zh-CN"));
  }, [jobs, managedJobGroups]);

  const hasUngroupedJobs = useMemo(
    () => jobs.some((job) => !normalizeJobGroup(job.group)),
    [jobs],
  );

  const hasAnyGroupFilterChips =
    managedJobGroups.length > 0 ||
    orphanGroupFilterOptions.length > 0 ||
    hasUngroupedJobs;

  const recentBatchOptions = useMemo(() => {
    const seenBatchIds = new Set<string>();
    const options: string[] = [];
    for (const job of jobs) {
      const batchId = job.batch_id?.trim();
      if (!batchId || seenBatchIds.has(batchId)) {
        continue;
      }
      seenBatchIds.add(batchId);
      options.push(batchId);
      if (options.length >= 8) {
        break;
      }
    }
    return options;
  }, [jobs]);

  const filteredJobs = useMemo(() => {
    const titleQuery = searchQuery.trim().toLowerCase();
    return jobs.filter((job) => {
      const jobGroupFilterKey = resolveJobGroupFilterKey(
        job.group,
        managedJobGroups,
      );
      if (statusFilter !== "all" && job.status !== statusFilter) {
        return false;
      }
      if (groupFilter === "ungrouped") {
        if (jobGroupFilterKey) {
          return false;
        }
      } else if (groupFilter !== "all") {
        if (jobGroupFilterKey !== groupFilter) {
          return false;
        }
      }
      if (batchFilter !== "all") {
        if ((job.batch_id ?? "") !== batchFilter) {
          return false;
        }
      }
      if (!titleQuery) {
        return true;
      }
      return job.title.toLowerCase().includes(titleQuery);
    });
  }, [
    batchFilter,
    groupFilter,
    jobs,
    managedJobGroups,
    searchQuery,
    statusFilter,
  ]);

  const stats = useMemo(() => {
    return {
      total: jobs.length,
      running: jobs.filter((job) => job.status === "running").length,
      queued: jobs.filter((job) => job.status === "queued").length,
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
    const currentTemplateIds = selectedJob.pipeline.template_ids ?? [];
    const templateIdsChanged =
      jobTemplateIds.length !== currentTemplateIds.length ||
      jobTemplateIds.some((id, index) => id !== currentTemplateIds[index]);
    return (
      jobProviderId !== currentProviderId ||
      jobTemplateId !== currentTemplateId ||
      templateIdsChanged ||
      jobModel !== currentModel
    );
  }, [jobModel, jobProviderId, jobTemplateId, jobTemplateIds, selectedJob]);

  useEffect(() => {
    if (!selectedJob) {
      setJobProviderId("");
      setJobModel("");
      setJobTemplateId("");
      setJobTemplateIds([]);
      setJobTitleDraft("");
      jobTitleDraftRef.current = "";
      setJobGroupDraft("");
      return;
    }
    setJobProviderId(selectedJob.pipeline.provider_profile_id ?? "");
    setJobModel(selectedJob.pipeline.model ?? "");
    setJobTemplateId(selectedJob.pipeline.template_id ?? "");
    const ids = selectedJob.pipeline.template_ids ?? [];
    setJobTemplateIds(
      ids.length > 0
        ? ids
        : selectedJob.pipeline.template_id
          ? [selectedJob.pipeline.template_id]
          : [],
    );
  }, [
    selectedJob?.id,
    selectedJob?.pipeline.provider_profile_id,
    selectedJob?.pipeline.model,
    selectedJob?.pipeline.template_id,
    selectedJob?.pipeline.template_ids,
  ]);

  useEffect(() => {
    if (!selectedJob) {
      setJobTitleDraft("");
      jobTitleDraftRef.current = "";
      setJobGroupDraft("");
      return;
    }
    const savedTitle = selectedJob.source.title?.trim() ?? "";
    setJobTitleDraft(savedTitle);
    jobTitleDraftRef.current = savedTitle;
    setJobGroupDraft(
      resolveJobGroupSelectValue(selectedJob.group, managedJobGroups),
    );
  }, [managedJobGroups, selectedJob?.id]);

  useEffect(() => {
    if (!selectedJob || isSavingJobTitle) {
      return;
    }
    const savedTitle = selectedJob.source.title?.trim() ?? "";
    // Only apply backend title updates when the input is still clean.
    if (jobTitleDraft === jobTitleDraftRef.current) {
      setJobTitleDraft(savedTitle);
      jobTitleDraftRef.current = savedTitle;
    }
  }, [isSavingJobTitle, jobTitleDraft, selectedJob?.source.title, selectedJob]);

  useEffect(() => {
    if (!selectedJob || isSavingJobGroup) {
      return;
    }
    setJobGroupDraft(
      resolveJobGroupSelectValue(selectedJob.group, managedJobGroups),
    );
  }, [isSavingJobGroup, managedJobGroups, selectedJob?.group, selectedJob]);

  useEffect(() => {
    if (groupFilter === "all" || groupFilter === "ungrouped") {
      return;
    }
    const managedStillExists = managedJobGroups.some(
      (groupEntry) => `id:${groupEntry.id}` === groupFilter,
    );
    const orphanStillExists = orphanGroupFilterOptions.some(
      (option) => option.filterKey === groupFilter,
    );
    if (!managedStillExists && !orphanStillExists) {
      setGroupFilter("all");
    }
  }, [groupFilter, managedJobGroups, orphanGroupFilterOptions]);

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
    setFormGroup("");
    setFormCookiesMode("inherit");
    setFormCookiesFile("");
    setFormCookiesBrowser("chrome");
    setFormMediaSaveMode("video");
    setFormLocalPath("");
    setFormSegmentMinutes(config?.default_segment_minutes ?? 30);
    setAutoTranscribe(config?.default_auto_transcribe ?? true);
    setAutoSummarize(config?.default_auto_summarize ?? false);
    setAutoChapterize(
      (config?.default_auto_chapterize ?? true) &&
        (config?.default_auto_summarize ?? false),
    );
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
    const resolvedFormTemplateIds =
      formTemplateIds.length > 0
        ? formTemplateIds
        : formTemplateId.trim()
          ? [formTemplateId.trim()]
          : [];
    const pipeline = {
      auto_transcribe: autoTranscribe,
      auto_summarize: autoSummarize,
      auto_chapterize: autoChapterize || autoSummarize,
      provider_profile_id: formProviderId.trim() || null,
      template_id:
        resolvedFormTemplateIds[0] ??
        (formTemplateId.trim() || null),
      template_ids: resolvedFormTemplateIds,
      model: formModel.trim() || null,
      transcribe_language: formTranscribeLanguage,
    };

    // formGroup stores a managed group id, or empty for ungrouped.
    const createGroup = normalizeJobGroup(formGroup);

    try {
      let focusJobId: string | null = null;
      if (createMode === "download") {
        const batchResult = await createDownloadJobsBatch({
          urls_text: formUrl,
          title: formTitle || undefined,
          group: createGroup,
          pipeline,
          auto_start: autoStart,
          download_cookies_mode: formCookiesMode,
          download_cookies_file:
            formCookiesMode === "file"
              ? formCookiesFile.trim() || null
              : null,
          download_cookies_from_browser:
            formCookiesMode === "browser"
              ? formCookiesBrowser.trim() || null
              : null,
          media_save_mode: formMediaSaveMode,
        });
        if (batchResult.jobs.length === 0) {
          throw new Error("未能创建任何下载任务");
        }
        focusJobId = batchResult.jobs[0]?.id ?? null;
        if (batchResult.batch_id) {
          setBatchFilter(batchResult.batch_id);
        }
        closeCreate();
        try {
          const refreshedConfig = await getConfig();
          applyConfigToSettings(refreshedConfig);
        } catch {
          // Job create already succeeded; catalog refresh is best-effort.
        }
        const createdCount = batchResult.jobs.length;
        setStatusMessage(
          createdCount > 1
            ? autoStart
              ? `已创建 ${createdCount} 个下载任务并入队（同批 batch 可筛选；下载为最佳努力）`
              : `已创建 ${createdCount} 个下载任务（同批 batch 可筛选，可在详情中手动运行）`
            : autoStart
              ? "任务已创建并开始执行（下载为最佳努力，失败请看日志）"
              : "任务已创建，可在详情中手动运行",
        );
      } else if (createMode === "live") {
        const created = await createLiveRecordJob({
          url: formUrl,
          title: formTitle || undefined,
          group: createGroup,
          segment_minutes: formSegmentMinutes,
          pipeline,
          auto_start: autoStart,
          media_save_mode: formMediaSaveMode,
        });
        focusJobId = created.id;
        closeCreate();
        try {
          const refreshedConfig = await getConfig();
          applyConfigToSettings(refreshedConfig);
        } catch {
          // Job create already succeeded; catalog refresh is best-effort.
        }
        setStatusMessage(
          autoStart
            ? "任务已创建并开始执行（下载为最佳努力，失败请看日志）"
            : "任务已创建，可在详情中手动运行",
        );
      } else {
        const created = await createImportJob({
          local_path: formLocalPath,
          title: formTitle || undefined,
          group: createGroup,
          pipeline,
          auto_start: autoStart,
        });
        focusJobId = created.id;
        closeCreate();
        try {
          const refreshedConfig = await getConfig();
          applyConfigToSettings(refreshedConfig);
        } catch {
          // Job create already succeeded; catalog refresh is best-effort.
        }
        setStatusMessage(
          autoStart
            ? "任务已创建并开始执行（下载为最佳努力，失败请看日志）"
            : "任务已创建，可在详情中手动运行",
        );
      }

      if (focusJobId) {
        await loadJobDetail(focusJobId);
      }
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
      const resolvedJobTemplateIds =
        jobTemplateIds.length > 0
          ? jobTemplateIds
          : jobTemplateId.trim()
            ? [jobTemplateId.trim()]
            : [];
      const updatedJob = await updateJobPipeline({
        job_id: selectedJob.id,
        provider_profile_id: jobProviderId.trim() || null,
        template_id:
          resolvedJobTemplateIds[0] ??
          (jobTemplateId.trim() || null),
        template_ids: resolvedJobTemplateIds,
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

  async function handleSaveJobTitle() {
    if (!selectedJob || isSavingJobTitle) {
      return;
    }
    if (selectedJob.status === "running") {
      const currentTitle = selectedJob.source.title?.trim() ?? "";
      setJobTitleDraft(currentTitle);
      jobTitleDraftRef.current = currentTitle;
      return;
    }
    const nextTitle = jobTitleDraft.trim();
    const currentTitle = selectedJob.source.title?.trim() ?? "";
    if (nextTitle === currentTitle) {
      setJobTitleDraft(currentTitle);
      jobTitleDraftRef.current = currentTitle;
      return;
    }

    setIsSavingJobTitle(true);
    setErrorMessage(null);
    try {
      const updatedJob = await updateJobTitle({
        job_id: selectedJob.id,
        title: nextTitle || null,
      });
      const savedTitle = updatedJob.source.title?.trim() ?? "";
      selectedJobRef.current = updatedJob;
      setSelectedJob(updatedJob);
      setJobs((previousJobs) =>
        previousJobs.map((entry) =>
          entry.id === updatedJob.id ? jobToListItem(updatedJob) : entry,
        ),
      );
      setJobTitleDraft(savedTitle);
      jobTitleDraftRef.current = savedTitle;
      setStatusMessage(
        savedTitle ? `标题已更新：${savedTitle}` : "已清除自定义标题",
      );
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setIsSavingJobTitle(false);
    }
  }

  async function handleJobMediaSaveModeChange(nextMode: MediaSaveMode) {
    if (!selectedJob || isSavingJobMediaSaveMode) {
      return;
    }
    if (
      selectedJob.source.kind !== "download" &&
      selectedJob.source.kind !== "live_record"
    ) {
      return;
    }
    if (selectedJob.status === "running") {
      return;
    }
    const currentMode = selectedJob.source.media_save_mode ?? "video";
    if (currentMode === nextMode) {
      return;
    }

    const hasExistingMediaProduct =
      (selectedJob.media_files?.length ?? 0) > 0 ||
      (selectedJob.media_segments?.length ?? 0) > 0 ||
      selectedJob.step_statuses.some(
        (stepProgress) =>
          stepProgress.step === "ingest" &&
          (stepProgress.status === "succeeded" ||
            stepProgress.status === "failed" ||
            stepProgress.status === "skipped"),
      );
    if (hasExistingMediaProduct) {
      const modeLabel = nextMode === "audio" ? "保存音频" : "保存视频";
      if (
        !(await confirmAction(
          `将改为「${modeLabel}」。\n\n已有媒体产物会被清除，下游转写/总结需在重新下载或录制后重跑。\n\n是否继续？`,
        ))
      ) {
        return;
      }
    }

    setIsSavingJobMediaSaveMode(true);
    setErrorMessage(null);
    try {
      const updatedJob = await updateJobMediaSaveMode({
        job_id: selectedJob.id,
        media_save_mode: nextMode,
      });
      selectedJobRef.current = updatedJob;
      setSelectedJob(updatedJob);
      setJobs((previousJobs) =>
        previousJobs.map((entry) =>
          entry.id === updatedJob.id ? jobToListItem(updatedJob) : entry,
        ),
      );
      const savedMode = updatedJob.source.media_save_mode ?? "video";
      setStatusMessage(
        savedMode === "audio"
          ? "已改为保存音频；如需新产物请重新运行下载/录制"
          : "已改为保存视频；如需新产物请重新运行下载/录制",
      );
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setIsSavingJobMediaSaveMode(false);
    }
  }

  async function handleJobGroupSelectChange(nextSelectValue: string) {
    if (!selectedJob || isSavingJobGroup) {
      return;
    }
    if (selectedJob.status === "running") {
      setJobGroupDraft(
        resolveJobGroupSelectValue(selectedJob.group, managedJobGroups),
      );
      return;
    }

    const nextGroupId = normalizeJobGroup(nextSelectValue);
    const currentSelectValue = resolveJobGroupSelectValue(
      selectedJob.group,
      managedJobGroups,
    );
    if ((nextGroupId ?? "") === currentSelectValue) {
      setJobGroupDraft(currentSelectValue);
      return;
    }

    setJobGroupDraft(nextGroupId ?? "");
    setIsSavingJobGroup(true);
    setErrorMessage(null);
    try {
      const updatedJob = await updateJobGroup({
        job_id: selectedJob.id,
        group: nextGroupId,
      });
      // Selecting a legacy free-text option may normalize to a catalog id.
      let catalog = managedJobGroups;
      try {
        const refreshedConfig = await getConfig();
        applyConfigToSettings(refreshedConfig);
        catalog = refreshedConfig.job_groups ?? managedJobGroups;
      } catch {
        // Job update already succeeded; catalog refresh is best-effort.
      }
      const savedSelectValue = resolveJobGroupSelectValue(
        updatedJob.group,
        catalog,
      );
      const savedLabel =
        resolveJobGroupLabel(updatedJob.group, catalog) ?? "";
      selectedJobRef.current = updatedJob;
      setSelectedJob(updatedJob);
      setJobs((previousJobs) =>
        previousJobs.map((entry) =>
          entry.id === updatedJob.id ? jobToListItem(updatedJob) : entry,
        ),
      );
      setJobGroupDraft(savedSelectValue);
      setStatusMessage(
        savedLabel ? `分组已更新：${savedLabel}` : "已清除任务分组",
      );
    } catch (error) {
      setJobGroupDraft(
        resolveJobGroupSelectValue(selectedJob.group, managedJobGroups),
      );
      setErrorMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setIsSavingJobGroup(false);
    }
  }

  async function handleDeleteJob(job: JobListItem) {
    if (
      !(await confirmAction(
        `确定永久删除任务“${job.title}”吗？\n\n任务目录中的媒体、字幕、总结和日志都会被删除，且无法恢复。`,
      ))
    ) {
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
    if (
      !(await confirmAction(
        "确定停止当前直播录制吗？\n\n将结束抓流并合并已有分段；已录内容会保留，但无法继续追加同一场录制。",
      ))
    ) {
      return;
    }
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
    if (
      !(await confirmAction(
        "确定导出该任务包吗？\n\n将打包任务目录中的媒体、转写、总结与日志（体积可能较大）。",
      ))
    ) {
      return;
    }
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
    if (
      job.transcript_edited_at &&
      !(await confirmAction(
        "该任务的转写文本已手工校对；修改选段会重建合并文字并覆盖校对结果。确定继续吗？",
      ))
    ) {
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
    const currentJob =
      selectedJobRef.current?.id === jobId ? selectedJobRef.current : null;
    if (
      currentJob?.transcript_edited_at &&
      !(await confirmAction(
        "该任务的转写文本已手工校对；重试分段会重建合并文字并覆盖校对结果。确定继续吗？",
      ))
    ) {
      return;
    }
    if (
      !(await confirmAction(
        `确定重试转写分段「${segmentId}」吗？\n\n该分段会重新转写；成功后需重跑合并字幕才能更新全文。`,
      ))
    ) {
      return;
    }
    setIsBusy(true);
    try {
      setTranscriptText("");
      setSummaryText("");
      setChaptersText("");
      setSegmentDiffText(null);
      await retryTranscriptSegment(jobId, segmentId);
      setStatusMessage(`已开始重试转写分段：${segmentId}`);
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setIsBusy(false);
    }
  }

  async function handleCompareSegment(jobId: string, segmentId: string) {
    setErrorMessage(null);
    try {
      const result = await getTranscriptSegmentTexts(jobId, segmentId);
      if (!result.previous?.trim()) {
        setSegmentDiffText(
          `分段 ${segmentId} 尚无上一版文本（仅在重试转写后会生成 .prev.txt）。\n\n—— 当前 ——\n${result.current || "（空）"}`,
        );
      } else {
        setSegmentDiffText(
          `分段 ${segmentId} 文本对比\n\n—— 上一版 ——\n${result.previous}\n\n—— 当前 ——\n${result.current || "（空）"}`,
        );
      }
      setJobDetailSection("segments");
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : String(error));
    }
  }

  async function handleRun(jobId: string, step?: JobStep | null) {
    const currentJob =
      selectedJobRef.current?.id === jobId ? selectedJobRef.current : null;
    const stepRegeneratesMergedTranscript =
      step == null ||
      step === "ingest" ||
      step === "transcribe" ||
      step === "merge_transcript";
    if (
      currentJob?.transcript_edited_at &&
      stepRegeneratesMergedTranscript &&
      !(await confirmAction(
        "该任务的转写文本已手工校对；重跑该步骤会重新生成合并文字并覆盖校对结果。确定继续吗？",
      ))
    ) {
      return;
    }
    const isFullOrIngest = step == null || step === "ingest";
    const hasExistingMediaProduct =
      currentJob != null &&
      ((currentJob.media_files?.length ?? 0) > 0 ||
        (currentJob.media_segments?.length ?? 0) > 0 ||
        currentJob.step_statuses.some(
          (stepProgress) =>
            stepProgress.step === "ingest" &&
            (stepProgress.status === "succeeded" ||
              stepProgress.status === "failed" ||
              stepProgress.status === "skipped"),
        ));
    if (
      isFullOrIngest &&
      hasExistingMediaProduct &&
      !(await confirmAction(
        step == null
          ? "确定重新运行整条流水线吗？\n\n将重新获取媒体，并可能覆盖已有转写/总结产物。"
          : "确定重新执行「获取媒体」吗？\n\n已有媒体可能被覆盖，下游转写/总结通常需要重跑。",
      ))
    ) {
      return;
    }
    if (
      step === "transcribe" &&
      !(await confirmAction(
        "确定重新执行转写吗？\n\n现有转写分段可能被覆盖，合并字幕与总结通常需要重跑。",
      ))
    ) {
      return;
    }
    if (
      (step === "summarize" || step === "chapterize") &&
      !(await confirmAction(
        step === "summarize"
          ? "确定重新生成 AI 总结吗？\n\n现有总结文档将被覆盖。"
          : "确定重新生成章节吗？\n\n现有章节产物将被覆盖。",
      ))
    ) {
      return;
    }
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

  const recoverySuggestion = useMemo(
    () => (selectedJob ? buildRecoverySuggestion(selectedJob) : null),
    [selectedJob],
  );

  function openSettingsSection(section: SettingsSection) {
    setView("settings");
    setSettingsSection(section);
  }

  function openJobDetailSection(section: JobDetailSection) {
    setJobDetailSection(section);
  }

  async function handleRecoveryAction(
    action: RecoveryAction,
    suggestion: RecoverySuggestion,
  ) {
    if (!selectedJob) {
      return;
    }
    switch (action.id) {
      case "retry_step":
        await handleRun(selectedJob.id, suggestion.retryStep);
        break;
      case "retry_pipeline":
        await handleRun(selectedJob.id, null);
        break;
      case "open_logs":
        openJobDetailSection("logs");
        break;
      case "open_directory":
        await handleOpenDirectory(selectedJob.id);
        break;
      case "open_settings_sidecars":
        openSettingsSection("sidecars");
        break;
      case "open_settings_pipeline":
        openSettingsSection(suggestion.settingsSection ?? "pipeline");
        break;
      case "open_settings_providers":
        openSettingsSection("providers");
        break;
      case "open_segments":
        openJobDetailSection("segments");
        break;
      case "open_pipeline":
        openJobDetailSection(suggestion.detailSection ?? "summarize");
        break;
      default:
        break;
    }
  }

  async function handleSaveSettings() {
    const previousWorkspace = config?.workspace_dir?.trim() ?? "";
    const nextWorkspace = settingsWorkspace.trim();
    if (
      previousWorkspace &&
      nextWorkspace &&
      previousWorkspace.replace(/\\/g, "/") !==
        nextWorkspace.replace(/\\/g, "/") &&
      !(await confirmAction(
        `确定切换工作区吗？\n\n从：\n${previousWorkspace}\n\n到：\n${nextWorkspace}\n\n任务列表将切换到新工作区；正在运行的任务会阻止保存。`,
      ))
    ) {
      return;
    }
    refreshRequestVersionRef.current += 1;
    setIsBusy(true);
    setErrorMessage(null);
    try {
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
      const normalizedGroupDrafts = groupDrafts
        .map((groupEntry) => ({
          id: groupEntry.id.trim(),
          name: groupEntry.name.trim(),
        }))
        .filter(
          (groupEntry) => groupEntry.id.length > 0 && groupEntry.name.length > 0,
        );
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
        default_auto_chapterize: settingsAutoChapterize,
        notify_on_job_finish: settingsNotifyOnJobFinish,
        proxy_url: settingsProxy,
        min_free_disk_gb: settingsMinDisk,
        live_reconnect_attempts: settingsReconnect,
        max_context_chars: settingsMaxContextChars,
        max_concurrent_jobs: settingsMaxConcurrentJobs,
        max_live_records: settingsMaxLiveRecords,
        download_cookies_file: settingsCookiesFile.trim() || null,
        download_cookies_from_browser: settingsCookiesBrowser.trim() || null,
        transcribe_model: settingsTranscribeModel,
        transcribe_language: settingsTranscribeLanguage,
        transcribe_model_preset: settingsTranscribeModelPreset,
        transcribe_model_presets: {
          speed: settingsModelPresetSpeed.trim() || null,
          balanced: settingsModelPresetBalanced.trim() || null,
          quality: settingsModelPresetQuality.trim() || null,
        } satisfies TranscribeModelPresets,
        glossary: {
          hotwords: settingsGlossaryHotwords
            .split(/[\n,]/)
            .map((value) => value.trim())
            .filter(Boolean),
          replacements: settingsGlossaryReplacements
            .split("\n")
            .map((line) => line.trim())
            .filter(Boolean)
            .map((line) => {
              const separatorIndex = line.includes("=>")
                ? line.indexOf("=>")
                : line.indexOf("→");
              if (separatorIndex < 0) {
                return { from: line, to: line };
              }
              return {
                from: line.slice(0, separatorIndex).trim(),
                to: line.slice(separatorIndex + (line.includes("=>") ? 2 : 1)).trim(),
              };
            })
            .filter((pair) => pair.from.length > 0),
          apply_as_whisper_prompt: settingsGlossaryWhisperPrompt,
          apply_post_replace: settingsGlossaryPostReplace,
        } satisfies GlossaryConfig,
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
        job_groups: normalizedGroupDrafts,
      });
      applyConfigToSettings(next);
      const nextJobs = await listJobs();
      setJobs(nextJobs);
      if (previousWorkspace && previousWorkspace !== next.workspace_dir) {
        clearSelectedJobState();
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

  async function handleInspectWorkspaceHealth() {
    setIsInspectingHealth(true);
    setErrorMessage(null);
    try {
      const report = await inspectWorkspaceHealth();
      setWorkspaceHealth(report);
      setStatusMessage("工作区诊断完成");
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setIsInspectingHealth(false);
    }
  }

  async function handleRefreshDependencyReport() {
    setIsLoadingP4Tools(true);
    setErrorMessage(null);
    try {
      const report = await getDependencyReport();
      setDependencyReport(report);
      setStatusMessage(
        report.all_required_ready
          ? "依赖检查通过：必需工具均已就绪"
          : `缺少必需工具：${report.missing_required.join("、")}`,
      );
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setIsLoadingP4Tools(false);
    }
  }

  async function handleRefreshModelInventory() {
    setIsLoadingP4Tools(true);
    setErrorMessage(null);
    try {
      const inventory = await listTranscribeModels();
      setModelInventory(inventory);
      setStatusMessage(
        inventory.selected_exists
          ? `已扫描 ${inventory.models.length} 个模型文件`
          : "当前选用模型文件不存在或未配置",
      );
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setIsLoadingP4Tools(false);
    }
  }

  async function handleOpenModelDirectory() {
    setErrorMessage(null);
    try {
      const directory = await openTranscribeModelDirectory();
      await openPath(directory);
      setStatusMessage(`已打开模型目录：${directory}`);
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : String(error));
    }
  }

  async function handleExportAppConfig() {
    setIsLoadingP4Tools(true);
    setErrorMessage(null);
    try {
      const packagePayload = await exportAppConfig(false);
      const jsonText = JSON.stringify(packagePayload, null, 2);
      await navigator.clipboard.writeText(jsonText);
      setStatusMessage(
        `配置已复制到剪贴板（已剥离 API Key，${packagePayload.providers.length} 个 Provider / ${packagePayload.templates.length} 个模板）`,
      );
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setIsLoadingP4Tools(false);
    }
  }

  async function handleImportAppConfigFromClipboard() {
    if (
      !(await confirmAction(
        "确定从剪贴板导入配置吗？\n\n将覆盖当前 Provider / 模板等设置（默认不导入密钥）。此操作不可自动撤销。",
      ))
    ) {
      return;
    }
    setIsLoadingP4Tools(true);
    setErrorMessage(null);
    try {
      const rawText = await navigator.clipboard.readText();
      const packagePayload = JSON.parse(rawText) as ConfigExportPackage;
      if (
        !packagePayload ||
        typeof packagePayload !== "object" ||
        !Array.isArray(packagePayload.providers)
      ) {
        throw new Error("剪贴板内容不是有效的配置导出包");
      }
      const result = await importAppConfig(packagePayload, false);
      const nextConfig = await getConfig();
      applyConfigToSettings(nextConfig);
      const nextSidecars = await probeSidecars();
      setSidecars(nextSidecars);
      setStatusMessage(result.message);
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setIsLoadingP4Tools(false);
    }
  }

  async function handleCheckAppUpdate() {
    setIsLoadingP4Tools(true);
    setErrorMessage(null);
    setUpdateProgress(null);
    try {
      const result = await checkAppUpdate();
      setUpdateCheckResult(result);
      setStatusMessage(result.message);
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setIsLoadingP4Tools(false);
    }
  }

  async function handleInstallAppUpdate() {
    if (isInstallingUpdate) {
      return;
    }
    const latestVersion =
      updateCheckResult?.latest_version?.trim() || "新版本";
    const installerName =
      updateCheckResult?.installer_name?.trim() || "安装包";
    if (
      !(await confirmAction(
        `将下载 ${installerName}（目标版本 ${latestVersion}）并直接静默安装（无安装向导）。\n\n安装完成后应用会自动退出，并在安装结束后重新启动。\n\n是否继续？`,
      ))
    ) {
      return;
    }
    setIsInstallingUpdate(true);
    setErrorMessage(null);
    setUpdateProgress({
      phase: "downloading",
      downloaded_bytes: 0,
      total_bytes: updateCheckResult?.installer_size_bytes ?? null,
      percent: 0,
      message: "准备下载…",
    });
    try {
      const result = await installAppUpdate();
      setStatusMessage(result.message);
      setUpdateProgress({
        phase: "done",
        downloaded_bytes: updateCheckResult?.installer_size_bytes ?? 0,
        total_bytes: updateCheckResult?.installer_size_bytes ?? null,
        percent: 100,
        message: result.will_restart
          ? `${result.message} 即将退出…`
          : result.message,
      });
      // Process exits shortly after a successful will_restart install; keep the
      // busy flag set so the user cannot start another install attempt.
      if (!result.will_restart) {
        setIsInstallingUpdate(false);
      }
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : String(error));
      setIsInstallingUpdate(false);
    }
  }

  async function handleOpenReleasePage() {
    const url =
      updateCheckResult?.release_page_url?.trim() ||
      "https://github.com/627157746/video-tool/releases";
    try {
      await openPath(url);
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : String(error));
    }
  }

  async function handleLoadSystemDiagnostics() {
    setIsLoadingP4Tools(true);
    setErrorMessage(null);
    try {
      const diagnostics = await getSystemDiagnostics();
      setSystemDiagnostics(diagnostics);
      setDependencyReport(diagnostics.dependency);
      setModelInventory(diagnostics.models);
      setWorkspaceHealth(diagnostics.workspace_health);
      setSidecars(diagnostics.sidecars);
      setStatusMessage("系统诊断已刷新");
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setIsLoadingP4Tools(false);
    }
  }

  async function handleRepairWorkspaceHealth() {
    if (
      !(await confirmAction(
        "确定修复工作区健康问题吗？\n\n会把残留的 running/queued 状态恢复为失败或待执行，并尝试重建空的媒体索引。不会删除任务目录。",
      ))
    ) {
      return;
    }
    setIsRepairingHealth(true);
    setErrorMessage(null);
    try {
      const report = await repairWorkspaceHealth();
      setWorkspaceHealth(report);
      const nextJobs = await listJobs();
      setJobs(nextJobs);
      if (selectedJobIdRef.current) {
        await loadJobDetail(selectedJobIdRef.current, logNameRef.current, false);
      }
      const repairedSummary =
        report.repaired.length > 0
          ? report.repaired.join("；")
          : "没有需要自动修复的项";
      setStatusMessage(`工作区修复完成：${repairedSummary}`);
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setIsRepairingHealth(false);
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

  function handleAddGroup() {
    const nextGroupIndex = groupDrafts.length;
    setGroupDrafts((currentGroups) => [
      ...currentGroups,
      {
        id: createClientGroupId(),
        name: `分组 ${currentGroups.length + 1}`,
      },
    ]);
    setSelectedGroupIndex(nextGroupIndex);
  }

  function handleDeleteGroup(groupIndex: number) {
    const remainingGroups = groupDrafts.filter(
      (_, currentIndex) => currentIndex !== groupIndex,
    );
    setGroupDrafts(remainingGroups);
    setSelectedGroupIndex((currentIndex) => {
      if (remainingGroups.length === 0) {
        return 0;
      }
      if (currentIndex > groupIndex) {
        return currentIndex - 1;
      }
      if (currentIndex >= remainingGroups.length) {
        return remainingGroups.length - 1;
      }
      return currentIndex;
    });
  }

  function handleMoveGroup(groupIndex: number, direction: -1 | 1) {
    const targetIndex = groupIndex + direction;
    if (targetIndex < 0 || targetIndex >= groupDrafts.length) {
      return;
    }
    setGroupDrafts((currentGroups) => {
      const nextGroups = [...currentGroups];
      const [movedGroup] = nextGroups.splice(groupIndex, 1);
      nextGroups.splice(targetIndex, 0, movedGroup);
      return nextGroups;
    });
    setSelectedGroupIndex(targetIndex);
  }

  function updateGroupDraft(
    groupIndex: number,
    updater: (groupEntry: JobGroupDefinition) => JobGroupDefinition,
  ) {
    setGroupDrafts((currentGroups) =>
      currentGroups.map((groupEntry, currentIndex) =>
        currentIndex === groupIndex ? updater(groupEntry) : groupEntry,
      ),
    );
  }

  const selectedGroupDraft = useMemo(() => {
    if (groupDrafts.length === 0) {
      return null;
    }
    const clampedIndex = Math.min(
      selectedGroupIndex,
      groupDrafts.length - 1,
    );
    return {
      index: clampedIndex,
      group: groupDrafts[clampedIndex],
    };
  }, [groupDrafts, selectedGroupIndex]);

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

  function handleJobDetailSectionNavigation(
    currentSection: JobDetailSection,
    pressedKey: string,
    availableSections: ReadonlyArray<{ id: JobDetailSection }>,
  ): boolean {
    const currentIndex = availableSections.findIndex(
      (section) => section.id === currentSection,
    );
    if (currentIndex < 0 || availableSections.length === 0) {
      return false;
    }
    let nextIndex: number;
    switch (pressedKey) {
      case "ArrowUp":
      case "ArrowLeft":
        nextIndex =
          (currentIndex - 1 + availableSections.length) %
          availableSections.length;
        break;
      case "ArrowDown":
      case "ArrowRight":
        nextIndex = (currentIndex + 1) % availableSections.length;
        break;
      case "Home":
        nextIndex = 0;
        break;
      case "End":
        nextIndex = availableSections.length - 1;
        break;
      default:
        return false;
    }
    const nextSection = availableSections[nextIndex];
    setJobDetailSection(nextSection.id);
    document.getElementById(`job-detail-nav-${nextSection.id}`)?.focus();
    return true;
  }

  const activeSettingsSectionMeta =
    SETTINGS_SECTIONS.find((section) => section.id === settingsSection) ??
    SETTINGS_SECTIONS[0];

  const hasTranscriptSegments =
    (selectedJob?.transcript_segments.length ?? 0) > 0;
  const hasSummaryArtifact =
    Boolean(summaryText) || summaryArtifacts.length > 0;
  const activeSummaryContent =
    summaryArtifacts.find(
      (item) => item.template_id === activeSummaryTemplateId,
    )?.content ??
    summaryText ??
    "";

  async function handleFullTextSearch() {
    const query = fullTextQuery.trim();
    if (!query) {
      setFullTextHits([]);
      setFullTextHasSearched(false);
      return;
    }
    setIsFullTextSearching(true);
    setErrorMessage(null);
    try {
      const hits = await searchWorkspace(query, 40);
      setFullTextHits(hits);
      setFullTextHasSearched(true);
    } catch (error) {
      setFullTextHasSearched(true);
      setErrorMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setIsFullTextSearching(false);
    }
  }

  function clearFullTextSearch() {
    setFullTextQuery("");
    setFullTextHits([]);
    setFullTextHasSearched(false);
  }

  function openFullTextHit(hit: SearchHit) {
    void loadJobDetail(hit.job_id);
    setJobDetailSection(hit.field === "transcript" ? "transcript" : "summary");
    setFullTextHits([]);
    setFullTextHasSearched(false);
  }

  async function handleRebuildSearchIndex() {
    if (
      !(await confirmAction(
        "确定重建全文搜索索引吗？\n\n将清空并重新扫描工作区内全部任务的转写与总结，大工作区可能需要一些时间。",
      ))
    ) {
      return;
    }
    setIsBusy(true);
    setErrorMessage(null);
    try {
      const count = await rebuildSearchIndex();
      setStatusMessage(`已重建搜索索引（${count} 个任务）`);
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setIsBusy(false);
    }
  }
  const hasTranscriptArtifact = Boolean(transcriptText);
  const availableJobDetailSections = useMemo(
    () =>
      JOB_DETAIL_SECTIONS.filter((section) => {
        if (section.id === "segments") {
          return hasTranscriptSegments;
        }
        if (section.id === "summary") {
          return hasSummaryArtifact;
        }
        if (section.id === "transcript") {
          return hasTranscriptArtifact;
        }
        if (section.id === "proofread") {
          return hasTranscriptArtifact;
        }
        return true;
      }),
    [hasSummaryArtifact, hasTranscriptArtifact, hasTranscriptSegments],
  );

  useEffect(() => {
    if (
      !availableJobDetailSections.some(
        (section) => section.id === jobDetailSection,
      )
    ) {
      setJobDetailSection(availableJobDetailSections[0]?.id ?? "overview");
    }
  }, [availableJobDetailSections, jobDetailSection]);

  const activeJobDetailSectionMeta =
    availableJobDetailSections.find(
      (section) => section.id === jobDetailSection,
    ) ??
    availableJobDetailSections[0] ??
    JOB_DETAIL_SECTIONS[0];

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

      <main
        className={view === "settings" ? "content content-settings" : "content"}
      >
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
              <div
                className="stat-grid"
                role="toolbar"
                aria-label="按状态筛选任务"
              >
                {(
                  [
                    {
                      key: "all" as const,
                      label: "全部",
                      value: stats.total,
                      className: "stat-card",
                    },
                    {
                      key: "running" as const,
                      label: "运行中",
                      value: stats.running,
                      className: "stat-card running",
                    },
                    {
                      key: "queued" as const,
                      label: "排队中",
                      value: stats.queued,
                      className: "stat-card",
                    },
                    {
                      key: "succeeded" as const,
                      label: "成功",
                      value: stats.succeeded,
                      className: "stat-card ok",
                    },
                    {
                      key: "failed" as const,
                      label: "失败",
                      value: stats.failed,
                      className: "stat-card bad",
                    },
                  ] as const
                ).map((statItem) => {
                  const isActive = statusFilter === statItem.key;
                  return (
                    <button
                      key={statItem.key}
                      type="button"
                      className={
                        isActive
                          ? `${statItem.className} active`
                          : statItem.className
                      }
                      aria-pressed={isActive}
                      aria-label={`按${statItem.label}筛选任务，当前 ${statItem.value} 个`}
                      onClick={() => setStatusFilter(statItem.key)}
                    >
                      <span className="stat-label">{statItem.label}</span>
                      <strong>{statItem.value}</strong>
                    </button>
                  );
                })}
              </div>
            </section>

            <div className="jobs-layout">
              <section className="panel list-panel">
                <div className="panel-header">
                  <div>
                    <h2>任务列表</h2>
                    <p className="muted small">点击任务查看步骤、日志与重试</p>
                  </div>
                  <div className="fulltext-search-bar">
                    <div className="fulltext-search-combo">
                      <div className="fulltext-search-field">
                        <input
                          className="search fulltext-search-input"
                          aria-label="跨任务全文检索"
                          placeholder="搜转写 / 总结…"
                          value={fullTextQuery}
                          onChange={(event) => {
                            setFullTextQuery(event.target.value);
                            if (event.target.value.trim() === "") {
                              setFullTextHits([]);
                              setFullTextHasSearched(false);
                            }
                          }}
                          onKeyDown={(event) => {
                            if (event.key === "Enter") {
                              event.preventDefault();
                              void handleFullTextSearch();
                            }
                            if (event.key === "Escape") {
                              clearFullTextSearch();
                            }
                          }}
                        />
                        {(fullTextQuery || fullTextHasSearched) && (
                          <button
                            type="button"
                            className="fulltext-clear-icon"
                            aria-label="清除全文检索"
                            onClick={() => clearFullTextSearch()}
                          >
                            ×
                          </button>
                        )}
                      </div>
                      <button
                        type="button"
                        className="btn small fulltext-search-submit"
                        disabled={isFullTextSearching}
                        onClick={() => void handleFullTextSearch()}
                      >
                        {isFullTextSearching ? "…" : "搜"}
                      </button>
                    </div>
                    {(fullTextHits.length > 0 || fullTextHasSearched) && (
                      <div
                        className="fulltext-dropdown"
                        role="listbox"
                        aria-label="全文检索结果"
                      >
                        <div className="fulltext-dropdown-head">
                          <span>
                            {fullTextHits.length > 0
                              ? `${fullTextHits.length} 条匹配`
                              : "无匹配结果"}
                          </span>
                          <button
                            type="button"
                            className="fulltext-reindex-link"
                            disabled={isBusy}
                            onClick={() => void handleRebuildSearchIndex()}
                          >
                            重建索引
                          </button>
                        </div>
                        {fullTextHits.length > 0 && (
                          <div className="fulltext-hits" role="list">
                            {fullTextHits.map((hit) => {
                              const fieldLabel =
                                hit.field === "transcript"
                                  ? "转写"
                                  : hit.field === "summary"
                                    ? "总结"
                                    : hit.field === "summary_template"
                                      ? "模板"
                                      : hit.field;
                              const cleanSnippet = hit.snippet
                                .replace(/\s+/g, " ")
                                .trim();
                              return (
                                <button
                                  key={`${hit.job_id}-${hit.path}-${cleanSnippet.slice(0, 20)}`}
                                  type="button"
                                  className="fulltext-hit"
                                  role="option"
                                  title={hit.title}
                                  onClick={() => openFullTextHit(hit)}
                                >
                                  <div className="fulltext-hit-top">
                                    <span className="fulltext-hit-title">
                                      {hit.title}
                                    </span>
                                    <span className="fulltext-hit-badge">
                                      {fieldLabel}
                                    </span>
                                  </div>
                                  <p className="fulltext-snippet">
                                    {cleanSnippet}
                                  </p>
                                </button>
                              );
                            })}
                          </div>
                        )}
                      </div>
                    )}
                  </div>
                </div>
                <div className="title-filter-bar">
                  <input
                    className="search"
                    aria-label="按标题筛选任务"
                    placeholder="筛选标题"
                    value={searchQuery}
                    onChange={(event) => setSearchQuery(event.target.value)}
                  />
                </div>

                {recentBatchOptions.length > 0 && (
                  <div
                    className="group-filter-bar"
                    role="toolbar"
                    aria-label="按批量创建筛选任务"
                  >
                    <button
                      type="button"
                      className={
                        batchFilter === "all"
                          ? "chip group-filter-chip active"
                          : "chip group-filter-chip"
                      }
                      aria-pressed={batchFilter === "all"}
                      onClick={() => setBatchFilter("all")}
                    >
                      全部批次
                    </button>
                    {recentBatchOptions.map((batchId) => {
                      const isActive = batchFilter === batchId;
                      return (
                        <button
                          key={batchId}
                          type="button"
                          className={
                            isActive
                              ? "chip group-filter-chip active"
                              : "chip group-filter-chip"
                          }
                          aria-pressed={isActive}
                          title={batchId}
                          onClick={() => setBatchFilter(batchId)}
                        >
                          批次 {batchId.slice(0, 8)}
                        </button>
                      );
                    })}
                  </div>
                )}

                {hasAnyGroupFilterChips && (
                  <div
                    className="group-filter-bar"
                    role="toolbar"
                    aria-label="按分组筛选任务"
                  >
                    <button
                      type="button"
                      className={
                        groupFilter === "all"
                          ? "chip group-filter-chip active"
                          : "chip group-filter-chip"
                      }
                      aria-pressed={groupFilter === "all"}
                      onClick={() => setGroupFilter("all")}
                    >
                      全部
                    </button>
                    {managedJobGroups.map((groupEntry) => {
                      const filterKey = `id:${groupEntry.id}`;
                      const isActive = groupFilter === filterKey;
                      return (
                        <button
                          key={groupEntry.id}
                          type="button"
                          className={
                            isActive
                              ? "chip group-filter-chip active"
                              : "chip group-filter-chip"
                          }
                          aria-pressed={isActive}
                          title={groupEntry.name}
                          onClick={() => setGroupFilter(filterKey)}
                        >
                          {groupEntry.name}
                        </button>
                      );
                    })}
                    {orphanGroupFilterOptions.map((option) => {
                      const isActive = groupFilter === option.filterKey;
                      return (
                        <button
                          key={option.filterKey}
                          type="button"
                          className={
                            isActive
                              ? "chip group-filter-chip active"
                              : "chip group-filter-chip"
                          }
                          aria-pressed={isActive}
                          title={`${option.label}（未在目录中）`}
                          onClick={() => setGroupFilter(option.filterKey)}
                        >
                          {option.label}
                        </button>
                      );
                    })}
                    {hasUngroupedJobs && (
                      <button
                        type="button"
                        className={
                          groupFilter === "ungrouped"
                            ? "chip group-filter-chip active"
                            : "chip group-filter-chip"
                        }
                        aria-pressed={groupFilter === "ungrouped"}
                        onClick={() => setGroupFilter("ungrouped")}
                      >
                        未分组
                      </button>
                    )}
                  </div>
                )}

                {isLoading ? (
                  <div className="empty">加载中…</div>
                ) : jobs.length === 0 ? (
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
                ) : filteredJobs.length === 0 ? (
                  <div className="empty empty-card">
                    <h3>没有匹配的任务</h3>
                    <p>
                      当前分组筛选或搜索没有结果。可切换「全部」分组，或清空搜索关键词。
                    </p>
                    <div className="empty-actions">
                      <button
                        className="btn secondary"
                        type="button"
                        onClick={() => {
                          setGroupFilter("all");
                          setSearchQuery("");
                        }}
                      >
                        清除筛选
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
                              {formatQueueStatusLabel(
                                job.status,
                                job.queue_position,
                              )}
                            </span>
                          </div>
                          {(resolveJobGroupLabel(job.group, managedJobGroups) ||
                            job.batch_id) && (
                            <div className="job-card-group">
                              {resolveJobGroupLabel(
                                job.group,
                                managedJobGroups,
                              ) && (
                                <span className="pill group-pill">
                                  {resolveJobGroupLabel(
                                    job.group,
                                    managedJobGroups,
                                  )}
                                </span>
                              )}
                              {job.batch_id && (
                                <span
                                  className="pill group-pill"
                                  title={job.batch_id}
                                >
                                  批次 {job.batch_id.slice(0, 8)}
                                </span>
                              )}
                            </div>
                          )}
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
                                : job.status === "queued"
                                  ? "从队列移除并永久删除任务"
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
                      在左侧列表点选任务，用顶部分区查看流水线、配置、日志与产物
                    </p>
                  </div>
                ) : (
                  <>
                    <div className="detail-header">
                      <div className="detail-header-main">
                        <div className="detail-kicker">
                          {KIND_LABEL[selectedJob.source.kind]} ·{" "}
                          <span className={`pill status-${selectedJob.status}`}>
                            {formatQueueStatusLabel(
                              selectedJob.status,
                              jobs.find((job) => job.id === selectedJob.id)
                                ?.queue_position,
                            )}
                          </span>
                        </div>
                        <label className="job-title-field">
                          <span className="visually-hidden">任务标题</span>
                          <input
                            className="job-title-input"
                            type="text"
                            value={jobTitleDraft}
                            placeholder={
                              selectedJob.source.url ||
                              selectedJob.source.local_path ||
                              selectedJob.id
                            }
                            disabled={
                              isSavingJobTitle ||
                              selectedJob.status === "running"
                            }
                            title={
                              selectedJob.status === "running"
                                ? "任务运行期间不能修改标题"
                                : undefined
                            }
                            aria-label="任务标题"
                            onChange={(event) => {
                              setJobTitleDraft(event.target.value);
                            }}
                            onBlur={() => {
                              void handleSaveJobTitle();
                            }}
                            onKeyDown={(event) => {
                              if (event.key === "Enter") {
                                event.preventDefault();
                                (event.target as HTMLInputElement).blur();
                              }
                              if (event.key === "Escape") {
                                event.preventDefault();
                                const savedTitle =
                                  selectedJob.source.title?.trim() ?? "";
                                setJobTitleDraft(savedTitle);
                                jobTitleDraftRef.current = savedTitle;
                                (event.target as HTMLInputElement).blur();
                              }
                            }}
                          />
                        </label>
                        <div className="mono muted small">{selectedJob.id}</div>
                      </div>
                      <div className="detail-actions job-detail-actions">
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

                    {recoverySuggestion && (
                      <div className="recovery-card" role="region" aria-label="修复建议">
                        <div className="recovery-card-header">
                          <div>
                            <div className="recovery-kicker">修复建议</div>
                            <h3 className="recovery-title">{recoverySuggestion.title}</h3>
                          </div>
                          <span className="pill recovery-code mono">
                            {recoverySuggestion.code}
                          </span>
                        </div>
                        <p className="recovery-summary">{recoverySuggestion.summary}</p>
                        {selectedJob.error_message && (
                          <div className="recovery-error mono small">
                            {selectedJob.error_message}
                          </div>
                        )}
                        {recoverySuggestion.hints.length > 0 && (
                          <ul className="recovery-hints">
                            {recoverySuggestion.hints.map((hint) => (
                              <li key={hint}>{hint}</li>
                            ))}
                          </ul>
                        )}
                        <div className="recovery-actions">
                          {recoverySuggestion.actions.map((action) => (
                            <button
                              key={`${action.id}-${action.label}`}
                              className={
                                action.primary ? "btn" : "btn secondary"
                              }
                              type="button"
                              disabled={
                                isBusy ||
                                selectedJob.status === "running" ||
                                selectedJob.status === "queued"
                              }
                              onClick={() =>
                                void handleRecoveryAction(
                                  action,
                                  recoverySuggestion,
                                )
                              }
                            >
                              {action.label}
                            </button>
                          ))}
                        </div>
                      </div>
                    )}

                    {!recoverySuggestion && selectedJob.error_message && (
                      <div className="banner error inline">
                        {selectedJob.error_message}
                      </div>
                    )}

                    <div className="job-detail-layout">
                      <nav
                        className="job-detail-tabs"
                        role="tablist"
                        aria-label="任务详情分区"
                        aria-orientation="horizontal"
                      >
                        {availableJobDetailSections.map((section) => {
                          const isActive = jobDetailSection === section.id;
                          const sectionCountLabel =
                            section.id === "segments"
                              ? String(
                                  selectedJob.selected_segment_ids.length,
                                )
                              : null;
                          return (
                            <button
                              key={section.id}
                              id={`job-detail-nav-${section.id}`}
                              type="button"
                              role="tab"
                              aria-selected={isActive}
                              aria-controls={`job-detail-panel-${section.id}`}
                              tabIndex={isActive ? 0 : -1}
                              className={
                                isActive
                                  ? "job-detail-tab active"
                                  : "job-detail-tab"
                              }
                              onClick={() => setJobDetailSection(section.id)}
                              onKeyDown={(event) => {
                                if (
                                  handleJobDetailSectionNavigation(
                                    jobDetailSection,
                                    event.key,
                                    availableJobDetailSections,
                                  )
                                ) {
                                  event.preventDefault();
                                }
                              }}
                            >
                              <span className="job-detail-tab-label">
                                {section.label}
                              </span>
                              {sectionCountLabel != null && (
                                <span className="job-detail-tab-count">
                                  {sectionCountLabel}
                                </span>
                              )}
                            </button>
                          );
                        })}
                      </nav>

                      <div className="job-detail-main">
                        <div className="settings-section-intro">
                          <h2 id={`job-detail-panel-title-${jobDetailSection}`}>
                            {activeJobDetailSectionMeta.label}
                          </h2>
                          <p className="muted small">
                            {activeJobDetailSectionMeta.description}
                          </p>
                        </div>

                        {jobDetailSection === "overview" && (
                          <div
                            id="job-detail-panel-overview"
                            className="settings-section-panel"
                            role="tabpanel"
                            aria-labelledby="job-detail-nav-overview"
                          >
                            <article className="card soft">
                              <h3 className="visually-hidden">来源概览</h3>
                              <dl className="meta-list">
                                <div className="job-group-field-row">
                                  <dt>分组</dt>
                                  <dd>
                                    <label className="job-group-field">
                                      <span className="visually-hidden">
                                        任务分组
                                      </span>
                                      <select
                                        className="job-group-select"
                                        value={jobGroupDraft}
                                        disabled={
                                          isSavingJobGroup ||
                                          selectedJob.status === "running"
                                        }
                                        title={
                                          selectedJob.status === "running"
                                            ? "任务运行期间不能修改分组"
                                            : managedJobGroups.length === 0
                                              ? "请先在设置 → 任务分组中创建分组"
                                              : undefined
                                        }
                                        aria-label="任务分组"
                                        onChange={(event) => {
                                          void handleJobGroupSelectChange(
                                            event.target.value,
                                          );
                                        }}
                                      >
                                        <option value="">未分组</option>
                                        {managedJobGroups.map((groupEntry) => (
                                          <option
                                            key={groupEntry.id}
                                            value={groupEntry.id}
                                          >
                                            {groupEntry.name}
                                          </option>
                                        ))}
                                        {/* Preserve orphan free-text groups not yet in catalog. */}
                                        {normalizeJobGroup(selectedJob.group) &&
                                          !managedJobGroups.some(
                                            (groupEntry) =>
                                              groupEntry.id ===
                                                normalizeJobGroup(
                                                  selectedJob.group,
                                                ) ||
                                              groupEntry.name
                                                .trim()
                                                .toLowerCase() ===
                                                (
                                                  normalizeJobGroup(
                                                    selectedJob.group,
                                                  ) ?? ""
                                                ).toLowerCase(),
                                          ) && (
                                            <option
                                              value={
                                                normalizeJobGroup(
                                                  selectedJob.group,
                                                ) ?? ""
                                              }
                                            >
                                              {normalizeJobGroup(
                                                selectedJob.group,
                                              )}{" "}
                                              （未在目录中）
                                            </option>
                                          )}
                                      </select>
                                    </label>
                                    {managedJobGroups.length === 0 && (
                                      <p className="muted small job-group-hint">
                                        暂无分组目录。请到「设置 → 任务分组」添加后再选择。
                                      </p>
                                    )}
                                  </dd>
                                </div>
                                <div>
                                  <dt>URL</dt>
                                  <dd className="mono">
                                    {selectedJob.source.url || "—"}
                                  </dd>
                                </div>
                                {(selectedJob.source.kind === "download" ||
                                  selectedJob.source.kind === "live_record") && (
                                  <div>
                                    <dt>保存形态</dt>
                                    <dd>
                                      <fieldset
                                        className="radio-fieldset"
                                        disabled={
                                          selectedJob.status === "running" ||
                                          isSavingJobMediaSaveMode
                                        }
                                      >
                                        <legend className="visually-hidden">
                                          保存形态
                                        </legend>
                                        <div className="checkbox-row">
                                          <label className="checkbox">
                                            <input
                                              type="radio"
                                              name="job-media-save-mode"
                                              value="video"
                                              checked={
                                                (selectedJob.source
                                                  .media_save_mode ??
                                                  "video") === "video"
                                              }
                                              disabled={
                                                selectedJob.status ===
                                                  "running" ||
                                                isSavingJobMediaSaveMode
                                              }
                                              onChange={() =>
                                                void handleJobMediaSaveModeChange(
                                                  "video",
                                                )
                                              }
                                            />
                                            保存视频
                                          </label>
                                          <label className="checkbox">
                                            <input
                                              type="radio"
                                              name="job-media-save-mode"
                                              value="audio"
                                              checked={
                                                (selectedJob.source
                                                  .media_save_mode ??
                                                  "video") === "audio"
                                              }
                                              disabled={
                                                selectedJob.status ===
                                                  "running" ||
                                                isSavingJobMediaSaveMode
                                              }
                                              onChange={() =>
                                                void handleJobMediaSaveModeChange(
                                                  "audio",
                                                )
                                              }
                                            />
                                            保存音频
                                          </label>
                                        </div>
                                        <p className="muted small">
                                          {isSavingJobMediaSaveMode
                                            ? "正在保存…"
                                            : selectedJob.status === "running"
                                              ? "任务运行中不可修改。"
                                              : "可随时重配；若已有媒体产物，切换后需重新下载/录制。"}
                                        </p>
                                      </fieldset>
                                    </dd>
                                  </div>
                                )}
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
                          </div>
                        )}

                        {jobDetailSection === "pipeline" && (
                          <div
                            id="job-detail-panel-pipeline"
                            className="settings-section-panel"
                            role="tabpanel"
                            aria-labelledby="job-detail-nav-pipeline"
                          >
                            <article className="card soft">
                              <h3 className="visually-hidden">流水线</h3>
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
                                        <span
                                          className={`pill step-${step.status}`}
                                        >
                                          {STEP_STATUS_LABEL[step.status]}
                                        </span>
                                        <button
                                          type="button"
                                          className="chip"
                                          disabled={
                                            isBusy ||
                                            selectedJob.status === "running"
                                          }
                                          onClick={() =>
                                            void handleRun(
                                              selectedJob.id,
                                              step.step,
                                            )
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
                                {selectedJob.pipeline.auto_transcribe
                                  ? "开"
                                  : "关"}{" "}
                                · 自动章节：
                                {selectedJob.pipeline.auto_chapterize
                                  ? "开"
                                  : "关"}{" "}
                                · 自动总结：
                                {selectedJob.pipeline.auto_summarize
                                  ? "开"
                                  : "关"}
                                {selectedJob.glossary_hash ? (
                                  <>
                                    {" "}
                                    · 术语表：
                                    <span className="mono">
                                      {selectedJob.glossary_hash.slice(0, 8)}
                                    </span>
                                  </>
                                ) : null}
                              </div>
                            </article>
                          </div>
                        )}

                        {jobDetailSection === "summarize" && (
                          <div
                            id="job-detail-panel-summarize"
                            className="settings-section-panel"
                            role="tabpanel"
                            aria-labelledby="job-detail-nav-summarize"
                          >
                            <article className="card soft summarize-config-card">
                              <h3 className="visually-hidden">总结配置</h3>
                              <p className="muted small summarize-config-hint">
                                选「使用全局默认 / Provider 默认」则跟随设置；指定后固化到本任务。保存后请重跑「AI 总结」。
                              </p>
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
                                        handleJobProviderChange(
                                          event.target.value,
                                        )
                                      }
                                    >
                                      <option value="">使用全局默认</option>
                                      {config?.providers.map((provider) => (
                                        <option
                                          key={provider.id}
                                          value={provider.id}
                                        >
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
                                      {jobProviderModelOptions.map(
                                        (modelName) => (
                                          <option
                                            key={modelName}
                                            value={modelName}
                                          >
                                            {modelName}
                                            {modelName ===
                                            selectedJobProvider?.default_model
                                              ? "（档案默认）"
                                              : ""}
                                          </option>
                                        ),
                                      )}
                                    </select>
                                  </label>
                                </div>
                                <div className="multi-template-picker">
                                  <span>总结模板（可多选，顺序=产出顺序）</span>
                                  <p className="muted small">
                                    第一项写入 summary/summary.md；其余写入
                                    summary/by_template/。不选则跟随全局默认。
                                  </p>
                                  <div className="multi-template-list">
                                    {config?.templates.map((template) => {
                                      const checked = jobTemplateIds.includes(
                                        template.id,
                                      );
                                      const orderIndex =
                                        jobTemplateIds.indexOf(template.id);
                                      return (
                                        <label
                                          key={template.id}
                                          className="multi-template-item"
                                        >
                                          <input
                                            type="checkbox"
                                            checked={checked}
                                            disabled={
                                              selectedJob.status ===
                                                "running" ||
                                              isSavingJobPipeline
                                            }
                                            onChange={() => {
                                              setJobTemplateIds((previous) => {
                                                if (
                                                  previous.includes(template.id)
                                                ) {
                                                  return previous.filter(
                                                    (id) => id !== template.id,
                                                  );
                                                }
                                                return [
                                                  ...previous,
                                                  template.id,
                                                ];
                                              });
                                              setJobTemplateId((previous) => {
                                                if (
                                                  jobTemplateIds.includes(
                                                    template.id,
                                                  )
                                                ) {
                                                  const next =
                                                    jobTemplateIds.filter(
                                                      (id) =>
                                                        id !== template.id,
                                                    );
                                                  return next[0] ?? "";
                                                }
                                                return (
                                                  previous || template.id
                                                );
                                              });
                                            }}
                                          />
                                          <span>
                                            {orderIndex >= 0
                                              ? `${orderIndex + 1}. `
                                              : ""}
                                            {template.name}
                                            {template.id ===
                                            config.default_template_id
                                              ? "（全局默认）"
                                              : ""}
                                          </span>
                                        </label>
                                      );
                                    })}
                                  </div>
                                </div>
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
                                    {isSavingJobPipeline
                                      ? "保存中…"
                                      : "保存总结配置"}
                                  </button>
                                  {jobPipelineIsDirty ? (
                                    <span className="muted small">
                                      有未保存的修改
                                    </span>
                                  ) : (
                                    <span className="muted small">
                                      当前与任务已保存配置一致
                                    </span>
                                  )}
                                </div>
                              </div>
                            </article>
                          </div>
                        )}

                        {jobDetailSection === "segments" &&
                          hasTranscriptSegments && (
                            <div
                              id="job-detail-panel-segments"
                              className="settings-section-panel"
                              role="tabpanel"
                              aria-labelledby="job-detail-nav-segments"
                            >
                              <article className="card soft segment-card">
                                <div className="log-header">
                                  <div>
                                    <h3 className="visually-hidden">
                                      总结选段
                                    </h3>
                                    <p className="muted small">
                                      取消不需要的分段后，依次重试“合并字幕”和“AI
                                      总结”。
                                    </p>
                                  </div>
                                  <span className="pill">
                                    已选{" "}
                                    {selectedJob.selected_segment_ids.length} /{" "}
                                    {selectedJob.transcript_segments.length}
                                  </span>
                                </div>
                                <div className="segment-list">
                                  {selectedJob.transcript_segments.map(
                                    (segment) => (
                                      <div
                                        key={segment.id}
                                        className="segment-row"
                                      >
                                        <input
                                          type="checkbox"
                                          aria-label={`选择转写分段 ${segment.id}`}
                                          checked={selectedJob.selected_segment_ids.includes(
                                            segment.id,
                                          )}
                                          disabled={
                                            selectedJob.status === "running" ||
                                            isUpdatingSegmentSelection
                                          }
                                          onChange={() =>
                                            void handleToggleSegment(
                                              selectedJob,
                                              segment.id,
                                            )
                                          }
                                        />
                                        <span>
                                          <strong>{segment.id}</strong>
                                          <span className="muted small">
                                            {segment.media_file}
                                          </span>
                                        </span>
                                        <div className="step-actions">
                                          <span
                                            className={`pill step-${segment.status}`}
                                          >
                                            {
                                              STEP_STATUS_LABEL[
                                                segment.status
                                              ]
                                            }
                                          </span>
                                          <button
                                            type="button"
                                            className="chip"
                                            disabled={
                                              isBusy ||
                                              selectedJob.status === "running"
                                            }
                                            onClick={() => {
                                              void handleRetrySegment(
                                                selectedJob.id,
                                                segment.id,
                                              );
                                            }}
                                          >
                                            重试转写
                                          </button>
                                          <button
                                            type="button"
                                            className="chip"
                                            disabled={isBusy}
                                            onClick={() => {
                                              void handleCompareSegment(
                                                selectedJob.id,
                                                segment.id,
                                              );
                                            }}
                                          >
                                            对比上一版
                                          </button>
                                        </div>
                                      </div>
                                    ),
                                  )}
                                </div>
                                {segmentDiffText && (
                                  <pre className="artifact-view transcript-view segment-diff-view">
                                    {segmentDiffText}
                                  </pre>
                                )}
                              </article>
                            </div>
                          )}

                        {jobDetailSection === "preview" && (
                          <div
                            id="job-detail-panel-preview"
                            className="settings-section-panel"
                            role="tabpanel"
                            aria-labelledby="job-detail-nav-preview"
                          >
                            <MediaPreviewPanel
                              jobId={selectedJob.id}
                              isJobBusy={
                                selectedJob.status === "running" ||
                                selectedJob.status === "queued"
                              }
                              onError={setErrorMessage}
                              onStatus={setStatusMessage}
                            />
                          </div>
                        )}

                        {jobDetailSection === "proofread" && (
                          <div
                            id="job-detail-panel-proofread"
                            className="settings-section-panel"
                            role="tabpanel"
                            aria-labelledby="job-detail-nav-proofread"
                          >
                            <TranscriptProofreadPanel
                              jobId={selectedJob.id}
                              isJobBusy={
                                selectedJob.status === "running" ||
                                selectedJob.status === "queued"
                              }
                              transcriptEditedAt={
                                selectedJob.transcript_edited_at
                              }
                              onError={setErrorMessage}
                              onStatus={setStatusMessage}
                              onSaved={(updatedJob) => {
                                selectedJobRef.current = updatedJob;
                                setSelectedJob(updatedJob);
                                setTranscriptText("");
                                setSummaryText("");
                                setChaptersText("");
                              }}
                            />
                          </div>
                        )}

                        {jobDetailSection === "logs" && (
                          <div
                            id="job-detail-panel-logs"
                            className="settings-section-panel"
                            role="tabpanel"
                            aria-labelledby="job-detail-nav-logs"
                          >
                            <article className="card soft log-card">
                              <div className="log-header">
                                <h3 className="visually-hidden">日志</h3>
                                <div
                                  className="log-tabs"
                                  role="tablist"
                                  aria-label="任务日志"
                                >
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
                                        logName === name
                                          ? "chip active"
                                          : "chip"
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
                          </div>
                        )}

                        {jobDetailSection === "summary" &&
                          hasSummaryArtifact && (
                            <div
                              id="job-detail-panel-summary"
                              className="settings-section-panel"
                              role="tabpanel"
                              aria-labelledby="job-detail-nav-summary"
                            >
                              <article className="card soft summary-card">
                                <div className="artifact-card-header">
                                  <h3 className="visually-hidden">
                                    Markdown 总结
                                  </h3>
                                  <span className="muted small">
                                    可读文档视图
                                  </span>
                                </div>
                                {summaryArtifacts.length > 1 && (
                                  <div
                                    className="summary-template-tabs"
                                    role="tablist"
                                    aria-label="多模板产物"
                                  >
                                    {summaryArtifacts.map((artifact) => (
                                      <button
                                        key={artifact.template_id}
                                        type="button"
                                        role="tab"
                                        className={
                                          activeSummaryTemplateId ===
                                          artifact.template_id
                                            ? "chip active"
                                            : "chip"
                                        }
                                        aria-selected={
                                          activeSummaryTemplateId ===
                                          artifact.template_id
                                        }
                                        onClick={() =>
                                          setActiveSummaryTemplateId(
                                            artifact.template_id,
                                          )
                                        }
                                      >
                                        {artifact.primary
                                          ? `主模板 · ${artifact.template_id}`
                                          : artifact.template_id}
                                      </button>
                                    ))}
                                  </div>
                                )}
                                <div className="markdown-view">
                                  <ReactMarkdown
                                    remarkPlugins={[remarkGfm]}
                                    components={{
                                      table: ({ children, ...tableProps }) => (
                                        <div className="markdown-table-scroll">
                                          <table {...tableProps}>
                                            {children}
                                          </table>
                                        </div>
                                      ),
                                    }}
                                  >
                                    {unwrapOuterMarkdownFence(
                                      activeSummaryContent,
                                    )}
                                  </ReactMarkdown>
                                </div>
                              </article>
                            </div>
                          )}

                        {jobDetailSection === "transcript" &&
                          hasTranscriptArtifact && (
                            <div
                              id="job-detail-panel-transcript"
                              className="settings-section-panel"
                              role="tabpanel"
                              aria-labelledby="job-detail-nav-transcript"
                            >
                              <article className="card soft transcript-card">
                                <div className="artifact-card-header">
                                  <h3 className="visually-hidden">合并字幕</h3>
                                  <span className="muted small">原文对照</span>
                                </div>
                                <pre className="artifact-view transcript-view">
                                  {transcriptText}
                                </pre>
                              </article>
                              {(chaptersText.trim().length > 0 ||
                                selectedJob.chapters_path) && (
                                <article className="card soft transcript-card">
                                  <div className="artifact-card-header">
                                    <h3>章节大纲</h3>
                                    <span className="muted small">
                                      {selectedJob.chapters_path ??
                                        "transcript/chapters.md"}
                                    </span>
                                  </div>
                                  <pre className="artifact-view transcript-view">
                                    {chaptersText || "（章节尚未生成）"}
                                  </pre>
                                </article>
                              )}
                            </div>
                          )}
                      </div>
                    </div>
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
                  工作区与 API Key 分离存放。按左侧分区管理，修改后点右下角保存。
                </p>
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
                    <label className="checkbox">
                      <input
                        type="checkbox"
                        checked={settingsAutoChapterize}
                        onChange={(event) =>
                          setSettingsAutoChapterize(event.target.checked)
                        }
                      />
                      默认自动章节大纲（总结前）
                    </label>
                    <label className="checkbox">
                      <input
                        type="checkbox"
                        checked={settingsNotifyOnJobFinish}
                        onChange={(event) =>
                          setSettingsNotifyOnJobFinish(event.target.checked)
                        }
                      />
                      任务完成/失败时发送系统通知（窗口前台时不弹）
                    </label>
                  </div>
                  <label>
                    <span>术语表热词（每行一个，用于 whisper 初始提示）</span>
                    <textarea
                      rows={3}
                      value={settingsGlossaryHotwords}
                      onChange={(event) =>
                        setSettingsGlossaryHotwords(event.target.value)
                      }
                      placeholder="例如：OpenAI&#10;张三"
                    />
                  </label>
                  <label>
                    <span>合并后替换（每行：错误写法 =&gt; 正确写法）</span>
                    <textarea
                      rows={3}
                      value={settingsGlossaryReplacements}
                      onChange={(event) =>
                        setSettingsGlossaryReplacements(event.target.value)
                      }
                      placeholder="openai =&gt; OpenAI"
                    />
                  </label>
                  <div className="checkbox-row">
                    <label className="checkbox">
                      <input
                        type="checkbox"
                        checked={settingsGlossaryWhisperPrompt}
                        onChange={(event) =>
                          setSettingsGlossaryWhisperPrompt(event.target.checked)
                        }
                      />
                      转写时注入热词 prompt
                    </label>
                    <label className="checkbox">
                      <input
                        type="checkbox"
                        checked={settingsGlossaryPostReplace}
                        onChange={(event) =>
                          setSettingsGlossaryPostReplace(event.target.checked)
                        }
                      />
                      合并后应用替换
                    </label>
                  </div>
                  <div className="transcribe-model-setup">
                    <div className="settings-block-title">
                      <h3>转写模型怎么选</h3>
                      <p className="muted small">
                        Whisper 需要本机一个{" "}
                        <code className="inline-code">.bin</code> 模型文件。
                        日常只需选好「当前使用」；下面三套路径是可选快捷切换（小模型更快，大模型更准）。
                      </p>
                    </div>
                    <label>
                      <span>当前使用</span>
                      <select
                        value={settingsTranscribeModelPreset}
                        onChange={(event) =>
                          setSettingsTranscribeModelPreset(event.target.value)
                        }
                      >
                        <option value="custom">下面这个主模型文件</option>
                        <option value="speed">
                          速度预设（小模型，快，略糙）
                        </option>
                        <option value="balanced">
                          平衡预设（推荐日常）
                        </option>
                        <option value="quality">
                          质量预设（大模型，慢，更准）
                        </option>
                      </select>
                    </label>
                    <p className="muted small transcribe-model-active-hint">
                      实际会用：
                      <span className="mono">
                        {settingsTranscribeModelPreset === "speed"
                          ? settingsModelPresetSpeed.trim() ||
                            settingsTranscribeModel.trim() ||
                            "（未配置速度预设，也无主模型）"
                          : settingsTranscribeModelPreset === "balanced"
                            ? settingsModelPresetBalanced.trim() ||
                              settingsTranscribeModel.trim() ||
                              "（未配置平衡预设，也无主模型）"
                            : settingsTranscribeModelPreset === "quality"
                              ? settingsModelPresetQuality.trim() ||
                                settingsTranscribeModel.trim() ||
                                "（未配置质量预设，也无主模型）"
                              : settingsTranscribeModel.trim() ||
                                "（尚未选择主模型文件）"}
                      </span>
                    </p>
                    {settingsTranscribeModelPreset !== "custom" && (
                      <p className="muted small">
                        若对应预设路径为空，会回退到主模型文件。
                      </p>
                    )}
                    <details className="transcribe-preset-details">
                      <summary>可选：配置速度 / 平衡 / 质量三套模型路径</summary>
                      <p className="muted small">
                        只有打算在「当前使用」里切换预设时才需要填。三套可以指向不同
                        ggml 文件，例如 tiny / small / medium。
                      </p>
                      <PathPickerField
                        label="速度预设 · 模型文件"
                        value={settingsModelPresetSpeed}
                        emptyValueLabel="未设置（会回退主模型）"
                        selectButtonLabel="选择文件"
                        isSelecting={
                          activeSettingsPathPicker === "model-preset-speed"
                        }
                        isDisabled={isBusy || settingsPathSelectionIsActive}
                        onSelect={() =>
                          void handleSelectSettingsPath({
                            pickerId: "model-preset-speed",
                            title: "选择速度预设 GGML 模型",
                            currentPath: settingsModelPresetSpeed,
                            selectionKind: "file",
                            filters: [
                              { name: "GGML 模型", extensions: ["bin"] },
                            ],
                            updatePath: setSettingsModelPresetSpeed,
                          })
                        }
                        onClear={() => setSettingsModelPresetSpeed("")}
                      />
                      <PathPickerField
                        label="平衡预设 · 模型文件"
                        value={settingsModelPresetBalanced}
                        emptyValueLabel="未设置（会回退主模型）"
                        selectButtonLabel="选择文件"
                        isSelecting={
                          activeSettingsPathPicker === "model-preset-balanced"
                        }
                        isDisabled={isBusy || settingsPathSelectionIsActive}
                        onSelect={() =>
                          void handleSelectSettingsPath({
                            pickerId: "model-preset-balanced",
                            title: "选择平衡预设 GGML 模型",
                            currentPath: settingsModelPresetBalanced,
                            selectionKind: "file",
                            filters: [
                              { name: "GGML 模型", extensions: ["bin"] },
                            ],
                            updatePath: setSettingsModelPresetBalanced,
                          })
                        }
                        onClear={() => setSettingsModelPresetBalanced("")}
                      />
                      <PathPickerField
                        label="质量预设 · 模型文件"
                        value={settingsModelPresetQuality}
                        emptyValueLabel="未设置（会回退主模型）"
                        selectButtonLabel="选择文件"
                        isSelecting={
                          activeSettingsPathPicker === "model-preset-quality"
                        }
                        isDisabled={isBusy || settingsPathSelectionIsActive}
                        onSelect={() =>
                          void handleSelectSettingsPath({
                            pickerId: "model-preset-quality",
                            title: "选择质量预设 GGML 模型",
                            currentPath: settingsModelPresetQuality,
                            selectionKind: "file",
                            filters: [
                              { name: "GGML 模型", extensions: ["bin"] },
                            ],
                            updatePath: setSettingsModelPresetQuality,
                          })
                        }
                        onClear={() => setSettingsModelPresetQuality("")}
                      />
                    </details>
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
                      <span>全局并发任务数</span>
                      <input
                        type="number"
                        min={1}
                        max={64}
                        value={settingsMaxConcurrentJobs}
                        onChange={(event) =>
                          setSettingsMaxConcurrentJobs(
                            Number(event.target.value) || 1,
                          )
                        }
                      />
                    </label>
                    <label>
                      <span>直播并发录制数</span>
                      <input
                        type="number"
                        min={1}
                        max={16}
                        value={settingsMaxLiveRecords}
                        onChange={(event) =>
                          setSettingsMaxLiveRecords(
                            Number(event.target.value) || 1,
                          )
                        }
                      />
                    </label>
                  </div>
                  <p className="muted small">
                    超出全局并发的任务会进入 FIFO 队列（状态「排队中」）。直播录制另受直播并发上限约束。
                  </p>
                  <PathPickerField
                    label="默认 cookies.txt（yt-dlp，可选）"
                    value={settingsCookiesFile}
                    emptyValueLabel="未配置 Cookie 文件"
                    selectButtonLabel="选择文件"
                    isSelecting={activeSettingsPathPicker === "cookies-file"}
                    isDisabled={isBusy || settingsPathSelectionIsActive}
                    onSelect={() =>
                      void handleSelectSettingsPath({
                        pickerId: "cookies-file",
                        title: "选择 Netscape cookies.txt",
                        currentPath: settingsCookiesFile,
                        selectionKind: "file",
                        filters: [
                          { name: "Cookies", extensions: ["txt"] },
                          { name: "所有文件", extensions: ["*"] },
                        ],
                        updatePath: setSettingsCookiesFile,
                      })
                    }
                    onClear={() => setSettingsCookiesFile("")}
                  />
                  <label>
                    <span>默认从浏览器导入 Cookie（yt-dlp，可选）</span>
                    <select
                      value={settingsCookiesBrowser}
                      onChange={(event) =>
                        setSettingsCookiesBrowser(event.target.value)
                      }
                    >
                      <option value="">不使用浏览器 Cookie</option>
                      {COOKIES_BROWSER_OPTIONS.map((option) => (
                        <option key={option.value} value={option.value}>
                          {option.label}
                        </option>
                      ))}
                    </select>
                  </label>
                  <p className="muted small">
                    仅路径/浏览器名会写入配置与任务元数据，不会保存 Cookie 原文。文件优先于浏览器；创建任务时可覆盖。
                  </p>
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
                    label="主模型文件（最常用，对应「当前使用 → 下面这个主模型文件」）"
                    value={settingsTranscribeModel}
                    emptyValueLabel="尚未选择 GGML 模型文件（转写会失败）"
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
                  <p className="muted small">
                    新手建议：只选一个常用模型（如 small/base），「当前使用」保持「下面这个主模型文件」。保存设置后，对新任务转写生效；已有任务需重跑转写。
                  </p>
                  <p className="muted small mono">
                    配置文件：{config?.config_path ?? "—"}
                  </p>
                </div>
              </article>
              </div>
                )}

                {settingsSection === "diagnostics" && (
              <div
                id="settings-panel-diagnostics"
                className="settings-section-panel"
                role="tabpanel"
                aria-labelledby="settings-nav-diagnostics"
              >
              <article className="card settings-wide">
                <h2 className="visually-hidden">系统诊断</h2>
                <p className="muted small">
                  聚合应用版本、sidecar、模型、磁盘与工作区健康。也可单独扫描工作区并修复可安全项。
                </p>
                <div className="detail-actions">
                  <button
                    type="button"
                    className="btn"
                    disabled={isBusy || isLoadingP4Tools}
                    onClick={() => void handleLoadSystemDiagnostics()}
                  >
                    {isLoadingP4Tools ? "诊断中…" : "完整系统诊断"}
                  </button>
                  <button
                    type="button"
                    className="btn secondary"
                    disabled={isBusy || isInspectingHealth || isRepairingHealth}
                    onClick={() => void handleInspectWorkspaceHealth()}
                  >
                    {isInspectingHealth ? "扫描中…" : "仅工作区扫描"}
                  </button>
                  <button
                    type="button"
                    className="btn secondary"
                    disabled={
                      isBusy ||
                      isInspectingHealth ||
                      isRepairingHealth ||
                      !workspaceHealth
                    }
                    onClick={() => void handleRepairWorkspaceHealth()}
                  >
                    {isRepairingHealth ? "修复中…" : "修复可安全项"}
                  </button>
                </div>
                {systemDiagnostics && (
                  <div className="stat-grid" style={{ marginTop: "1rem" }} aria-label="系统摘要">
                    <div className="stat-card">
                      <span className="stat-label">应用版本</span>
                      <strong>{systemDiagnostics.app_version}</strong>
                    </div>
                    <div
                      className={
                        systemDiagnostics.dependency.all_required_ready
                          ? "stat-card ok"
                          : "stat-card bad"
                      }
                    >
                      <span className="stat-label">必需依赖</span>
                      <strong>
                        {systemDiagnostics.dependency.all_required_ready
                          ? "就绪"
                          : "缺失"}
                      </strong>
                    </div>
                    <div
                      className={
                        systemDiagnostics.models.selected_exists
                          ? "stat-card ok"
                          : "stat-card"
                      }
                    >
                      <span className="stat-label">转写模型</span>
                      <strong>
                        {systemDiagnostics.models.selected_exists
                          ? "可用"
                          : "未就绪"}
                      </strong>
                    </div>
                    <div
                      className={
                        systemDiagnostics.disk_below_threshold
                          ? "stat-card bad"
                          : "stat-card ok"
                      }
                    >
                      <span className="stat-label">磁盘</span>
                      <strong>
                        {systemDiagnostics.free_disk_gb ?? "?"} GB
                      </strong>
                    </div>
                  </div>
                )}
                {workspaceHealth ? (
                  <div className="form-grid" style={{ marginTop: "1rem" }}>
                    <div className="stat-grid" aria-label="诊断摘要">
                      <div className="stat-card">
                        <span className="stat-label">剩余空间 (GB)</span>
                        <strong>
                          {workspaceHealth.free_disk_gb ?? "未知"}
                        </strong>
                      </div>
                      <div
                        className={
                          workspaceHealth.disk_below_threshold
                            ? "stat-card bad"
                            : "stat-card ok"
                        }
                      >
                        <span className="stat-label">磁盘阈值</span>
                        <strong>
                          {workspaceHealth.disk_below_threshold
                            ? "低于阈值"
                            : "正常"}
                        </strong>
                      </div>
                      <div className="stat-card">
                        <span className="stat-label">孤儿目录</span>
                        <strong>
                          {workspaceHealth.orphan_directories.length}
                        </strong>
                      </div>
                      <div className="stat-card bad">
                        <span className="stat-label">损坏任务</span>
                        <strong>{workspaceHealth.corrupt_jobs.length}</strong>
                      </div>
                      <div className="stat-card">
                        <span className="stat-label">中断 running</span>
                        <strong>
                          {workspaceHealth.interrupted_running_jobs.length}
                        </strong>
                      </div>
                      <div className="stat-card">
                        <span className="stat-label">残留 queued</span>
                        <strong>
                          {workspaceHealth.stale_queued_jobs.length}
                        </strong>
                      </div>
                      <div className="stat-card">
                        <span className="stat-label">空媒体索引</span>
                        <strong>
                          {workspaceHealth.empty_media_index_jobs.length}
                        </strong>
                      </div>
                    </div>
                    <p className="muted small mono">
                      工作区：{workspaceHealth.workspace_dir}
                      {" · "}
                      阈值：{workspaceHealth.min_free_disk_gb} GB
                    </p>
                    {workspaceHealth.repaired.length > 0 && (
                      <div>
                        <div className="theme-section-label">最近修复</div>
                        <ul className="muted small">
                          {workspaceHealth.repaired.map((item) => (
                            <li key={item}>{item}</li>
                          ))}
                        </ul>
                      </div>
                    )}
                    {[
                      {
                        title: "孤儿目录",
                        items: workspaceHealth.orphan_directories,
                      },
                      {
                        title: "损坏的 source.json",
                        items: workspaceHealth.corrupt_jobs,
                      },
                      {
                        title: "中断的 running 任务",
                        items: workspaceHealth.interrupted_running_jobs,
                      },
                      {
                        title: "残留 queued 任务",
                        items: workspaceHealth.stale_queued_jobs,
                      },
                      {
                        title: "空媒体索引",
                        items: workspaceHealth.empty_media_index_jobs,
                      },
                    ].map((section) =>
                      section.items.length > 0 ? (
                        <div key={section.title}>
                          <div className="theme-section-label">
                            {section.title}（{section.items.length}）
                          </div>
                          <ul className="muted small">
                            {section.items.map((finding) => (
                              <li key={`${finding.job_id_or_name}-${finding.path}`}>
                                <span className="mono">
                                  {finding.job_id_or_name}
                                </span>
                                {" — "}
                                {finding.message}
                              </li>
                            ))}
                          </ul>
                        </div>
                      ) : null,
                    )}
                  </div>
                ) : (
                  <p className="muted small" style={{ marginTop: "1rem" }}>
                    尚未扫描。点击「开始扫描」生成诊断报告。
                  </p>
                )}
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

              <article className="card settings-wide">
                <div className="log-header">
                  <div>
                    <h2>依赖安装向导</h2>
                    <p className="muted small">
                      探测必需/可选工具；缺失时给出安装指引。不自动执行系统级安装。
                    </p>
                  </div>
                  <button
                    type="button"
                    className="btn secondary small"
                    disabled={isBusy || isLoadingP4Tools}
                    onClick={() => void handleRefreshDependencyReport()}
                  >
                    {isLoadingP4Tools ? "检查中…" : "重新检查依赖"}
                  </button>
                </div>
                {dependencyReport ? (
                  <div className="dependency-list">
                    <p
                      className={
                        dependencyReport.all_required_ready
                          ? "muted small"
                          : "banner error inline"
                      }
                    >
                      {dependencyReport.all_required_ready
                        ? "必需工具均已就绪"
                        : `缺少：${dependencyReport.missing_required.join("、")}`}
                    </p>
                    {dependencyReport.items.map((item) => (
                      <div key={item.name} className="dependency-item">
                        <div className="dependency-item-header">
                          <strong>
                            {item.display_name}
                            {item.required ? "" : "（可选）"}
                          </strong>
                          <span
                            className={`pill source-${item.status.source}`}
                          >
                            {item.status.source}
                          </span>
                        </div>
                        <p className="muted small">{item.guidance}</p>
                        {item.status.source === "missing" ? (
                          <p className="small">{item.install_hint}</p>
                        ) : (
                          <p className="mono muted small">
                            {item.status.path ?? "—"} ·{" "}
                            {item.status.version ?? "无版本"}
                          </p>
                        )}
                      </div>
                    ))}
                  </div>
                ) : (
                  <p className="muted small">
                    点击「重新检查依赖」生成向导报告。
                  </p>
                )}
              </article>
              </div>
                )}

                {settingsSection === "models" && (
              <div
                id="settings-panel-models"
                className="settings-section-panel"
                role="tabpanel"
                aria-labelledby="settings-nav-models"
              >
              <article className="card settings-wide">
                <div className="log-header">
                  <div>
                    <h2>转写模型管理</h2>
                    <p className="muted small">
                      扫描已配置模型路径所在目录；校验当前选用文件是否存在。不自动下载大体积模型。
                    </p>
                  </div>
                  <div className="detail-actions">
                    <button
                      type="button"
                      className="btn secondary small"
                      disabled={isBusy || isLoadingP4Tools}
                      onClick={() => void handleRefreshModelInventory()}
                    >
                      扫描模型
                    </button>
                    <button
                      type="button"
                      className="btn secondary small"
                      disabled={isBusy || isLoadingP4Tools}
                      onClick={() => void handleOpenModelDirectory()}
                    >
                      打开目录
                    </button>
                  </div>
                </div>
                {modelInventory ? (
                  <>
                    <dl className="meta-list">
                      <div>
                        <dt>当前选用</dt>
                        <dd className="mono">
                          {modelInventory.selected_path ?? "未配置"}
                        </dd>
                      </div>
                      <div>
                        <dt>文件状态</dt>
                        <dd>
                          {modelInventory.selected_exists
                            ? "存在"
                            : "不存在 / 未配置"}
                        </dd>
                      </div>
                      <div>
                        <dt>扫描目录</dt>
                        <dd className="mono muted small">
                          {modelInventory.scan_directories.length > 0
                            ? modelInventory.scan_directories.join("；")
                            : "无"}
                        </dd>
                      </div>
                    </dl>
                    <div className="model-file-list">
                      {modelInventory.models.length === 0 ? (
                        <p className="muted small">
                          未发现模型文件。请在「工作区与流水线」中配置
                          transcribe 模型路径。
                        </p>
                      ) : (
                        modelInventory.models.map((model) => (
                          <div
                            key={model.path}
                            className={
                              model.is_selected
                                ? "model-file-row selected"
                                : "model-file-row"
                            }
                          >
                            <div>
                              <strong>{model.file_name}</strong>
                              {model.is_selected && (
                                <span className="pill">当前</span>
                              )}
                              <span className="muted small">
                                {" "}
                                {model.kind} ·{" "}
                                {(model.size_bytes / (1024 * 1024)).toFixed(1)}{" "}
                                MB
                              </span>
                            </div>
                            <div className="mono muted small">{model.path}</div>
                          </div>
                        ))
                      )}
                    </div>
                  </>
                ) : (
                  <p className="muted small">点击「扫描模型」加载清单。</p>
                )}
              </article>
              </div>
                )}

                {settingsSection === "capacity" && (
                  <div
                    id="settings-panel-capacity"
                    className="settings-section-panel"
                    role="tabpanel"
                    aria-labelledby="settings-nav-capacity"
                  >
                    <CapacityPanel
                      onOpenJob={(jobId) => {
                        setView("jobs");
                        void loadJobDetail(jobId);
                      }}
                      onJobsChanged={() => {
                        void (async () => {
                          const nextJobs = await listJobs();
                          setJobs(nextJobs);
                          if (selectedJobIdRef.current) {
                            await loadJobDetail(
                              selectedJobIdRef.current,
                              logNameRef.current,
                              false,
                            );
                          }
                        })();
                      }}
                      onError={setErrorMessage}
                      onStatus={setStatusMessage}
                    />
                  </div>
                )}

                {settingsSection === "backup" && (
              <div
                id="settings-panel-backup"
                className="settings-section-panel"
                role="tabpanel"
                aria-labelledby="settings-nav-backup"
              >
              <article className="card settings-wide">
                <h2>配置导入 / 导出</h2>
                <p className="muted small">
                  导出 Provider、模板、分组、流水线默认与 sidecar 路径等。
                  <strong>默认剥离 API Key</strong>
                  ；导入时同 ID 的本地 Key 会保留。
                </p>
                <div className="detail-actions">
                  <button
                    type="button"
                    className="btn"
                    disabled={isBusy || isLoadingP4Tools}
                    onClick={() => void handleExportAppConfig()}
                  >
                    导出到剪贴板
                  </button>
                  <button
                    type="button"
                    className="btn secondary"
                    disabled={isBusy || isLoadingP4Tools}
                    onClick={() => void handleImportAppConfigFromClipboard()}
                  >
                    从剪贴板导入
                  </button>
                </div>
              </article>
              <article className="card settings-wide">
                <h2>检查应用更新</h2>
                <p className="muted small">
                  从 GitHub Releases 查询最新版本；有安装包时可
                  <strong>应用内下载并静默安装</strong>
                  （需确认一次，无安装向导界面；安装完成后
                  <strong>自动重启</strong>
                  ）。默认仓库 627157746/video-tool。
                </p>
                <div className="detail-actions">
                  <button
                    type="button"
                    className="btn"
                    disabled={
                      isBusy || isLoadingP4Tools || isInstallingUpdate
                    }
                    onClick={() => void handleCheckAppUpdate()}
                  >
                    {isLoadingP4Tools ? "检查中…" : "检查更新"}
                  </button>
                  <button
                    type="button"
                    className="btn"
                    disabled={
                      isBusy ||
                      isInstallingUpdate ||
                      !updateCheckResult?.update_available ||
                      !updateCheckResult?.can_install
                    }
                    onClick={() => void handleInstallAppUpdate()}
                  >
                    {isInstallingUpdate
                      ? "下载/安装中…"
                      : "下载并安装更新"}
                  </button>
                  <button
                    type="button"
                    className="btn secondary"
                    disabled={isBusy || isInstallingUpdate}
                    onClick={() => void handleOpenReleasePage()}
                  >
                    打开发布页
                  </button>
                </div>
                {updateCheckResult && (
                  <div className="form-grid" style={{ marginTop: "0.75rem" }}>
                    <p
                      className={
                        updateCheckResult.update_available
                          ? "small status-succeeded"
                          : "small"
                      }
                    >
                      {updateCheckResult.message}
                    </p>
                    <p className="muted small mono">
                      当前 {updateCheckResult.current_version}
                      {updateCheckResult.latest_version
                        ? ` · 远端 ${updateCheckResult.latest_version}`
                        : ""}
                      {updateCheckResult.update_available ? " · 可更新" : ""}
                      {updateCheckResult.can_install
                        ? " · 可应用内安装"
                        : ""}
                    </p>
                    {updateCheckResult.installer_name && (
                      <p className="muted small mono">
                        安装包 {updateCheckResult.installer_name}
                        {typeof updateCheckResult.installer_size_bytes ===
                        "number"
                          ? ` · ${Math.max(
                              1,
                              Math.round(
                                updateCheckResult.installer_size_bytes /
                                  (1024 * 1024),
                              ),
                            )} MB`
                          : ""}
                      </p>
                    )}
                    {updateCheckResult.release_page_url && (
                      <p className="muted small mono">
                        {updateCheckResult.release_page_url}
                      </p>
                    )}
                    {updateProgress && (
                      <p className="small">
                        {updateProgress.message}
                        {typeof updateProgress.percent === "number"
                          ? `（${updateProgress.percent.toFixed(1)}%）`
                          : ""}
                      </p>
                    )}
                    {updateCheckResult.release_notes && (
                      <pre className="artifact-view" style={{ maxHeight: "10rem" }}>
                        {updateCheckResult.release_notes}
                      </pre>
                    )}
                  </div>
                )}
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

                {settingsSection === "groups" && (
              <div
                id="settings-panel-groups"
                className="settings-section-panel"
                role="tabpanel"
                aria-labelledby="settings-nav-groups"
              >
              <article className="card settings-wide">
                <div className="log-header">
                  <div>
                    <h2 className="visually-hidden">任务分组</h2>
                    <p className="muted small settings-collection-hint">
                      共 {groupDrafts.length} 个分组；列表顺序即筛选芯片顺序。删除后，原归属任务将变为未分组。
                    </p>
                  </div>
                  <button
                    type="button"
                    className="btn secondary small"
                    onClick={handleAddGroup}
                  >
                    添加分组
                  </button>
                </div>
                <div className="settings-split">
                  <div
                    className="settings-item-list"
                    role="listbox"
                    aria-label="任务分组列表"
                  >
                    {groupDrafts.length === 0 ? (
                      <div className="settings-item-empty muted">
                        还没有分组，点击「添加分组」开始整理任务。
                      </div>
                    ) : (
                      groupDrafts.map((groupEntry, index) => {
                        const isSelected =
                          selectedGroupDraft?.index === index;
                        return (
                          <button
                            key={groupEntry.id}
                            type="button"
                            role="option"
                            aria-selected={isSelected}
                            className={
                              isSelected
                                ? "settings-item selected"
                                : "settings-item"
                            }
                            onClick={() => setSelectedGroupIndex(index)}
                          >
                            <div className="settings-item-top">
                              <strong>
                                {groupEntry.name.trim() || "未命名分组"}
                              </strong>
                            </div>
                            <div
                              className="settings-item-sub muted small mono"
                              title={groupEntry.id}
                            >
                              {groupEntry.id}
                            </div>
                          </button>
                        );
                      })
                    )}
                  </div>
                  <div className="settings-item-detail">
                    {selectedGroupDraft ? (
                      <div className="profile-editor settings-detail-editor">
                        <div className="settings-detail-title">
                          <strong>
                            {selectedGroupDraft.group.name.trim() ||
                              "未命名分组"}
                          </strong>
                          <span className="muted small mono">
                            {selectedGroupDraft.group.id}
                          </span>
                        </div>
                        <label>
                          <span>名称</span>
                          <input
                            value={selectedGroupDraft.group.name}
                            maxLength={80}
                            onChange={(event) =>
                              updateGroupDraft(
                                selectedGroupDraft.index,
                                (groupEntry) => ({
                                  ...groupEntry,
                                  name: event.target.value,
                                }),
                              )
                            }
                            placeholder="例如：学习笔记"
                          />
                        </label>
                        <p className="muted small">
                          分组 ID 在创建后保持稳定；重命名只改显示名称，不会打散已归属任务。
                        </p>
                        <div className="detail-actions">
                          <button
                            type="button"
                            className="btn secondary small"
                            disabled={selectedGroupDraft.index === 0}
                            onClick={() =>
                              handleMoveGroup(selectedGroupDraft.index, -1)
                            }
                          >
                            上移
                          </button>
                          <button
                            type="button"
                            className="btn secondary small"
                            disabled={
                              selectedGroupDraft.index >=
                              groupDrafts.length - 1
                            }
                            onClick={() =>
                              handleMoveGroup(selectedGroupDraft.index, 1)
                            }
                          >
                            下移
                          </button>
                          <button
                            type="button"
                            className="btn danger small"
                            onClick={() =>
                              handleDeleteGroup(selectedGroupDraft.index)
                            }
                          >
                            删除
                          </button>
                        </div>
                      </div>
                    ) : (
                      <div className="settings-item-empty muted">
                        选择或添加一个分组以开始编辑。
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

      {view === "settings" && (
        <div className="settings-fab" role="group" aria-label="设置操作">
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
      )}

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
                      ? "URL / 多行批量 / 抖音分享文案"
                      : "URL / 流地址"}
                  </span>
                  {createMode === "download" ? (
                    <textarea
                      ref={downloadUrlInputRef}
                      value={formUrl}
                      onChange={(event) => setFormUrl(event.target.value)}
                      placeholder={
                        "一行一个链接可批量创建多个任务。\n也可粘贴单条抖音分享文案（含 v.douyin.com 短链）。最佳努力下载。"
                      }
                      rows={6}
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

              {createMode === "download" && (
                <>
                  <label>
                    <span>Cookie 辅助（yt-dlp）</span>
                    <select
                      value={formCookiesMode}
                      onChange={(event) => setFormCookiesMode(event.target.value)}
                    >
                      <option value="inherit">跟随全局默认</option>
                      <option value="none">不使用 Cookie</option>
                      <option value="file">cookies.txt 文件</option>
                      <option value="browser">从浏览器导入</option>
                    </select>
                  </label>
                  {formCookiesMode === "file" && (
                    <PathPickerField
                      label="cookies.txt 路径"
                      value={formCookiesFile}
                      emptyValueLabel="请选择 Netscape cookies.txt"
                      selectButtonLabel="选择文件"
                      isSelecting={activeSettingsPathPicker === "cookies-file"}
                      isDisabled={isBusy || settingsPathSelectionIsActive}
                      onSelect={() =>
                        void handleSelectSettingsPath({
                          pickerId: "cookies-file",
                          title: "选择 Netscape cookies.txt",
                          currentPath: formCookiesFile,
                          selectionKind: "file",
                          filters: [
                            { name: "Cookies", extensions: ["txt"] },
                            { name: "所有文件", extensions: ["*"] },
                          ],
                          updatePath: setFormCookiesFile,
                        })
                      }
                      onClear={() => setFormCookiesFile("")}
                    />
                  )}
                  {formCookiesMode === "browser" && (
                    <label>
                      <span>浏览器</span>
                      <select
                        value={formCookiesBrowser}
                        onChange={(event) =>
                          setFormCookiesBrowser(event.target.value)
                        }
                      >
                        {COOKIES_BROWSER_OPTIONS.map((option) => (
                          <option key={option.value} value={option.value}>
                            {option.label}
                          </option>
                        ))}
                      </select>
                    </label>
                  )}
                  <p className="muted small">
                    仅作用于 yt-dlp 路径；任务只记录模式/路径/浏览器名，不含 Cookie 原文。
                  </p>
                </>
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

              <label>
                <span>分组（可选）</span>
                <select
                  value={formGroup}
                  onChange={(event) => setFormGroup(event.target.value)}
                  title={
                    managedJobGroups.length === 0
                      ? "请先在设置 → 任务分组中创建分组"
                      : undefined
                  }
                >
                  <option value="">未分组</option>
                  {managedJobGroups.map((groupEntry) => (
                    <option key={groupEntry.id} value={groupEntry.id}>
                      {groupEntry.name}
                    </option>
                  ))}
                </select>
                {managedJobGroups.length === 0 && (
                  <span className="muted small">
                    暂无分组目录。可在「设置 → 任务分组」中添加。
                  </span>
                )}
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

              {(createMode === "download" || createMode === "live") && (
                <fieldset className="radio-fieldset">
                  <legend>保存形态</legend>
                  <div className="checkbox-row">
                    <label className="checkbox">
                      <input
                        type="radio"
                        name="media-save-mode"
                        value="video"
                        checked={formMediaSaveMode === "video"}
                        onChange={() => setFormMediaSaveMode("video")}
                      />
                      保存视频
                    </label>
                    <label className="checkbox">
                      <input
                        type="radio"
                        name="media-save-mode"
                        value="audio"
                        checked={formMediaSaveMode === "audio"}
                        onChange={() => setFormMediaSaveMode("audio")}
                      />
                      保存音频
                    </label>
                  </div>
                  <p className="muted small">
                    二选一。保存音频时会尽量直接拉取/输出音频，不会先完整下载视频再转换。
                  </p>
                </fieldset>
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
                        if (config?.default_auto_chapterize ?? true) {
                          setAutoChapterize(true);
                        }
                      }
                    }}
                  />
                  转写后自动总结
                </label>
                <label className="checkbox">
                  <input
                    type="checkbox"
                    checked={autoChapterize}
                    disabled={autoSummarize && autoChapterize}
                    onChange={(event) => {
                      const checked = event.target.checked;
                      setAutoChapterize(checked);
                      if (checked) {
                        setAutoTranscribe(true);
                      }
                    }}
                  />
                  合并后生成章节大纲
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
                  <div className="multi-template-picker">
                    <span>总结模板（可多选）</span>
                    <div className="multi-template-list">
                      {config?.templates.map((template) => {
                        const checked = formTemplateIds.includes(template.id);
                        const orderIndex = formTemplateIds.indexOf(template.id);
                        return (
                          <label
                            key={template.id}
                            className="multi-template-item"
                          >
                            <input
                              type="checkbox"
                              checked={checked}
                              onChange={() => {
                                setFormTemplateIds((previous) => {
                                  if (previous.includes(template.id)) {
                                    return previous.filter(
                                      (id) => id !== template.id,
                                    );
                                  }
                                  return [...previous, template.id];
                                });
                                setFormTemplateId((previous) => {
                                  if (formTemplateIds.includes(template.id)) {
                                    const next = formTemplateIds.filter(
                                      (id) => id !== template.id,
                                    );
                                    return next[0] ?? "";
                                  }
                                  return previous || template.id;
                                });
                              }}
                            />
                            <span>
                              {orderIndex >= 0 ? `${orderIndex + 1}. ` : ""}
                              {template.name}
                              {template.id === config.default_template_id
                                ? "（全局默认）"
                                : ""}
                            </span>
                          </label>
                        );
                      })}
                    </div>
                  </div>
                  <p className="muted small">
                    不选模板则跟随全局默认；多选时按勾选顺序依次生成，首项为主产物
                    summary/summary.md。
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

      <ConfirmDialogHost />
    </div>
  );
}

export default App;

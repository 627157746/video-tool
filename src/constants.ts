export type CreateMode = "download" | "live" | "import" | null;
export type MainView = "jobs" | "settings";
export type SettingsSection =
  | "appearance"
  | "pipeline"
  | "providers"
  | "templates"
  | "groups"
  | "sidecars"
  | "models"
  | "backup"
  | "diagnostics";
export type JobDetailSection =
  | "overview"
  | "pipeline"
  | "summarize"
  | "segments"
  | "logs"
  | "summary"
  | "transcript";
export type SettingsPathPickerId =
  | "workspace"
  | "transcribe-model"
  | "model-preset-speed"
  | "model-preset-balanced"
  | "model-preset-quality"
  | "cookies-file"
  | "yt-dlp"
  | "ffmpeg"
  | "ffprobe"
  | "streamlink"
  | "transcribe";

/** yt-dlp --cookies-from-browser browser ids shown in settings / create form. */
export const COOKIES_BROWSER_OPTIONS: ReadonlyArray<{
  value: string;
  label: string;
}> = [
  { value: "chrome", label: "Chrome" },
  { value: "edge", label: "Edge" },
  { value: "firefox", label: "Firefox" },
  { value: "brave", label: "Brave" },
  { value: "opera", label: "Opera" },
  { value: "vivaldi", label: "Vivaldi" },
  { value: "chromium", label: "Chromium" },
];
export type LogName =
  | "download"
  | "record"
  | "transcribe"
  | "merge_transcript"
  | "chapterize"
  | "summarize";

export const SETTINGS_SECTIONS: ReadonlyArray<{
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
    description: "工作区、并发队列、默认 Provider/模板、转写与磁盘保护等全局默认。",
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
    id: "groups",
    label: "任务分组",
    description: "维护分组目录：新增、重命名、排序、删除；任务挂接分组 ID。",
  },
  {
    id: "sidecars",
    label: "Sidecar 工具",
    description:
      "可选覆盖可执行路径、依赖向导与解析结果。解析顺序：内置 → 配置路径 → PATH。",
  },
  {
    id: "models",
    label: "转写模型",
    description: "扫描 GGML 模型目录、校验当前选用文件是否存在，并打开目录。",
  },
  {
    id: "backup",
    label: "配置备份",
    description:
      "导出/导入 Provider、模板、分组与流水线默认；默认剥离 API Key。检查应用更新。",
  },
  {
    id: "diagnostics",
    label: "系统诊断",
    description:
      "聚合版本、sidecar、模型、磁盘与工作区健康；扫描并修复可安全项。",
  },
];

export const JOB_DETAIL_SECTIONS: ReadonlyArray<{
  id: JobDetailSection;
  label: string;
  description: string;
}> = [
  {
    id: "overview",
    label: "来源概览",
    description: "任务来源、进度、媒体文件与所用工具。",
  },
  {
    id: "pipeline",
    label: "流水线",
    description: "查看各步骤状态，按步骤重试或继续运行。",
  },
  {
    id: "summarize",
    label: "总结配置",
    description: "为本任务指定 Provider、模型与总结模板；空值跟随全局默认。",
  },
  {
    id: "segments",
    label: "总结选段",
    description: "勾选参与合并与总结的转写分段；改选后需重跑合并与总结。",
  },
  {
    id: "logs",
    label: "日志",
    description: "按阶段查看下载、录制、转写、合并与总结日志。",
  },
  {
    id: "summary",
    label: "Markdown 总结",
    description: "查看本任务生成的 Markdown 总结文档。",
  },
  {
    id: "transcript",
    label: "合并字幕",
    description: "查看合并后的字幕/转写原文。",
  },
];

/** whisper.cpp `-l` codes; `auto` means omit `-l` and let whisper detect. */
export const TRANSCRIBE_LANGUAGE_OPTIONS: ReadonlyArray<{
  value: string;
  label: string;
}> = [
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

export const LOG_NAMES: LogName[] = [
  "download",
  "record",
  "transcribe",
  "merge_transcript",
  "chapterize",
  "summarize",
];

use crate::error::{AppError, AppResult};
use crate::models::SaveConfigRequest;
use crate::storage;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

const APP_CONFIG_DIR_NAME: &str = "video-tool";
const APP_CONFIG_FILE_NAME: &str = "config.json";

/// User-managed job grouping entry stored in app config.
/// Jobs store `Job.group` as this entry's `id` (legacy free-text names are
/// resolved by name when possible).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct JobGroupDefinition {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderProfile {
    pub id: String,
    pub name: String,
    /// `openai` | `anthropic`
    pub protocol: String,
    pub base_url: String,
    pub api_key: Option<String>,
    pub api_key_env: Option<String>,
    /// Default model used when a job does not override `pipeline.model`.
    pub default_model: String,
    /// Available models under this provider (same base URL / API key).
    /// Older configs without this field deserialize as empty and are normalized
    /// from `default_model` on load/validate.
    #[serde(default)]
    pub models: Vec<String>,
    #[serde(default)]
    pub extra_headers: Vec<(String, String)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SummaryTemplate {
    pub id: String,
    pub name: String,
    pub system_prompt: String,
    pub user_template: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SidecarPaths {
    pub ffmpeg: Option<String>,
    pub ffprobe: Option<String>,
    pub yt_dlp: Option<String>,
    pub streamlink: Option<String>,
    pub transcribe: Option<String>,
}

/// Global terminology / hotword list for transcription quality (v0.2 P2).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct GlossaryReplacement {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GlossaryConfig {
    /// Words/phrases preferred during ASR (also used to build whisper `-prompt`).
    #[serde(default)]
    pub hotwords: Vec<String>,
    /// Optional whole-string `from → to` replacements after merge.
    #[serde(default)]
    pub replacements: Vec<GlossaryReplacement>,
    /// When true, pass hotwords (and replacement targets) as whisper initial prompt.
    #[serde(default = "default_true")]
    pub apply_as_whisper_prompt: bool,
    /// When true, apply `replacements` to merged plain text.
    #[serde(default = "default_true")]
    pub apply_post_replace: bool,
}

impl Default for GlossaryConfig {
    fn default() -> Self {
        Self {
            hotwords: Vec::new(),
            replacements: Vec::new(),
            apply_as_whisper_prompt: true,
            apply_post_replace: true,
        }
    }
}

/// Local whisper model path presets (user supplies files on disk).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct TranscribeModelPresets {
    pub speed: Option<String>,
    pub balanced: Option<String>,
    pub quality: Option<String>,
}

fn default_true() -> bool {
    true
}

fn default_transcribe_model_preset() -> String {
    "custom".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub workspace_dir: String,
    pub default_segment_minutes: u32,
    pub default_auto_transcribe: bool,
    pub default_auto_summarize: bool,
    pub default_provider_profile_id: Option<String>,
    pub default_template_id: Option<String>,
    pub proxy_url: Option<String>,
    pub min_free_disk_gb: u32,
    pub live_reconnect_attempts: u32,
    #[serde(default = "default_max_context_chars")]
    pub max_context_chars: usize,
    /// Max jobs running at once; additional accepted work waits as `queued`.
    #[serde(default = "default_max_concurrent_jobs")]
    pub max_concurrent_jobs: u32,
    /// Max concurrent live-record captures (also counts toward max_concurrent_jobs).
    #[serde(default = "default_max_live_records")]
    pub max_live_records: u32,
    /// Default Netscape cookies.txt path for yt-dlp (path only, never contents).
    #[serde(default)]
    pub download_cookies_file: Option<String>,
    /// Default browser for yt-dlp `--cookies-from-browser` when file is unset.
    #[serde(default)]
    pub download_cookies_from_browser: Option<String>,
    #[serde(default)]
    pub transcribe_model: Option<String>,
    #[serde(default = "default_transcribe_language")]
    pub transcribe_language: String,
    /// Active preset key: `speed` | `balanced` | `quality` | `custom`.
    #[serde(default = "default_transcribe_model_preset")]
    pub transcribe_model_preset: String,
    /// Optional model paths for speed / balanced / quality presets.
    #[serde(default)]
    pub transcribe_model_presets: TranscribeModelPresets,
    /// Global glossary (hotwords + replacements). Older configs omit → empty.
    #[serde(default)]
    pub glossary: GlossaryConfig,
    /// When true, auto pipeline runs Chapterize after merge when summarizing.
    #[serde(default = "default_true")]
    pub default_auto_chapterize: bool,
    pub sidecar_paths: SidecarPaths,
    pub providers: Vec<ProviderProfile>,
    pub templates: Vec<SummaryTemplate>,
    /// Ordered catalog of custom job groups. Empty on older configs.
    #[serde(default)]
    pub job_groups: Vec<JobGroupDefinition>,
}

impl Default for AppConfig {
    fn default() -> Self {
        let workspace_dir = default_workspace_dir().to_string_lossy().replace('\\', "/");

        Self {
            workspace_dir,
            default_segment_minutes: 30,
            default_auto_transcribe: true,
            default_auto_summarize: false,
            default_provider_profile_id: Some("example-openai".to_string()),
            default_template_id: Some("default-overview".to_string()),
            proxy_url: None,
            min_free_disk_gb: 5,
            live_reconnect_attempts: 3,
            max_context_chars: default_max_context_chars(),
            max_concurrent_jobs: default_max_concurrent_jobs(),
            max_live_records: default_max_live_records(),
            download_cookies_file: None,
            download_cookies_from_browser: None,
            transcribe_model: None,
            transcribe_language: default_transcribe_language(),
            transcribe_model_preset: default_transcribe_model_preset(),
            transcribe_model_presets: TranscribeModelPresets::default(),
            glossary: GlossaryConfig::default(),
            default_auto_chapterize: true,
            sidecar_paths: SidecarPaths {
                ffmpeg: None,
                ffprobe: None,
                yt_dlp: None,
                streamlink: None,
                transcribe: None,
            },
            job_groups: Vec::new(),
            providers: vec![
                ProviderProfile {
                    id: "example-openai".to_string(),
                    name: "OpenAI-compatible (example)".to_string(),
                    protocol: "openai".to_string(),
                    base_url: "https://api.openai.com/v1".to_string(),
                    api_key: None,
                    api_key_env: Some("OPENAI_API_KEY".to_string()),
                    default_model: "gpt-4o-mini".to_string(),
                    models: vec!["gpt-4o-mini".to_string(), "gpt-4o".to_string()],
                    extra_headers: vec![],
                },
                ProviderProfile {
                    id: "example-anthropic".to_string(),
                    name: "Anthropic (example)".to_string(),
                    protocol: "anthropic".to_string(),
                    base_url: "https://api.anthropic.com".to_string(),
                    api_key: None,
                    api_key_env: Some("ANTHROPIC_API_KEY".to_string()),
                    default_model: "claude-sonnet-4-5".to_string(),
                    models: vec![
                        "claude-sonnet-4-5".to_string(),
                        "claude-opus-4-5".to_string(),
                    ],
                    extra_headers: vec![],
                },
            ],
            templates: default_templates(),
        }
    }
}

fn default_templates() -> Vec<SummaryTemplate> {
    vec![
        SummaryTemplate {
            id: "default-overview".to_string(),
            name: "内容概览".to_string(),
            system_prompt: "你是一个严谨的中文内容助理，根据字幕整理结构化总结。直接输出 Markdown 正文，不要用代码围栏（```）包裹整篇回答。".to_string(),
            user_template: concat!(
                "标题：{{title}}\n",
                "来源：{{source_url}}\n",
                "时长：{{duration}}\n\n",
                "请根据以下字幕输出 Markdown 总结，包含：概述、要点列表、时间线（如有）、可执行事项。\n",
                "直接输出 Markdown 正文，不要用 ```markdown 代码块包裹整篇。\n\n",
                "{{transcript}}\n"
            )
            .to_string(),
        },
        SummaryTemplate {
            id: "tutorial-keypoints".to_string(),
            name: "教程要点".to_string(),
            system_prompt: "你擅长把教程口播提炼成可操作步骤。直接输出 Markdown 正文，不要用代码围栏（```）包裹整篇回答。".to_string(),
            user_template: concat!(
                "视频：{{title}}\n\n",
                "请提炼：目标读者、前置条件、分步操作、常见坑、一句话结论。\n",
                "直接输出 Markdown 正文，不要用 ```markdown 代码块包裹整篇。\n\n",
                "{{transcript}}\n"
            )
            .to_string(),
        },
        SummaryTemplate {
            id: "live-talk-notes".to_string(),
            name: "直播口播纪要".to_string(),
            system_prompt: "你擅长整理直播口播为会议纪要风格笔记。直接输出 Markdown 正文，不要用代码围栏（```）包裹整篇回答。".to_string(),
            user_template: concat!(
                "场次：{{title}}\n来源：{{source_url}}\n\n",
                "请输出：主题、关键发言、承诺/行动项、待核实信息。\n",
                "直接输出 Markdown 正文，不要用 ```markdown 代码块包裹整篇。\n\n",
                "{{transcript}}\n"
            )
            .to_string(),
        },
    ]
}

fn default_workspace_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(APP_CONFIG_DIR_NAME)
        .join("workspace")
}

pub fn app_config_dir() -> AppResult<PathBuf> {
    let config_dir = dirs::config_dir()
        .ok_or_else(|| AppError::message("无法定位系统配置目录"))?
        .join(APP_CONFIG_DIR_NAME);
    fs::create_dir_all(&config_dir)?;
    Ok(config_dir)
}

pub fn app_config_path() -> AppResult<PathBuf> {
    Ok(app_config_dir()?.join(APP_CONFIG_FILE_NAME))
}

impl AppConfig {
    pub fn load_or_init() -> AppResult<Self> {
        let path = app_config_path()?;
        if path.exists() {
            let raw = fs::read_to_string(&path)?;
            let mut config: AppConfig = serde_json::from_str(&raw)?;
            config.normalize_provider_models();
            config.normalize_job_groups();
            config.validate()?;
            return Ok(config);
        }

        let mut config = AppConfig::default();
        config.normalize_provider_models();
        config.normalize_job_groups();
        config.validate()?;
        config.save()?;
        Ok(config)
    }

    pub fn save(&self) -> AppResult<()> {
        let path = app_config_path()?;
        storage::write_json_atomically(&path, self)?;
        Ok(())
    }

    /// Ensure every provider has a non-empty `models` list that includes
    /// `default_model`. Used for backward-compatible configs and draft saves.
    pub fn normalize_provider_models(&mut self) {
        for provider in &mut self.providers {
            normalize_provider_models(provider);
        }
    }

    pub fn candidate_with_update(&self, request: SaveConfigRequest) -> AppResult<Self> {
        let mut candidate = self.clone();

        if let Some(workspace_dir) = request.workspace_dir {
            let trimmed = workspace_dir.trim();
            if trimmed.is_empty() {
                return Err(AppError::message("工作区路径不能为空"));
            }
            candidate.workspace_dir = trimmed.replace('\\', "/");
        }
        if let Some(value) = request.default_segment_minutes {
            candidate.default_segment_minutes = value;
        }
        if let Some(value) = request.default_auto_transcribe {
            candidate.default_auto_transcribe = value;
        }
        if let Some(value) = request.default_auto_summarize {
            candidate.default_auto_summarize = value;
        }
        if candidate.default_auto_summarize {
            candidate.default_auto_transcribe = true;
        }
        if let Some(value) = request.default_provider_profile_id {
            candidate.default_provider_profile_id = empty_to_none(value);
        }
        if let Some(value) = request.default_template_id {
            candidate.default_template_id = empty_to_none(value);
        }
        if let Some(value) = request.proxy_url {
            candidate.proxy_url = empty_to_none(value);
        }
        if let Some(value) = request.min_free_disk_gb {
            candidate.min_free_disk_gb = value;
        }
        if let Some(value) = request.live_reconnect_attempts {
            candidate.live_reconnect_attempts = value;
        }
        if let Some(value) = request.max_context_chars {
            candidate.max_context_chars = value;
        }
        if let Some(value) = request.max_concurrent_jobs {
            candidate.max_concurrent_jobs = value;
        }
        if let Some(value) = request.max_live_records {
            candidate.max_live_records = value;
        }
        if let Some(value) = request.download_cookies_file {
            candidate.download_cookies_file = empty_to_none(value);
        }
        if let Some(value) = request.download_cookies_from_browser {
            candidate.download_cookies_from_browser = empty_to_none(value);
        }
        if let Some(value) = request.transcribe_model {
            candidate.transcribe_model = empty_to_none(value);
        }
        if let Some(value) = request.transcribe_language {
            candidate.transcribe_language = if value.trim().is_empty() {
                "auto".to_string()
            } else {
                value.trim().to_string()
            };
        }
        if let Some(value) = request.transcribe_model_preset {
            let trimmed = value.trim().to_ascii_lowercase();
            candidate.transcribe_model_preset = match trimmed.as_str() {
                "speed" | "balanced" | "quality" | "custom" => trimmed,
                _ => "custom".to_string(),
            };
        }
        if let Some(presets) = request.transcribe_model_presets {
            candidate.transcribe_model_presets = TranscribeModelPresets {
                speed: empty_to_none(presets.speed.unwrap_or_default()),
                balanced: empty_to_none(presets.balanced.unwrap_or_default()),
                quality: empty_to_none(presets.quality.unwrap_or_default()),
            };
        }
        if let Some(glossary) = request.glossary {
            candidate.glossary = normalize_glossary(glossary);
        }
        if let Some(value) = request.default_auto_chapterize {
            candidate.default_auto_chapterize = value;
        }
        // When a named preset is active, sync `transcribe_model` from that slot.
        candidate.apply_transcribe_model_preset();
        if let Some(paths) = request.sidecar_paths {
            candidate.sidecar_paths = paths;
        }
        if let Some(mut providers) = request.providers {
            for provider in &mut providers {
                let existing_provider = self
                    .providers
                    .iter()
                    .find(|existing| existing.id == provider.id);
                if provider
                    .api_key
                    .as_ref()
                    .is_none_or(|api_key| api_key.trim().is_empty())
                {
                    provider.api_key =
                        existing_provider.and_then(|existing| existing.api_key.clone());
                }
                if let Some(existing_provider) = existing_provider {
                    for (header_name, header_value) in &mut provider.extra_headers {
                        let should_preserve = is_sensitive_header_name(header_name)
                            && (header_value.trim().is_empty() || header_value == "***REDACTED***");
                        if should_preserve {
                            if let Some((_, existing_value)) = existing_provider
                                .extra_headers
                                .iter()
                                .find(|(existing_name, _)| {
                                    existing_name.eq_ignore_ascii_case(header_name)
                                })
                            {
                                *header_value = existing_value.clone();
                            }
                        }
                    }
                }
            }
            candidate.providers = providers;
            candidate.normalize_provider_models();
        }
        if let Some(templates) = request.templates {
            candidate.templates = templates;
        }
        if let Some(job_groups) = request.job_groups {
            candidate.job_groups = job_groups;
        }

        candidate.normalize_job_groups();
        candidate.validate()?;
        Ok(candidate)
    }

    pub fn validate(&self) -> AppResult<()> {
        if self.workspace_dir.trim().is_empty() {
            return Err(AppError::message("工作区路径不能为空"));
        }
        if !(1..=1_440).contains(&self.default_segment_minutes) {
            return Err(AppError::message("默认直播分段必须在 1 到 1440 分钟之间"));
        }
        if self.live_reconnect_attempts > 100 {
            return Err(AppError::message("直播重连次数不能超过 100"));
        }
        if self.min_free_disk_gb > 1_000_000 {
            return Err(AppError::message("磁盘保护阈值不能超过 1000000 GB"));
        }
        if !(1_000..=10_000_000).contains(&self.max_context_chars) {
            return Err(AppError::message(
                "总结最大输入字符数必须在 1000 到 10000000 之间",
            ));
        }
        if !(1..=64).contains(&self.max_concurrent_jobs) {
            return Err(AppError::message("全局并发任务数必须在 1 到 64 之间"));
        }
        if !(1..=16).contains(&self.max_live_records) {
            return Err(AppError::message("直播并发录制数必须在 1 到 16 之间"));
        }
        if self.max_live_records > self.max_concurrent_jobs {
            return Err(AppError::message("直播并发录制数不能大于全局并发任务数"));
        }
        if let Some(browser) = self.download_cookies_from_browser.as_deref() {
            validate_cookies_browser(browser)?;
        }
        if let Some(proxy_url) = self.proxy_url.as_deref() {
            validate_url(
                proxy_url,
                &["http", "https", "socks4", "socks5", "socks5h"],
                "代理 URL",
            )?;
        }
        if self.providers.is_empty() {
            return Err(AppError::message("至少保留一个 Provider 档案"));
        }
        let mut provider_ids = HashSet::new();
        for provider in &self.providers {
            if provider.id.trim().is_empty()
                || provider.name.trim().is_empty()
                || provider.base_url.trim().is_empty()
                || provider.default_model.trim().is_empty()
            {
                return Err(AppError::message(
                    "Provider ID、名称、Base URL 和默认模型不能为空",
                ));
            }
            if provider.models.is_empty() {
                return Err(AppError::message(format!(
                    "Provider {} 至少需要一个可用模型",
                    provider.id
                )));
            }
            for model_name in &provider.models {
                if model_name.trim().is_empty() {
                    return Err(AppError::message(format!(
                        "Provider {} 的模型列表不能包含空名称",
                        provider.id
                    )));
                }
            }
            if !provider
                .models
                .iter()
                .any(|model_name| model_name == &provider.default_model)
            {
                return Err(AppError::message(format!(
                    "Provider {} 的默认模型必须在模型列表中",
                    provider.id
                )));
            }
            if !provider_ids.insert(provider.id.as_str()) {
                return Err(AppError::message(format!(
                    "Provider ID 重复: {}",
                    provider.id
                )));
            }
            if provider.protocol != "openai" && provider.protocol != "anthropic" {
                return Err(AppError::message(format!(
                    "Provider {} 的协议必须是 openai 或 anthropic",
                    provider.id
                )));
            }
            validate_url(&provider.base_url, &["http", "https"], "Provider Base URL")?;
            for (header_name, header_value) in &provider.extra_headers {
                reqwest::header::HeaderName::from_bytes(header_name.as_bytes())
                    .map_err(|error| AppError::message(format!("额外 Header 名无效: {error}")))?;
                reqwest::header::HeaderValue::from_str(header_value)
                    .map_err(|error| AppError::message(format!("额外 Header 值无效: {error}")))?;
            }
        }
        if let Some(default_provider_id) = self.default_provider_profile_id.as_deref() {
            if !provider_ids.contains(default_provider_id) {
                return Err(AppError::message(format!(
                    "默认 Provider 不存在: {default_provider_id}"
                )));
            }
        }

        if self.templates.is_empty() {
            return Err(AppError::message("至少保留一个总结模板"));
        }
        let mut template_ids = HashSet::new();
        for template in &self.templates {
            if template.id.trim().is_empty()
                || template.name.trim().is_empty()
                || template.user_template.trim().is_empty()
            {
                return Err(AppError::message("总结模板 ID、名称和用户模板不能为空"));
            }
            if !template_ids.insert(template.id.as_str()) {
                return Err(AppError::message(format!(
                    "总结模板 ID 重复: {}",
                    template.id
                )));
            }
        }
        if let Some(default_template_id) = self.default_template_id.as_deref() {
            if !template_ids.contains(default_template_id) {
                return Err(AppError::message(format!(
                    "默认总结模板不存在: {default_template_id}"
                )));
            }
        }

        let mut job_group_ids = HashSet::new();
        let mut job_group_names = HashSet::new();
        for group in &self.job_groups {
            let group_id = group.id.trim();
            let group_name = group.name.trim();
            if group_id.is_empty() || group_name.is_empty() {
                return Err(AppError::message("任务分组 ID 和名称不能为空"));
            }
            if !job_group_ids.insert(group_id.to_string()) {
                return Err(AppError::message(format!("任务分组 ID 重复: {group_id}")));
            }
            let name_key = group_name.to_lowercase();
            if !job_group_names.insert(name_key) {
                return Err(AppError::message(format!("任务分组名称重复: {group_name}")));
            }
        }
        Ok(())
    }

    /// Trim group ids/names and drop blank entries. Does not invent ids.
    pub fn normalize_job_groups(&mut self) {
        let mut seen_ids = HashSet::new();
        let mut normalized_groups = Vec::new();
        for group in self.job_groups.drain(..) {
            let group_id = group.id.trim().to_string();
            let group_name = group.name.trim().to_string();
            if group_id.is_empty() || group_name.is_empty() {
                continue;
            }
            if !seen_ids.insert(group_id.clone()) {
                continue;
            }
            normalized_groups.push(JobGroupDefinition {
                id: group_id,
                name: group_name,
            });
        }
        self.job_groups = normalized_groups;
    }

    /// Resolve a job group input to a catalog id.
    /// Empty input clears. Known id/name reuses the entry. Unknown name creates
    /// a new catalog entry and returns its id (caller must persist config).
    pub fn resolve_or_create_job_group(
        &mut self,
        raw_group: Option<String>,
    ) -> AppResult<Option<String>> {
        let Some(trimmed_input) = raw_group
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| value.to_string())
        else {
            return Ok(None);
        };

        if let Some(existing_by_id) = self
            .job_groups
            .iter()
            .find(|group| group.id == trimmed_input)
        {
            return Ok(Some(existing_by_id.id.clone()));
        }

        let input_name_key = trimmed_input.to_lowercase();
        if let Some(existing_by_name) = self
            .job_groups
            .iter()
            .find(|group| group.name.trim().to_lowercase() == input_name_key)
        {
            return Ok(Some(existing_by_name.id.clone()));
        }

        let new_group = JobGroupDefinition {
            id: Uuid::new_v4().to_string(),
            name: trimmed_input,
        };
        let new_group_id = new_group.id.clone();
        self.job_groups.push(new_group);
        Ok(Some(new_group_id))
    }

    #[allow(dead_code)]
    pub fn job_group_label(&self, group_value: Option<&str>) -> Option<String> {
        let trimmed_value = group_value?.trim();
        if trimmed_value.is_empty() {
            return None;
        }
        if let Some(by_id) = self
            .job_groups
            .iter()
            .find(|group| group.id == trimmed_value)
        {
            return Some(by_id.name.clone());
        }
        if let Some(by_name) = self
            .job_groups
            .iter()
            .find(|group| group.name.trim().eq_ignore_ascii_case(trimmed_value))
        {
            return Some(by_name.name.clone());
        }
        Some(trimmed_value.to_string())
    }

    pub fn workspace_path(&self) -> PathBuf {
        PathBuf::from(&self.workspace_dir)
    }

    pub fn ensure_workspace(&self) -> AppResult<PathBuf> {
        let workspace_path = self.workspace_path();
        fs::create_dir_all(workspace_path.join("jobs"))?;
        Ok(workspace_path)
    }

    pub fn resolve_api_key(&self, provider: &ProviderProfile) -> Option<String> {
        if let Some(env_name) = &provider.api_key_env {
            if let Ok(value) = std::env::var(env_name) {
                if !value.trim().is_empty() {
                    return Some(value);
                }
            }
        }
        provider
            .api_key
            .as_ref()
            .filter(|value| !value.trim().is_empty())
            .cloned()
    }

    pub fn secret_values(&self) -> Vec<String> {
        let mut secrets = Vec::new();
        for provider in &self.providers {
            if let Some(api_key) = self.resolve_api_key(provider) {
                secrets.push(api_key);
            }
            secrets.extend(
                provider
                    .extra_headers
                    .iter()
                    .filter(|(name, value)| {
                        is_sensitive_header_name(name) && !value.trim().is_empty()
                    })
                    .map(|(_, value)| value.clone()),
            );
        }
        secrets
    }

    pub fn public_view(&self) -> AppConfigPublic {
        AppConfigPublic {
            workspace_dir: self.workspace_dir.clone(),
            default_segment_minutes: self.default_segment_minutes,
            default_auto_transcribe: self.default_auto_transcribe,
            default_auto_summarize: self.default_auto_summarize,
            default_provider_profile_id: self.default_provider_profile_id.clone(),
            default_template_id: self.default_template_id.clone(),
            proxy_url: self.proxy_url.clone(),
            min_free_disk_gb: self.min_free_disk_gb,
            live_reconnect_attempts: self.live_reconnect_attempts,
            max_context_chars: self.max_context_chars,
            max_concurrent_jobs: self.max_concurrent_jobs,
            max_live_records: self.max_live_records,
            download_cookies_file: self.download_cookies_file.clone(),
            download_cookies_from_browser: self.download_cookies_from_browser.clone(),
            transcribe_model: self.transcribe_model.clone(),
            transcribe_language: self.transcribe_language.clone(),
            transcribe_model_preset: self.transcribe_model_preset.clone(),
            transcribe_model_presets: self.transcribe_model_presets.clone(),
            glossary: self.glossary.clone(),
            default_auto_chapterize: self.default_auto_chapterize,
            sidecar_paths: self.sidecar_paths.clone(),
            providers: self
                .providers
                .iter()
                .map(|provider| ProviderProfilePublic {
                    id: provider.id.clone(),
                    name: provider.name.clone(),
                    protocol: provider.protocol.clone(),
                    base_url: provider.base_url.clone(),
                    api_key_env: provider.api_key_env.clone(),
                    has_api_key: self.resolve_api_key(provider).is_some(),
                    default_model: provider.default_model.clone(),
                    models: provider.models.clone(),
                    extra_headers: provider
                        .extra_headers
                        .iter()
                        .map(|(name, value)| {
                            (
                                name.clone(),
                                if is_sensitive_header_name(name) {
                                    "***REDACTED***".to_string()
                                } else {
                                    value.clone()
                                },
                            )
                        })
                        .collect(),
                })
                .collect(),
            templates: self.templates.clone(),
            job_groups: self.job_groups.clone(),
            config_path: app_config_path()
                .map(|path| path_to_string(&path))
                .unwrap_or_default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderProfilePublic {
    pub id: String,
    pub name: String,
    pub protocol: String,
    pub base_url: String,
    pub api_key_env: Option<String>,
    pub has_api_key: bool,
    pub default_model: String,
    pub models: Vec<String>,
    pub extra_headers: Vec<(String, String)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfigPublic {
    pub workspace_dir: String,
    pub default_segment_minutes: u32,
    pub default_auto_transcribe: bool,
    pub default_auto_summarize: bool,
    pub default_provider_profile_id: Option<String>,
    pub default_template_id: Option<String>,
    pub proxy_url: Option<String>,
    pub min_free_disk_gb: u32,
    pub live_reconnect_attempts: u32,
    pub max_context_chars: usize,
    pub max_concurrent_jobs: u32,
    pub max_live_records: u32,
    #[serde(default)]
    pub download_cookies_file: Option<String>,
    #[serde(default)]
    pub download_cookies_from_browser: Option<String>,
    pub transcribe_model: Option<String>,
    pub transcribe_language: String,
    #[serde(default = "default_transcribe_model_preset")]
    pub transcribe_model_preset: String,
    #[serde(default)]
    pub transcribe_model_presets: TranscribeModelPresets,
    #[serde(default)]
    pub glossary: GlossaryConfig,
    #[serde(default = "default_true")]
    pub default_auto_chapterize: bool,
    pub sidecar_paths: SidecarPaths,
    pub providers: Vec<ProviderProfilePublic>,
    pub templates: Vec<SummaryTemplate>,
    #[serde(default)]
    pub job_groups: Vec<JobGroupDefinition>,
    pub config_path: String,
}

impl AppConfig {
    /// Resolve the model path used by whisper (preset path or custom field).
    pub fn resolve_transcribe_model_path(&self) -> Option<String> {
        match self.transcribe_model_preset.as_str() {
            "speed" => self
                .transcribe_model_presets
                .speed
                .clone()
                .filter(|value| !value.trim().is_empty())
                .or_else(|| self.transcribe_model.clone()),
            "balanced" => self
                .transcribe_model_presets
                .balanced
                .clone()
                .filter(|value| !value.trim().is_empty())
                .or_else(|| self.transcribe_model.clone()),
            "quality" => self
                .transcribe_model_presets
                .quality
                .clone()
                .filter(|value| !value.trim().is_empty())
                .or_else(|| self.transcribe_model.clone()),
            _ => self.transcribe_model.clone(),
        }
        .and_then(|value| {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.replace('\\', "/"))
            }
        })
    }

    fn apply_transcribe_model_preset(&mut self) {
        if let Some(resolved) = self.resolve_transcribe_model_path() {
            // Keep `transcribe_model` as the effective path for older call sites.
            if self.transcribe_model_preset != "custom" {
                self.transcribe_model = Some(resolved);
            }
        }
    }
}

fn normalize_glossary(mut glossary: GlossaryConfig) -> GlossaryConfig {
    glossary.hotwords = glossary
        .hotwords
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect();
    glossary.replacements = glossary
        .replacements
        .into_iter()
        .filter_map(|pair| {
            let from = pair.from.trim().to_string();
            if from.is_empty() {
                return None;
            }
            Some(GlossaryReplacement {
                from,
                to: pair.to.trim().to_string(),
            })
        })
        .collect();
    glossary
}

fn default_max_context_chars() -> usize {
    400_000
}

fn default_max_concurrent_jobs() -> u32 {
    2
}

fn default_max_live_records() -> u32 {
    1
}

fn default_transcribe_language() -> String {
    "auto".to_string()
}

/// yt-dlp `--cookies-from-browser` browser id (optional `browser:profile` form).
pub fn validate_cookies_browser(browser: &str) -> AppResult<()> {
    let trimmed = browser.trim();
    if trimmed.is_empty() {
        return Ok(());
    }
    let browser_name = trimmed
        .split_once(':')
        .map(|(name, _)| name)
        .unwrap_or(trimmed)
        .to_ascii_lowercase();
    const ALLOWED: &[&str] = &[
        "chrome", "chromium", "edge", "firefox", "opera", "brave", "vivaldi", "safari", "whale",
    ];
    if !ALLOWED.iter().any(|allowed| *allowed == browser_name) {
        return Err(AppError::message(format!(
            "不支持的 cookies 浏览器标识: {trimmed}（可用 chrome / edge / firefox 等）"
        )));
    }
    Ok(())
}

fn empty_to_none(value: String) -> Option<String> {
    let trimmed = value.trim().to_string();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

/// Trim model names, drop blanks/duplicates (order-preserving), and ensure
/// `default_model` is present and non-empty when possible.
fn normalize_provider_models(provider: &mut ProviderProfile) {
    let mut seen_model_names = HashSet::new();
    let mut normalized_models = Vec::new();
    for model_name in provider.models.drain(..) {
        let trimmed_model_name = model_name.trim().to_string();
        if trimmed_model_name.is_empty() {
            continue;
        }
        if seen_model_names.insert(trimmed_model_name.clone()) {
            normalized_models.push(trimmed_model_name);
        }
    }

    let trimmed_default_model = provider.default_model.trim().to_string();
    if !trimmed_default_model.is_empty() {
        provider.default_model = trimmed_default_model.clone();
        if !seen_model_names.contains(&trimmed_default_model) {
            normalized_models.insert(0, trimmed_default_model);
        }
    } else if let Some(first_model) = normalized_models.first() {
        provider.default_model = first_model.clone();
    }

    provider.models = normalized_models;
}

fn is_sensitive_header_name(name: &str) -> bool {
    let normalized = name.to_ascii_lowercase();
    normalized == "authorization"
        || normalized == "proxy-authorization"
        || normalized.contains("api-key")
        || normalized.contains("token")
        || normalized.contains("secret")
}

fn validate_url(value: &str, allowed_schemes: &[&str], label: &str) -> AppResult<()> {
    let parsed = reqwest::Url::parse(value)
        .map_err(|error| AppError::message(format!("{label} 无效: {error}")))?;
    if !allowed_schemes.contains(&parsed.scheme()) {
        return Err(AppError::message(format!(
            "{label} 协议不支持: {}",
            parsed.scheme()
        )));
    }
    Ok(())
}

fn path_to_string(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_duplicate_provider_ids_without_changing_original() {
        let original = AppConfig::default();
        let mut duplicate_providers = original.providers.clone();
        duplicate_providers[1].id = duplicate_providers[0].id.clone();
        let request = SaveConfigRequest {
            providers: Some(duplicate_providers),
            ..empty_save_request()
        };

        let error = original
            .candidate_with_update(request)
            .expect_err("duplicate IDs must fail");
        assert!(error.to_string().contains("Provider ID 重复"));
        assert_ne!(original.providers[0].id, original.providers[1].id);
    }

    #[test]
    fn normalizes_legacy_provider_without_models_list() {
        let mut original = AppConfig::default();
        original.providers[0].models = vec![];
        original.providers[0].default_model = "gpt-4o-mini".into();
        original.normalize_provider_models();
        assert_eq!(
            original.providers[0].models,
            vec!["gpt-4o-mini".to_string()]
        );
        original
            .validate()
            .expect("legacy provider must validate after normalize");
    }

    #[test]
    fn preserves_existing_keys_and_masks_sensitive_public_headers() {
        let mut original = AppConfig::default();
        original.providers[0].api_key = Some("secret-api-key".into());
        original.providers[0]
            .extra_headers
            .push(("Authorization".into(), "Bearer secret".into()));
        let mut incoming_providers = original.providers.clone();
        incoming_providers[0].api_key = Some(String::new());
        incoming_providers[0].extra_headers[0].1 = "***REDACTED***".into();

        let candidate = original
            .candidate_with_update(SaveConfigRequest {
                providers: Some(incoming_providers),
                ..empty_save_request()
            })
            .expect("build candidate");

        assert_eq!(
            candidate.providers[0].api_key.as_deref(),
            Some("secret-api-key")
        );
        assert_eq!(candidate.providers[0].extra_headers[0].1, "Bearer secret");
        assert_eq!(
            candidate.public_view().providers[0].extra_headers[0].1,
            "***REDACTED***"
        );
    }

    fn empty_save_request() -> SaveConfigRequest {
        SaveConfigRequest {
            workspace_dir: None,
            default_segment_minutes: None,
            default_auto_transcribe: None,
            default_auto_summarize: None,
            default_provider_profile_id: None,
            default_template_id: None,
            proxy_url: None,
            min_free_disk_gb: None,
            live_reconnect_attempts: None,
            max_context_chars: None,
            max_concurrent_jobs: None,
            max_live_records: None,
            download_cookies_file: None,
            download_cookies_from_browser: None,
            transcribe_model: None,
            transcribe_language: None,
            transcribe_model_preset: None,
            transcribe_model_presets: None,
            glossary: None,
            default_auto_chapterize: None,
            sidecar_paths: None,
            providers: None,
            templates: None,
            job_groups: None,
        }
    }

    #[test]
    fn resolves_or_creates_job_group_by_name() {
        let mut config = AppConfig::default();
        let first_id = config
            .resolve_or_create_job_group(Some(" 学习笔记 ".into()))
            .expect("create group")
            .expect("group id");
        let second_id = config
            .resolve_or_create_job_group(Some("学习笔记".into()))
            .expect("reuse group")
            .expect("group id");
        assert_eq!(first_id, second_id);
        assert_eq!(config.job_groups.len(), 1);
        assert_eq!(config.job_groups[0].name, "学习笔记");
        assert_eq!(
            config.job_group_label(Some(&first_id)).as_deref(),
            Some("学习笔记")
        );
    }

    #[test]
    fn rejects_duplicate_job_group_names() {
        let original = AppConfig::default();
        let request = SaveConfigRequest {
            job_groups: Some(vec![
                JobGroupDefinition {
                    id: "g1".into(),
                    name: "学习".into(),
                },
                JobGroupDefinition {
                    id: "g2".into(),
                    name: "学习".into(),
                },
            ]),
            ..empty_save_request()
        };
        let error = original
            .candidate_with_update(request)
            .expect_err("duplicate names must fail");
        assert!(error.to_string().contains("任务分组名称重复"));
    }
}

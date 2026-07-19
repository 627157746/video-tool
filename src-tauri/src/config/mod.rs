use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

const APP_CONFIG_DIR_NAME: &str = "video-tool";
const APP_CONFIG_FILE_NAME: &str = "config.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderProfile {
    pub id: String,
    pub name: String,
    /// `openai` | `anthropic`
    pub protocol: String,
    pub base_url: String,
    pub api_key: Option<String>,
    pub api_key_env: Option<String>,
    pub default_model: String,
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
    pub sidecar_paths: SidecarPaths,
    pub providers: Vec<ProviderProfile>,
    pub templates: Vec<SummaryTemplate>,
}

impl Default for AppConfig {
    fn default() -> Self {
        let workspace_dir = default_workspace_dir()
            .to_string_lossy()
            .replace('\\', "/");

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
            sidecar_paths: SidecarPaths {
                ffmpeg: None,
                ffprobe: None,
                yt_dlp: None,
                streamlink: None,
                transcribe: None,
            },
            providers: vec![
                ProviderProfile {
                    id: "example-openai".to_string(),
                    name: "OpenAI-compatible (example)".to_string(),
                    protocol: "openai".to_string(),
                    base_url: "https://api.openai.com/v1".to_string(),
                    api_key: None,
                    api_key_env: Some("OPENAI_API_KEY".to_string()),
                    default_model: "gpt-4o-mini".to_string(),
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
            system_prompt: "你是一个严谨的中文内容助理，根据字幕整理结构化总结。".to_string(),
            user_template: concat!(
                "标题：{{title}}\n",
                "来源：{{source_url}}\n",
                "时长：{{duration}}\n\n",
                "请根据以下字幕输出 Markdown 总结，包含：概述、要点列表、时间线（如有）、可执行事项。\n\n",
                "{{transcript}}\n"
            )
            .to_string(),
        },
        SummaryTemplate {
            id: "tutorial-keypoints".to_string(),
            name: "教程要点".to_string(),
            system_prompt: "你擅长把教程口播提炼成可操作步骤。".to_string(),
            user_template: concat!(
                "视频：{{title}}\n\n",
                "请提炼：目标读者、前置条件、分步操作、常见坑、一句话结论。\n\n",
                "{{transcript}}\n"
            )
            .to_string(),
        },
        SummaryTemplate {
            id: "live-talk-notes".to_string(),
            name: "直播口播纪要".to_string(),
            system_prompt: "你擅长整理直播口播为会议纪要风格笔记。".to_string(),
            user_template: concat!(
                "场次：{{title}}\n来源：{{source_url}}\n\n",
                "请输出：主题、关键发言、承诺/行动项、待核实信息。\n\n",
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
            let config: AppConfig = serde_json::from_str(&raw)?;
            return Ok(config);
        }

        let config = AppConfig::default();
        config.save()?;
        Ok(config)
    }

    pub fn save(&self) -> AppResult<()> {
        let path = app_config_path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let raw = serde_json::to_string_pretty(self)?;
        fs::write(path, raw)?;
        Ok(())
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
                })
                .collect(),
            templates: self.templates.clone(),
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
    pub sidecar_paths: SidecarPaths,
    pub providers: Vec<ProviderProfilePublic>,
    pub templates: Vec<SummaryTemplate>,
    pub config_path: String,
}

fn path_to_string(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

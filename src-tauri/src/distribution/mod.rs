//! Install / distribution helpers: dependency wizard, model scan,
//! config import-export (no secrets), and lightweight update check.

use crate::config::{
    AppConfig, AppConfigPublic, JobGroupDefinition, ProviderProfile, SidecarPaths, SummaryTemplate,
};
use crate::error::{AppError, AppResult};
use crate::sidecar::{BinarySource, ResolvedBinary, SidecarStatus};
use crate::workspace::{self, WorkspaceHealthReport};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

const EXPORT_FORMAT_VERSION: u32 = 1;

/// Built-in GitHub release source for this project.
/// Override with `VIDEO_TOOL_RELEASE_API` / `VIDEO_TOOL_RELEASE_PAGE` if needed.
const DEFAULT_RELEASE_OWNER: &str = "627157746";
const DEFAULT_RELEASE_REPO: &str = "video-tool";

fn default_release_api_url() -> String {
    format!(
        "https://api.github.com/repos/{DEFAULT_RELEASE_OWNER}/{DEFAULT_RELEASE_REPO}/releases/latest"
    )
}

fn default_release_page_url() -> String {
    format!("https://github.com/{DEFAULT_RELEASE_OWNER}/{DEFAULT_RELEASE_REPO}/releases")
}

/// GitHub releases API URL (override with env VIDEO_TOOL_RELEASE_API).
fn release_api_url() -> String {
    std::env::var("VIDEO_TOOL_RELEASE_API")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(default_release_api_url)
}

fn release_page_url() -> String {
    std::env::var("VIDEO_TOOL_RELEASE_PAGE")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(default_release_page_url)
}

/// Optional token for private repositories (env VIDEO_TOOL_GITHUB_TOKEN only).
fn release_github_token() -> Option<String> {
    std::env::var("VIDEO_TOOL_GITHUB_TOKEN")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

#[derive(Debug, Clone, Serialize)]
pub struct DependencyHint {
    pub name: String,
    pub display_name: String,
    pub required: bool,
    pub status: ResolvedBinary,
    pub guidance: String,
    pub install_hint: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DependencyReport {
    pub items: Vec<DependencyHint>,
    pub all_required_ready: bool,
    pub missing_required: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelFileInfo {
    pub path: String,
    pub file_name: String,
    pub size_bytes: u64,
    pub exists: bool,
    pub is_selected: bool,
    pub kind: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelInventory {
    pub selected_path: Option<String>,
    pub selected_exists: bool,
    pub scan_directories: Vec<String>,
    pub models: Vec<ModelFileInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigExportPackage {
    pub format_version: u32,
    pub exported_at: String,
    pub app_version: String,
    pub include_secrets: bool,
    pub workspace_dir: Option<String>,
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
    pub download_cookies_file: Option<String>,
    pub download_cookies_from_browser: Option<String>,
    pub transcribe_model: Option<String>,
    pub transcribe_language: String,
    pub transcribe_model_preset: String,
    pub transcribe_model_presets: crate::config::TranscribeModelPresets,
    pub glossary: crate::config::GlossaryConfig,
    pub default_auto_chapterize: bool,
    pub sidecar_paths: SidecarPaths,
    pub providers: Vec<ProviderExport>,
    pub templates: Vec<SummaryTemplate>,
    pub job_groups: Vec<JobGroupDefinition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderExport {
    pub id: String,
    pub name: String,
    pub protocol: String,
    pub base_url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    pub api_key_env: Option<String>,
    pub default_model: String,
    pub models: Vec<String>,
    pub extra_headers: Vec<(String, String)>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ConfigImportResult {
    pub providers: usize,
    pub templates: usize,
    pub job_groups: usize,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct UpdateCheckResult {
    pub current_version: String,
    pub latest_version: Option<String>,
    pub update_available: bool,
    pub release_page_url: String,
    pub release_notes: Option<String>,
    pub message: String,
    /// GitHub release asset URL for the preferred Windows installer (if any).
    pub installer_url: Option<String>,
    pub installer_name: Option<String>,
    pub installer_size_bytes: Option<u64>,
    /// True when an installer asset is available for this platform.
    pub can_install: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct AppUpdateProgress {
    pub phase: String,
    pub downloaded_bytes: u64,
    pub total_bytes: Option<u64>,
    pub percent: Option<f64>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct AppUpdateInstallResult {
    pub installer_path: String,
    pub installer_name: String,
    pub launched: bool,
    pub message: String,
}

#[derive(Debug, Clone)]
struct ReleaseInstallerAsset {
    url: String,
    name: String,
    size_bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SystemDiagnostics {
    pub app_name: String,
    pub app_version: String,
    pub config_path: String,
    pub workspace_dir: String,
    pub free_disk_gb: Option<u64>,
    pub min_free_disk_gb: u32,
    pub disk_below_threshold: bool,
    pub sidecars: SidecarStatus,
    pub dependency: DependencyReport,
    pub models: ModelInventory,
    pub workspace_health: WorkspaceHealthReport,
}

pub fn build_dependency_report(status: &SidecarStatus) -> DependencyReport {
    let items = vec![
        hint(
            "ffmpeg",
            "FFmpeg",
            true,
            &status.ffmpeg,
            "音视频转码、直播分段与合并依赖 ffmpeg。",
            "从 https://ffmpeg.org/download.html 安装，或在设置中指定可执行文件路径。",
        ),
        hint(
            "ffprobe",
            "FFprobe",
            true,
            &status.ffprobe,
            "探测媒体时长与元数据依赖 ffprobe（通常与 ffmpeg 同包）。",
            "安装 ffmpeg 完整包，或单独指定 ffprobe 路径。",
        ),
        hint(
            "yt-dlp",
            "yt-dlp",
            true,
            &status.yt_dlp,
            "链接下载依赖 yt-dlp。",
            "pip install -U yt-dlp，或从 GitHub Releases 下载二进制并配置路径。",
        ),
        hint(
            "streamlink",
            "Streamlink",
            false,
            &status.streamlink,
            "部分直播源可用 streamlink 作为补充入口（可选）。",
            "pip install -U streamlink，或在设置中指定路径。",
        ),
        hint(
            "transcribe",
            "whisper-cli",
            true,
            &status.transcribe,
            "本地转写依赖 whisper.cpp 的 whisper-cli。",
            "编译或下载 whisper.cpp 可执行文件，并在设置中指定路径与 GGML 模型。",
        ),
    ];
    let missing_required: Vec<String> = items
        .iter()
        .filter(|item| item.required && item.status.source == BinarySource::Missing)
        .map(|item| item.display_name.clone())
        .collect();
    let all_required_ready = missing_required.is_empty();
    DependencyReport {
        items,
        all_required_ready,
        missing_required,
    }
}

fn hint(
    name: &str,
    display_name: &str,
    required: bool,
    status: &ResolvedBinary,
    guidance: &str,
    install_hint: &str,
) -> DependencyHint {
    DependencyHint {
        name: name.to_string(),
        display_name: display_name.to_string(),
        required,
        status: status.clone(),
        guidance: guidance.to_string(),
        install_hint: install_hint.to_string(),
    }
}

pub fn scan_models(config: &AppConfig) -> ModelInventory {
    let selected_path = config.resolve_transcribe_model_path();
    let selected_exists = selected_path
        .as_ref()
        .map(|path| Path::new(path).is_file())
        .unwrap_or(false);

    let mut scan_dirs: Vec<PathBuf> = Vec::new();
    let mut push_dir = |path: Option<&str>| {
        let Some(raw) = path.map(str::trim).filter(|value| !value.is_empty()) else {
            return;
        };
        let path = PathBuf::from(raw);
        let directory = if path.is_file() {
            path.parent().map(|parent| parent.to_path_buf())
        } else {
            Some(path)
        };
        if let Some(directory) = directory {
            if directory.is_dir() && !scan_dirs.iter().any(|existing| existing == &directory) {
                scan_dirs.push(directory);
            }
        }
    };

    push_dir(selected_path.as_deref());
    push_dir(config.transcribe_model.as_deref());
    push_dir(config.transcribe_model_presets.speed.as_deref());
    push_dir(config.transcribe_model_presets.balanced.as_deref());
    push_dir(config.transcribe_model_presets.quality.as_deref());

    let mut models: Vec<ModelFileInfo> = Vec::new();
    let mut seen = std::collections::HashSet::new();

    if let Some(selected) = selected_path.as_ref() {
        let path = PathBuf::from(selected);
        let key = path_to_string(&path);
        if seen.insert(key.clone()) {
            models.push(model_info(&path, true));
        }
    }

    for directory in &scan_dirs {
        let Ok(entries) = fs::read_dir(directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            if !is_model_file(&path) {
                continue;
            }
            let key = path_to_string(&path);
            if !seen.insert(key.clone()) {
                continue;
            }
            let is_selected = selected_path
                .as_ref()
                .is_some_and(|selected| paths_equal(selected, &key));
            models.push(model_info(&path, is_selected));
        }
    }

    models.sort_by(|left, right| left.file_name.cmp(&right.file_name));

    ModelInventory {
        selected_path: selected_path.clone(),
        selected_exists,
        scan_directories: scan_dirs.iter().map(|path| path_to_string(path)).collect(),
        models,
    }
}

fn is_model_file(path: &Path) -> bool {
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    name.ends_with(".bin")
        || name.ends_with(".ggml")
        || name.ends_with(".gguf")
        || name.contains("ggml")
}

fn model_info(path: &Path, is_selected: bool) -> ModelFileInfo {
    let exists = path.is_file();
    let size_bytes = if exists {
        fs::metadata(path).map(|meta| meta.len()).unwrap_or(0)
    } else {
        0
    };
    let file_name = path
        .file_name()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|| path_to_string(path));
    let kind = if file_name.to_ascii_lowercase().ends_with(".gguf") {
        "gguf"
    } else if file_name.to_ascii_lowercase().contains("ggml")
        || file_name.to_ascii_lowercase().ends_with(".bin")
    {
        "ggml"
    } else {
        "other"
    }
    .to_string();
    ModelFileInfo {
        path: path_to_string(path),
        file_name,
        size_bytes,
        exists,
        is_selected,
        kind,
    }
}

pub fn export_config_package(config: &AppConfig, include_secrets: bool) -> ConfigExportPackage {
    let providers = config
        .providers
        .iter()
        .map(|provider| {
            let mut extra_headers = provider.extra_headers.clone();
            if !include_secrets {
                for (name, value) in &mut extra_headers {
                    if is_sensitive_header_name(name) {
                        *value = String::new();
                    }
                }
            }
            ProviderExport {
                id: provider.id.clone(),
                name: provider.name.clone(),
                protocol: provider.protocol.clone(),
                base_url: provider.base_url.clone(),
                api_key: if include_secrets {
                    provider.api_key.clone()
                } else {
                    None
                },
                api_key_env: provider.api_key_env.clone(),
                default_model: provider.default_model.clone(),
                models: provider.models.clone(),
                extra_headers,
            }
        })
        .collect();

    ConfigExportPackage {
        format_version: EXPORT_FORMAT_VERSION,
        exported_at: chrono::Utc::now().to_rfc3339(),
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        include_secrets,
        workspace_dir: Some(config.workspace_dir.clone()),
        default_segment_minutes: config.default_segment_minutes,
        default_auto_transcribe: config.default_auto_transcribe,
        default_auto_summarize: config.default_auto_summarize,
        default_provider_profile_id: config.default_provider_profile_id.clone(),
        default_template_id: config.default_template_id.clone(),
        proxy_url: config.proxy_url.clone(),
        min_free_disk_gb: config.min_free_disk_gb,
        live_reconnect_attempts: config.live_reconnect_attempts,
        max_context_chars: config.max_context_chars,
        max_concurrent_jobs: config.max_concurrent_jobs,
        max_live_records: config.max_live_records,
        download_cookies_file: config.download_cookies_file.clone(),
        download_cookies_from_browser: config.download_cookies_from_browser.clone(),
        transcribe_model: config.transcribe_model.clone(),
        transcribe_language: config.transcribe_language.clone(),
        transcribe_model_preset: config.transcribe_model_preset.clone(),
        transcribe_model_presets: config.transcribe_model_presets.clone(),
        glossary: config.glossary.clone(),
        default_auto_chapterize: config.default_auto_chapterize,
        sidecar_paths: config.sidecar_paths.clone(),
        providers,
        templates: config.templates.clone(),
        job_groups: config.job_groups.clone(),
    }
}

pub fn apply_import_package(
    current: &AppConfig,
    package: ConfigExportPackage,
    import_secrets: bool,
) -> AppResult<(AppConfig, ConfigImportResult)> {
    if package.format_version == 0 || package.format_version > EXPORT_FORMAT_VERSION {
        return Err(AppError::message(format!(
            "不支持的配置导出格式版本: {}",
            package.format_version
        )));
    }
    if package.providers.is_empty() {
        return Err(AppError::message("导入包至少需要一个 Provider 档案"));
    }
    if package.templates.is_empty() {
        return Err(AppError::message("导入包至少需要一个总结模板"));
    }

    let mut next = current.clone();
    if let Some(workspace) = package
        .workspace_dir
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        next.workspace_dir = workspace.replace('\\', "/");
    }
    next.default_segment_minutes = package.default_segment_minutes;
    next.default_auto_transcribe = package.default_auto_transcribe;
    next.default_auto_summarize = package.default_auto_summarize;
    next.default_provider_profile_id = package.default_provider_profile_id;
    next.default_template_id = package.default_template_id;
    next.proxy_url = package.proxy_url;
    next.min_free_disk_gb = package.min_free_disk_gb;
    next.live_reconnect_attempts = package.live_reconnect_attempts;
    next.max_context_chars = package.max_context_chars;
    next.max_concurrent_jobs = package.max_concurrent_jobs;
    next.max_live_records = package.max_live_records;
    next.download_cookies_file = package.download_cookies_file;
    next.download_cookies_from_browser = package.download_cookies_from_browser;
    next.transcribe_model = package.transcribe_model;
    next.transcribe_language = package.transcribe_language;
    next.transcribe_model_preset = package.transcribe_model_preset;
    next.transcribe_model_presets = package.transcribe_model_presets;
    next.glossary = package.glossary;
    next.default_auto_chapterize = package.default_auto_chapterize;
    next.sidecar_paths = package.sidecar_paths;
    next.templates = package.templates;
    next.job_groups = package.job_groups;

    next.providers = package
        .providers
        .into_iter()
        .map(|imported| {
            let existing = current
                .providers
                .iter()
                .find(|provider| provider.id == imported.id);
            let api_key = if import_secrets {
                imported
                    .api_key
                    .filter(|value| !value.trim().is_empty())
                    .or_else(|| existing.and_then(|provider| provider.api_key.clone()))
            } else {
                existing.and_then(|provider| provider.api_key.clone())
            };
            let mut extra_headers = imported.extra_headers;
            if !import_secrets {
                for (name, value) in &mut extra_headers {
                    if is_sensitive_header_name(name) {
                        if let Some(existing_provider) = existing {
                            if let Some((_, existing_value)) = existing_provider
                                .extra_headers
                                .iter()
                                .find(|(existing_name, _)| existing_name.eq_ignore_ascii_case(name))
                            {
                                *value = existing_value.clone();
                            } else {
                                *value = String::new();
                            }
                        }
                    }
                }
            }
            ProviderProfile {
                id: imported.id,
                name: imported.name,
                protocol: imported.protocol,
                base_url: imported.base_url,
                api_key,
                api_key_env: imported.api_key_env,
                default_model: imported.default_model,
                models: imported.models,
                extra_headers,
            }
        })
        .collect();

    next.normalize_provider_models();
    next.normalize_job_groups();
    next.validate()?;

    let result = ConfigImportResult {
        providers: next.providers.len(),
        templates: next.templates.len(),
        job_groups: next.job_groups.len(),
        message: if import_secrets {
            "配置已导入（含密钥字段，若导出包中存在）".to_string()
        } else {
            "配置已导入（已剥离 API Key；本地原有 Key 按同 ID 保留）".to_string()
        },
    };
    Ok((next, result))
}

fn empty_update_check_result(
    current_version: impl Into<String>,
    page: String,
    message: impl Into<String>,
) -> UpdateCheckResult {
    UpdateCheckResult {
        current_version: current_version.into(),
        latest_version: None,
        update_available: false,
        release_page_url: page,
        release_notes: None,
        message: message.into(),
        installer_url: None,
        installer_name: None,
        installer_size_bytes: None,
        can_install: false,
    }
}

pub fn check_app_update() -> AppResult<UpdateCheckResult> {
    let current_version = env!("CARGO_PKG_VERSION").to_string();
    let page = release_page_url();
    let client = build_release_http_client(&current_version, std::time::Duration::from_secs(15))?;
    let body = match resolve_latest_release_body(&client, &current_version, &page)? {
        Ok(body) => body,
        Err(result) => return Ok(result),
    };

    let latest_raw = body
        .get("tag_name")
        .or_else(|| body.get("name"))
        .and_then(|value| value.as_str())
        .unwrap_or("")
        .trim()
        .trim_start_matches('v')
        .to_string();
    let latest_version = if latest_raw.is_empty() {
        None
    } else {
        Some(latest_raw)
    };
    let notes = body
        .get("body")
        .and_then(|value| value.as_str())
        .map(|value| {
            let trimmed = value.trim();
            if trimmed.chars().count() > 2_000 {
                format!("{}…", trimmed.chars().take(2_000).collect::<String>())
            } else {
                trimmed.to_string()
            }
        })
        .filter(|value| !value.is_empty());
    let html_url = body
        .get("html_url")
        .and_then(|value| value.as_str())
        .map(|value| value.to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| page.clone());
    let installer = pick_platform_installer_asset(&body);
    let can_install = installer.is_some();
    let update_available = latest_version
        .as_ref()
        .is_some_and(|latest| version_is_newer(latest, &current_version));
    let message = if update_available {
        if can_install {
            format!(
                "发现新版本 {}（当前 {}）。可在应用内下载并静默安装（无向导）。",
                latest_version.as_deref().unwrap_or("?"),
                current_version
            )
        } else {
            format!(
                "发现新版本 {}（当前 {}）。发布页暂无本平台安装包，请手动打开页面下载。",
                latest_version.as_deref().unwrap_or("?"),
                current_version
            )
        }
    } else if latest_version.is_some() {
        format!(
            "已是最新版本 {}（远端 {}）。",
            current_version,
            latest_version.as_deref().unwrap_or("?")
        )
    } else {
        format!("当前版本 {current_version}；未能解析远程版本号。")
    };
    Ok(UpdateCheckResult {
        current_version,
        latest_version,
        update_available,
        release_page_url: html_url,
        release_notes: notes,
        message,
        installer_url: installer.as_ref().map(|asset| asset.url.clone()),
        installer_name: installer.as_ref().map(|asset| asset.name.clone()),
        installer_size_bytes: installer.and_then(|asset| asset.size_bytes),
        can_install,
    })
}

/// Download the preferred installer for a newer release and launch the interactive installer.
/// Does not silent-install. Re-fetches release metadata; does not trust frontend-provided URLs.
pub fn install_app_update(
    emit_progress: &mut dyn FnMut(AppUpdateProgress),
) -> AppResult<AppUpdateInstallResult> {
    static INSTALL_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _guard = INSTALL_LOCK
        .try_lock()
        .map_err(|_| AppError::message("已有应用更新任务在进行，请稍候。"))?;

    let check = check_app_update()?;
    if !check.update_available {
        return Err(AppError::message(format!(
            "当前已是最新版本 {}，无需安装更新。",
            check.current_version
        )));
    }
    let installer_url = check
        .installer_url
        .as_deref()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            AppError::message("远端发布没有可用的本平台安装包。请打开发布页手动下载。")
        })?;
    if !is_trusted_release_asset_url(installer_url) {
        return Err(AppError::message(
            "安装包地址不受信任，已拒绝下载。请打开发布页手动下载。",
        ));
    }
    let installer_name = check
        .installer_name
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "video-tool-setup.bin".to_string());
    let safe_name = sanitize_installer_file_name(&installer_name);

    emit_progress(AppUpdateProgress {
        phase: "downloading".into(),
        downloaded_bytes: 0,
        total_bytes: check.installer_size_bytes,
        percent: Some(0.0),
        message: format!("正在下载 {safe_name}…"),
    });

    let current_version = env!("CARGO_PKG_VERSION").to_string();
    let client =
        build_release_http_client(&current_version, std::time::Duration::from_secs(30 * 60))?;
    let mut request = client
        .get(installer_url)
        .header("Accept", "application/octet-stream")
        .header("X-GitHub-Api-Version", "2022-11-28");
    if let Some(token) = release_github_token() {
        request = request.bearer_auth(token);
    }
    let mut response = request
        .send()
        .map_err(|error| AppError::message(format!("下载安装包失败: {error}")))?;
    if !response.status().is_success() {
        return Err(AppError::message(format!(
            "下载安装包失败：HTTP {}",
            response.status().as_u16()
        )));
    }
    let total_bytes = response.content_length().or(check.installer_size_bytes);
    if let Some(total) = total_bytes {
        const MAX_INSTALLER_BYTES: u64 = 800 * 1024 * 1024;
        if total > MAX_INSTALLER_BYTES {
            return Err(AppError::message(format!(
                "安装包过大（{total} 字节），已拒绝下载。"
            )));
        }
    }

    let download_dir = std::env::temp_dir().join("video-tool-updates");
    fs::create_dir_all(&download_dir)
        .map_err(|error| AppError::message(format!("创建更新下载目录失败: {error}")))?;
    let installer_path = download_dir.join(&safe_name);
    if installer_path.exists() {
        let _ = fs::remove_file(&installer_path);
    }
    let mut file = fs::File::create(&installer_path)
        .map_err(|error| AppError::message(format!("创建安装包文件失败: {error}")))?;

    use std::io::{Read, Write};
    let mut buffer = [0_u8; 64 * 1024];
    let mut downloaded_bytes = 0_u64;
    let mut last_emit_at = 0_u64;
    loop {
        let read_count = response
            .read(&mut buffer)
            .map_err(|error| AppError::message(format!("读取安装包数据失败: {error}")))?;
        if read_count == 0 {
            break;
        }
        file.write_all(&buffer[..read_count])
            .map_err(|error| AppError::message(format!("写入安装包失败: {error}")))?;
        downloaded_bytes = downloaded_bytes.saturating_add(read_count as u64);
        if let Some(total) = total_bytes {
            const MAX_INSTALLER_BYTES: u64 = 800 * 1024 * 1024;
            if downloaded_bytes > total.saturating_add(1024 * 1024)
                || downloaded_bytes > MAX_INSTALLER_BYTES
            {
                let _ = fs::remove_file(&installer_path);
                return Err(AppError::message("下载体积异常，已中止更新。".to_string()));
            }
        }
        if downloaded_bytes.saturating_sub(last_emit_at) >= 256 * 1024 || total_bytes.is_some() {
            let percent = total_bytes.map(|total| {
                if total == 0 {
                    0.0
                } else {
                    ((downloaded_bytes as f64) / (total as f64) * 100.0).min(100.0)
                }
            });
            emit_progress(AppUpdateProgress {
                phase: "downloading".into(),
                downloaded_bytes,
                total_bytes,
                percent,
                message: match (percent, total_bytes) {
                    (Some(value), Some(total)) => {
                        format!(
                            "正在下载 {safe_name}… {value:.1}%（{downloaded_bytes}/{total} 字节）"
                        )
                    }
                    _ => format!("正在下载 {safe_name}… 已下载 {downloaded_bytes} 字节"),
                },
            });
            last_emit_at = downloaded_bytes;
        }
    }
    file.flush()
        .map_err(|error| AppError::message(format!("落盘安装包失败: {error}")))?;
    drop(file);

    if downloaded_bytes == 0 {
        let _ = fs::remove_file(&installer_path);
        return Err(AppError::message("下载结果为空，安装包无效。".to_string()));
    }

    emit_progress(AppUpdateProgress {
        phase: "installing".into(),
        downloaded_bytes,
        total_bytes: total_bytes.or(Some(downloaded_bytes)),
        percent: Some(100.0),
        message: "正在静默安装（无向导）…".into(),
    });

    launch_installer_silent(&installer_path)?;

    emit_progress(AppUpdateProgress {
        phase: "done".into(),
        downloaded_bytes,
        total_bytes: total_bytes.or(Some(downloaded_bytes)),
        percent: Some(100.0),
        message: "静默安装已启动".into(),
    });

    Ok(AppUpdateInstallResult {
        installer_path: path_to_string(&installer_path),
        installer_name: safe_name,
        launched: true,
        message: format!(
            "已下载并启动静默安装（目标版本 {}）。安装在后台进行、无向导界面；完成后请关闭本应用并重新打开。",
            check.latest_version.as_deref().unwrap_or("?")
        ),
    })
}

fn build_release_http_client(
    current_version: &str,
    timeout: std::time::Duration,
) -> AppResult<reqwest::blocking::Client> {
    reqwest::blocking::Client::builder()
        .timeout(timeout)
        .redirect(reqwest::redirect::Policy::limited(10))
        .user_agent(format!("video-tool/{current_version}"))
        .build()
        .map_err(|error| AppError::message(format!("创建 HTTP 客户端失败: {error}")))
}

/// Ok(Ok(body)) when release metadata is available; Ok(Err(result)) for soft HTTP/content failures.
fn resolve_latest_release_body(
    client: &reqwest::blocking::Client,
    current_version: &str,
    page: &str,
) -> AppResult<Result<serde_json::Value, UpdateCheckResult>> {
    let api_url = release_api_url();
    match fetch_release_json(client, &api_url)? {
        Ok(body) => Ok(Ok(body)),
        Err(404) => match release_list_fallback_url(&api_url) {
            Some(list_url) => match fetch_release_json(client, &list_url)? {
                Ok(list_body) => match first_stable_release_from_list(&list_body) {
                    Some(release) => Ok(Ok(release)),
                    None => Ok(Err(empty_update_check_result(
                        current_version,
                        page.to_string(),
                        format!(
                            "当前版本 {current_version}。尚未找到稳定发布版本。可打开发布页手动核对。"
                        ),
                    ))),
                },
                Err(list_status) => Ok(Err(update_check_http_error(
                    current_version,
                    page.to_string(),
                    list_status,
                ))),
            },
            None => Ok(Err(update_check_http_error(
                current_version,
                page.to_string(),
                404,
            ))),
        },
        Err(status_code) => Ok(Err(update_check_http_error(
            current_version,
            page.to_string(),
            status_code,
        ))),
    }
}

fn update_check_http_error(
    current_version: &str,
    page: String,
    status_code: u16,
) -> UpdateCheckResult {
    let message = match status_code {
        404 => format!(
            "当前版本 {current_version}。尚未找到发布版本，或仓库为私有且未配置 VIDEO_TOOL_GITHUB_TOKEN。可打开发布页手动核对。"
        ),
        401 | 403 => format!(
            "当前版本 {current_version}。远程鉴权失败（HTTP {status_code}）。私有仓库请设置 VIDEO_TOOL_GITHUB_TOKEN 后重试。"
        ),
        _ => format!(
            "当前版本 {current_version}。远程检查返回 HTTP {status_code}，请稍后重试或手动打开发布页。"
        ),
    };
    empty_update_check_result(current_version, page, message)
}

/// Prefer NSIS setup exe, then MSI; prefer x64 names on Windows.
fn pick_platform_installer_asset(release: &serde_json::Value) -> Option<ReleaseInstallerAsset> {
    #[cfg(target_os = "windows")]
    {
        let assets = release.get("assets")?.as_array()?;
        let mut best: Option<(i32, ReleaseInstallerAsset)> = None;
        for asset in assets {
            let name = asset
                .get("name")
                .and_then(|value| value.as_str())
                .unwrap_or("")
                .trim()
                .to_string();
            if name.is_empty() {
                continue;
            }
            let lower = name.to_ascii_lowercase();
            let url = asset
                .get("browser_download_url")
                .and_then(|value| value.as_str())
                .unwrap_or("")
                .trim()
                .to_string();
            if url.is_empty() || !is_trusted_release_asset_url(&url) {
                continue;
            }
            let size_bytes = asset.get("size").and_then(|value| value.as_u64());
            let mut score = 0;
            if lower.ends_with(".msi") {
                score += 40;
            } else if lower.ends_with("-setup.exe") || lower.ends_with("_x64-setup.exe") {
                score += 50;
            } else if lower.ends_with(".exe") {
                score += 30;
            } else {
                continue;
            }
            if lower.contains("x64") || lower.contains("x86_64") || lower.contains("amd64") {
                score += 20;
            }
            if lower.contains("arm64") || lower.contains("aarch64") {
                score -= 30;
            }
            if lower.contains("debug") {
                score -= 50;
            }
            let candidate = ReleaseInstallerAsset {
                url,
                name,
                size_bytes,
            };
            match &best {
                Some((best_score, _)) if *best_score >= score => {}
                _ => best = Some((score, candidate)),
            }
        }
        best.map(|(_, asset)| asset)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = release;
        None
    }
}

fn is_trusted_release_asset_url(url: &str) -> bool {
    let trimmed = url.trim();
    if !trimmed.starts_with("https://") {
        return false;
    }
    let prefix = format!(
        "https://github.com/{DEFAULT_RELEASE_OWNER}/{DEFAULT_RELEASE_REPO}/releases/download/"
    );
    if trimmed.starts_with(&prefix) {
        return true;
    }
    // Custom release API may point at another public GitHub repo's assets.
    if trimmed.starts_with("https://github.com/") && trimmed.contains("/releases/download/") {
        return true;
    }
    false
}

fn sanitize_installer_file_name(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-' | ' ') {
                ch
            } else {
                '_'
            }
        })
        .collect();
    let trimmed = cleaned.trim().trim_start_matches('.');
    if trimmed.is_empty() {
        "video-tool-setup.bin".to_string()
    } else {
        trimmed.to_string()
    }
}

/// Launch installer without interactive wizard UI.
/// - NSIS setup.exe: `/S` (silent)
/// - MSI: `msiexec /i … /qn /norestart` (quiet, no reboot prompt)
fn launch_installer_silent(path: &Path) -> AppResult<()> {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let extension = path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        let spawn_result = if extension == "msi" {
            // Quiet install: no wizard. Files in use may require restart after close.
            std::process::Command::new("msiexec")
                .args(["/i"])
                .arg(path)
                .args(["/qn", "/norestart"])
                .creation_flags(CREATE_NO_WINDOW)
                .spawn()
        } else {
            // Tauri NSIS installer supports /S for fully silent install.
            std::process::Command::new(path)
                .arg("/S")
                .creation_flags(CREATE_NO_WINDOW)
                .spawn()
        };
        spawn_result.map_err(|error| AppError::message(format!("启动静默安装失败: {error}")))?;
        Ok(())
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = path;
        Err(AppError::message(
            "当前平台尚未支持应用内静默安装，请打开发布页手动安装。".to_string(),
        ))
    }
}

/// GET release JSON. Ok(Ok(body)) on success, Ok(Err(status)) on non-success HTTP, Err on transport/parse.
fn fetch_release_json(
    client: &reqwest::blocking::Client,
    url: &str,
) -> AppResult<Result<serde_json::Value, u16>> {
    let mut request = client
        .get(url)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28");
    if let Some(token) = release_github_token() {
        request = request.bearer_auth(token);
    }
    let response = request
        .send()
        .map_err(|error| AppError::message(format!("检查更新失败: {error}")))?;
    let status = response.status();
    if !status.is_success() {
        return Ok(Err(status.as_u16()));
    }
    let body: serde_json::Value = response
        .json()
        .map_err(|error| AppError::message(format!("解析更新响应失败: {error}")))?;
    Ok(Ok(body))
}

/// When `.../releases/latest` fails, try `.../releases?per_page=10`.
fn release_list_fallback_url(api_url: &str) -> Option<String> {
    let trimmed = api_url.trim().trim_end_matches('/');
    if let Some(base) = trimmed.strip_suffix("/releases/latest") {
        return Some(format!("{base}/releases?per_page=10"));
    }
    if trimmed.ends_with("/releases") {
        return Some(format!("{trimmed}?per_page=10"));
    }
    None
}

fn first_stable_release_from_list(body: &serde_json::Value) -> Option<serde_json::Value> {
    let releases = body.as_array()?;
    releases
        .iter()
        .find(|release| {
            let draft = release
                .get("draft")
                .and_then(|value| value.as_bool())
                .unwrap_or(false);
            let prerelease = release
                .get("prerelease")
                .and_then(|value| value.as_bool())
                .unwrap_or(false);
            !draft && !prerelease
        })
        .cloned()
}

/// Compare dotted numeric versions (e.g. 1.2.3). Non-numeric tails are ignored.
pub fn version_is_newer(candidate: &str, current: &str) -> bool {
    let parse = |raw: &str| -> Vec<u64> {
        raw.split(|ch: char| !ch.is_ascii_digit())
            .filter(|part| !part.is_empty())
            .filter_map(|part| part.parse::<u64>().ok())
            .collect()
    };
    let left = parse(candidate);
    let right = parse(current);
    let max_len = left.len().max(right.len());
    for index in 0..max_len {
        let left_part = left.get(index).copied().unwrap_or(0);
        let right_part = right.get(index).copied().unwrap_or(0);
        if left_part > right_part {
            return true;
        }
        if left_part < right_part {
            return false;
        }
    }
    false
}

pub fn build_system_diagnostics(
    config: &AppConfig,
    sidecars: SidecarStatus,
    active_runner_job_ids: &std::collections::HashSet<String>,
) -> AppResult<SystemDiagnostics> {
    let public: AppConfigPublic = config.public_view();
    let dependency = build_dependency_report(&sidecars);
    let models = scan_models(config);
    let workspace_health = workspace::inspect_workspace_health(
        config.workspace_path(),
        config.min_free_disk_gb,
        active_runner_job_ids,
    )?;
    Ok(SystemDiagnostics {
        app_name: "video-tool".to_string(),
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        config_path: public.config_path,
        workspace_dir: config.workspace_dir.clone(),
        free_disk_gb: workspace_health.free_disk_gb,
        min_free_disk_gb: config.min_free_disk_gb,
        disk_below_threshold: workspace_health.disk_below_threshold,
        sidecars,
        dependency,
        models,
        workspace_health,
    })
}

fn path_to_string(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn paths_equal(left: &str, right: &str) -> bool {
    left.replace('\\', "/")
        .eq_ignore_ascii_case(&right.replace('\\', "/"))
}

fn is_sensitive_header_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.contains("authorization")
        || lower.contains("api-key")
        || lower.contains("apikey")
        || lower.contains("x-api-key")
        || lower.contains("token")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AppConfig;
    use crate::sidecar::{BinarySource, ResolvedBinary, SidecarStatus};
    use uuid::Uuid;

    fn missing_binary(name: &str) -> ResolvedBinary {
        ResolvedBinary {
            name: name.to_string(),
            path: None,
            version: None,
            source: BinarySource::Missing,
        }
    }

    fn ready_binary(name: &str) -> ResolvedBinary {
        ResolvedBinary {
            name: name.to_string(),
            path: Some(format!("/bin/{name}")),
            version: Some("1.0".into()),
            source: BinarySource::Path,
        }
    }

    #[test]
    fn dependency_report_flags_missing_required() {
        let status = SidecarStatus {
            ffmpeg: missing_binary("ffmpeg"),
            ffprobe: ready_binary("ffprobe"),
            yt_dlp: ready_binary("yt-dlp"),
            streamlink: missing_binary("streamlink"),
            transcribe: ready_binary("transcribe"),
        };
        let report = build_dependency_report(&status);
        assert!(!report.all_required_ready);
        assert!(report.missing_required.iter().any(|name| name == "FFmpeg"));
        assert!(!report
            .missing_required
            .iter()
            .any(|name| name == "Streamlink"));
    }

    #[test]
    fn export_strips_api_keys_by_default() {
        let mut config = AppConfig::default();
        config.providers[0].api_key = Some("sk-secret".into());
        let package = export_config_package(&config, false);
        assert!(!package.include_secrets);
        assert!(package
            .providers
            .iter()
            .all(|provider| provider.api_key.is_none()));
    }

    #[test]
    fn import_preserves_local_keys_when_stripped() {
        let mut current = AppConfig::default();
        current.providers[0].api_key = Some("local-key".into());
        let package = export_config_package(&current, false);
        let (imported, _) = apply_import_package(&current, package, false).expect("import");
        assert_eq!(imported.providers[0].api_key.as_deref(), Some("local-key"));
    }

    #[test]
    fn version_compare_detects_newer() {
        assert!(version_is_newer("0.2.0", "0.1.0"));
        assert!(version_is_newer("v1.0.1", "1.0.0"));
        assert!(version_is_newer("1.0", "0.9.9"));
        assert!(!version_is_newer("1.0.0", "1.0.0"));
        assert!(!version_is_newer("0.9.9", "1.0.0"));
        assert!(!version_is_newer("1.0.0-beta", "1.0.0"));
    }

    #[test]
    fn default_release_urls_point_at_project_repo() {
        assert!(release_api_url().contains("627157746/video-tool"));
        assert!(release_api_url().ends_with("/releases/latest"));
        assert_eq!(
            release_page_url(),
            "https://github.com/627157746/video-tool/releases"
        );
    }

    #[test]
    fn release_list_fallback_url_from_latest() {
        let fallback = release_list_fallback_url(
            "https://api.github.com/repos/627157746/video-tool/releases/latest",
        );
        assert_eq!(
            fallback.as_deref(),
            Some("https://api.github.com/repos/627157746/video-tool/releases?per_page=10")
        );
    }

    #[test]
    fn first_stable_release_skips_draft_and_prerelease() {
        let body = serde_json::json!([
            { "tag_name": "v0.3.0-rc1", "draft": false, "prerelease": true },
            { "tag_name": "v0.2.0", "draft": false, "prerelease": false, "body": "notes" }
        ]);
        let release = first_stable_release_from_list(&body).expect("stable release");
        assert_eq!(
            release.get("tag_name").and_then(|value| value.as_str()),
            Some("v0.2.0")
        );
    }

    #[test]
    fn trusted_release_asset_url_accepts_project_download() {
        assert!(is_trusted_release_asset_url(
            "https://github.com/627157746/video-tool/releases/download/v0.2.1/video-tool_0.2.1_x64_en-US.msi"
        ));
        assert!(!is_trusted_release_asset_url("http://evil.example/a.msi"));
        assert!(!is_trusted_release_asset_url(
            "https://evil.example/github.com/627157746/video-tool/releases/download/x.msi"
        ));
    }

    #[test]
    fn sanitize_installer_name_strips_path_separators() {
        assert_eq!(
            sanitize_installer_file_name(r"..\evil/video-tool.msi"),
            "_evil_video-tool.msi"
        );
        assert_eq!(sanitize_installer_file_name(""), "video-tool-setup.bin");
    }

    #[test]
    fn pick_windows_installer_prefers_setup_exe() {
        let release = serde_json::json!({
            "assets": [
                {
                    "name": "notes.txt",
                    "browser_download_url": "https://github.com/627157746/video-tool/releases/download/v1/notes.txt",
                    "size": 12
                },
                {
                    "name": "video-tool_1.0.0_x64_en-US.msi",
                    "browser_download_url": "https://github.com/627157746/video-tool/releases/download/v1/video-tool_1.0.0_x64_en-US.msi",
                    "size": 100
                },
                {
                    "name": "video-tool_1.0.0_x64-setup.exe",
                    "browser_download_url": "https://github.com/627157746/video-tool/releases/download/v1/video-tool_1.0.0_x64-setup.exe",
                    "size": 120
                }
            ]
        });
        let asset = pick_platform_installer_asset(&release).expect("installer");
        assert!(asset.name.ends_with("-setup.exe") || asset.name.ends_with(".msi"));
        assert!(asset.url.contains("/releases/download/"));
    }

    #[test]
    fn scans_model_directory() {
        let root = std::env::temp_dir().join(format!("video-tool-models-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let model_path = root.join("ggml-base.bin");
        fs::write(&model_path, b"fake").unwrap();
        let config = AppConfig {
            transcribe_model: Some(path_to_string(&model_path)),
            transcribe_model_preset: "custom".into(),
            ..AppConfig::default()
        };
        let inventory = scan_models(&config);
        assert!(inventory.selected_exists);
        assert!(inventory
            .models
            .iter()
            .any(|model| model.file_name == "ggml-base.bin"));
        let _ = fs::remove_dir_all(root);
    }
}

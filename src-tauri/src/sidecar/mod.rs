use crate::config::SidecarPaths;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedBinary {
    pub name: String,
    pub path: Option<String>,
    pub version: Option<String>,
    pub source: BinarySource,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BinarySource {
    Bundled,
    Configured,
    Path,
    Missing,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SidecarStatus {
    pub ffmpeg: ResolvedBinary,
    pub ffprobe: ResolvedBinary,
    pub yt_dlp: ResolvedBinary,
    pub streamlink: ResolvedBinary,
    pub transcribe: ResolvedBinary,
}

pub fn resolve_all(configured: &SidecarPaths, app_dir: Option<&Path>) -> SidecarStatus {
    SidecarStatus {
        ffmpeg: resolve_binary("ffmpeg", configured.ffmpeg.as_deref(), app_dir, &["-version"]),
        ffprobe: resolve_binary(
            "ffprobe",
            configured.ffprobe.as_deref(),
            app_dir,
            &["-version"],
        ),
        yt_dlp: resolve_binary("yt-dlp", configured.yt_dlp.as_deref(), app_dir, &["--version"]),
        streamlink: resolve_binary(
            "streamlink",
            configured.streamlink.as_deref(),
            app_dir,
            &["--version"],
        ),
        transcribe: resolve_binary(
            "transcribe",
            configured.transcribe.as_deref(),
            app_dir,
            &["--help"],
        ),
    }
}

fn resolve_binary(
    name: &str,
    configured_path: Option<&str>,
    app_dir: Option<&Path>,
    version_args: &[&str],
) -> ResolvedBinary {
    // PRODUCT_SPEC order: bundled → user-configured → PATH
    if let Some(app_dir) = app_dir {
        for candidate in bundled_candidates(app_dir, name) {
            if candidate.exists() {
                return with_version(name, candidate, BinarySource::Bundled, version_args);
            }
        }
    }

    if let Some(configured) = configured_path {
        let path = PathBuf::from(configured);
        if path.exists() {
            return with_version(name, path, BinarySource::Configured, version_args);
        }
    }

    if let Ok(path) = which::which(name) {
        return with_version(name, path, BinarySource::Path, version_args);
    }

    if cfg!(windows) {
        if let Ok(path) = which::which(format!("{name}.exe")) {
            return with_version(name, path, BinarySource::Path, version_args);
        }
    }

    ResolvedBinary {
        name: name.to_string(),
        path: None,
        version: None,
        source: BinarySource::Missing,
    }
}

fn bundled_candidates(app_dir: &Path, name: &str) -> Vec<PathBuf> {
    let mut names = vec![name.to_string()];
    if cfg!(windows) {
        names.push(format!("{name}.exe"));
    }

    let mut candidates = Vec::new();
    for file_name in names {
        candidates.push(app_dir.join("sidecars").join(&file_name));
        candidates.push(app_dir.join("bin").join(&file_name));
        candidates.push(app_dir.join(&file_name));
    }
    candidates
}

fn with_version(
    name: &str,
    path: PathBuf,
    source: BinarySource,
    version_args: &[&str],
) -> ResolvedBinary {
    let version = probe_version(&path, version_args);
    ResolvedBinary {
        name: name.to_string(),
        path: Some(path.to_string_lossy().replace('\\', "/")),
        version,
        source,
    }
}

fn probe_version(path: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new(path).args(args).output().ok()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = if !stdout.trim().is_empty() {
        stdout
    } else {
        stderr
    };
    combined
        .lines()
        .next()
        .map(|line| line.trim().to_string())
        .filter(|line| !line.is_empty())
}

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
        ffmpeg: resolve_binary(
            "ffmpeg",
            configured.ffmpeg.as_deref(),
            app_dir,
            &["-version"],
        ),
        ffprobe: resolve_binary(
            "ffprobe",
            configured.ffprobe.as_deref(),
            app_dir,
            &["-version"],
        ),
        yt_dlp: resolve_binary(
            "yt-dlp",
            configured.yt_dlp.as_deref(),
            app_dir,
            &["--version"],
        ),
        streamlink: resolve_binary(
            "streamlink",
            configured.streamlink.as_deref(),
            app_dir,
            &["--version"],
        ),
        transcribe: resolve_transcribe(configured.transcribe.as_deref(), app_dir),
    }
}

fn resolve_transcribe(configured_path: Option<&str>, app_dir: Option<&Path>) -> ResolvedBinary {
    let aliases = ["whisper-cli", "whisper-cpp", "main"];
    if let Some(app_dir) = app_dir {
        for alias in aliases {
            for candidate in bundled_candidates(app_dir, alias) {
                if candidate.exists() {
                    return ResolvedBinary {
                        name: "transcribe".to_string(),
                        ..with_version(alias, candidate, BinarySource::Bundled, &["--help"])
                    };
                }
            }
        }
    }

    if let Some(configured_path) = configured_path {
        let configured_path = PathBuf::from(configured_path);
        if configured_path.exists() {
            return ResolvedBinary {
                name: "transcribe".to_string(),
                ..with_version(
                    "whisper-cli",
                    configured_path,
                    BinarySource::Configured,
                    &["--help"],
                )
            };
        }
    }

    for alias in aliases {
        let path_result = which::which(alias).or_else(|_| which::which(format!("{alias}.exe")));
        if let Ok(path) = path_result {
            return ResolvedBinary {
                name: "transcribe".to_string(),
                ..with_version(alias, path, BinarySource::Path, &["--help"])
            };
        }
    }

    ResolvedBinary {
        name: "transcribe".to_string(),
        path: None,
        version: None,
        source: BinarySource::Missing,
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn executable_name(name: &str) -> String {
        if cfg!(windows) {
            format!("{name}.exe")
        } else {
            name.to_string()
        }
    }

    #[test]
    fn bundled_binary_has_priority_over_configured_binary() {
        let root = std::env::temp_dir().join(format!(
            "video-tool-sidecar-priority-{}",
            uuid::Uuid::new_v4()
        ));
        let bundled_directory = root.join("sidecars");
        fs::create_dir_all(&bundled_directory).expect("create bundled directory");
        let bundled_path = bundled_directory.join(executable_name("ffmpeg"));
        let configured_path = root.join(executable_name("configured-ffmpeg"));
        fs::write(&bundled_path, b"fixture").expect("write bundled fixture");
        fs::write(&configured_path, b"fixture").expect("write configured fixture");

        let resolved = resolve_binary(
            "ffmpeg",
            configured_path.to_str(),
            Some(&root),
            &["--version"],
        );

        assert_eq!(resolved.source, BinarySource::Bundled);
        let expected_path = bundled_path.to_string_lossy().replace('\\', "/");
        assert_eq!(resolved.path.as_deref(), Some(expected_path.as_str()));
        fs::remove_dir_all(root).expect("remove sidecar fixture");
    }

    #[test]
    fn configured_binary_is_used_when_bundle_is_missing() {
        let root = std::env::temp_dir().join(format!(
            "video-tool-sidecar-configured-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&root).expect("create sidecar directory");
        let configured_path = root.join(executable_name("configured-ffmpeg"));
        fs::write(&configured_path, b"fixture").expect("write configured fixture");

        let resolved = resolve_binary(
            "ffmpeg",
            configured_path.to_str(),
            Some(&root),
            &["--version"],
        );

        assert_eq!(resolved.source, BinarySource::Configured);
        fs::remove_dir_all(root).expect("remove sidecar fixture");
    }

    #[test]
    fn bundled_transcribe_alias_beats_configured_binary() {
        let root = std::env::temp_dir().join(format!(
            "video-tool-transcribe-priority-{}",
            uuid::Uuid::new_v4()
        ));
        let bundled_directory = root.join("sidecars");
        fs::create_dir_all(&bundled_directory).expect("create bundled directory");
        let bundled_path = bundled_directory.join(executable_name("whisper-cpp"));
        let configured_path = root.join(executable_name("configured-whisper"));
        fs::write(&bundled_path, b"fixture").expect("write bundled fixture");
        fs::write(&configured_path, b"fixture").expect("write configured fixture");

        let resolved = resolve_transcribe(configured_path.to_str(), Some(&root));

        assert_eq!(resolved.source, BinarySource::Bundled);
        let expected_path = bundled_path.to_string_lossy().replace('\\', "/");
        assert_eq!(resolved.path.as_deref(), Some(expected_path.as_str()));
        fs::remove_dir_all(root).expect("remove sidecar fixture");
    }

    #[test]
    fn path_lookup_is_the_final_fallback() {
        if which::which("rustc").is_err() {
            return;
        }
        let resolved = resolve_binary("rustc", None, None, &["--version"]);
        assert_eq!(resolved.source, BinarySource::Path);
    }
}

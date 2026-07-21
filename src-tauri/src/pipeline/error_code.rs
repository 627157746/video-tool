//! Stable machine-readable failure codes for recovery UI (v0.2 P1).
//!
//! Classification is intentionally heuristic over human-readable messages so
//! existing call sites keep `AppError::message(...)` without a large refactor.
//! Prefer adding keyword patterns here when new failure classes appear.

use crate::models::{JobKind, JobStep};

pub const SIDECAR_MISSING: &str = "SIDECAR_MISSING";
pub const AUTH_REQUIRED: &str = "AUTH_REQUIRED";
pub const CONTEXT_TOO_LONG: &str = "CONTEXT_TOO_LONG";
pub const DISK_GUARD: &str = "DISK_GUARD";
pub const NETWORK: &str = "NETWORK";
pub const DOWNLOAD_FAILED: &str = "DOWNLOAD_FAILED";
pub const TRANSCRIBE_FAILED: &str = "TRANSCRIBE_FAILED";
pub const SUMMARIZE_FAILED: &str = "SUMMARIZE_FAILED";
pub const INTERRUPTED: &str = "INTERRUPTED";
pub const UNKNOWN: &str = "UNKNOWN";

/// Classify a failure from redacted error text and optional pipeline step / job kind.
pub fn classify_error(message: &str, step: Option<&JobStep>, kind: Option<&JobKind>) -> String {
    let lower = message.to_lowercase();
    let compact = lower.replace([' ', '\n', '\r', '\t'], "");

    if contains_any(
        &lower,
        &[
            "用户停止",
            "已请求停止",
            "stop requested",
            "interrupted",
            "cancelled",
            "canceled",
        ],
    ) {
        return INTERRUPTED.to_string();
    }

    if contains_any(
        &lower,
        &[
            "磁盘空间不足",
            "disk space",
            "no space left",
            "enospc",
            "min_free_disk",
            "free disk",
        ],
    ) {
        return DISK_GUARD.to_string();
    }

    if contains_any(
        &lower,
        &[
            "context too long",
            "上下文过长",
            "token limit",
            "maximum context",
            "max_context",
            "context length",
            "too many tokens",
            "prompt is too long",
            "input is too long",
        ],
    ) {
        return CONTEXT_TOO_LONG.to_string();
    }

    if contains_any(
        &lower,
        &[
            "cookie",
            "cookies",
            "登录",
            "未登录",
            "sign in",
            "login required",
            "authentication",
            "unauthorized",
            "401",
            "403",
            "private video",
            "members only",
            "age-restricted",
            "age restricted",
            "confirm your age",
            "auth required",
            "需要登录",
            "鉴权",
        ],
    ) {
        return AUTH_REQUIRED.to_string();
    }

    if contains_any(
        &lower,
        &[
            "未找到",
            "not found",
            "no such file",
            "sidecar",
            "whisper-cli",
            "whisper.cpp",
            "yt-dlp",
            "ffmpeg",
            "streamlink",
            "可执行文件",
            "executable",
            "配置转写可执行",
            "配置",
        ],
    ) && contains_any(
        &lower,
        &[
            "未找到",
            "not found",
            "no such file",
            "不存在",
            "missing",
            "please install",
            "请在设置",
            "请配置",
            "未配置",
        ],
    ) {
        return SIDECAR_MISSING.to_string();
    }

    if contains_any(
        &lower,
        &[
            "network",
            "connection",
            "timed out",
            "timeout",
            "dns",
            "tls",
            "ssl",
            "proxy",
            "econnreset",
            "econnrefused",
            "unreachable",
            "无法连接",
            "连接失败",
            "网络",
            "http 客户端",
        ],
    ) {
        return NETWORK.to_string();
    }

    if matches!(step, Some(JobStep::Summarize))
        || contains_any(
            &lower,
            &[
                "总结",
                "summarize",
                "provider",
                "api key",
                "chat/completions",
                "base_url",
            ],
        )
    {
        return SUMMARIZE_FAILED.to_string();
    }

    if matches!(
        step,
        Some(JobStep::Transcribe) | Some(JobStep::MergeTranscript)
    ) || contains_any(&lower, &["转写", "transcribe", "whisper"])
    {
        return TRANSCRIBE_FAILED.to_string();
    }

    if matches!(step, Some(JobStep::Ingest))
        || matches!(kind, Some(JobKind::Download) | Some(JobKind::LiveRecord))
        || contains_any(
            &lower,
            &["下载", "download", "yt-dlp", "streamlink", "录制", "ingest"],
        )
    {
        // Prefer DOWNLOAD_FAILED over UNKNOWN for ingest-shaped failures.
        if !compact.is_empty() {
            return DOWNLOAD_FAILED.to_string();
        }
    }

    UNKNOWN.to_string()
}

fn contains_any(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| {
        if needle.is_ascii() {
            haystack.contains(&needle.to_lowercase())
        } else {
            haystack.contains(needle)
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_sidecar_missing() {
        let code = classify_error(
            "未找到 whisper.cpp whisper-cli，请在设置中配置转写可执行文件",
            Some(&JobStep::Transcribe),
            None,
        );
        assert_eq!(code, SIDECAR_MISSING);
    }

    #[test]
    fn classifies_disk_guard() {
        let code = classify_error("磁盘空间不足：需要至少 5 GB 可用", None, None);
        assert_eq!(code, DISK_GUARD);
    }

    #[test]
    fn classifies_auth_cookie() {
        let code = classify_error(
            "Cookie 模式为文件，但未配置 cookies.txt 路径",
            Some(&JobStep::Ingest),
            Some(&JobKind::Download),
        );
        assert_eq!(code, AUTH_REQUIRED);
    }

    #[test]
    fn classifies_context_too_long() {
        let code = classify_error(
            "context length exceeded / 上下文过长",
            Some(&JobStep::Summarize),
            None,
        );
        assert_eq!(code, CONTEXT_TOO_LONG);
    }

    #[test]
    fn classifies_network() {
        let code = classify_error("HTTP 客户端初始化失败: connection timed out", None, None);
        assert_eq!(code, NETWORK);
    }

    #[test]
    fn classifies_interrupted() {
        let code = classify_error("用户已请求停止录制", Some(&JobStep::Ingest), None);
        assert_eq!(code, INTERRUPTED);
    }

    #[test]
    fn step_fallback_transcribe() {
        let code = classify_error("something went wrong", Some(&JobStep::Transcribe), None);
        assert_eq!(code, TRANSCRIBE_FAILED);
    }

    #[test]
    fn step_fallback_summarize() {
        let code = classify_error("provider returned 500", Some(&JobStep::Summarize), None);
        assert_eq!(code, SUMMARIZE_FAILED);
    }
}

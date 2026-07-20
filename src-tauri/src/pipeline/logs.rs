use crate::error::AppResult;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

pub fn log_path(job_dir: &Path, step_name: &str) -> PathBuf {
    job_dir.join("logs").join(format!("{step_name}.log"))
}

pub fn append_log(job_dir: &Path, step_name: &str, message: &str) -> AppResult<()> {
    let logs_dir = job_dir.join("logs");
    fs::create_dir_all(&logs_dir)?;
    let path = log_path(job_dir, step_name);
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    let stamped = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ");
    let redacted_message = redact_secrets(message, &[]);
    let body = if redacted_message.ends_with('\n') {
        redacted_message
    } else {
        format!("{redacted_message}\n")
    };
    // Keep multi-line tool output readable; only stamp single-line heartbeats if caller asks.
    if body.lines().count() == 1 && body.contains("heartbeat") {
        write!(file, "[{stamped}] {body}")?;
    } else {
        file.write_all(body.as_bytes())?;
    }
    Ok(())
}

pub fn read_log(job_dir: &Path, step_name: &str, max_chars: usize) -> AppResult<String> {
    let path = log_path(job_dir, step_name);
    if !path.exists() {
        return Ok(String::new());
    }
    let raw = fs::read_to_string(path)?;
    if raw.chars().count() <= max_chars {
        return Ok(raw);
    }
    let skipped = raw.chars().count().saturating_sub(max_chars);
    let tail: String = raw.chars().skip(skipped).collect();
    Ok(format!("…（已截断前部日志）\n{tail}"))
}

pub fn clear_log(job_dir: &Path, step_name: &str) -> AppResult<()> {
    let path = log_path(job_dir, step_name);
    if path.exists() {
        fs::write(path, "")?;
    }
    Ok(())
}

/// Redact secrets from log/prompt text before persistence.
pub fn redact_secrets(text: &str, secrets: &[String]) -> String {
    let mut output = text.to_string();
    for secret in secrets {
        let trimmed = secret.trim();
        if trimmed.len() < 6 {
            continue;
        }
        output = output.replace(trimmed, "***REDACTED***");
    }
    // Common env-style key patterns in pasted logs
    let patterns = [
        (
            r#"(?i)([\"']?api[_-]?key[\"']?\s*[:=]\s*[\"']?)([^\"'\s,;&]+)"#,
            "${1}***REDACTED***",
        ),
        (
            r"(?i)(authorization\s*:\s*bearer\s+)([^\s]+)",
            "${1}***REDACTED***",
        ),
        (r"(?i)(x-api-key\s*:\s*)([^\s]+)", "${1}***REDACTED***"),
        (
            r"(?i)([?&](?:access[_-]?token|token|api[_-]?key|sig|signature|auth|authorization|credential|x-amz-signature|x-amz-credential|x-amz-security-token)=)([^&\s]+)",
            "${1}***REDACTED***",
        ),
        (
            r"(?i)(https?://[^/@:\s]+:)([^@/\s]+)(@)",
            "${1}***REDACTED***${3}",
        ),
    ];
    for (pattern, replacement) in patterns {
        if let Ok(regex) = regex::Regex::new(pattern) {
            output = regex.replace_all(&output, replacement).to_string();
        }
    }
    output
}

pub fn truncate_for_log(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let kept: String = text.chars().take(max_chars).collect();
    format!(
        "{kept}\n…（prompt 已截断，共 {} 字符）",
        text.chars().count()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_explicit_secrets_and_bearer() {
        let text = "Authorization: Bearer sk-abc123456789 and key=sk-abc123456789";
        let redacted = redact_secrets(text, &["sk-abc123456789".to_string()]);
        assert!(!redacted.contains("sk-abc123456789"));
        assert!(redacted.contains("***REDACTED***"));
    }

    #[test]
    fn redacts_signed_urls_and_json_api_keys() {
        let text = concat!(
            "https://example.com/video?token=secret-token&quality=best ",
            r#"{"api_key":"secret-json-key"}"#,
        );
        let redacted = redact_secrets(text, &[]);
        assert!(!redacted.contains("secret-token"));
        assert!(!redacted.contains("secret-json-key"));
        assert!(redacted.contains("quality=best"));
    }

    #[test]
    fn truncates_long_prompt() {
        let text = "a".repeat(100);
        let out = truncate_for_log(&text, 20);
        assert!(out.contains("已截断"));
        assert!(out.starts_with("aaaaaaaaaaaaaaaaaaaa"));
    }
}

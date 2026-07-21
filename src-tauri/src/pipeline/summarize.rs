use super::{logs, paths};
use crate::config::{AppConfig, ProviderProfile};
use crate::error::{AppError, AppResult};
use crate::models::Job;
use reqwest::blocking::{Client, RequestBuilder};
use reqwest::header::{HeaderName, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use serde_json::{json, Value};
use std::fs;
use std::path::Path;
use std::time::Duration;

pub fn summarize_job(job_dir: &Path, job: &mut Job, config: &AppConfig) -> AppResult<String> {
    let transcript_path = job_dir.join("transcript").join("plain.txt");
    if !transcript_path.exists() {
        return Err(AppError::message(
            "缺少 transcript/plain.txt，请先运行转写和合并步骤",
        ));
    }
    let transcript = fs::read_to_string(&transcript_path)?;
    let character_count = transcript.chars().count();
    if character_count > config.max_context_chars {
        return Err(AppError::message(format!(
            "合并全文约 {character_count} 字符，超过当前限制 {}。请选择较少分段重新合并，或提高限制/改用更大上下文模型；内容未截断。",
            config.max_context_chars
        )));
    }

    let provider = resolve_provider(job, config)?;
    let model_name = resolve_model(job, provider)?;
    let available_ids: Vec<String> = config.templates.iter().map(|t| t.id.clone()).collect();
    let template_ids = job
        .pipeline
        .effective_template_ids(config.default_template_id.as_deref(), &available_ids);
    if template_ids.is_empty() {
        return Err(AppError::message("未选择总结模板"));
    }

    let api_key = config.resolve_api_key(provider).ok_or_else(|| {
        AppError::message(format!(
            "Provider“{}”缺少 API Key；请配置 {} 或在配置文件填写 Key",
            provider.name,
            provider.api_key_env.as_deref().unwrap_or("对应环境变量")
        ))
    })?;
    let secret_values = config.secret_values();
    let chapters_text = super::chapterize::chapters_template_text(job_dir);

    paths::ensure_job_layout(job_dir)?;
    logs::clear_log(job_dir, "summarize")?;
    let client = build_client(config.proxy_url.as_deref())?;
    let summary_dir = paths::summary_dir(job_dir);
    fs::create_dir_all(&summary_dir)?;
    let by_template_dir = paths::summary_by_template_dir(job_dir);
    if template_ids.len() > 1 {
        fs::create_dir_all(&by_template_dir)?;
    }

    let mut primary_summary = String::new();
    let mut template_results: Vec<Value> = Vec::new();
    let mut first_error: Option<String> = None;
    let mut success_count = 0usize;

    for (index, template_id) in template_ids.iter().enumerate() {
        let template = config
            .templates
            .iter()
            .find(|entry| &entry.id == template_id)
            .ok_or_else(|| AppError::message(format!("总结模板不存在: {template_id}")))?;

        let user_prompt = render_template(
            &template.user_template,
            &job.display_title(),
            job.source.url.as_deref().unwrap_or("本地文件"),
            job.duration_label.as_deref().unwrap_or("未知"),
            &transcript,
            &chapters_text,
        );
        let redacted_prompt = logs::redact_secrets(&user_prompt, &secret_values);
        let safe_prompt = logs::truncate_for_log(&redacted_prompt, 2_000);
        logs::append_log(
            job_dir,
            "summarize",
            &format!(
                "=== template {} ({}/{}) ===\nprovider: {}\nprotocol: {}\nmodel: {}\nbase_url: {}\nprompt_preview:\n{}\n",
                template.id,
                index + 1,
                template_ids.len(),
                provider.id,
                provider.protocol,
                model_name,
                provider.base_url,
                safe_prompt
            ),
        )?;

        let call_result = match provider.protocol.as_str() {
            "openai" => call_openai(
                &client,
                provider,
                &model_name,
                &api_key,
                &secret_values,
                &template.system_prompt,
                &user_prompt,
            ),
            "anthropic" => call_anthropic(
                &client,
                provider,
                &model_name,
                &api_key,
                &secret_values,
                &template.system_prompt,
                &user_prompt,
            ),
            protocol => Err(AppError::message(format!(
                "不支持的 Provider 协议: {protocol}"
            ))),
        };

        match call_result {
            Ok(raw_summary) => {
                let summary = unwrap_outer_markdown_fence(&raw_summary);
                let relative_path = if index == 0 {
                    fs::write(summary_dir.join("summary.md"), &summary)?;
                    "summary/summary.md".to_string()
                } else {
                    let file_name = sanitize_template_file_name(&template.id);
                    fs::write(by_template_dir.join(format!("{file_name}.md")), &summary)?;
                    format!("summary/by_template/{file_name}.md")
                };
                if index == 0 {
                    primary_summary = summary;
                }
                success_count += 1;
                template_results.push(json!({
                    "template_id": template.id,
                    "status": "succeeded",
                    "path": relative_path,
                    "primary": index == 0,
                }));
                logs::append_log(
                    job_dir,
                    "summarize",
                    &format!("template {} saved: {relative_path}", template.id),
                )?;
            }
            Err(error) => {
                let detail = error.to_string();
                logs::append_log(
                    job_dir,
                    "summarize",
                    &format!("template {} failed: {detail}", template.id),
                )?;
                template_results.push(json!({
                    "template_id": template.id,
                    "status": "failed",
                    "error": detail,
                    "primary": index == 0,
                }));
                if first_error.is_none() {
                    first_error = Some(detail);
                }
            }
        }
    }

    fs::write(
        summary_dir.join("meta.json"),
        serde_json::to_string_pretty(&json!({
            "provider_profile_id": provider.id,
            "template_id": template_ids.first(),
            "template_ids": template_ids,
            "templates": template_results,
            "model": model_name,
            "protocol": provider.protocol,
            "created_at": chrono::Utc::now(),
            "input_characters": character_count,
            "selected_segment_ids": job.selected_segment_ids,
            "succeeded": success_count,
            "requested": template_ids.len(),
        }))?,
    )?;

    if success_count == 0 {
        return Err(AppError::message(
            first_error.unwrap_or_else(|| "所有总结模板均失败".to_string()),
        ));
    }

    job.summary_path = Some("summary/summary.md".to_string());
    if let Some(error_detail) = first_error {
        // Partial success: keep artifacts, surface a soft warning via log only.
        logs::append_log(
            job_dir,
            "summarize",
            &format!(
                "partial success: {success_count}/{} templates; first error: {error_detail}",
                template_ids.len()
            ),
        )?;
    }
    Ok(primary_summary)
}

fn sanitize_template_file_name(template_id: &str) -> String {
    let sanitized: String = template_id
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect();
    if sanitized.is_empty() {
        "template".to_string()
    } else {
        sanitized
    }
}

pub fn test_provider(config: &AppConfig, provider_id: &str) -> AppResult<String> {
    let provider = config
        .providers
        .iter()
        .find(|profile| profile.id == provider_id)
        .ok_or_else(|| AppError::message(format!("Provider 不存在: {provider_id}")))?;
    let api_key = config
        .resolve_api_key(provider)
        .ok_or_else(|| AppError::message("Provider 缺少 API Key"))?;
    let secret_values = config.secret_values();
    let client = build_client(config.proxy_url.as_deref())?;
    let endpoint = models_endpoint(provider);
    let mut request = client.get(&endpoint);
    request = apply_provider_headers(request, provider, &api_key)?;
    let response = request
        .send()
        .map_err(|error| AppError::message(format!("Provider 连接失败（{endpoint}）: {error}")))?;
    let status = response.status();
    let body = response.text().unwrap_or_default();
    if !status.is_success() {
        return Err(AppError::message(format!(
            "Provider 连通测试失败：HTTP {status}，{}",
            safe_http_error(&body, &secret_values)
        )));
    }
    Ok(format!(
        "Provider“{}”连通成功：HTTP {status}",
        provider.name
    ))
}

pub fn render_template(
    template: &str,
    title: &str,
    source_url: &str,
    duration: &str,
    transcript: &str,
    chapters: &str,
) -> String {
    template
        .replace("{{title}}", title)
        .replace("{{source_url}}", source_url)
        .replace("{{duration}}", duration)
        .replace("{{transcript}}", transcript)
        .replace("{{chapters}}", chapters)
}

/// Strip a single outer fenced code block when the model wraps the whole answer.
///
/// Common failure mode: response starts with ` ```markdown ` and the UI then
/// renders the entire summary as a monospaced code block (headings stay as `#`).
/// Only the outermost document fence is removed; nested code blocks inside the
/// body are preserved. A missing closing fence is still unwrapped.
pub fn unwrap_outer_markdown_fence(text: &str) -> String {
    let trimmed = text.trim();
    if !trimmed.starts_with("```") {
        return trimmed.to_string();
    }

    let mut lines = trimmed.lines();
    let opening_line = match lines.next() {
        Some(line) => line.trim(),
        None => return trimmed.to_string(),
    };
    // Opening fence may be bare ``` or ```markdown / ```md / ```Markdown
    if !opening_line.starts_with("```") {
        return trimmed.to_string();
    }
    let language_tag = opening_line.trim_start_matches('`').trim();
    if language_tag.chars().any(|character| {
        !(character.is_ascii_alphanumeric() || character == '-' || character == '_')
    }) {
        return trimmed.to_string();
    }

    let mut body_lines: Vec<&str> = lines.collect();
    if body_lines.last().is_some_and(|line| line.trim() == "```") {
        body_lines.pop();
    }

    body_lines.join("\n").trim().to_string()
}

fn resolve_provider<'a>(job: &Job, config: &'a AppConfig) -> AppResult<&'a ProviderProfile> {
    let provider_id = job
        .pipeline
        .provider_profile_id
        .as_ref()
        .or(config.default_provider_profile_id.as_ref())
        .ok_or_else(|| AppError::message("未选择 Provider 档案"))?;
    config
        .providers
        .iter()
        .find(|profile| &profile.id == provider_id)
        .ok_or_else(|| AppError::message(format!("Provider 档案不存在: {provider_id}")))
}

/// Prefer job-level `pipeline.model`; otherwise use the provider default.
/// Free-form model names are allowed so users can try models not yet listed.
fn resolve_model(job: &Job, provider: &ProviderProfile) -> AppResult<String> {
    if let Some(model_name) = job
        .pipeline
        .model
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return Ok(model_name.to_string());
    }
    let default_model = provider.default_model.trim();
    if default_model.is_empty() {
        return Err(AppError::message(format!(
            "Provider“{}”未配置默认模型",
            provider.name
        )));
    }
    Ok(default_model.to_string())
}

fn build_client(proxy_url: Option<&str>) -> AppResult<Client> {
    let mut builder = Client::builder().timeout(Duration::from_secs(180));
    if let Some(proxy_url) = proxy_url.filter(|value| !value.trim().is_empty()) {
        builder = builder.proxy(
            reqwest::Proxy::all(proxy_url)
                .map_err(|error| AppError::message(format!("代理配置无效: {error}")))?,
        );
    }
    builder
        .build()
        .map_err(|error| AppError::message(format!("HTTP 客户端初始化失败: {error}")))
}

fn call_openai(
    client: &Client,
    provider: &ProviderProfile,
    model_name: &str,
    api_key: &str,
    secret_values: &[String],
    system_prompt: &str,
    user_prompt: &str,
) -> AppResult<String> {
    let endpoint = openai_chat_endpoint(provider);
    let request = client
        .post(&endpoint)
        .header(AUTHORIZATION, format!("Bearer {api_key}"))
        .header(CONTENT_TYPE, "application/json")
        .json(&json!({
            "model": model_name,
            "messages": [
                { "role": "system", "content": system_prompt },
                { "role": "user", "content": user_prompt }
            ]
        }));
    let request = apply_extra_headers(request, provider)?;
    let value = send_json(request, &endpoint, secret_values)?;
    value
        .pointer("/choices/0/message/content")
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .ok_or_else(|| AppError::message("OpenAI 响应缺少 choices[0].message.content"))
}

fn call_anthropic(
    client: &Client,
    provider: &ProviderProfile,
    model_name: &str,
    api_key: &str,
    secret_values: &[String],
    system_prompt: &str,
    user_prompt: &str,
) -> AppResult<String> {
    let endpoint = anthropic_messages_endpoint(provider);
    let request = client
        .post(&endpoint)
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header(CONTENT_TYPE, "application/json")
        .json(&json!({
            "model": model_name,
            "max_tokens": 4096,
            "system": system_prompt,
            "messages": [{ "role": "user", "content": user_prompt }]
        }));
    let request = apply_extra_headers(request, provider)?;
    let value = send_json(request, &endpoint, secret_values)?;
    extract_anthropic_text(&value)
}

fn send_json(request: RequestBuilder, endpoint: &str, secrets: &[String]) -> AppResult<Value> {
    let response = request
        .send()
        .map_err(|error| AppError::message(format!("总结请求失败（{endpoint}）: {error}")))?;
    let status = response.status();
    let body = response.text().unwrap_or_default();
    if !status.is_success() {
        return Err(AppError::message(format!(
            "总结 API 返回 HTTP {status}：{}",
            safe_http_error(&body, secrets)
        )));
    }
    serde_json::from_str(&body)
        .map_err(|error| AppError::message(format!("总结 API 响应不是有效 JSON: {error}")))
}

fn extract_anthropic_text(value: &Value) -> AppResult<String> {
    let text_blocks: Vec<&str> = value
        .get("content")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|block| block.get("type").and_then(Value::as_str) == Some("text"))
        .filter_map(|block| block.get("text").and_then(Value::as_str))
        .collect();
    if text_blocks.is_empty() {
        return Err(AppError::message("Anthropic 响应缺少文本 content block"));
    }
    Ok(text_blocks.join("\n"))
}

fn apply_provider_headers(
    request: RequestBuilder,
    provider: &ProviderProfile,
    api_key: &str,
) -> AppResult<RequestBuilder> {
    let request = if provider.protocol == "anthropic" {
        request
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01")
    } else {
        request.header(AUTHORIZATION, format!("Bearer {api_key}"))
    };
    apply_extra_headers(request, provider)
}

fn apply_extra_headers(
    mut request: RequestBuilder,
    provider: &ProviderProfile,
) -> AppResult<RequestBuilder> {
    for (name, value) in &provider.extra_headers {
        let header_name = HeaderName::from_bytes(name.as_bytes())
            .map_err(|error| AppError::message(format!("额外 Header 名无效: {error}")))?;
        let header_value = HeaderValue::from_str(value)
            .map_err(|error| AppError::message(format!("额外 Header 值无效: {error}")))?;
        request = request.header(header_name, header_value);
    }
    Ok(request)
}

fn normalized_base(base_url: &str) -> String {
    base_url.trim().trim_end_matches('/').to_string()
}

fn openai_chat_endpoint(provider: &ProviderProfile) -> String {
    let base = normalized_base(&provider.base_url);
    if base.ends_with("/v1") {
        format!("{base}/chat/completions")
    } else {
        format!("{base}/v1/chat/completions")
    }
}

fn anthropic_messages_endpoint(provider: &ProviderProfile) -> String {
    let base = normalized_base(&provider.base_url);
    if base.ends_with("/v1") {
        format!("{base}/messages")
    } else {
        format!("{base}/v1/messages")
    }
}

fn models_endpoint(provider: &ProviderProfile) -> String {
    let base = normalized_base(&provider.base_url);
    if base.ends_with("/v1") {
        format!("{base}/models")
    } else {
        format!("{base}/v1/models")
    }
}

fn safe_http_error(body: &str, secrets: &[String]) -> String {
    let compact = body.replace(['\r', '\n'], " ");
    let redacted = logs::redact_secrets(&compact, secrets);
    logs::truncate_for_log(&redacted, 800)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_all_supported_variables() {
        let rendered = render_template(
            "{{title}}|{{source_url}}|{{duration}}|{{transcript}}|{{chapters}}",
            "A",
            "B",
            "C",
            "D",
            "E",
        );
        assert_eq!(rendered, "A|B|C|D|E");
    }

    #[test]
    fn unwraps_markdown_fence_with_language_and_closing() {
        let raw = "```markdown\n# Title\n\n- item\n```\n";
        assert_eq!(unwrap_outer_markdown_fence(raw), "# Title\n\n- item");
    }

    #[test]
    fn unwraps_markdown_fence_without_closing() {
        let raw = "```md\n## Heading\n\nbody text";
        assert_eq!(unwrap_outer_markdown_fence(raw), "## Heading\n\nbody text");
    }

    #[test]
    fn leaves_plain_markdown_unchanged() {
        let raw = "# Title\n\n**bold** and a real `code` span.";
        assert_eq!(unwrap_outer_markdown_fence(raw), raw);
    }

    #[test]
    fn leaves_inner_code_blocks_when_not_whole_document_fence() {
        let raw = "Intro\n\n```rust\nfn main() {}\n```\n";
        assert_eq!(unwrap_outer_markdown_fence(raw), raw.trim());
    }

    #[test]
    fn endpoints_handle_versioned_base_urls() {
        let provider = ProviderProfile {
            id: "x".into(),
            name: "x".into(),
            protocol: "openai".into(),
            base_url: "https://example.com/v1/".into(),
            api_key: None,
            api_key_env: None,
            default_model: "m".into(),
            models: vec!["m".into()],
            extra_headers: vec![],
        };
        assert_eq!(
            openai_chat_endpoint(&provider),
            "https://example.com/v1/chat/completions"
        );
        assert_eq!(models_endpoint(&provider), "https://example.com/v1/models");
    }

    #[test]
    fn joins_all_anthropic_text_blocks() {
        let response = json!({
            "content": [
                { "type": "text", "text": "first" },
                { "type": "tool_use", "id": "tool" },
                { "type": "text", "text": "second" }
            ]
        });
        assert_eq!(
            extract_anthropic_text(&response).expect("extract text"),
            "first\nsecond"
        );
    }
}

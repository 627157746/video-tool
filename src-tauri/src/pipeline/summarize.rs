use super::{logs, paths};
use crate::config::{AppConfig, ProviderProfile, SummaryTemplate};
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
    let template = resolve_template(job, config)?;
    let api_key = config.resolve_api_key(provider).ok_or_else(|| {
        AppError::message(format!(
            "Provider“{}”缺少 API Key；请配置 {} 或在配置文件填写 Key",
            provider.name,
            provider.api_key_env.as_deref().unwrap_or("对应环境变量")
        ))
    })?;
    let secret_values = config.secret_values();
    let user_prompt = render_template(
        &template.user_template,
        &job.display_title(),
        job.source.url.as_deref().unwrap_or("本地文件"),
        job.duration_label.as_deref().unwrap_or("未知"),
        &transcript,
    );

    paths::ensure_job_layout(job_dir)?;
    logs::clear_log(job_dir, "summarize")?;
    let redacted_prompt = logs::redact_secrets(&user_prompt, &secret_values);
    let safe_prompt = logs::truncate_for_log(&redacted_prompt, 2_000);
    logs::append_log(
        job_dir,
        "summarize",
        &format!(
            "provider: {}\nprotocol: {}\nmodel: {}\nbase_url: {}\nprompt_preview:\n{}\n",
            provider.id, provider.protocol, provider.default_model, provider.base_url, safe_prompt
        ),
    )?;

    let client = build_client(config.proxy_url.as_deref())?;
    let summary = match provider.protocol.as_str() {
        "openai" => call_openai(
            &client,
            provider,
            &api_key,
            &secret_values,
            &template.system_prompt,
            &user_prompt,
        )?,
        "anthropic" => call_anthropic(
            &client,
            provider,
            &api_key,
            &secret_values,
            &template.system_prompt,
            &user_prompt,
        )?,
        protocol => {
            return Err(AppError::message(format!(
                "不支持的 Provider 协议: {protocol}"
            )))
        }
    };

    let summary_dir = paths::summary_dir(job_dir);
    fs::create_dir_all(&summary_dir)?;
    fs::write(summary_dir.join("summary.md"), &summary)?;
    fs::write(
        summary_dir.join("meta.json"),
        serde_json::to_string_pretty(&json!({
            "provider_profile_id": provider.id,
            "template_id": template.id,
            "model": provider.default_model,
            "protocol": provider.protocol,
            "created_at": chrono::Utc::now(),
            "input_characters": character_count,
            "selected_segment_ids": job.selected_segment_ids,
        }))?,
    )?;
    logs::append_log(job_dir, "summarize", "summary saved: summary/summary.md")?;
    job.summary_path = Some("summary/summary.md".to_string());
    Ok(summary)
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
) -> String {
    template
        .replace("{{title}}", title)
        .replace("{{source_url}}", source_url)
        .replace("{{duration}}", duration)
        .replace("{{transcript}}", transcript)
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

fn resolve_template<'a>(job: &Job, config: &'a AppConfig) -> AppResult<&'a SummaryTemplate> {
    let template_id = job
        .pipeline
        .template_id
        .as_ref()
        .or(config.default_template_id.as_ref())
        .ok_or_else(|| AppError::message("未选择总结模板"))?;
    config
        .templates
        .iter()
        .find(|template| &template.id == template_id)
        .ok_or_else(|| AppError::message(format!("总结模板不存在: {template_id}")))
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
            "model": provider.default_model,
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
            "model": provider.default_model,
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
            "{{title}}|{{source_url}}|{{duration}}|{{transcript}}",
            "A",
            "B",
            "C",
            "D",
        );
        assert_eq!(rendered, "A|B|C|D");
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

use crate::error::{AppError, AppResult};
use regex::Regex;
use reqwest::blocking::Client;
use reqwest::header::USER_AGENT;
use serde_json::Value;
use std::sync::OnceLock;
use std::time::Duration;

/// Mobile UA used by the Douyin share-page scrape path (best-effort).
pub const DOUYIN_MOBILE_USER_AGENT: &str = concat!(
    "Mozilla/5.0 (Linux; Android 15; Pixel 9) ",
    "AppleWebKit/537.36 (KHTML, like Gecko) ",
    "Chrome/150.0.0.0 Mobile Safari/537.36"
);

const SHARE_PAGE_TIMEOUT_SECS: u64 = 45;
const VIDEO_ID_PATH_PATTERN: &str = r"(?i)/(?:share/)?video/(\d{10,})";
const DOUYIN_HOST_PATTERN: &str =
    r"(?i)(?:https?://)?(?:[\w-]+\.)?(?:douyin\.com|iesdouyin\.com|snssdk\.com)\b";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedDouyinMedia {
    pub source_url: String,
    pub video_id: String,
    pub play_url: String,
    pub title: Option<String>,
}

pub fn looks_like_douyin_input(raw_input: &str) -> bool {
    extract_douyin_url(raw_input).is_some()
        || extract_video_id_from_text(raw_input).is_some()
        || douyin_host_regex().is_match(raw_input)
}

pub fn resolve_douyin_media(raw_input: &str) -> AppResult<ResolvedDouyinMedia> {
    let client = build_http_client()?;
    let source_url = extract_douyin_url(raw_input)
        .or_else(|| {
            extract_video_id_from_text(raw_input)
                .as_deref()
                .map(share_page_url)
        })
        .ok_or_else(|| {
            AppError::message(
                "未在输入中找到抖音链接或视频 ID。请粘贴分享文案或 v.douyin.com / douyin.com 链接。",
            )
        })?;

    let video_id = resolve_video_id(&client, &source_url)?;
    let share_url = share_page_url(&video_id);
    let html = fetch_share_page_html(&client, &share_url)?;
    let router_data = extract_router_data_json(&html)?;
    let (play_url_raw, title) = extract_play_addr_and_title(&router_data)?;
    let play_url = rewrite_playwm_to_play(&play_url_raw);

    if play_url.trim().is_empty() {
        return Err(AppError::message(
            "已解析分享页，但播放地址为空。请查看 logs/download.log。",
        ));
    }

    Ok(ResolvedDouyinMedia {
        source_url,
        video_id,
        play_url,
        title,
    })
}

pub fn rewrite_playwm_to_play(play_url: &str) -> String {
    play_url.replacen("playwm", "play", 1)
}

fn build_http_client() -> AppResult<Client> {
    Client::builder()
        .timeout(Duration::from_secs(SHARE_PAGE_TIMEOUT_SECS))
        .redirect(reqwest::redirect::Policy::limited(10))
        .user_agent(DOUYIN_MOBILE_USER_AGENT)
        .build()
        .map_err(|error| AppError::message(format!("HTTP 客户端初始化失败: {error}")))
}

fn share_page_url(video_id: &str) -> String {
    format!("https://www.iesdouyin.com/share/video/{video_id}/")
}

fn resolve_video_id(client: &Client, source_url: &str) -> AppResult<String> {
    if let Some(video_id) = extract_video_id_from_text(source_url) {
        return Ok(video_id);
    }

    let response = client
        .get(source_url)
        .header(USER_AGENT, DOUYIN_MOBILE_USER_AGENT)
        .send()
        .map_err(|error| {
            AppError::message(format!(
                "解析抖音短链失败（网络错误）: {error}。请检查网络后重试。"
            ))
        })?;

    if !response.status().is_success() && !response.status().is_redirection() {
        // reqwest follows redirects; still surface non-success final status.
        let status = response.status();
        let final_url = response.url().clone();
        if let Some(video_id) = extract_video_id_from_text(final_url.as_str()) {
            return Ok(video_id);
        }
        return Err(AppError::message(format!(
            "解析抖音短链失败（HTTP {status}），最终地址: {final_url}"
        )));
    }

    let final_url = response.url().clone();
    if let Some(video_id) = extract_video_id_from_text(final_url.as_str()) {
        return Ok(video_id);
    }

    // Some short-link landings only expose the id inside HTML/location fragments.
    let body = response
        .text()
        .map_err(|error| AppError::message(format!("读取短链响应失败: {error}")))?;
    if let Some(video_id) = extract_video_id_from_text(&body) {
        return Ok(video_id);
    }

    Err(AppError::message(format!(
        "无法从短链解析视频 ID。最终地址: {final_url}。请改用完整分享页链接。"
    )))
}

fn fetch_share_page_html(client: &Client, share_url: &str) -> AppResult<String> {
    let response = client
        .get(share_url)
        .header(USER_AGENT, DOUYIN_MOBILE_USER_AGENT)
        .send()
        .map_err(|error| {
            AppError::message(format!(
                "请求抖音分享页失败: {error}。地址: {share_url}"
            ))
        })?;

    let status = response.status();
    if !status.is_success() {
        return Err(AppError::message(format!(
            "请求抖音分享页失败（HTTP {status}）。地址: {share_url}"
        )));
    }

    response
        .text()
        .map_err(|error| AppError::message(format!("读取抖音分享页 HTML 失败: {error}")))
}

fn extract_router_data_json(html: &str) -> AppResult<Value> {
    const MARKER: &str = "window._ROUTER_DATA";
    let marker_position = html
        .find(MARKER)
        .ok_or_else(|| AppError::message("分享页中未找到 window._ROUTER_DATA，页面结构可能已变化。"))?;

    let after_marker = &html[marker_position + MARKER.len()..];
    let equals_position = after_marker
        .find('=')
        .ok_or_else(|| AppError::message("分享页 _ROUTER_DATA 赋值语法无法识别。"))?;
    let after_equals = after_marker[equals_position + 1..].trim_start();
    let json_text = extract_balanced_json_object(after_equals).ok_or_else(|| {
        AppError::message("无法从分享页提取 _ROUTER_DATA JSON（括号不匹配或内容被截断）。")
    })?;

    serde_json::from_str(json_text).map_err(|error| {
        AppError::message(format!(
            "解析 _ROUTER_DATA JSON 失败: {error}。页面结构可能已变化。"
        ))
    })
}

fn extract_balanced_json_object(source: &str) -> Option<&str> {
    let start = source.find('{')?;
    let bytes = source.as_bytes();
    let mut depth = 0_i32;
    let mut in_string = false;
    let mut is_escaped = false;

    for index in start..bytes.len() {
        let byte = bytes[index];
        if in_string {
            if is_escaped {
                is_escaped = false;
            } else if byte == b'\\' {
                is_escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
            continue;
        }

        match byte {
            b'"' => in_string = true,
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&source[start..=index]);
                }
            }
            _ => {}
        }
    }

    None
}

fn extract_play_addr_and_title(router_data: &Value) -> AppResult<(String, Option<String>)> {
    let loader_data = router_data
        .get("loaderData")
        .and_then(Value::as_object)
        .ok_or_else(|| AppError::message("分享页 JSON 缺少 loaderData。"))?;

    let preferred_key = "video_(id)/page";
    let page_value = loader_data
        .get(preferred_key)
        .or_else(|| {
            loader_data.values().find(|value| {
                value
                    .pointer("/videoInfoRes/item_list/0/video/play_addr/url_list/0")
                    .is_some()
            })
        })
        .ok_or_else(|| {
            AppError::message(
                "分享页 loaderData 中未找到 video_(id)/page 或可用的 videoInfoRes。",
            )
        })?;

    let item = page_value
        .pointer("/videoInfoRes/item_list/0")
        .ok_or_else(|| AppError::message("分享页 JSON 缺少 videoInfoRes.item_list[0]。"))?;

    let play_url = item
        .pointer("/video/play_addr/url_list/0")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            AppError::message("分享页 JSON 缺少 video.play_addr.url_list[0] 播放地址。")
        })?
        .to_string();

    let title = item
        .get("desc")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);

    Ok((play_url, title))
}

pub fn extract_douyin_url(raw_input: &str) -> Option<String> {
    let url_regex = url_in_text_regex();
    for capture in url_regex.captures_iter(raw_input) {
        let candidate = capture.get(0)?.as_str().trim_end_matches([
            ')', ']', '}', '"', '\'', '，', '。', '、', '；', '！', '？', ',', '.', ';', '!', '?',
        ]);
        if is_douyin_related_url(candidate) {
            let normalized = normalize_url(candidate);
            return Some(normalized);
        }
    }
    None
}

fn extract_video_id_from_text(text: &str) -> Option<String> {
    let regex = video_id_path_regex();
    regex
        .captures(text)
        .and_then(|capture| capture.get(1))
        .map(|matched| matched.as_str().to_string())
}

fn is_douyin_related_url(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    lower.contains("douyin.com")
        || lower.contains("iesdouyin.com")
        || lower.contains("snssdk.com")
        || lower.contains("v.douyin.com")
}

fn normalize_url(url: &str) -> String {
    if url.starts_with("http://") || url.starts_with("https://") {
        url.to_string()
    } else {
        format!("https://{url}")
    }
}

fn url_in_text_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r#"(?i)https?://[^\s<>"']+|//[^\s<>"']+|(?:www\.)?(?:v\.)?douyin\.com/[^\s<>"']+"#)
            .expect("url regex")
    })
}

fn video_id_path_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(VIDEO_ID_PATH_PATTERN).expect("video id regex"))
}

fn douyin_host_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(DOUYIN_HOST_PATTERN).expect("douyin host regex"))
}

/// Guess a media file extension from response headers / URL.
pub fn guess_media_extension(content_type: Option<&str>, play_url: &str) -> String {
    if let Some(content_type) = content_type {
        let mime = content_type
            .split(';')
            .next()
            .unwrap_or(content_type)
            .trim()
            .to_ascii_lowercase();
        match mime.as_str() {
            "video/mp4" => return "mp4".to_string(),
            "video/webm" => return "webm".to_string(),
            "video/quicktime" => return "mov".to_string(),
            "audio/mpeg" => return "mp3".to_string(),
            "audio/mp4" | "audio/aac" => return "m4a".to_string(),
            _ => {}
        }
    }

    let path = play_url
        .split('?')
        .next()
        .unwrap_or(play_url)
        .to_ascii_lowercase();
    for extension in ["mp4", "webm", "mov", "m4a", "mp3"] {
        if path.ends_with(&format!(".{extension}")) {
            return extension.to_string();
        }
    }

    "mp4".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn extracts_short_link_from_share_text() {
        let text = "7.15 :5pm 08/28 Okp:/ o@D.Hv 既然世界没有魔法，那就从零亲手创造魔法！ # 动漫解说 # 日漫 # 二次元 # 异世界动漫  https://v.douyin.com/XMiZsZXN0Gw/ 复制此链接，打开Dou音搜索，直接观看视频！";
        let url = extract_douyin_url(text).expect("url");
        assert_eq!(url, "https://v.douyin.com/XMiZsZXN0Gw/");
        assert!(looks_like_douyin_input(text));
    }

    #[test]
    fn extracts_video_id_from_share_and_web_paths() {
        assert_eq!(
            extract_video_id_from_text("https://www.iesdouyin.com/share/video/7659741979496025378/")
                .as_deref(),
            Some("7659741979496025378")
        );
        assert_eq!(
            extract_video_id_from_text("https://www.douyin.com/video/7662996900462726440?previous_page=app_code_link")
                .as_deref(),
            Some("7662996900462726440")
        );
    }

    #[test]
    fn rewrites_playwm_to_play_once() {
        let input = "https://aweme.snssdk.com/aweme/v1/playwm/?line=0&video_id=abc";
        assert_eq!(
            rewrite_playwm_to_play(input),
            "https://aweme.snssdk.com/aweme/v1/play/?line=0&video_id=abc"
        );
    }

    #[test]
    fn extracts_play_url_from_router_data() {
        let router = json!({
            "loaderData": {
                "video_(id)/page": {
                    "videoInfoRes": {
                        "item_list": [{
                            "desc": "测试标题",
                            "video": {
                                "play_addr": {
                                    "url_list": [
                                        "https://aweme.snssdk.com/aweme/v1/playwm/?video_id=v1"
                                    ]
                                }
                            }
                        }]
                    }
                }
            }
        });
        let (play_url, title) = extract_play_addr_and_title(&router).expect("play");
        assert!(play_url.contains("playwm"));
        assert_eq!(title.as_deref(), Some("测试标题"));
        assert!(rewrite_playwm_to_play(&play_url).contains("/play/"));
    }

    #[test]
    fn extracts_balanced_json_with_nested_braces_and_strings() {
        let source = r#" = {"a":{"b":1},"c":"}"} ; other"#;
        let json = extract_balanced_json_object(source).expect("json");
        assert_eq!(json, r#"{"a":{"b":1},"c":"}"}"#);
    }

    #[test]
    fn non_douyin_input_is_rejected() {
        assert!(!looks_like_douyin_input("https://www.youtube.com/watch?v=abc"));
        assert!(extract_douyin_url("hello world").is_none());
    }

    #[test]
    fn guesses_extension_from_content_type() {
        assert_eq!(
            guess_media_extension(Some("video/mp4; charset=binary"), "https://x/y"),
            "mp4"
        );
        assert_eq!(guess_media_extension(None, "https://cdn.example/a.webm?x=1"), "webm");
    }
}

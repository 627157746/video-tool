use crate::error::{AppError, AppResult};
use regex::Regex;
use reqwest::blocking::Client;
use reqwest::header::{COOKIE, REFERER, USER_AGENT};
use serde_json::Value;
use std::sync::OnceLock;
use std::time::Duration;
use uuid::Uuid;

/// Mobile UA used by the Douyin share-page scrape path (best-effort).
pub const DOUYIN_MOBILE_USER_AGENT: &str = concat!(
    "Mozilla/5.0 (Linux; Android 15; Pixel 9) ",
    "AppleWebKit/537.36 (KHTML, like Gecko) ",
    "Chrome/150.0.0.0 Mobile Safari/537.36"
);

/// Desktop UA for live room pages (SSR payload is more complete in this path).
pub const DOUYIN_LIVE_DESKTOP_USER_AGENT: &str = concat!(
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) ",
    "AppleWebKit/537.36 (KHTML, like Gecko) ",
    "Chrome/120.0.0.0 Safari/537.36"
);

pub const DOUYIN_LIVE_REFERER: &str = "https://live.douyin.com/";

const SHARE_PAGE_TIMEOUT_SECS: u64 = 45;
const LIVE_PAGE_TIMEOUT_SECS: u64 = 45;
const LIVE_STATUS_ON: i64 = 2;
const VIDEO_ID_PATH_PATTERN: &str = r"(?i)/(?:share/)?video/(\d{10,})";
const DOUYIN_HOST_PATTERN: &str =
    r"(?i)(?:https?://)?(?:[\w-]+\.)?(?:douyin\.com|iesdouyin\.com|snssdk\.com)\b";
const LIVE_ROOM_PATH_PATTERN: &str =
    r"(?i)(?:live\.douyin\.com/|douyin\.com/live/)([A-Za-z0-9_\-]+)";

/// Prefer higher qualities first when selecting from flv/hls pull maps.
const LIVE_QUALITY_PREFERENCE: &[&str] = &[
    "FULL_HD1", "ORIGIN", "HD1", "SD1", "SD2", "LD1", "LD",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedDouyinMedia {
    pub source_url: String,
    pub video_id: String,
    pub play_url: String,
    pub title: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedDouyinLiveStream {
    pub source_url: String,
    pub room_id: String,
    pub stream_url: String,
    pub quality: String,
    pub protocol: String,
    pub title: Option<String>,
}

pub fn looks_like_douyin_input(raw_input: &str) -> bool {
    extract_douyin_url(raw_input).is_some()
        || extract_video_id_from_text(raw_input).is_some()
        || douyin_host_regex().is_match(raw_input)
}

pub fn looks_like_douyin_live_url(raw_input: &str) -> bool {
    let Some(url) = extract_douyin_url(raw_input).or_else(|| {
        if douyin_host_regex().is_match(raw_input) {
            Some(normalize_url(raw_input.trim()))
        } else {
            None
        }
    }) else {
        return false;
    };
    let lower = url.to_ascii_lowercase();
    lower.contains("live.douyin.com/")
        || lower.contains("douyin.com/live/")
        || lower.contains("webcast.amemv.com/")
}

/// Resolve a Douyin live room page into a direct FLV/HLS pull URL for ffmpeg.
///
/// Streamlink's Douyin plugin currently expects `streamStore.streamData.H264_streamData`,
/// which is often empty on modern SSR pages. The usable pull URLs live under
/// `roomStore.roomInfo.room.stream_url` instead.
pub fn resolve_douyin_live_stream(raw_input: &str) -> AppResult<ResolvedDouyinLiveStream> {
    let source_url = extract_douyin_url(raw_input)
        .filter(|url| looks_like_douyin_live_url(url))
        .or_else(|| {
            let trimmed = raw_input.trim();
            if looks_like_douyin_live_url(trimmed) {
                Some(normalize_url(trimmed))
            } else {
                None
            }
        })
        .ok_or_else(|| {
            AppError::message(
                "未识别为抖音直播间链接。请使用 https://live.douyin.com/<房间号> 形式。",
            )
        })?;

    let room_id_from_url = extract_live_room_id(&source_url).unwrap_or_default();
    let client = build_live_http_client()?;
    let html = fetch_live_room_html(&client, &source_url)?;
    let room = extract_live_room_value(&html).ok_or_else(|| {
        AppError::message(
            "无法从抖音直播页解析房间数据（页面结构可能已变化，或需要登录 Cookie）。",
        )
    })?;

    let status = room
        .get("status")
        .and_then(Value::as_i64)
        .unwrap_or_default();
    if status != LIVE_STATUS_ON {
        return Err(AppError::message(format!(
            "抖音直播间当前未开播（status={status}）。开播后再试，或确认房间号正确。"
        )));
    }

    let title = room
        .get("title")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    let room_id = room
        .get("id_str")
        .and_then(Value::as_str)
        .map(str::to_string)
        .filter(|value| !value.is_empty())
        .or_else(|| {
            if room_id_from_url.is_empty() {
                None
            } else {
                Some(room_id_from_url)
            }
        })
        .unwrap_or_else(|| "unknown".to_string());

    let stream_url_value = room.get("stream_url").ok_or_else(|| {
        AppError::message("直播间数据中缺少 stream_url，无法获取拉流地址。")
    })?;
    let (quality, protocol, stream_url) = select_live_pull_url(stream_url_value).ok_or_else(|| {
        AppError::message(
            "直播间已开播，但未找到可用的 flv/hls 拉流地址。可稍后重试或检查是否被风控。",
        )
    })?;

    Ok(ResolvedDouyinLiveStream {
        source_url,
        room_id,
        stream_url,
        quality,
        protocol,
        title,
    })
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

fn build_live_http_client() -> AppResult<Client> {
    Client::builder()
        .timeout(Duration::from_secs(LIVE_PAGE_TIMEOUT_SECS))
        .redirect(reqwest::redirect::Policy::limited(10))
        .user_agent(DOUYIN_LIVE_DESKTOP_USER_AGENT)
        .build()
        .map_err(|error| AppError::message(format!("直播 HTTP 客户端初始化失败: {error}")))
}

fn fetch_live_room_html(client: &Client, room_url: &str) -> AppResult<String> {
    // Streamlink and web clients seed a short __ac_nonce cookie before reading SSR.
    let ac_nonce = Uuid::new_v4().simple().to_string();
    let ac_nonce = &ac_nonce[..21.min(ac_nonce.len())];
    let response = client
        .get(room_url)
        .header(USER_AGENT, DOUYIN_LIVE_DESKTOP_USER_AGENT)
        .header(REFERER, DOUYIN_LIVE_REFERER)
        .header(COOKIE, format!("__ac_nonce={ac_nonce}"))
        .send()
        .map_err(|error| {
            AppError::message(format!(
                "请求抖音直播页失败: {error}。地址: {room_url}"
            ))
        })?;

    let status = response.status();
    if !status.is_success() {
        return Err(AppError::message(format!(
            "请求抖音直播页失败（HTTP {status}）。地址: {room_url}"
        )));
    }

    response.text().map_err(|error| {
        AppError::message(format!("读取抖音直播页 HTML 失败: {error}"))
    })
}

fn extract_live_room_id(url: &str) -> Option<String> {
    live_room_path_regex()
        .captures(url)
        .and_then(|capture| capture.get(1))
        .map(|matched| matched.as_str().to_string())
}

fn extract_live_room_value(html: &str) -> Option<Value> {
    for payload in parse_pace_f_payloads(html) {
        if let Some(room) = find_room_store_room(&payload) {
            return Some(room);
        }
    }
    None
}

fn parse_pace_f_payloads(html: &str) -> Vec<Value> {
    let mut payloads = Vec::new();
    let marker = "self.__pace_f.push(";
    let mut search_from = 0usize;
    while let Some(relative_index) = html[search_from..].find(marker) {
        let array_start = search_from + relative_index + marker.len();
        let Some(array_text) = extract_balanced_delimited(&html[array_start..], '[', ']') else {
            search_from = array_start;
            continue;
        };
        search_from = array_start + array_text.len();
        let Ok(Value::Array(items)) = serde_json::from_str(array_text) else {
            continue;
        };
        if items.len() < 2 {
            continue;
        }
        let Some(Value::String(encoded)) = items.get(1) else {
            continue;
        };
        // RSC chunk format: "<id>:{json}"
        let Some((_chunk_id, payload_text)) = encoded.split_once(':') else {
            continue;
        };
        if let Ok(payload) = serde_json::from_str::<Value>(payload_text) {
            payloads.push(payload);
        }
    }
    payloads
}

fn find_room_store_room(value: &Value) -> Option<Value> {
    match value {
        Value::Object(map) => {
            if let Some(room) = map
                .get("roomStore")
                .and_then(|store| store.pointer("/roomInfo/room"))
            {
                if room.get("stream_url").is_some() || room.get("status").is_some() {
                    return Some(room.clone());
                }
            }
            for child in map.values() {
                if let Some(room) = find_room_store_room(child) {
                    return Some(room);
                }
            }
            None
        }
        Value::Array(items) => {
            for item in items {
                if let Some(room) = find_room_store_room(item) {
                    return Some(room);
                }
            }
            None
        }
        _ => None,
    }
}

fn select_live_pull_url(stream_url_value: &Value) -> Option<(String, String, String)> {
    if let Some(flv_map) = stream_url_value.get("flv_pull_url") {
        if let Some((quality, url)) = pick_quality_url(flv_map) {
            return Some((quality, "flv".to_string(), force_https_url(&url)));
        }
    }

    if let Some(hls_map) = stream_url_value.get("hls_pull_url_map") {
        if let Some((quality, url)) = pick_quality_url(hls_map) {
            return Some((quality, "hls".to_string(), force_https_url(&url)));
        }
    }

    if let Some(hls_url) = stream_url_value
        .get("hls_pull_url")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return Some((
            "default".to_string(),
            "hls".to_string(),
            force_https_url(hls_url),
        ));
    }

    None
}

fn pick_quality_url(map_value: &Value) -> Option<(String, String)> {
    let object = map_value.as_object()?;
    for quality in LIVE_QUALITY_PREFERENCE {
        if let Some(url) = object
            .get(*quality)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return Some(((*quality).to_string(), url.to_string()));
        }
    }
    for (quality, value) in object {
        if let Some(url) = value.as_str().map(str::trim).filter(|value| !value.is_empty()) {
            return Some((quality.clone(), url.to_string()));
        }
    }
    None
}

fn force_https_url(url: &str) -> String {
    if let Some(rest) = url.strip_prefix("http://") {
        format!("https://{rest}")
    } else {
        url.to_string()
    }
}

fn extract_balanced_delimited<'a>(
    source: &'a str,
    open: char,
    close: char,
) -> Option<&'a str> {
    let open_byte = open as u8;
    let close_byte = close as u8;
    let bytes = source.as_bytes();
    let start = bytes.iter().position(|byte| *byte == open_byte)?;
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

        if byte == b'"' {
            in_string = true;
            continue;
        }
        if byte == open_byte {
            depth += 1;
        } else if byte == close_byte {
            depth -= 1;
            if depth == 0 {
                return Some(&source[start..=index]);
            }
        }
    }
    None
}

fn live_room_path_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(LIVE_ROOM_PATH_PATTERN).expect("live room path regex"))
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
            AppError::message(format!("请求抖音分享页失败: {error}。地址: {share_url}"))
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
    let marker_position = html.find(MARKER).ok_or_else(|| {
        AppError::message("分享页中未找到 window._ROUTER_DATA，页面结构可能已变化。")
    })?;

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
            AppError::message("分享页 loaderData 中未找到 video_(id)/page 或可用的 videoInfoRes。")
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
        Regex::new(
            r#"(?i)https?://[^\s<>"']+|//[^\s<>"']+|(?:www\.)?(?:v\.)?douyin\.com/[^\s<>"']+"#,
        )
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
            extract_video_id_from_text(
                "https://www.iesdouyin.com/share/video/7659741979496025378/"
            )
            .as_deref(),
            Some("7659741979496025378")
        );
        assert_eq!(
            extract_video_id_from_text(
                "https://www.douyin.com/video/7662996900462726440?previous_page=app_code_link"
            )
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
        assert!(!looks_like_douyin_input(
            "https://www.youtube.com/watch?v=abc"
        ));
        assert!(extract_douyin_url("hello world").is_none());
    }

    #[test]
    fn guesses_extension_from_content_type() {
        assert_eq!(
            guess_media_extension(Some("video/mp4; charset=binary"), "https://x/y"),
            "mp4"
        );
        assert_eq!(
            guess_media_extension(None, "https://cdn.example/a.webm?x=1"),
            "webm"
        );
    }

    #[test]
    fn detects_douyin_live_room_urls() {
        assert!(looks_like_douyin_live_url(
            "https://live.douyin.com/167597605969"
        ));
        assert!(looks_like_douyin_live_url(
            "https://www.douyin.com/live/167597605969"
        ));
        assert!(!looks_like_douyin_live_url(
            "https://www.douyin.com/video/7662996900462726440"
        ));
        assert_eq!(
            extract_live_room_id("https://live.douyin.com/167597605969?foo=1").as_deref(),
            Some("167597605969")
        );
    }

    #[test]
    fn selects_full_hd_flv_over_lower_qualities() {
        let stream_url = json!({
            "flv_pull_url": {
                "SD1": "http://cdn.example/sd.flv",
                "FULL_HD1": "http://cdn.example/full.flv",
                "HD1": "http://cdn.example/hd.flv"
            },
            "hls_pull_url": "http://cdn.example/index.m3u8"
        });
        let (quality, protocol, url) = select_live_pull_url(&stream_url).expect("url");
        assert_eq!(quality, "FULL_HD1");
        assert_eq!(protocol, "flv");
        assert_eq!(url, "https://cdn.example/full.flv");
    }

    #[test]
    fn finds_room_store_room_in_nested_rsc_payload() {
        let payload = json!([
            "$",
            "$L3",
            null,
            {
                "state": {
                    "roomStore": {
                        "roomInfo": {
                            "room": {
                                "id_str": "room-1",
                                "status": 2,
                                "title": "测试直播",
                                "stream_url": {
                                    "flv_pull_url": {
                                        "HD1": "http://cdn.example/live.flv"
                                    }
                                }
                            }
                        }
                    }
                }
            }
        ]);
        let room = find_room_store_room(&payload).expect("room");
        assert_eq!(room["id_str"], "room-1");
        assert_eq!(room["status"], 2);
    }

    #[test]
    fn force_https_rewrites_http_only() {
        assert_eq!(
            force_https_url("http://cdn.example/a.flv"),
            "https://cdn.example/a.flv"
        );
        assert_eq!(
            force_https_url("https://cdn.example/a.flv"),
            "https://cdn.example/a.flv"
        );
    }

    /// Network smoke test for the live room the user reported.
    /// Run manually: cargo test -p video-tool --lib resolve_live_room_network_smoke -- --ignored --nocapture
    #[test]
    #[ignore = "network smoke; depends on a live Douyin room"]
    fn resolve_live_room_network_smoke() {
        let resolved = resolve_douyin_live_stream("https://live.douyin.com/167597605969")
            .expect("resolve live room");
        assert!(
            resolved.stream_url.contains(".flv") || resolved.stream_url.contains(".m3u8"),
            "unexpected stream url: {}",
            resolved.stream_url
        );
        assert_eq!(resolved.protocol, "flv");
        eprintln!(
            "resolved room_id={} quality={} url={}",
            resolved.room_id, resolved.quality, resolved.stream_url
        );
    }
}

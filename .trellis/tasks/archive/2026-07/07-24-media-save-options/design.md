# Design: media save mode (video | audio, exclusive)

## Overview

Single exclusive field on `JobSource` (not two combinable booleans):

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MediaSaveMode {
    /// Keep a video container in media/ (default; may still contain an audio track).
    Video,
    /// Keep only a standalone audio file; never leave a full video as the final artifact.
    Audio,
}

// JobSource:
#[serde(default)]
pub media_save_mode: MediaSaveMode, // Default::default() => Video
```

Create requests carry the same field (optional → default `Video`):

- `CreateDownloadJobRequest`
- `CreateDownloadJobsBatchRequest`
- `CreateLiveRecordJobRequest`

Frontend: radio group `formMediaSaveMode: "video" | "audio"` (default `"video"`).

**Do not** expose `save_video` + `save_audio` dual flags in public API for MVP — avoids illegal `(true, true)` / `(false, false)`.

## Naming and defaults

| Field | Serde when omitted | Create default |
| --- | --- | --- |
| `media_save_mode` | `video` | `video` |

```rust
impl Default for MediaSaveMode {
    fn default() -> Self {
        Self::Video
    }
}
```

Unknown enum values: reject at create with 简体中文（勿静默映射）。

## Artifact layout

| Mode | Files under `media/` |
| --- | --- |
| `video` | `original.<ext>` or live `segment_%03d.ts` |
| `audio` | `original.<audio_ext>` or live audio segments only |

No dual-file layout (`original` + `original.audio`).

## yt-dlp (`pipeline/download.rs`)

```rust
pub fn yt_dlp_format_args(mode: MediaSaveMode) -> Vec<String> { ... }
```

### `MediaSaveMode::Audio`

```text
-f ba/b
-x
--audio-format m4a
--audio-quality 0
-o media/original.%(ext)s
```

- Primary path is **direct audio format / extract**, not full video download then convert.
- Unit-test: args contain `-x` / `ba`.

### `MediaSaveMode::Video`

MVP: keep current no-`-f` best-effort behavior (closest to today). Log: `media_save_mode=video`.

**Forbidden for `Audio`:** write full video into `media/` as the job artifact (even then delete). Temp under job `tmp/` that is cleaned and never listed is acceptable only if pure format path is insufficient; prefer pure audio selection.

## Douyin (`pipeline/douyin.rs` + `download.rs`)

Resolver still yields the existing **video** `play_url` (no required separate audio URL for MVP).

| Mode | Behavior |
| --- | --- |
| `Video` | Existing HTTP download of play stream → `media/original.<ext>` |
| `Audio` | **ffmpeg 直接吃视频 play URL**，参数输出音频，产物仅 `media/original.m4a`（或等价音频扩展名） |

### Audio mode ffmpeg contract (canonical)

Reuse the same headers policy as today where needed (`User-Agent`, `Referer` via `-headers` / `-user_agent` as the project already does for network ffmpeg).

Example shape (exact flags may match project sidecar helpers):

```text
ffmpeg -y -user_agent <mobile_ua> -headers "Referer: https://www.douyin.com/\r\n" \
  -i "<play_url>" \
  -vn \
  -c:a aac -b:a 192k \
  media/original.m4a
```

Semantics:

1. Input is the **same Douyin video play URL** used for video download.
2. Output is **audio only** (`-vn` / map audio only) — this is “ffmpeg 下载视频路径 + 参数出音频”, not “先落 original.mp4 再二次转码”.
3. **Forbidden:** write complete video to `media/original.mp4` (or any listed final video) then run a second ffmpeg extract for the job artifact.
4. Optional: stream to a `.part` then rename, same as other download paths; never leave a full video final artifact in audio mode.
5. Need ffmpeg binary from sidecar status (same as live/transcribe).

Fallback: if Douyin resolve fails, existing yt-dlp fallback with `MediaSaveMode::Audio` (`-x` / `ba`).

**Out of scope for Douyin audio:** parsing `music` / dedicated audio CDN URL as primary path (can be a later optimization; not required for MVP).

## Live record (`pipeline/record.rs`)

Thread `media_save_mode` into `LiveRecordOptions`.

### `Video` (current)

```text
-map 0 -c copy -f segment ... segment_%03d.ts
```

### `Audio`

```text
-map 0:a:0?
-c:a copy   # fallback aac if needed
-f segment
segment_%03d.m4a
```

No dual-output / both-mode. Missing audio stream → 简体中文 + `logs/record.log`.

## IPC / frontend

1. Rust: `MediaSaveMode` + `JobSource` + create requests.
2. `src/types.ts`: `export type MediaSaveMode = "video" | "audio"`.
3. API: pass `media_save_mode` on batch download + live create.
4. Create dialog: radio（下载 / 直播；导入不显示）.
5. Detail: 「保存形态：视频」或「保存形态：音频」.

## Runner

Pass `job.source.media_save_mode` into download / record. No new JobStep.

## Tests

| Test | Assert |
| --- | --- |
| serde default | omit → `video` |
| `yt_dlp_format_args(Audio)` | has `-x` / `ba` |
| `yt_dlp_format_args(Video)` | no forced `-x` |
| create with invalid string | error (if applicable) |
| Douyin audio ffmpeg arg builder | includes `-vn` / audio out; input is play_url; no final video path |

## Spec impact (post-implement)

- `.trellis/spec/backend/job-and-pipeline.md`
- `.trellis/spec/tauri-ipc/data-contracts.md`

## Implementation order

1. Model enum + TS + create validation defaults
2. yt-dlp args + tests
3. Wire download create + runner
4. Douyin audio mode: ffmpeg `-i play_url` → audio only
5. Live audio mode
6. Frontend radio + detail
7. Manual smoke: yt-dlp audio logs `-x`/`ba`；抖音 audio 日志可见 ffmpeg 命令与 `original.m4a`

## Risks

| Risk | Mitigation |
| --- | --- |
| Platform has no separate audio | Clear error + log |
| Live audio-only codec mismatch | Prefer copy; fallback aac |
| Silent video file for transcribe | Existing extract_audio path; improve message if needed |

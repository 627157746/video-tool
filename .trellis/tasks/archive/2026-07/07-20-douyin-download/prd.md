# PRD: Douyin share-link download resolver

## Goal

Support pasting Douyin share text (or short/full links) into the download job
entry, resolve a direct play URL via the official share page `_ROUTER_DATA`,
and download media into the Job `media/` directory without relying on yt-dlp
for this path.

## Flow

1. Accept free-text input containing `https://v.douyin.com/...` (or full share /
   video URLs).
2. Extract the first Douyin URL and resolve the numeric video id (follow short
   link redirects when needed).
3. `GET https://www.iesdouyin.com/share/video/{id}/` with a mobile User-Agent.
4. Parse `window._ROUTER_DATA = {...}` and read
   `loaderData["video_(id)/page"].videoInfoRes.item_list[0].video.play_addr.url_list[0]`.
5. Replace `playwm` with `play` in that URL.
6. Download the direct URL into `jobs/<id>/media/`, write `logs/download.log`.

## Acceptance

- Share text with short link creates a download Job and succeeds when the share
  page is reachable.
- Pure `v.douyin.com` / `iesdouyin.com/share/video/{id}` / `douyin.com/video/{id}`
  inputs also work.
- Non-Douyin URLs still use the existing yt-dlp path.
- Failures are actionable in Simplified Chinese and point at `download.log`.
- Unit tests cover URL extraction, video-id parsing, and playwm→play rewrite.
- `cargo test` / `cargo clippy` for the backend pass for the changed modules.

## Out of scope

- Guaranteeing long-term Douyin page stability (best-effort, same product policy).
- Frontend redesign beyond a clearer placeholder/hint.
- Account login / cookie-based private video access.

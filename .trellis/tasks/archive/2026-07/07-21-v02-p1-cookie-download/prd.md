# v0.2 P1 Cookie / cookies-from-browser

## Goal

Improve best-effort yt-dlp downloads with optional Netscape cookies.txt and `--cookies-from-browser`, without storing cookie content in Jobs/export/logs.

## Requirements

- Global defaults: cookies file path and/or browser name (chrome/edge/firefox/...).
- Job-level override: inherit / none / file / browser.
- `source.json` stores only path or browser label, never cookie body.
- yt-dlp download path only (Douyin native path unchanged unless it falls back to yt-dlp).
- Logs mention mode/path/browser only; export packages remain free of cookie files content.

## Acceptance Criteria

- [x] Settings can set cookies file + browser default.
- [x] Create download can override auth mode.
- [x] yt-dlp gets `--cookies` or `--cookies-from-browser` when resolved.
- [x] Unit tests for arg builder; typecheck/build/clippy pass.

# Implement plan: media-save-options

## Order

1. **Model / IPC**
   - `MediaSaveMode` enum: `video` | `audio`（默认 `video`）
   - `JobSource.media_save_mode`
   - Create download / batch / live request fields
   - `src/types.ts` + `api.ts`
2. **yt-dlp**
   - `yt_dlp_format_args(mode)` — audio 走直接音频；video 保持现状
   - Wire `run_yt_dlp_download` / `run_download`
   - Unit tests
3. **Douyin**
   - 解析仍用现有 `play_url`（视频地址）
   - Audio：`ffmpeg -i <play_url> -vn ... media/original.m4a`（直接出音频，不落完整视频产物）
   - 日志记录 ffmpeg 参数；缺 ffmpeg 时给出可操作中文错误
4. **Live**
   - `LiveRecordOptions` + ffmpeg map by mode（无双输出）
5. **Frontend**
   - Create form **radio 二选一**（download + live）
   - Detail read-only label
6. **Validate**
   - `cargo test` + clippy（改动模块）
   - `pnpm typecheck`
   - Manual: audio 路径日志含 `-x`/`ba`

## Definition of done

Matches `prd.md`：严格二选一、无组合保存；音频模式禁止先完整视频再转。

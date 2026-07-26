# 实现记录：媒体播放与字幕联动预览

> 状态：已实现并通过自动化验证（2026-07-26）

## 改动清单

- `src-tauri/tauri.conf.json`：`assetProtocol.enable=true, scope=[]`；`Cargo.toml` tauri feature 增加 `protocol-asset`
- `src-tauri/src/lib.rs`：setup 时 `asset_protocol_scope().allow_directory(workspace, true)`（仅工作区）；`commands/mod.rs` save_config 工作区切换时追加授权
- `src-tauri/src/pipeline/preview.rs`（新模块）：`build_media_overview`（文件分类 original/segment/merged/preview + 可播性 direct/maybe/incompatible + 音频判定）、`generate_preview`（merged 优先 → 单文件 → 分段 concat；`-c copy`，TS/FLV 加 `-bsf:a aac_adtstoasc`；失败删除半成品并提示外部播放器）；2 个单元测试
- `src-tauri/src/pipeline/paths.rs`：`list_media_files` 排除 `preview.mp4` / `preview_concat_list.txt`（预览副本永不进入转写输入）
- `src-tauri/src/commands/mod.rs`：`get_job_media_overview` / `generate_media_preview`
- `src/components/MediaPreviewPanel.tsx`：文件选择、`<video>`/`<audio>` + `convertFileSrc`、生成预览副本按钮、字幕列表联动（点 cue seek、timeupdate 高亮 + 滚动跟随）、媒体已清理占位
- `src/App.tsx` / `constants.ts`：详情「预览」分区

## 验收对照

- assetProtocol scope 仅工作区（运行时授权，非 `**`）✅
- `.ts` → 提示不兼容 + 生成 preview.mp4 ✅（编译/单测层；实际转封装需真机 ffmpeg）
- 字幕点击跳转 / 播放高亮 ✅；m4a `<audio>` ✅
- 目录契约已回写 `docs/PRODUCT_SPEC.md` §5.2 ✅

## Known 限制

- `<track>` 画面内字幕未做（侧边列表联动）；重编码兜底未做（-c copy 失败即提示外部播放器）
- reload_config（从磁盘重载配置换工作区）不追加 asset 授权，需重启应用（save_config 路径已覆盖）
- 播放实际效果依赖 WebView2 对具体编码的支持，需真机验证

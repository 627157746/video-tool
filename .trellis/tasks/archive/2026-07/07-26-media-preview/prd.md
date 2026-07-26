# PRD: 媒体播放与字幕联动预览

> 父任务：`07-26-v03-usability`
> 状态：规划中（PRD 已定稿，待 design/implement）

## 目标

在任务详情内直接播放 Job 媒体产物，字幕以列表联动（点 cue 跳转时间点、播放中高亮当前 cue），把"核对转写质量"从外部播放器 + 肉眼对照，变成应用内一次完成。

## 决策（2026-07-26 brainstorm）

- 不兼容容器策略：**一键生成预览副本**（方案 B）——ffmpeg `-c copy` 转封装 `media/preview.mp4`，不重编码；否决了"仅原生格式"与"全量预转码"。

## 代码库确认事实

- 前端无任何 `<video>/<audio>`；`src-tauri/tauri.conf.json` 未启用 assetProtocol，WebView 目前不能读媒体文件。
- 容器现状（`src-tauri/src/pipeline/`）：
  - 下载：yt-dlp 默认选择，无 `--merge-output-format`（`download.rs:144-146, 584-586`），常见 mp4/webm 但不保证；抖音直连按 Content-Type 猜扩展名（`download.rs:313`）。
  - 录制分段：`segment_%03d.ts`（视频）/ `.m4a`（音频）（`record.rs:84-87, 169-171`）。
  - 合并：`merged.mkv` / `merged.m4a`（`record.rs:377-405`）。
  - 音频模式统一 m4a。
  - → `.ts` WebView 必不能播；`.mkv` 不保证；预览副本是录制类 Job 的刚需。
- SRT 解析仅 Rust 侧（`chapterize.rs:229` `parse_srt_cues`）；前端需新增解析（或 IPC 复用 Rust 解析返回结构化 cue，倾向后者，design 定）。

## 需求

- R1 媒体可达：启用 assetProtocol 并将 scope 限定在工作区目录（或等价安全方案，design 定）；前端经 `convertFileSrc` 播放本地文件。
- R2 预览区 UI：任务详情新增"预览"区（独立组件文件）：
  - 媒体选择：original / segments / merged / preview 副本中可选；
  - 视频用 `<video>`，纯音频（m4a）用 `<audio>`。
- R3 兼容检测与预览副本：
  - 按扩展名判定（MVP）：mp4 / webm / m4a 直接播；mkv 尝试播放并允许失败提示；`.ts` / `.flv` 等直接提示不兼容；
  - 不兼容时提供"生成预览副本"按钮 → 新 IPC 调 ffmpeg `-c copy` 产出 `media/preview.mp4`（多段 `.ts` 先 concat 或对选中单段转封装，design 定）；
  - 转封装失败（编码不兼容）时给出可读错误并引导外部播放器打开；
  - 生成预览副本受全局队列约束或明确为轻量即时操作（design 定，倾向即时 + 单 Job 互斥）。
- R4 字幕联动：读取 `srt.srt` 结构化 cue：
  - 侧边字幕列表，点 cue → `currentTime` 跳转；
  - 播放中当前 cue 高亮并自动滚动；
  - 与校对视图打通：校对界面可从 cue 跳到对应播放位置（若校对任务已交付）。
- R5 契约与联动：
  - `media/preview.mp4` 写入 `docs/PRODUCT_SPEC.md` §5.2 目录契约；
  - 预览副本不进入搜索索引、不计入转写输入；
  - 容量治理"清理媒体"应连同 preview.mp4 一并删除（media/ 整目录语义，天然覆盖）。
- R6 无字幕 Job 仅播放（隐藏字幕列表）；媒体已被清理的 Job 预览区给出"媒体已清理"占位。

## 验收标准

- mp4 下载 Job：应用内可播；点字幕行视频跳转；播放中高亮跟随。
- `.ts` 录制 Job：提示不兼容 → 生成 preview.mp4 成功 → 可播放且字幕联动正常。
- m4a 音频 Job：`<audio>` 可播。
- assetProtocol scope 仅覆盖工作区（不暴露任意磁盘路径）。
- 目录契约文档已更新；Rust fmt/clippy/test、`pnpm typecheck`/`pnpm build` 通过。

## 非目标

- 视频画面内嵌字幕渲染（`<track>`/overlay 可后置；MVP 用侧边列表联动）。
- 重编码兜底、逐帧定位、剪辑导出。
- 自动为所有 Job 预生成预览副本。

## 依赖与顺序

- 建议在 `07-26-transcript-edit` 之后实施：字幕联动直接消费校对后的 `srt.srt`；校对视图与预览的跳转打通在本任务完成。

# PRD: 新建任务「保存视频 / 保存音频」二选一

## Goal

在创建 **下载** 与 **直播录制** 任务时，提供 **互斥二选一** 的保存形态：

- **保存视频**（默认）
- **保存音频**

贯通：

- 通用链接下载（yt-dlp）
- 抖音分享链 / 短链（原生 resolver + 必要时 yt-dlp 回退）
- 直播分段录制（ffmpeg / streamlink）

**不支持「视频 + 独立音频」组合保存。**

**关键性能与语义约束：选择「保存音频」时，禁止「先完整下载/落盘视频，再 ffmpeg 转音频」作为正式产物路径。**

## Problem

当前 Ingest 行为固定为「下载/录制整段媒体」（多为带画面的容器），无法表达「只要音频」：

| 用户意图 | 现状 |
| --- | --- |
| 只要音频（转写/归档） | 仍拉完整视频流，体积与时间浪费大 |
| 只要视频 | 默认行为，但无显式声明 |

抖音路径始终拉 `play_addr` 视频流；直播始终 `-c copy` 全轨分段。转写阶段的临时 16kHz WAV 提取（`transcribe::extract_audio`）是 pipeline 内部步骤，**不是**用户意义上的「保存音频」。

## Scope

### In scope

1. **创建 UI**（下载 / 直播表单）：
   - 控件为 **单选（radio）二选一**：「保存视频」|「保存音频」
   - **不可同时选中**；无「两项皆空」状态（单选总有一项）
   - 默认：**保存视频**
2. **Job 持久化**：`source.json` 记录模式；旧 Job 缺字段时默认 `video`
3. **yt-dlp 下载**：
   - 视频：现有 best-effort 行为（可含容器内音轨）
   - 音频：`-f ba` / `-x` 等 **直接音频** 路径，最终产物仅为音频文件
4. **抖音原生下载**：
   - 视频：现有 HTTP 拉 `play` 流落盘（与今一致）
   - 音频：**复用同一视频 play URL 作为 ffmpeg 输入**，用 ffmpeg 参数直接输出音频文件（如 `-vn` + 音频编码/容器）；**禁止**先把完整视频写成 `media/` 最终产物再转音频。不依赖单独 music/音频 URL 解析作为主路径
5. **直播录制**：
   - 视频：现有全轨/视频分段
   - 音频：仅 map 音频轨 + 音频容器，不先落完整视频再转码
6. **批量下载**：同批任务共享同一单选值
7. **详情展示**：只读显示「保存视频」或「保存音频」
8. **转写兼容**：音频模式下 `media/` 为音频文件即可转写；视频模式下沿用现有从容器抽临时 WAV

### Out of scope

- **同时保存视频 + 独立音频文件**（明确不做；若以后需要另开任务）
- 本地导入（`ImportLocal`）格式转换 UI
- 画质/码率精细选择
- 保证各平台永远提供独立音频轨（best-effort）
- 改变转写内部临时 WAV 机制
- Cookie / 代理等鉴权能力变更

## User-facing behavior

### 创建表单

- 下载、直播模式显示 **单选组**：
  - ○ 保存视频（默认）
  - ○ 保存音频
- 文案提示（简中）：
  - 「保存音频时会尽量直接拉取音频流，不会先完整下载视频再转换。」

### 产物约定（用户可见）

| 模式 | 期望 `media/` |
| --- | --- |
| 保存视频 | 视频容器（如 mp4/ts/mkv 等）；**不**另存独立音频文件 |
| 保存音频 | **仅** 音频文件（如 m4a/mp3/opus）；**无** 完整视频文件作为最终产物 |

文件命名尽量沿用现有 `original.*` / `segment_*` 约定。

### 日志

- `logs/download.log` / `logs/record.log` 记录生效模式与 format/map 策略（不含密钥）

## Acceptance criteria

- [x] 下载 / 直播创建表单为「保存视频 | 保存音频」**二选一**，默认视频
- [x] **无** 同时勾选 / 组合保存 UI 与 API 语义
- [x] 选项写入 Job（枚举或等价字段），详情可回看；旧 `source.json` 缺字段 → 视频
- [x] **保存音频 + yt-dlp**：不先完整落盘视频再转音频；产物为音频；日志可证明音频 format / `-x` 类策略
- [x] **保存音频 + 抖音**：以 play 视频 URL 为 ffmpeg 输入、参数直接出音频；最终 `media/` 无完整视频文件
- [x] **保存音频 + 直播**：分段结果为音频，不先整段视频落盘再转
- [x] 保存视频路径与现有行为兼容；失败时指向对应 log
- [x] `src/types.ts` 与 Rust 请求/Job 字段同步；`pnpm typecheck` 通过
- [x] 相关单元测试（模式解析、yt-dlp 参数、默认兼容）；后端 `cargo test` / clippy 针对改动模块通过

## Non-goals / risks

- 部分直播源无音频轨：best-effort，失败信息需可操作
- 抖音分享页结构变更：与现有 best-effort 策略一致
- 「保存视频」是否强制剥离音轨：MVP **否**（容器内可保留音轨）；只是不额外导出独立音频文件

## Open questions (resolved for MVP)

| 问题 | MVP 决定 |
| --- | --- |
| 可组合？ | **否，严格二选一** |
| 默认 | 保存视频 |
| 仅音频实现 | yt-dlp：直接音频 format；抖音：ffmpeg 以 play 视频 URL 为输入参数输出音频；均禁止完整视频落盘后再转 |
| 抖音音频是否解析独立 music URL | **否（MVP）**；统一 ffmpeg 吃视频 play 路径出音频 |
| 是否影响本地导入 | 否 |

## Related code (starting points)

- `src/App.tsx` — 创建表单
- `src/api.ts` / `src/types.ts` — IPC 镜像
- `src-tauri/src/models/job.rs` — `JobSource` / create requests
- `src-tauri/src/commands/mod.rs` — create download / live
- `src-tauri/src/pipeline/download.rs` — yt-dlp / Douyin 落盘
- `src-tauri/src/pipeline/douyin.rs` — 分享页解析
- `src-tauri/src/pipeline/record.rs` — 直播分段
- `src-tauri/src/pipeline/runner.rs` — ingest 调度

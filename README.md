# video-tool

桌面端视频工具（私用）：链接下载 / 直播录制 / 本地转写 / AI 总结。

- 仓库：<https://github.com/627157746/video-tool>
- 发布页：<https://github.com/627157746/video-tool/releases>
- 产品共识：[`docs/PRODUCT_SPEC.md`](docs/PRODUCT_SPEC.md)

应用内「检查更新」默认查询上述 GitHub Releases；发现新版本且有 Windows 安装包时可「下载并安装更新」（确认后**静默安装并自动重启**，无向导）。首次安装推荐 NSIS 一键包。私有镜像可用环境变量 `VIDEO_TOOL_RELEASE_API` / `VIDEO_TOOL_RELEASE_PAGE` 覆盖，私有仓库可设 `VIDEO_TOOL_GITHUB_TOKEN`。

## 技术栈

- **Tauri 2** + **Rust** 编排
- **React 19** + **TypeScript** + **Vite** 前端
- 包管理：**pnpm**（请勿使用 npm / yarn 安装依赖）
- Sidecar：`ffmpeg` / `ffprobe` / `yt-dlp` / `streamlink` / whisper.cpp

## 当前版本

应用版本 **`0.3.3`**（`package.json` / `src-tauri/Cargo.toml` / `tauri.conf.json`）。

### v0.3.3

- 二次确认改为**应用内模态**（与任务中心 UI 一致），不再使用系统原生对话框
- 修复：确认框有时需点两次才弹出（StrictMode 下队列被吃掉）
- 补全确认：删除 Provider / 总结模板 / 任务分组、放弃校对修改
- 重跑/重试与「已校对」风险合并为**单次确认**，避免连弹两次

### v0.3.2

- 修复：下载过程中误报「任务不存在」（进度写盘节流 + 读取短重试 + 运行中 UI 软处理）
- 修复：应用内更新后**自动重启**更可靠（独立 helper 等待进程退出再安装并拉起）
- 重要操作二次确认：删除、清理媒体、重跑媒体/转写/总结、停止录制、导出、导入配置、修复工作区、重建索引等
- `.cursor/mcp.json` 加入 gitignore，避免 MCP 密钥误提交

### v0.3.1

- 应用内更新：静默安装完成后**自动退出并重新启动**，无需手动关闭再打开
- 已有下载 / 直播任务可在详情中重配「保存视频 | 保存音频」；切换后清除旧媒体产物并标记需重新下载/录制

### v0.3.0

- 任务完成系统通知：Job 成功/失败时发 Windows 通知（前台聚焦不弹；设置可关）
- 工作区容量治理：占用统计、按媒体体积排序、单任务「清理媒体保留文字资产」
- 转写文本校对：任务详情按字幕行编辑，保存同步回写 `srt.srt` + `plain.txt` 并自动备份；章节/总结失效重跑
- 媒体播放与字幕联动预览：应用内播放产物；`.ts`/`.mkv` 一键转封装 `media/preview.mp4`；点字幕行跳转、播放高亮跟随
- 抖音视频下载修复：拉流 403 时自动改用 yt-dlp 直下已解析的 play URL（无需 Cookie），并保留解析标题

### v0.2.5

- 抖音直播：原生解析房间页 `stream_url`（FLV/HLS），不再依赖已失效的 streamlink 字段
- 直播解析失败时不再把房间 HTML 页交给 ffmpeg（避免 `Invalid data found when processing input`）
- 录制时为抖音拉流附带 User-Agent / Referer

### v0.2.4

- 新建下载 / 直播任务支持「保存视频 | 保存音频」二选一（`media_save_mode`）
- 仅音频：yt-dlp 直接音频 format；抖音以 play URL + ffmpeg `-vn`（含 `.part` 强制 muxer）；直播仅 map 音频轨
- 旧 Job 缺字段默认保存视频；任务详情可回看保存形态

### v0.1 基线

- 统一 Job 任务中心：列表、搜索、步骤状态、日志、产物查看和目录打开
- 三种入口：链接下载、直播分段录制、本地媒体导入
- 自动流水线与单步重试：导入/下载/录制 → 分段转写 → 合并文字 → AI 总结
- 直播录制保护：按时长分段、断流重连、磁盘阈值、心跳、停止与媒体合并
- whisper.cpp 本地转写、单段重试、选段和字幕/纯文本合并
- OpenAI 兼容与 Anthropic 双协议总结、自定义 base URL、代理和 Markdown 模板
- Provider/模板多档案、环境变量 Key 覆盖、连通测试和日志脱敏
- Sidecar 探测（内置 → 配置路径 → PATH）、版本展示和 yt-dlp 更新操作
- Job 导出、启动恢复、单实例锁，以及录制期间托盘/关窗保活
- 工作区目录契约：`workspace/jobs/<job_id>/{media,transcript,summary,logs,source.json}`

### v0.2 增量（P0–P4 MVP，已交付）

- 全局并发队列 / 排队状态、工作区健康诊断与可安全修复
- 批量 URL + `batch_id`、Cookie / 浏览器登录态辅助下载
- 稳定 `error_code` 与失败修复建议
- 术语表、转写模型档位、章节大纲（`Chapterize`）、多模板总结
- 跨 Job 本地全文检索（`workspace/index/`）
- 依赖向导、模型扫描、配置导入导出（默认去 Key）、检查更新

细节与边界见 [`docs/PRODUCT_SPEC.md`](docs/PRODUCT_SPEC.md) 第 14–15 节。

当前实现不等于目标环境已验收。真实下载、直播、转写和云端总结仍依赖
本机 sidecar、whisper.cpp 模型、网络、代理及有效 API Key；安装包是否携带
预期 sidecar 也应以实际打包产物为准。

## 开发环境

### 依赖

- Node.js 20+
- **pnpm** 9+（推荐 10.x）
- Rust 1.88+（Windows：`x86_64-pc-windows-msvc`）
- [Tauri 2 系统依赖](https://v2.tauri.app/start/prerequisites/)（Windows 需 WebView2；MSVC 构建工具用于完整桌面构建）

业务链路运行依赖（按使用能力安装或配置）：

- `ffmpeg` / `ffprobe`
- `yt-dlp`
- `streamlink`
- whisper.cpp `whisper-cli` 与 GGML 模型

### 安装与运行

```bash
# 若尚未安装 pnpm
corepack enable
corepack prepare pnpm@latest --activate
# 或: npm install -g pnpm

pnpm install
pnpm tauri:dev
```

仅前端（浏览器无 Tauri IPC 时会报错，适合改 UI）：

```bash
pnpm dev
```

类型检查 / 前端构建：

```bash
pnpm typecheck
pnpm build
```

Rust 检查：

```bash
cargo +stable fmt --manifest-path "src-tauri/Cargo.toml" --all -- --check
cargo +stable test --manifest-path "src-tauri/Cargo.toml"
cargo +stable clippy --manifest-path "src-tauri/Cargo.toml" --all-targets -- -D warnings
```

### 包管理约定

| 正确 | 错误 |
|------|------|
| `pnpm install` | `npm install` |
| `pnpm add <pkg>` | `npm i <pkg>` |
| 提交 `pnpm-lock.yaml` | 提交 `package-lock.json` |

## 配置与工作区

- 应用配置：系统配置目录下 `video-tool/config.json`  
  （Windows 通常为 `%APPDATA%/video-tool/config.json`）
- 默认工作区：本地数据目录下 `video-tool/workspace`  
  （Windows 通常为 `%LOCALAPPDATA%/video-tool/workspace`）
- API Key：支持配置文件字段或环境变量（**环境变量优先**），例如：
  - `OPENAI_API_KEY`
  - `ANTHROPIC_API_KEY`

密钥不会写入 Job 的 `source.json`。

## 仓库结构

```text
docs/PRODUCT_SPEC.md     # 产品规格（单一事实来源）
src/                     # React 前端
src-tauri/               # Tauri + Rust 核心
.trellis/                # Trellis 工作流
pnpm-lock.yaml           # 依赖锁文件
```

## License

Private / personal use.

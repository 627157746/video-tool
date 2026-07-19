# video-tool

桌面端视频工具（私用）：链接下载 / 直播录制 / 本地转写 / AI 总结。

产品共识见 [`docs/PRODUCT_SPEC.md`](docs/PRODUCT_SPEC.md)。

## 技术栈

- **Tauri 2** + **Rust** 编排
- **React 19** + **TypeScript** + **Vite** 前端
- 包管理：**pnpm**（请勿使用 npm / yarn 安装依赖）
- Sidecar：`ffmpeg` / `yt-dlp` / `streamlink` / 本地转写（后续接入）

## 当前进度（初始化骨架）

已具备：

- 任务中心 UI（列表、搜索、三个新建入口、设置只读预览）
- Job 目录契约：`workspace/jobs/<job_id>/{media,transcript,summary,logs,source.json}`
- 配置加载（应用配置目录 + 工作区分离）
- Provider / 模板默认档案
- Sidecar 探测（内置 → 配置路径 → PATH）

尚未实现：

- 真实下载 / 直播分段录制
- 本地转写与字幕合并
- 云端总结（OpenAI / Anthropic + 自定义 base URL）
- 流水线自动执行、重试、导出、托盘保活等

## 开发环境

### 依赖

- Node.js 20+
- **pnpm** 9+（推荐 10.x）
- Rust 1.88+（Windows：`x86_64-pc-windows-msvc`）
- [Tauri 2 系统依赖](https://v2.tauri.app/start/prerequisites/)（Windows 需 WebView2；MSVC 构建工具用于完整桌面构建）

可选（业务真正跑通时）：

- `ffmpeg` / `ffprobe`
- `yt-dlp`
- `streamlink`

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
cd src-tauri
cargo check
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

# video-tool 产品规格（共识文档）

> 状态：设计共识已锁定（来源：产品 grill 会话）  
> 目标用户：仅自己使用，架构按「可演进到小范围分发」预留  
> 版本目标：v0.1 = **接近自用定稿**（非「仅骨架能跑」）  
> 包管理：**pnpm**（不要使用 npm / yarn）  
> 文档用途：实现与任务拆分时的单一事实来源；改需求先改本文再改代码

---

## 1. 产品一句话

桌面端视频工具：用宽入口拿到视频（链接下载 / 直播录制 / 本地文件），本地转写为文字，再通过自建 API（OpenAI 兼容或 Anthropic 格式）做 Markdown 总结；一切以统一任务（Job）为中心。

---

## 2. 范围与非目标

### 2.1 必须覆盖的能力（并行推进）

| 能力 | 说明 |
|------|------|
| 链接下载 | 粘贴 URL，尽力下载为本地媒体（宽入口，不绑死平台） |
| 直播录制 | 通用流等入口，按时长分段录制，可合并 |
| 本地处理 | 已有视频进入同一 Job 流水线 |
| 转写 | 本地语音转文字，支持分段转写后合并全文 |
| AI 总结 | 云端 API，只上传文本，自定义 Markdown 模板 |

### 2.2 平台策略

- **「能下就行」**：优先依赖 `yt-dlp`、streamlink、ffmpeg 等通用工具链。
- **不承诺**某一平台（抖音 / B 站 / 某直播间一键解析等）长期稳定可用。
- UI 与错误文案需写明：**最佳努力**；失败时给出可排障信息（工具版本、日志路径、原始 URL）。
- 每个 Job 记录：原始 URL、工具与版本、关键参数、完整日志，便于自用排障。

### 2.3 明确非目标（v0.1）

- 商业化、账号体系、多租户 SaaS
- 平台专项深适配（如「抖音无水印专版」作为核心承诺）
- 总结强制 JSON 结构化双产出
- 超长文本静默截断或自动 map-reduce
- macOS / Linux 作为 v0.1 交付承诺（可预留结构，不保证可用）
- 完备自动更新与签名分发方案（可后置）

---

## 3. 用户与验收标准

### 3.1 用户

- 第一用户：**仅自己**
- 安装与 UI 可接受「开发者向」细节，但任务状态、日志、重试必须清晰

### 3.2 v0.1「接近自用定稿」验收清单

在「三条主链路都能跑通」之上，还需包含：

| 项 | 要求 |
|----|------|
| 下载 | 宽入口 URL → 落入 Job 的 `media/`，任务列表可见成功/失败/日志 |
| 直播 | 分段录制 + 断流重连 + 磁盘保护 + 心跳日志；可停录；段可合并 |
| 转写 | 本地转写；**分段转写**；**按时间顺序合并文字** |
| 总结 | 对**合并后全文**套模板生成 `summary/summary.md`；OpenAI + Anthropic + 自定义 base URL |
| 流水线 | 创建任务时可勾选：下载/录制后自动转写、转写后自动总结 |
| 分段批量转写 | 多段可排队，状态可追踪 |
| 单步重试 | 可只重跑下载 / 转写 / 合并 / 总结中的某一步 |
| Provider | 多配置档案、默认档案、任务可覆盖、连通测试 |
| 模板 | 多 Markdown 模板档案、变量替换、内置 2～3 个示例 |
| 配置 | 混合配置 + 环境变量覆盖 Key；工作区与 Key 分离 |
| 代理 | 总结出网可配代理 |
| 导出 | 导出任务包（**不含** API Key） |
| 日志脱敏 | 不落完整 Key；prompt 可截断记录 |
| 托盘/关窗 | 录制中避免误关导致进程被杀（至少保活录制） |
| 历史 | 按标题 / URL / 状态 / 时间等简单搜索 Job |
| 依赖 | Sidecar 解析与版本展示；yt-dlp 可检查更新 |

未齐上述清单，不宣称 v0.1 完成。

### 3.3 建议内部里程碑（验收仍按定稿清单）

1. Job 契约 + 任务中心 + sidecar 解析 + 配置 / provider  
2. 下载 + 直播分段录制 + 媒体合并  
3. 分段转写 + transcript 合并  
4. 双协议总结 + 模板 + 超长失败与选段缩小范围  
5. 流水线、重试、导出、搜索、托盘保活、体验打磨  

---

## 4. 技术架构

### 4.1 技术选型

| 层 | 选型 |
|----|------|
| 桌面壳 | Tauri 2 |
| 核心编排 | Rust（任务队列、进程/sidecar 管理、工作区、配置、IPC） |
| UI | React 19 + TypeScript + Vite |
| 包管理 | **pnpm** |
| 下载/流 | yt-dlp、streamlink、ffmpeg 等 sidecar |
| 转写 | 本地 whisper.cpp（`whisper-cli`；可执行文件、GGML 模型路径与语言可配置） |
| 总结 | 云端 HTTP API；**视频文件永不上传** |

### 4.1.1 工程初始化状态

仓库已完成 Tauri 2 骨架初始化：

- 包名：`video-tool` / identifier `com.videotool.app`
- 前端：`src/`（任务中心、步骤/分段重试、产物查看、Provider/模板/sidecar 设置）
- 后端：`src-tauri/`（Job 状态机、完整流水线、workspace、sidecar 进程监管、双协议总结与 IPC）
- 配置文件：`%APPDATA%/video-tool/config.json`（Windows）
- 默认工作区：`%LOCALAPPDATA%/video-tool/workspace`（Windows；可用配置覆盖）
- 包管理：`pnpm`（`pnpm-lock.yaml`；禁止提交 `package-lock.json`）

v0.1 实现已覆盖下载、本地导入、直播分段录制、本地转写与合并、双协议总结、自动流水线、单步/单段重试、选段、导出和录制托盘保活。外部工具与云端 Provider 的真实可用性按目标环境配置验收。

### 4.2 模块边界

```
┌─────────────────────────────────────────────┐
│  Frontend（任务中心 / 新建 / 设置）           │
└─────────────────────┬───────────────────────┘
                      │ Tauri IPC
┌─────────────────────▼───────────────────────┐
│  Rust Core                                   │
│  · Job store & state machine                 │
│  · Pipeline（可选自动步骤）                   │
│  · Sidecar resolver & process supervisor     │
│  · Config / provider / template store        │
│  · Workspace I/O                             │
└─────┬───────────┬───────────┬───────────────┘
      │           │           │
      ▼           ▼           ▼
  yt-dlp      ffmpeg/      本地转写
  streamlink  录制合并     运行时
      │           │           │
      └───────────┴─────┬─────┘
                        ▼
                 云端 Summarizer
              (OpenAI | Anthropic)
```

### 4.3 运行平台与 sidecar

| 项 | 决策 |
|----|------|
| 主平台 | Windows 10/11 x64（日常只保证此平台） |
| 跨平台 | 路径与 sidecar 查找按 OS 分支**预留**，v0.1 不承诺其它 OS 可用 |
| Sidecar 查找顺序 | 1) 应用内置路径 → 2) 用户配置路径 → 3) 系统 PATH |
| 设置页 | 展示当前实际使用的二进制路径与版本 |
| yt-dlp | 支持单独「检查更新」（变更频繁） |
| 转写 | 可配置可执行文件 / 模型目录，不绑死单一安装方式 |

---

## 5. Job 模型与落盘结构

### 5.1 设计原则

- 任何来源最终落到：**本地媒体 + 元数据**；转写与总结只消费工作区产物，不直接依赖平台协议。
- 下载、直播、转写、总结均为可调度步骤，共享进度、日志、失败与重试。

### 5.2 目录约定

```text
workspace/
  jobs/
    <job_id>/
      source.json           # 来源、参数、工具版本、provider/template id（无 Key）
      media/
        original.*          # 下载或录制原始产物（可多段）
        segment_001.*       # 直播分段示例命名（实现可调整，需在 source.json 索引）
        merged.*            # 可选：分段合并后的媒体
      transcript/
        segments/           # 每段转写原始结果
          segment_001.json
          segment_001.txt
          segment_001.srt
        plain.txt           # 合并后的纯文本（总结主输入）
        srt.srt             # 可选：合并字幕
        raw.json            # 可选：合并后带时间轴结构
      summary/
        summary.md          # 正式总结产出
        meta.json           # 使用的 provider_profile_id、template_id、模型名、时间等
      logs/
        download.log
        record.log
        transcribe.log
        merge_transcript.log
        summarize.log
```

### 5.3 流水线语义

默认步骤顺序：

```text
ingest（download | live-record | import-local）
  →（可选）auto-transcribe
      → 分段转写
      → 合并 transcript → plain.txt 等
  →（可选）auto-summarize
      → 读取合并后全文 + 模板
      → 调用云端 API
      → summary/summary.md
```

**推荐默认勾选：**

- 默认勾选：自动转写  
- 默认不勾选：自动总结（耗 Key、长文更慢；宜先看字幕再总结）

### 5.4 直播录制行为

| 项 | 决策 |
|----|------|
| 策略 | 按**时长**分段 |
| 默认段长 | **30 分钟**（设置可改） |
| 磁盘保护 | 剩余空间低于阈值自动停录 |
| 断流 | 自动重连，有限次数；仍失败则标记失败/可续录 |
| 心跳 | 录制中写日志与存活状态，避免假死无反馈 |
| 合并 | 支持将多段媒体合并为完整文件（允许重封装；不追求完美无缝剪辑级） |

### 5.5 转写与总结的文本路径（关键）

**正式路径（已锁定）：**

1. 视频（含直播多段）**分段转写**  
2. 按时间顺序 **合并文字**  
3. 对 **合并后的全文** 做 AI 总结  

| 项 | 决策 |
|----|------|
| 总结输入 | `transcript/plain.txt`（或等价合并结果），**不是**各段 summary |
| 分段摘要 map-reduce | v0.1 **不做** |
| 静默截断 | v0.1 **不做** |
| 超长上下文 | **任务失败**，错误需可读：大致长度、限制原因、建议 |
| 缩短范围 | 支持只选择部分 segment 参与合并后再总结（或换更大上下文模型） |

---

## 6. 界面信息架构

### 6.1 主界面：任务中心

- **首页** = 全部 Job 列表：状态、进度、步骤、日志入口、打开目录、按步重试  
- **新建入口**（顶栏或侧栏）：  
  1. 下载链接  
  2. 录制直播  
  3. 本地转写 / 总结（导入已有视频）  
- **设置页**独立：工作区路径、sidecar、whisper/模型、provider、模板、代理、分段时长、流水线默认勾选等  

### 6.2 不采用

- 三栏互不相关工作台作为第一公民（易与统一 Job 模型冲突）  
- 纯向导导致高级重试/多任务难用  

---

## 7. 配置、密钥与隐私

### 7.1 配置布局（混合）

| 内容 | 存放 |
|------|------|
| 非敏感应用配置 | 系统应用数据目录（如 Windows `%AppData%/video-tool/`） |
| API Key / 端点 | 配置文件 **或** 环境变量；**环境变量优先覆盖文件** |
| 工作区 | 用户可配置路径；与 Key **分离**（避免备份素材时带走 Key） |

### 7.2 必须能力

| ID | 能力 |
|----|------|
| A | 设置中「测试 API 连通」（按 provider 档案） |
| B | 日志脱敏：不写完整 Key；prompt 可截断 |
| C | 一键导出任务包（媒体/字幕/总结/元数据，**不含 Key**） |

### 7.3 出网边界

- 视频下载/录制：按 URL 与工具行为出网  
- 转写：**本地**，视频与音频不因总结而出网  
- 总结：仅文本请求；可走系统代理或自定义 HTTP(S) 代理  

---

## 8. AI Provider 与总结模板

### 8.1 Provider 档案（多配置）

每个档案至少包含：

| 字段 | 说明 |
|------|------|
| `id` | 稳定标识 |
| `name` | 展示名 |
| `protocol` | `openai` \| `anthropic` |
| `base_url` | 自定义 API 地址（自建中转/网关） |
| `api_key` / `api_key_env` | 直填或环境变量名；env 优先 |
| `default_model` | 默认模型名 |
| `extra_headers` | 可选 |

行为：

- 全局 **默认档案**  
- 创建总结或流水线时 **可覆盖** 选用其它档案  
- Job 元数据记录 `provider_profile_id`、模型名等，**永不写入 Key**  
- 连通测试按档案执行  

示意（非最终实现格式）：

```toml
[[providers]]
id = "my-openai-relay"
protocol = "openai"
base_url = "https://your.example/v1"
api_key_env = "MY_OPENAI_KEY"
default_model = "gpt-4o-mini"

[[providers]]
id = "my-anthropic-relay"
protocol = "anthropic"
base_url = "https://your.example"
api_key_env = "MY_ANTHROPIC_KEY"
default_model = "claude-sonnet-4-5"
```

### 8.2 总结模板（多模板 Markdown）

| 项 | 决策 |
|----|------|
| 形态 | 系统 prompt + 用户模板正文 |
| 变量（示例） | `{{title}}` `{{transcript}}` `{{duration}}` `{{source_url}}` |
| 产出 | `summary/summary.md` |
| 档案 | 多模板；全局默认；任务可覆盖 |
| 内置 | 2～3 个示例模板 |
| 不做 | 强制固定 JSON schema 双产出（用户可在模板中自行要求 JSON 文本） |

---

## 9. 错误与边界行为摘要

| 场景 | 行为 |
|------|------|
| yt-dlp/平台失败 | Job 失败；保留日志与命令上下文；可重试下载步 |
| 直播断流 | 重连至上限；分段已落盘部分保留 |
| 磁盘不足 | 停录并标记原因 |
| 转写失败 | 可单段/全任务重试；不自动删媒体 |
| 合并全文超模型上下文 | **失败** + 明确建议（换模型 / 选段缩小） |
| API Key 无效 | 连通测试与总结步明确错误；脱敏日志 |

---

## 10. 工程与开发约定

### 10.1 包管理（强制 pnpm）

- 安装依赖：`pnpm install`
- 开发：`pnpm tauri:dev` / `pnpm dev`
- 构建前端：`pnpm build`
- 类型检查：`pnpm typecheck`
- **禁止**使用 `npm install` / `yarn` 作为日常依赖安装方式
- 锁文件只保留 `pnpm-lock.yaml`；`package-lock.json` / `yarn.lock` 应删除且被 gitignore
- Tauri `beforeDevCommand` / `beforeBuildCommand` 必须调用 **pnpm**

### 10.2 实现约束

- 小步可逆改动；先定 Job 目录契约与状态机，再填三条业务线。  
- 三条业务线并行时，**禁止**各自发明互不兼容的输出目录。  
- 新增平台适配只能以 Source/Adapter 形式扩展，不改转写/总结核心。  
- 不把假设写成已验证事实；外部工具行为以本机实际 sidecar 版本为准。  
- 用户可见文案与本产品文档默认使用简体中文。  

---

## 11. 决策日志（grill 锁定顺序）

1. 用户：仅自己用  
2. 能力：下载 + 直播 + 转写总结都要；**并行**推进  
3. 技术：Tauri 2 + Rust + 前端；sidecar 重活  
4. 平台：宽入口「能下就行」  
5. AI 算力：转写本地 + 总结云端（OpenAI 兼容）；后扩展为 **同时 Anthropic + 自定义 base URL**  
6. 任务：可选流水线 + 统一 `jobs/<id>/`  
7. 直播：按时长分段（默认 30 分钟）+ 磁盘/重连/心跳  
8. UI：任务中心为主  
9. 配置：混合 + env 覆盖；A/B/C 全要  
10. Provider：多档案 + 任务可覆盖  
11. v0.1：接近自用定稿（非仅骨架）  
12. 模板：多 Markdown 模板 + 变量  
13. OS/依赖：Windows 为主并预留跨平台结构；sidecar 混合解析（内置 → 配置 → PATH）  
14. 文本路径：分段转写 → **合并文字** → 总结  
15. 超长：失败并提示；可缩小 segment 范围；不做截断/自动 map-reduce  
16. 工程：包管理使用 **pnpm**  

---

## 12. 文档维护

- 实现阶段若变更已锁定决策：**先更新本节与对应章节，再改代码**。  
- v0.1 转写引擎已锁定为 whisper.cpp `whisper-cli`；配置字段为 `sidecar_paths.transcribe`、`transcribe_model`、`transcribe_language`。

---

## 13. 下一步

- [x] 产品规格文档  
- [x] Tauri 2 工程骨架（任务中心 + Job 落盘 + 配置/sidecar 探测）  
- [x] 切换为 pnpm  
- [x] 下载执行器（yt-dlp）
- [x] 直播分段录制
- [x] 本地转写 + 合并文字
- [x] 双协议总结 + 模板
- [x] 流水线调度 / 重试 / 导出 / 托盘

实现验证：当前 28 项 Rust 单元测试、严格 Clippy、Rust 格式检查、TypeScript 类型检查与前端生产构建均已重新执行并通过。此前交付已生成 Tauri release 可执行文件与 MSI；NSIS 打包当时因从 GitHub 下载外部工具包超时而未完成。自动检查不替代真实环境验收；下载、直播、转写和云端总结仍依赖本机安装对应 sidecar、模型文件、网络及有效 API Key，应在目标环境按实际来源完成验收。

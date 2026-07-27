# video-tool 产品规格（共识文档）

> 状态：**v0.3 已交付**（应用版本 `0.3.1`）；v0.1 为基线，第 14 节为 v0.2 已实现能力说明（原路线图，非未交付清单）  
> 目标用户：仅自己使用，架构按「可演进到小范围分发」预留  
> 版本：`0.3.1` = `0.3.0` + 更新后自动重启 + 已有任务可重配 `media_save_mode`（决策日志 26 条）；后置能力仍见各节「非目标」  
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

### 2.4 v0.2 及以后仍明确不做（边界）

以下即使在路线图讨论中出现，也**不作为当前版本承诺**，避免范围膨胀：

- 账号体系、云同步媒体、多租户
- 内嵌浏览器登录页 / 自动扫盘窃取 Cookie
- 平台专版稳定性承诺（Cookie 与下载仍属「最佳努力」）
- 强制 JSON schema 双产出为唯一总结路径
- 全自动 map-reduce 静默总结（可用「章节大纲 + 选段/按章」近似，见 14.4）
- 说话人 diarization、非线性剪辑时间线
- 向量语义搜索、笔记库双向同步（导出到本地 Markdown/Obsidian 可后置讨论，不在本轮锁定）
- 签名静默自动更新安装包（「检查更新 + 打开发布页」可做）
- 播放列表深度编排、父子 Job 复杂工作流（批量 URL 以「多 Job + 可选 batch_id」为 MVP）

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
| Provider | 多配置档案、每档案多模型列表、默认档案/默认模型、任务可覆盖模型、连通测试 |
| 模板 | 多 Markdown 模板档案、变量替换、内置 2～3 个示例 |
| 配置 | 混合配置 + 环境变量覆盖 Key；工作区与 Key 分离 |
| 代理 | 总结出网可配代理 |
| 导出 | 导出任务包（**不含** API Key） |
| 日志脱敏 | 不落完整 Key；prompt 可截断记录 |
| 托盘/关窗 | 录制中避免误关导致进程被杀（至少保活录制） |
| 历史 | 按标题 / URL / 状态 / 时间等简单搜索 Job；支持自定义分组目录（设置中管理）与按分组筛选 |
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
  index/
    search.sqlite3          # 跨 Job 全文检索 FTS 索引（v0.2 P3；无密钥）
  jobs/
    <job_id>/
      source.json           # 来源、参数、工具版本、provider/template id（无 Key）
      media/
        original.*          # 下载或录制原始产物（可多段）
        segment_001.*       # 直播分段示例命名（实现可调整，需在 source.json 索引）
        merged.*            # 可选：分段合并后的媒体
        preview.mp4         # 可选：应用内预览副本（ffmpeg -c copy 转封装；不进入流水线媒体索引，v0.3）
      transcript/
        segments/           # 每段转写原始结果
          segment_001.json
          segment_001.txt
          segment_001.srt
        plain.txt           # 合并后的纯文本（总结主输入）
        plain.prev.txt      # 可选：手工校对前的上一版全文备份（v0.3）
        srt.srt             # 可选：合并字幕
        srt.prev.srt        # 可选：手工校对前的上一版字幕备份（v0.3）
        raw.json            # 可选：合并后带时间轴结构
      summary/
        summary.md          # 主模板总结产出（兼容路径）
        by_template/        # 多模板额外产物：<template_id>.md（v0.2 P3）
        meta.json           # provider_profile_id、template_ids、模型名、时间等（无 Key）
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
| `models` | 该档案下可用模型列表（同一 Key / Base URL） |
| `default_model` | 默认模型名（必须属于 `models`） |
| `extra_headers` | 可选 |

行为：

- 全局 **默认档案**  
- 创建总结或流水线时 **可覆盖** 选用其它档案，并在该档案的模型列表中 **切换模型**  
- 若任务仍跟随全局默认（未显式覆盖），Job 的 `provider_profile_id` / `template_id` / `model` 存 `null`，**总结运行时**再解析当前全局默认；改设置后重跑总结会用新默认  
- 若创建时显式选了非默认档案/模型，则固化到 Job，后续改全局默认不影响该任务  
- **已创建任务**可在任务详情「总结配置」中修改 Provider / 模型 / 模板（空=跟随默认）；保存后使总结产物失效，需重跑「AI 总结」  
- 历史任务若曾写死旧默认 ID，也可在详情中改为「使用全局默认」或指定新档案  
- 总结产物 `summary/meta.json` 记录实际使用的 `provider_profile_id`、`model` 等，**永不写入 Key**  
- 连通测试按档案执行（验证端点与 Key，不绑定某一个模型）  
- 旧配置仅有 `default_model`、无 `models` 时，加载时自动补全为单模型列表  

示意（非最终实现格式）：

```toml
[[providers]]
id = "my-openai-relay"
protocol = "openai"
base_url = "https://your.example/v1"
api_key_env = "MY_OPENAI_KEY"
default_model = "gpt-4o-mini"
models = ["gpt-4o-mini", "gpt-4o", "o3-mini"]

[[providers]]
id = "my-anthropic-relay"
protocol = "anthropic"
base_url = "https://your.example"
api_key_env = "MY_ANTHROPIC_KEY"
default_model = "claude-sonnet-4-5"
models = ["claude-sonnet-4-5", "claude-opus-4-5"]
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
17. v0.2 路线图选型锁定（见第 14 节）：全局队列、跨 Job 检索、转写质量与失败向导、批量 URL、Cookie 辅助下载、章节大纲、术语表、多模板多产物、以及依赖/模型/配置迁移/检查更新等安装分发体验；**先写规格再实现**  
18. **v0.2 交付**（2026-07-21）：第 14 节 P0–P4 MVP 已实现；应用版本号升至 **`0.2.0`**（`package.json` / `src-tauri/Cargo.toml` / `tauri.conf.json`）  
19. **保存视频 / 保存音频二选一**（2026-07-24，应用 **`0.2.4`**）：`JobSource.media_save_mode` 为 exclusive `video` \| `audio`；音频路径禁止先完整落盘视频再转（yt-dlp 直接音频；抖音 ffmpeg `-i play_url -vn`；直播仅 map 音频轨）  
20. **抖音直播原生流解析**（2026-07-24，应用 **`0.2.5`**）：`live.douyin.com` 房间页优先解析 `room.stream_url` 的 FLV/HLS 拉流地址；streamlink 失败时不得把房间 HTML 当作 ffmpeg 输入  
21. **任务完成系统通知**（2026-07-26，v0.3 起）：`tauri-plugin-notification`；仅 Job 终态（成功/失败各一条）触发，主窗口聚焦时抑制；配置 `notify_on_job_finish`（默认 `true`，Serde 默认兼容旧配置，配置导出包含该开关）；通知文案含任务标题（截断）与失败 `error_code`，不含 Key/Cookie  
22. **工作区容量治理**（2026-07-26，v0.3）：纯手动逐 Job 清理——`purge_job_media` 删除 `media/` 全部内容（含 preview 副本），保留 transcript/summary/logs/source.json；`Job.media_purged_at` 标记；清理后转写/分段重试被禁止（后端守卫 + UI 禁用），下载类重跑 ingest 成功后清除标记；`get_workspace_usage` 提供总占用/剩余空间/按媒体体积降序的 Job 列表；**不做**自动清理策略  
23. **转写文本校对**（2026-07-26，v0.3）：按字幕 cue 编辑（`srt.srt` 为编辑事实源），保存同步回写 `srt.srt` + `plain.txt`（时间轴不变，仅文本；空文本删行）；保存前备份 `srt.prev.srt` / `plain.prev.txt`（覆盖式单版本）；`Job.transcript_edited_at` 标记；保存后 Chapterize/Summarize 失效需重跑；重跑合并/分段重试/选段变更前 UI 弹覆盖警告，执行后清除标记；无 SRT 任务降级整篇编辑 `plain.txt`；不回写 `transcript/segments/*`，`raw.json` 保持原样（known 不一致）  
24. **媒体播放与字幕联动预览**（2026-07-26，v0.3）：启用 assetProtocol（运行时 `allow_directory` 仅授权工作区目录，含工作区切换）；任务详情「预览」分区应用内播放；不兼容容器（.ts/.flv 等）一键 `ffmpeg -c copy` 转封装 `media/preview.mp4`（TS/FLV 源附加 `-bsf:a aac_adtstoasc`；多段 concat）；preview.mp4 **不进入**流水线媒体索引与转写输入；字幕列表联动（点 cue 跳转、播放高亮跟随）；不做画面内嵌字幕渲染与重编码兜底  
25. **抖音视频下载降级链**（2026-07-26，v0.3.0）：play 端点对 reqwest 直连 403（请求头无关，疑似 TLS 指纹风控；同 URL yt-dlp 可下）。降级链锁定为：原生 HTTP（桌面裸请求 → 移动 UA+Referer+分享页 Cookie，逐 play_addr.url_list 候选）→ **yt-dlp 直下已解析 play URL**（generic 提取器，无需 Cookie，保留解析标题）→ yt-dlp 分享页短链兜底；每次尝试的档位与状态写入 download.log
26. **更新后自动重启 + 已有任务重配保存形态**（2026-07-27，应用 **`0.3.1`**）：Windows 应用内静默安装调度「退出 → 安装 → 自动启动」；`update_job_media_save_mode` 允许 download/live 任务在非运行态切换 video|audio，已有产物时清空 `media/` 并失效下游，不自动重跑

---

## 12. 文档维护

- 实现阶段若变更已锁定决策：**先更新本节与对应章节，再改代码**。  
- 转写引擎锁定为 whisper.cpp `whisper-cli`；配置字段为 `sidecar_paths.transcribe`、`transcribe_model`、`transcribe_language`（及 v0.2 档位预设）。  
- **v0.2 能力说明以第 14 节为准**（已交付 MVP）；开 task 时核对各节「非目标 / 后置」，避免把后置项当本期交付。  
- 发版时同步更新：`package.json`、`src-tauri/Cargo.toml`、`src-tauri/tauri.conf.json` 的 `version` 与本文版本声明。

---

## 13. v0.1 完成状态（历史基线）

- [x] 产品规格文档  
- [x] Tauri 2 工程骨架（任务中心 + Job 落盘 + 配置/sidecar 探测）  
- [x] 切换为 pnpm  
- [x] 下载执行器（yt-dlp）
- [x] 直播分段录制
- [x] 本地转写 + 合并文字
- [x] 双协议总结 + 模板
- [x] 流水线调度 / 重试 / 导出 / 托盘
- [x] 任务自定义分组（设置中管理分组目录 + 列表按分组筛选）

实现验证（历史记录）：Rust 单元测试、严格 Clippy、格式检查、TypeScript 类型检查与前端生产构建曾通过；曾生成 Tauri release 与 MSI。自动检查不替代真实环境验收。

**v0.1 基线之上的增量见第 14 节（现为 v0.2 已交付能力）。**

---

## 14. v0.2 能力说明（原路线图 · 已交付 MVP）

> **交付状态（2026-07-21）**：P0–P4 MVP **均已实现**；应用版本 **`0.2.0`**。  
> 安装分发体验为 MVP 深度：探测指引、模型扫描、配置迁移、检查更新（非静默系统安装 / 非自动下模型 / 非签名静默更新）。  
> 原则：仍以统一 Job 为中心；小步可逆；密钥与 Cookie 永不进入任务导出包；平台相关能力保持「最佳努力」。  
> 实现顺序（已完成）：**地基 → 吞吐获取 → 文字质量 → 知识产出 → 安装分发**。

### 14.1 分期总览

| 分期 | 主题 | 包含能力 |
|------|------|----------|
| **P0 地基** | 可扩展与可恢复 | 前端结构拆分；工作区健康检查；全局并发与队列 |
| **P1 吞吐与获取** | 多任务与能下 | 批量 URL；Cookie / 浏览器登录态辅助下载；失败处继续向导 |
| **P2 文字质量** | 转写可控与长内容结构 | 术语表 / 强制词表；转写质量工具；章节 / 大纲自动切分 |
| **P3 知识产出** | 多形态总结与可检索 | 多模板一次多份产出；跨 Job 全文检索 |
| **P4 安装与分发体验** | 小范围分发预留 | 依赖安装向导；模型管理；配置导入导出；应用检查更新 |

推荐 backlog 序号（开 task 时可直接引用）：

1. 前端拆分  
2. 全局并发队列 + 排队 UI  
3. 工作区健康检查  
4. 批量 URL + 可选 `batch_id`  
5. Cookie / cookies-from-browser  
6. 稳定 `error_code` + 失败恢复向导  
7. 术语表 + 转写质量（语言 / 热词 / 单段 diff）  
8. 章节 / 大纲步骤（`Chapterize`）  
9. 多模板多产物  
10. 跨 Job 全文检索  
11. 依赖安装向导 + 模型管理  
12. 配置导入导出  
13. 检查更新  

### 14.2 P0 — 地基

> **实现状态（2026-07-21）**：P0 三项已落地（见 task `07-21-v02-p0-foundation`）。自动化：Rust 单元测试 / Clippy / fmt、`pnpm typecheck` / `pnpm build` 已通过；真实多任务排队与 sidecar 环境仍需本机验收。

#### 14.2.1 前端结构拆分

| 项 | 决策 |
|----|------|
| 动机 | 任务中心单文件过大，后续队列、向导、批量创建、设置诊断难以安全改动 |
| MVP | 按界面边界拆为任务列表、任务详情、创建对话框、设置分区与共享 hooks；**不改变 IPC 语义与业务行为** |
| 非目标 | 借拆分重做信息架构或换 UI 框架 |

- [x] 抽出 `labels.ts` / `constants.ts` / `jobUtils.ts` / `components/PathPickerField.tsx`；`App.tsx` 仍负责编排与主界面合成（后续可继续按面板拆文件）

#### 14.2.2 工作区健康检查

| 项 | 决策 |
|----|------|
| 扫描 | 启动或设置「诊断」：孤儿目录、损坏的 `source.json`、持久化为 `running` 但无活跃 runner 的任务 |
| 修复 | 标记 interrupted/失败并给出重试指引；必要时从 `media/` 重建 segment 索引（复用已有重建逻辑） |
| 磁盘 | 展示工作区所在卷剩余空间与阈值提示（与直播磁盘保护同一心智） |
| 非目标 | 自动删除用户媒体；云端修复 |

- [x] IPC：`inspect_workspace_health` / `repair_workspace_health`
- [x] 设置页「工作区诊断」分区：扫描摘要 + 修复可安全项（中断 running、残留 queued、空媒体索引重建）
- [x] 启动恢复：`running`→失败；`queued`→pending（内存队列不跨进程）

#### 14.2.3 全局并发与队列

| 项 | 决策 |
|----|------|
| 现状约束 | 现有 runner 仅防止**同一 Job 重入**，不限制全局并发 |
| 配置 | 至少 `max_concurrent_jobs`（默认宜偏保守，如 1～2）；可选分池：`max_download` / `max_transcribe`；直播可单独 `max_live_records`（建议默认 1） |
| 调度 | 创建或触发运行时入队；有空位再开始执行；FIFO 即可 |
| 状态 | 建议显式 `Queued`（或等价可序列化状态）+ 可选队列位置展示 |
| UI | 列表可见「排队中 / 第 N 位」；不要求本期做手动插队 |
| 非目标 | 复杂优先级策略、跨机器调度 |

- [x] 配置：`max_concurrent_jobs`（默认 2）、`max_live_records`（默认 1）；旧配置 Serde 默认兼容
- [x] 调度：创建 / run / 重试 / 分段重试统一入队；FIFO + 结束时 pump
- [x] 状态：`JobStatus::Queued`；列表 `queue_position`（1-based）

### 14.3 P1 — 吞吐与获取

#### 14.3.1 批量 URL

| 项 | 决策 |
|----|------|
| 入口 | 创建流支持多行粘贴：一行一个 URL（或等价批量输入） |
| 任务模型 | **每个 URL 一个独立 Job**；共享分组、流水线勾选、转写语言等创建参数 |
| 批次关联 | 可选 `batch_id`（创建时生成 UUID），便于列表筛选「本批」；不做完整 Batch 实体 / 父子 Job |
| 执行 | 仅创建并入队；由全局队列执行，禁止无限制同时开跑 |
| 失败 | 单 Job 失败不影响同批其它 Job |
| 非目标 | 播放列表深度解析、父子编排、批内依赖图（后置） |

- [x] IPC：`create_download_jobs_batch`（服务端拆行；单条/单短链分享仍为 1 个 Job 且 `batch_id=null`；≥2 条共享 `batch_id`）
- [x] Job / 列表字段：`batch_id`（旧 `source.json` 缺省兼容）
- [x] UI：下载创建支持多行；列表批次筛选芯片 + 搜索/卡片展示 batch 前缀

#### 14.3.2 Cookie / 浏览器登录态辅助下载

| 项 | 决策 |
|----|------|
| 定位 | 提高会员/登录态内容下载成功率；**不改变**「宽入口、不承诺平台」策略 |
| MVP 方式 | （1）用户提供 Netscape `cookies.txt` 路径；（2）yt-dlp `--cookies-from-browser`（如 chrome / edge / firefox） |
| 配置 | 全局默认 + 任务级可选覆盖；`source.json` 只记「使用了 cookie 文件或浏览器源」类元数据，**不写入 Cookie 内容** |
| 安全 | 任务导出包、公开配置响应、日志均不得包含 Cookie 原文；日志继续脱敏 |
| 范围 | MVP 优先绑定 **yt-dlp 下载路径**；其它解析路径（如站点专用 resolver）是否共用同一认证源可后置，默认不自动扩展 |
| 非目标 | 内嵌登录 WebView、自动扫描浏览器配置目录、平台官方 OAuth 集成 |

- [x] 配置：`download_cookies_file` / `download_cookies_from_browser`（Serde 默认兼容旧配置）
- [x] Job：`source.download_cookies_mode`（inherit/none/file/browser）+ 路径/浏览器元数据
- [x] yt-dlp：注入 `--cookies` 或 `--cookies-from-browser`；日志只写路径/浏览器名
- [x] UI：设置默认 Cookie；新建下载可覆盖

#### 14.3.3 从失败处继续向导

| 项 | 决策 |
|----|------|
| 触发 | 任务失败时在详情展示「修复建议」卡片，而非仅堆栈式错误 |
| 后端 | 逐步引入稳定 **`error_code`**（示例：`SIDECAR_MISSING`、`AUTH_REQUIRED`、`CONTEXT_TOO_LONG`、`DISK_GUARD`、`NETWORK` 等），错误文案仍可读 |
| 前端 | 按 `current_step` + `error_code`（及必要上下文）映射一键动作：重试本步、打开日志/目录、检查 sidecar、缩小选段、切换 Provider/模型、引导补充 Cookie 等 |
| 非目标 | 静默全自动修复、机器学习决策树 |

- [x] Job / 列表：`error_code`（旧 `source.json` 缺省兼容）；失败落盘时分类写入
- [x] 分类器：启发式关键词 + `current_step`（`SIDECAR_MISSING` / `AUTH_REQUIRED` / `CONTEXT_TOO_LONG` / `DISK_GUARD` / `NETWORK` / 步骤级失败码等）
- [x] UI：任务详情「修复建议」卡片 + 一键动作（重试本步、日志/目录、设置、分段、Provider）

### 14.4 P2 — 文字质量

#### 14.4.1 术语表 / 强制词表

| 项 | 决策 |
|----|------|
| 档案 | 全局术语表（可多档案后置；MVP 一份全局 + 任务覆盖即可） |
| 形态 | 热词列表，和/或 `from → to` 替换对 |
| 作用点 | （1）转写时作为 whisper 初始/附加 prompt（在 sidecar 能力范围内）；（2）合并文本后可选整词替换后处理 |
| 落盘 | Job 记录所用术语表标识或快照哈希（无敏感）；替换规则本身可进配置而非每个 Job 复制全文（任务覆盖时需可复现） |
| 非目标 | 在线词典、复杂 NLP 分词、跨语言对齐 |

#### 14.4.2 转写质量工具

| 项 | 决策 |
|----|------|
| 语言 | 强化任务级 `transcribe_language` 与失败提示（配置字段已存在则补齐 UX） |
| 热词 | 与术语表打通 |
| 单段对比 | 单段重跑后可查看与上一版文本差异（保留上一版或可 diff 即可） |
| 模型档位 | 设置中提供速度 / 平衡 / 质量等预设映射到模型路径（具体文件由用户本机提供） |
| 后置 | 说话人 diarization、自动下载模型（见 P4 模型管理） |

#### 14.4.3 章节 / 大纲自动切分

| 项 | 决策 |
|----|------|
| 流水线位置 | 合并 transcript 之后、总结之前；建议独立步骤 **`Chapterize`**，支持单步重试与可选跳过 |
| 输入 | `transcript/plain.txt`，以及可用时的时间轴（`raw.json` / srt） |
| 输出 | 至少 `transcript/chapters.json`；可选人类可读 `chapters.md`（标题、时间范围、短摘要） |
| 算法 MVP | 启发式（段边界 / 静音近似）和/或一次 LLM 仅生成大纲结构（短上下文）；实现择一或组合，但产物 schema 需稳定 |
| 与总结 | 模板可增加变量如 `{{chapters}}`；**不**在本期强制 map-reduce 全文总结；超长仍可失败 + 选段缩小（与 v0.1 一致） |
| 非目标 | 写入视频容器章节轨、剪辑级切条导出 |

- [x] 配置：全局 `glossary`（热词 + `from→to`）、`apply_as_whisper_prompt` / `apply_post_replace`；转写模型档位 `speed|balanced|quality|custom` + 路径预设
- [x] 转写：whisper `--prompt` 注入热词；合并后可选整词替换；Job 记录 `glossary_hash`
- [x] 单段对比：重试前复制 `.prev.txt`；详情「对比上一版」
- [x] `JobStep::Chapterize`：`transcript/chapters.json` + `chapters.md`（SRT 间隙 / 段落启发式）；自动总结前可选执行
- [x] 模板变量 `{{chapters}}`；UI 设置术语表/档位/章节开关；流水线展示章节步骤

### 14.5 P3 — 知识产出与检索

#### 14.5.1 多模板一次跑多份产出

| 项 | 决策 |
|----|------|
| 选择 | 全局默认与任务级支持**多个**模板 ID（有序列表） |
| 落盘 | 保留 `summary/summary.md` 作为「主模板」兼容路径（列表第一项或显式 primary）；其余写入 `summary/by_template/<template_id>.md`；`summary/meta.json` 记录实际使用的模板列表与模型等（仍无 Key） |
| 运行 | `Summarize` 步按序执行；单模板失败应可报告部分失败，且不删除其它已成功产物 |
| 兼容 | 仅一个模板时行为与 v0.1 一致 |
| 非目标 | 强制固定 JSON schema；模板之间互相依赖或流水线式引用 |

#### 14.5.2 跨 Job 全文检索

| 项 | 决策 |
|----|------|
| 索引内容 | 至少 `transcript/plain.txt` 与 `summary/summary.md`；多模板产物稳定后纳入 `summary/by_template/*` |
| 实现倾向 | 本地索引（如 SQLite FTS 或等价）；Job 产物变更后增量更新；索引可重建 |
| UI | 任务中心顶栏（或等价入口）搜索；结果展示任务标识、片段高亮，跳转详情对应区域 |
| 隐私 | 仅本地；索引目录建议在工作区下独立路径（如 `workspace/index/`），不进入含密钥的配置区 |
| 非目标 | 云端索引、向量语义搜索（后置） |

**P3 实现勾选（2026-07-21）**

- [x] `pipeline.template_ids` 有序列表 + 兼容 `template_id`；创建/更新流水线支持多模板
- [x] `summarize` 按序调用；主产物 `summary/summary.md`，其余 `summary/by_template/<id>.md`；`meta.json` 记录 `template_ids` / 部分失败
- [x] 单模板失败不删已成功产物；全部失败才标记步骤失败
- [x] SQLite FTS5：`workspace/index/search.sqlite3`；索引 plain + summary + by_template
- [x] 增量：`persist` 后 upsert；删除 Job 时 remove；`rebuild_search_index` IPC
- [x] UI：多模板勾选、多产物切换、任务列表全文检索与结果跳转

### 14.6 P4 — 安装与分发体验

| 能力 | MVP | 非目标 / 后置 |
|------|-----|----------------|
| 依赖安装向导 | 探测 ffmpeg / ffprobe / yt-dlp / streamlink / whisper-cli；缺失时给出明确指引与打开设置/文档入口 | 不强制静默系统级安装（权限与杀软环境差异大） |
| 模型管理 | 扫描配置的模型目录、展示当前选用、校验文件存在、打开目录 | 自动下载大体积 GGML、多镜像源可作为二期 |
| 配置导入导出 | 导出/导入 Provider、模板、分组、流水线默认等；**默认剥离 API Key** | 含 Key 的加密导出可后置 |
| 应用更新 | 「检查更新」：比较版本、展示说明、打开发布页下载 | 完备签名静默更新与强制升级 |
| 与 P0 健康检查 | 设置中「诊断」可聚合 sidecar、磁盘、工作区、版本信息 | — |

**P4 实现勾选（2026-07-21）**

- [x] 依赖向导：`get_dependency_report` + 设置「Sidecar」页指引（必需缺失汇总）
- [x] 模型管理：`list_transcribe_models` / 打开目录；扫描 GGML/GGUF；校验当前选用
- [x] 配置导出/导入：默认剥离 Key；导入保留本机同 ID Key；任务运行中禁止导入
- [x] 检查更新：`check_app_update`（默认 GitHub `627157746/video-tool` Releases；可覆盖 `VIDEO_TOOL_RELEASE_API` / `VIDEO_TOOL_RELEASE_PAGE`，私有仓可用 `VIDEO_TOOL_GITHUB_TOKEN`）
- [x] 应用内更新：`install_app_update`（用户确认后下载安装包并静默安装：NSIS `/S`、MSI `/qn`；事件 `app-update-progress`）
- [x] 系统诊断：`get_system_diagnostics` 聚合版本 / sidecar / 模型 / 磁盘 / 工作区
- [x] UI：设置分区 models / backup；diagnostics 完整诊断

### 14.7 架构挂钩（实现约束）

| 能力 | 挂载约定 |
|------|----------|
| 全局队列 | 扩展 runner / 调度层；创建与 `run` 路径统一入队；保持单 Job 互斥 |
| 批量 URL | 后端批量创建 IPC 或严格串行创建接口；禁止前端无控并发狂轰 IPC 而无队列 |
| Cookie | 下载执行器向 yt-dlp 追加参数；配置与 Job 元数据字段需文档化并做脱敏 |
| 术语表与转写质量 | 配置 + transcribe / merge 路径；替换规则可测试 |
| 章节 | 新 `JobStep`（推荐 `Chapterize`）或等价可重试步骤；产物路径写入目录契约 |
| 多模板 | summarize 循环与 `summary/` 子路径契约；export 包含新产物、仍不含密钥 |
| 全文检索 | 独立 search/index 模块；不把索引当唯一数据源（源文件仍在 Job 目录） |
| 失败向导 | 错误码在 Rust 边界稳定输出；前端映射表可版本化 |
| 目录契约扩展 | 新增路径必须更新本节与实现，三条业务线禁止私自发明不兼容目录 |

建议在实现对应能力时同步扩展第 5 节目录约定（如 `chapters.*`、`summary/by_template/`、`workspace/index/`），避免只写代码不写契约。

### 14.8 依赖关系

```text
前端拆分 ──────────────────────────────────────────┐
工作区健康 ──┐                                     │
全局队列 ────┼─→ 批量 URL                          │
             │                                     ├─→ 失败向导（error_code 越稳越好）
Cookie 下载 ─┘                                     │
术语表 ──→ 转写质量工具 ──→ 章节/大纲 ─────────────┤
多模板多产物 ──→ 跨 Job 全文检索 ──────────────────┘
依赖向导 / 模型管理 / 配置迁移 / 检查更新（可与 P1–P3 并行）
```

### 14.9 验收与文档纪律

- 第 14 节 MVP 能力**已交付**（勾选列表为实现记录）；新增后置能力时：先改本节与第 11 节决策日志，再改代码，并补测试与脱敏审查。  
- 自动化测试仍不替代 sidecar / 网络 / API Key / Cookie 有效性等环境验收。  
- **v0.2 发版检查**：`package.json`、`Cargo.toml`、`tauri.conf.json` 版本一致为 `0.2.1`；用户可见「检查更新」比较该版本。

---

## 15. v0.2 完成状态

- [x] **P0** 地基：工作区健康、全局队列 / `Queued`、前端辅助拆分  
- [x] **P1** 吞吐：批量 URL + `batch_id`、Cookie 辅助下载、`error_code` 失败向导  
- [x] **P2** 文字：术语表、转写质量工具、`Chapterize`、模型档位  
- [x] **P3** 知识：多模板多产物、跨 Job SQLite FTS  
- [x] **P4** 安装分发 MVP：依赖向导、模型扫描、配置导入导出、检查更新、系统诊断  
- [x] 应用版本号 **`0.2.0`**

验证（发版前自动化）：Rust lib 测试、Clippy、`fmt`、TypeScript `tsc --noEmit` 以当前仓库命令为准；不替代本机 sidecar / 模型 / 网络验收。

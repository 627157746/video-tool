# PRD: v0.3 使用体验增强（校对/预览/通知/容量）

> 状态：规划中（brainstorm 进行中）
> 父任务：本任务；子任务见下表

## 目标

在 v0.2.5 已交付能力之上，补齐四个日常使用断点：

| 子任务 | 目录 | 一句话 |
|--------|------|--------|
| 转写文本校对/编辑 | `07-26-transcript-edit` | 在任务详情内直接编辑转写文本，保存后使下游产物失效可重跑 |
| 媒体播放与字幕联动预览 | `07-26-media-preview` | 应用内播放媒体，点字幕行跳转对应时间点，核对转写质量 |
| 任务完成系统通知 | `07-26-job-notifications` | 长耗时任务结束（成功/失败）时发 Windows 系统通知 |
| 工作区容量治理 | `07-26-workspace-capacity` | 展示占用、按体积排序、可"清理媒体保留文字"归档 |

## 代码库确认事实（2026-07-26 探查）

- 产物目前只读：IPC 以字符串返回产物内容，无任何写入产物的 IPC。
- 总结/下游失效机制已存在：`Job::invalidate_after_step`（`src-tauri/src/commands/mod.rs:735`，MergeTranscript 后失效即触发总结需重跑）。
- 前端无任何 `<video>/<audio>`；`src-tauri/tauri.conf.json` 未启用 assetProtocol；媒体文件当前不能被 WebView 读取。
- SRT 解析仅存在于 Rust：`parse_srt_cues`（`src-tauri/src/pipeline/chapterize.rs:229`）；前端无 SRT 工具。
- 无 tauri-plugin-notification 依赖，capabilities 中无通知权限；Rust 侧在 Job 持久化时 emit `job-updated` 事件，前端已监听。
- 磁盘保护仅覆盖直播录制（阈值停录）；工作区健康检查（`inspect_workspace_health`）不含体积统计；`delete_job` 删除整个 Job 目录并同步移除搜索索引条目。
- `src/App.tsx` 共 6388 行；任务详情面板约 3407–4080 行；产物查看已有 tab 化区域。
- 目录契约（`docs/PRODUCT_SPEC.md` §5.2）：`transcript/plain.txt` 为总结主输入；`transcript/segments/`、`srt.srt`、`summary/`、`media/` 均有固定路径。

## 跨子任务约束

- 遵循 `docs/PRODUCT_SPEC.md` §10.2：先改规格再改代码；新增目录/字段需回写 §5.2 契约。
- 遵循统一 Job 模型；不引入新的顶层实体。
- 密钥/Cookie 脱敏纪律不变；新增 IPC 需登记 `.trellis/spec/tauri-ipc/`。
- `App.tsx` 已过大：四个子任务新增 UI 原则上以独立组件文件实现，不再向 `App.tsx` 堆叠大段 JSX。

## 已锁定决策（2026-07-26 brainstorm）

| 子任务 | 决策 |
|--------|------|
| 转写校对 | 按字幕 cue 编辑，同步回写 `srt.srt` + `plain.txt`；无 srt 时降级整篇编辑；重合并前覆盖警告 |
| 媒体预览 | 不兼容容器（.ts/.mkv 等）一键 ffmpeg `-c copy` 转封装 `media/preview.mp4` |
| 系统通知 | 仅 Job 终态通知（成功/失败），前台聚焦不弹，设置可关（默认开） |
| 容量治理 | 纯手动逐 Job 清理 `media/`（保留文字资产）+ 占用统计/排序，无自动策略 |

## 实施顺序（建议）

1. `07-26-job-notifications`（最小、无依赖）
2. `07-26-workspace-capacity`（独立；清理语义覆盖未来 preview.mp4）
3. `07-26-transcript-edit`（校对回写 srt，是预览字幕联动的上游）
4. `07-26-media-preview`（消费校对后 srt；与校对视图打通跳转）

依赖说明已分别写入各子任务 PRD 的「依赖与顺序」节；父子结构不隐含依赖。

## 验收（父任务层面）

- 四个子任务分别通过各自 PRD 验收标准并归档。
- `docs/PRODUCT_SPEC.md` 增补对应章节与决策日志条目，版本号按发版纪律更新。
- Rust fmt/clippy/test 与 `pnpm typecheck`/`pnpm build` 通过。

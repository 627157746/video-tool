# PRD: 转写文本校对/编辑

> 父任务：`07-26-v03-usability`
> 状态：规划中（PRD 已定稿，待 design/implement）

## 目标

在任务详情内按字幕 cue 粒度校对转写文本，保存后同步回写 `transcript/srt.srt` 与 `transcript/plain.txt`，并使下游产物（章节、总结）失效可重跑，让"预览看到的字幕"与"总结吃到的全文"始终一致。

## 决策（2026-07-26 brainstorm）

- 编辑粒度：**按字幕 cue 编辑，同步回写 `srt.srt` + `plain.txt`**（方案 C；否决了仅编辑 plain.txt 与按 segment 编辑）。
- 只编辑文本，不编辑时间轴；不拆分/合并 cue。

## 代码库确认事实

- `srt.srt` 是**条件产物**：仅当至少一个 segment 有 `.srt` 时生成；合并结果为空时会删除已有 `srt.srt`（`src-tauri/src/pipeline/transcribe.rs:285-292`）。
- `plain.txt` 由各 segment 的 `.txt` 拼接（`\n\n` 连接）+ 术语表整词替换生成（`transcribe.rs:230-284`），**不是**从 SRT 派生。
- whisper 调用始终请求 `-osrt`，但缺失 `.srt` 被容忍（`transcribe.rs:114-123, 188-190`）。
- 重新合并会**无条件重建** `plain.txt` / `raw.json`，并覆盖或删除 `srt.srt`（`transcribe.rs:284-297`）；选段变更还会预删下游产物（`paths.rs:140-143`）。→ 手工编辑会被冲掉，必须加保护。
- 失效机制已存在：`Job::invalidate_after_step(JobStep::MergeTranscript)`（`src-tauri/src/commands/mod.rs:735`）。
- 单段重试已有 `.prev.txt` 备份先例（`transcribe.rs:100-104` 附近），备份心智可复用。
- 产物目前只读：无任何写产物的 IPC；需新增写入命令并登记 `.trellis/spec/tauri-ipc/`。

## 需求

- R1 校对视图：任务详情新增"校对"区（独立组件文件，不堆入 `App.tsx`）：按 cue 列表展示序号、时间范围、可编辑文本；支持仅文本修改。
- R2 保存 IPC：新增命令（如 `save_transcript_edit`）：
  - 原子回写 `srt.srt`（cue 文本替换，时间轴不变）与 `plain.txt`（由校对后 cue 文本重建，连接规则在 design.md 固化）；
  - 保存前备份上一版为 `srt.prev.srt` / `plain.prev.txt`（各保留一份，覆盖式）；
  - 保存时**不**重复应用术语表替换（以用户文本为准）；
  - 运行中的 Job（转写/合并/总结进行中）拒绝保存。
- R3 失效联动：保存成功后调用 `invalidate_after_step(MergeTranscript)`，UI 明确提示"章节/总结已失效，需重跑"。
- R4 编辑标记：Job 元数据记录 `transcript_edited_at`（`source.json`，无敏感内容）；详情展示"已手工校对"。
- R5 覆盖保护：已编辑 Job 再触发单段重试、重新合并、选段变更时，弹确认警告"将覆盖手工校对结果"，确认后才执行并清除编辑标记。
- R6 无字幕降级：`srt.srt` 不存在的 Job 降级为整篇 `plain.txt` 文本编辑，同样走备份 + 失效 + 标记链路。

## 验收标准

- 编辑若干 cue → 保存：`srt.srt` 与 `plain.txt` 同步更新、备份文件存在、章节/总结标记失效；重跑总结使用校对后全文。
- 已编辑 Job 触发单段重试/选段变更时出现覆盖警告；取消则产物不变。
- 无 `srt.srt` 的 Job 可整篇编辑并正常失效重跑。
- 搜索索引在保存后与新 `plain.txt` 一致（复用 persist 后 upsert 链路）。
- Rust fmt/clippy/test、`pnpm typecheck`/`pnpm build` 通过。

## 非目标

- 时间轴编辑、cue 拆分/合并、说话人标注。
- 回写 `transcript/segments/*.txt`（保持原始产物；以合并层为准，known 不一致记录在文档）。
- 多版本历史（仅保留一份 prev 备份）。

## 依赖与顺序

- 建议在 `07-26-media-preview` 之前实施：预览的字幕联动直接读取校对后的 `srt.srt`。

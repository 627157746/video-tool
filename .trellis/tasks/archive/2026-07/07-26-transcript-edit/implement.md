# 实现记录：转写文本校对/编辑

> 状态：已实现并通过自动化验证（2026-07-26）

## 改动清单

- `src-tauri/src/pipeline/transcript_edit.rs`（新模块）：SRT 解析/序列化（时间轴逐字保留、空文本删行）、`save_cue_edits`（备份 srt.prev.srt/plain.prev.txt → 回写 srt.srt + 重建 plain.txt，cue 文本 `\n` 连接）、`save_plain_edit`（无 SRT 降级整篇编辑）；4 个单元测试
- `src-tauri/src/models/job.rs`：`Job.transcript_edited_at`（Serde default）
- `src-tauri/src/commands/mod.rs`：`get_transcript_cues` / `save_transcript_edit`（运行/排队拒绝；保存后 `invalidate_after_step(MergeTranscript)` + 删除章节/总结产物 + 搜索索引 upsert + emit job-updated）
- `src-tauri/src/pipeline/runner.rs`：MergeTranscript 成功、分段重试时清除编辑标记
- `src-tauri/src/commands/mod.rs` `select_job_segments`：选段变更清除编辑标记
- `src/components/TranscriptProofreadPanel.tsx`：cue 列表编辑（仅提交改动行）、整篇降级、脏行高亮、"已手工校对"徽标
- `src/App.tsx`：详情「校对」分区；重跑合并/转写/分段重试/选段变更前 `window.confirm` 覆盖警告

## 验收对照

- cue 编辑 → srt.srt + plain.txt 同步、备份存在、章节/总结失效 ✅（单元测试覆盖文件层）
- 覆盖警告（重跑/分段重试/选段）✅；无 SRT 整篇编辑 ✅
- 搜索索引保存后 upsert ✅

## Known 限制

- 不回写 `transcript/segments/*.txt`；`raw.json` 保持原样（与 srt/plain 存在已知不一致，重合并时重建）
- plain.txt 重建后 cue 文本以单个换行连接（原为段落 `\n\n`；文字内容一致）
- 校对界面与预览播放器的跨分区跳转未实现（预览分区自身有 cue 点击跳转）

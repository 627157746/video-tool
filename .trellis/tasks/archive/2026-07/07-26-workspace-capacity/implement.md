# 实现记录：工作区容量治理

> 状态：已实现并通过自动化验证（2026-07-26）

## 改动清单

- `src-tauri/src/models/job.rs`：`Job.media_purged_at: Option<DateTime>`（Serde default 兼容旧 source.json）
- `src-tauri/src/workspace/mod.rs`：`compute_workspace_usage`（总占用/媒体占用/剩余空间/按媒体体积降序 Job 列表，含 index/ 目录）、`purge_job_media`（清空 media/ 内容，保留文字资产，写入标记）
- `src-tauri/src/commands/mod.rs`：`get_workspace_usage` / `purge_job_media`（运行/排队中拒绝）
- `src-tauri/src/pipeline/runner.rs`：媒体已清理时拒绝 `Some(Transcribe)` 重跑与分段重试；Ingest 成功后清除 `media_purged_at`
- `src/components/CapacityPanel.tsx`：设置「容量治理」分区（统计、排序列表、二次确认清理、跳转详情）
- `src/constants.ts` / `src/types.ts` / `src/api.ts` / `src/App.tsx`：分区注册、类型镜像、IPC 封装、渲染接入

## 验收对照

- 占用统计/排序/剩余空间 ✅（自动化验证覆盖编译层；数值准确性需真机抽查）
- 清理保留文字资产、运行中拒绝、purged 后转写重试禁用（后端守卫）✅
- 下载类重跑 ingest 后清除标记 ✅（runner Ok(Ingest) 分支）
- 搜索索引不受影响（未触碰索引）✅

## Known 限制

- 任务列表卡片无"已清理"徽标（容量面板与预览分区有提示）；批量多选清理未做（PRD 非目标）
- 占用统计为同步遍历；超大工作区首次统计可能秒级耗时

# v0.2 P0 foundation: frontend split, global queue, workspace health

## Goal

Deliver **v0.2 P0 地基** from `docs/PRODUCT_SPEC.md` §14.2 so later P1+ features (batch URL, failure wizard, multi-template) can land safely:

1. Frontend structure split (no IPC/behavior change as the primary goal of the extract)
2. Global concurrency queue + queue UI
3. Workspace health check + lightweight repair

## Requirements

### R1 — Frontend structure split (14.2.1)

- Split the oversized task-center UI by interface boundaries:
  - job list
  - job detail
  - create-job dialog
  - settings sections
  - shared labels/helpers/hooks as needed
- **Do not** change product information architecture or replace the UI stack.
- Preserve request-version guards, event merge, and busy/disabled behavior.

### R2 — Global concurrency queue (14.2.3)

- Today the runner only prevents **same-Job** re-entry; concurrent Jobs are unlimited.
- Add config (with Serde defaults for older `config.json`):
  - `max_concurrent_jobs` (default **2**, min 1)
  - `max_live_records` (default **1**, min 1; live records also count against global concurrency)
- Create / run / retry / segment-retry all enter a **FIFO** queue when no free slot.
- Persist an explicit serializable status **`queued`** while waiting.
- List/detail show “排队中” and optional **1-based queue position**.
- On app restart, jobs left as `queued` become `pending` (in-memory queue is gone); do not leave permanent “排队中”.
- Non-goals: manual reordering, priority classes, multi-machine scheduling.

### R3 — Workspace health check (14.2.2)

- Provide a diagnostic command used at settings “诊断” (and optionally after load):
  - orphan job directories (non-UUID / missing `source.json`)
  - corrupted `source.json`
  - jobs persisted as `running` while no active runner (repair path; reuse recovery semantics)
  - free disk space on the workspace volume vs `min_free_disk_gb`
- Repair actions (explicit user trigger where destructive):
  - mark interrupted running jobs failed with retry guidance
  - rebuild media segment index from `media/` when media exists but index empty (reuse `rebuild_media_segments_from_files`)
- Non-goals: auto-delete user media; cloud repair.

## Acceptance Criteria

- [x] Shared labels/constants/utils/`PathPickerField` extracted; `App.tsx` remains shell/orchestrator (further panel files optional follow-up).
- [x] Global FIFO queue with `JobStatus::Queued` + `queue_position`; create/run/retry paths admit or queue.
- [x] Live recording uses `max_live_records` (default 1) via live slot holders.
- [x] Settings edits `max_concurrent_jobs` / `max_live_records`; Serde defaults for older configs.
- [x] Settings「工作区诊断」：scan + repair (interrupted/queued/empty media index); no auto-delete of media.
- [x] Secrets not added to health reports / public config.
- [x] `cargo test` (46), Clippy `-D warnings`, fmt, `pnpm typecheck`, `pnpm build` pass.

## Notes

- Source of truth: `docs/PRODUCT_SPEC.md` §14.2 / §14.7–14.9.
- Package manager remains **pnpm**.
- Follow `.trellis/spec/{frontend,backend,tauri-ipc}/` contracts.

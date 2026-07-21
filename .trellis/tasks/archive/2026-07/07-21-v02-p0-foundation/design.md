# Design — v0.2 P0 foundation

## 1. Frontend split

### Target layout

```text
src/
  App.tsx                      # shell: nav, toast, data orchestration, composition
  api.ts / types.ts / theme.ts # unchanged ownership
  labels.ts                    # KIND/STATUS/STEP label maps
  jobUtils.ts                  # list merge, group resolve, markdown fence, provider models
  constants.ts                 # section defs, log names, language options
  components/
    PathPickerField.tsx
    JobListPanel.tsx
    JobDetailPanel.tsx
    CreateJobDialog.tsx
    SettingsView.tsx
```

### Rules

- Props-down / callbacks-up; no new global store.
- Keep CSS in `App.css` for this pass (no CSS module migration).
- Extraction must preserve request version refs and `job-updated` merge.

## 2. Global queue

### Config

| Field | Default | Validation |
|-------|---------|------------|
| `max_concurrent_jobs` | 2 | 1..=64 |
| `max_live_records` | 1 | 1..=16 |

Both on `AppConfig` / `AppConfigPublic` / `SaveConfigRequest` with Serde defaults.

### Runtime (`RunnerState`)

```text
running_job_ids: HashSet
live_recording_ids: HashSet
queue: VecDeque<QueuedWork>
  - job_id
  - work: FullRun { step: Option<JobStep> } | SegmentRetry { segment_id }
  - is_live_record: bool   # for live slot accounting at start time
```

### Admission

1. Reject if `job_id` already running or already queued.
2. If free global slot (and free live slot when work is live ingest full-run): `try_begin` + spawn.
3. Else: set `Job.status = Queued`, clear error, persist, emit, push FIFO.

### Completion pump

On every `runner.end(job_id)`:

1. Remove from running/live.
2. While queue non-empty and slots free: pop front, re-check job still exists, start.

Config for the next start is reloaded via `AppConfig::load_or_init()` (disk = last saved) to avoid `commands` ↔ `pipeline` cycles. If load fails, use the previous snapshot passed into the finishing thread when available.

### Status model

- Add `JobStatus::Queued` (`"queued"`).
- `Job::derived_status` does **not** invent Queued; only the scheduler sets it.
- Startup `recover_interrupted_jobs`:
  - `Running` → Failed (existing)
  - `Queued` → Pending (queue memory lost)

### List UI fields

- `JobListItem.queue_position: Option<u32>` (1-based; only when status is queued and present in memory queue).
- Full `Job` may mirror `queue_position` for detail header (optional, default null).

## 3. Workspace health

### Types

```rust
WorkspaceHealthReport {
  workspace_dir,
  free_disk_gb: Option<u64>,
  min_free_disk_gb,
  disk_below_threshold: bool,
  orphan_directories: Vec<HealthFinding>,
  corrupt_jobs: Vec<HealthFinding>,
  interrupted_running_jobs: Vec<HealthFinding>,
  empty_media_index_jobs: Vec<HealthFinding>,  // media files present, media_segments empty
  repaired: Vec<String>, // filled after repair
}

HealthFinding { job_id_or_name, path, message }
```

### Commands

| Command | Behavior |
|---------|----------|
| `inspect_workspace_health` | scan only |
| `repair_workspace_health` | recover interrupted + rebuild empty media indexes; **does not** delete orphans |

Orphan directories are reported with path for user action; auto-delete is out of scope.

### Disk

Reuse `pipeline::paths::free_disk_gb` against workspace root.

## 4. Cross-layer checklist

- Rust enum + TS union + STATUS_LABEL + CSS `status-queued` if needed
- `generate_handler!` + `api.ts` + settings form dirty/save
- Unit tests: queue admission, live limit, recover queued→pending, health scan fixtures

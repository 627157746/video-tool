# Design: Job custom groups

## Data model

```rust
// Job
#[serde(default)]
pub group: Option<String>,

// JobListItem
#[serde(default)]
pub group: Option<String>,
```

- Persist on `source.json` with the Job document.
- Old files omit the field → Serde default `None`.
- Normalization: trim; empty → `None`. No length hard-cap beyond reasonable UI (`maxLength` ~80).

## IPC

### `update_job_group`

```rust
UpdateJobGroupRequest { job_id: String, group: Option<String> }
```

- Reject when job is running (same as title).
- Emit `job-updated` with full Job.
- Empty/whitespace `group` clears.

### Create requests

Optional `group` on:

- `CreateDownloadJobRequest`
- `CreateLiveRecordJobRequest`
- `CreateImportJobRequest`

Applied after `Job::new` / before `create_job_directories`.

## Frontend

| Area | Behavior |
|------|----------|
| State | `groupFilter: "all" \| "ungrouped" \| string` (string = exact group name) |
| List filter | `groupFilter` ∩ `searchQuery` |
| Chips | 全部 + sorted distinct groups + 未分组（若存在未分组任务） |
| Card | optional pill with group name |
| Create form | optional text input `formGroup` |
| Detail overview | draft input + save (blur/Enter), mirror title save pattern |

## Compatibility

- No migration rewrite; `#[serde(default)]` only.
- Export package includes group as part of Job JSON (no secrets).

## Files

- `src-tauri/src/models/job.rs`
- `src-tauri/src/commands/mod.rs`
- `src-tauri/src/lib.rs`
- `src/types.ts`
- `src/api.ts`
- `src/App.tsx`
- `src/App.css`
- `docs/PRODUCT_SPEC.md` (history/search row note)

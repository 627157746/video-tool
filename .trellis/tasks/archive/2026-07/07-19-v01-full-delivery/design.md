# Design: v0.1 full delivery stabilization

## Context

The current worktree already contains the main v0.1 implementation. This design defines the contracts required to stabilize that implementation rather than introducing a second architecture.

The application remains a React frontend invoking a Tauri command layer. Rust owns configuration, job persistence, sidecar execution, pipeline state, artifact generation, and tray lifecycle.

## Module Boundaries

### Frontend

- `src/types.ts` owns serialized frontend representations of Rust command responses.
- `src/api.ts` is the only Tauri invocation boundary and owns JavaScript command argument naming.
- `src/App.tsx` owns task-center orchestration, selection state, event handling, settings drafts, and artifact presentation.
- `src/App.css` supplies the existing single-page visual system and accessibility-visible focus states.

### Backend

- `commands` validates IPC input, clones shared state needed by long operations, and delegates business behavior.
- `config` owns candidate validation, public redaction, and atomic config persistence.
- `workspace` owns validated job paths, atomic job persistence, listing, and startup recovery of interrupted jobs.
- `models` owns serialized requests, job state, step dependencies, status derivation, and invalidation rules.
- `pipeline::runner` owns one-run-per-job concurrency, ordered execution, progress persistence, and event emission.
- `pipeline::{download,record,transcribe,summarize,export}` own tool-specific execution and artifacts.
- `sidecar` owns bundled/configured/`PATH` resolution and version probing.
- `lib` resolves application resource paths, initializes shared state, registers commands, and implements tray/window behavior.

## Cross-Layer Contracts

### Tauri argument naming

Direct Tauri command parameters use JavaScript `camelCase`, for example `{ jobId }`. Fields inside a serialized `request` object continue to use the Rust/Serde `snake_case` schema. `src/api.ts` is the single owner of this distinction.

### Validated job paths

All externally supplied job IDs are parsed as UUIDs before path construction. `workspace::validated_job_dir` returns a directory beneath `<workspace>/jobs`; commands and export code do not join raw identifiers independently.

Job deletion reuses this boundary, verifies that the persisted job exists, and removes the complete validated job directory. The command holds the operation lock and rejects deletion while the runner owns that job. The frontend requires explicit confirmation because media, transcripts, summaries, and logs are permanently removed together.

### Job status derivation

The required step set comes from normalized pipeline options. Job status is derived from step state:

1. Any required step `running` makes the job `running`.
2. Otherwise, any required step `failed` makes the job `failed`.
3. All required steps `succeeded` or intentionally `skipped` makes the job `succeeded`.
4. Otherwise the job remains `pending`.

Executing a single step never assigns overall success directly.

### Dependency invalidation

- Ingest invalidates transcribe, merge, and summarize.
- Transcribe or segment retry invalidates merge and summarize.
- Segment selection invalidates merge and summarize without invalidating successful segment transcripts.
- Merge invalidates summarize before producing a new merged transcript.

Invalidation resets dependent step states and removes or clears references to stale generated artifacts. Source media and successful per-segment transcript files are retained unless the owning step explicitly replaces them.

## Live Recording Outcome

`record_live_segments` distinguishes setup failures from terminal recording outcomes. A successful process invocation returns media metadata plus one of:

- `EndedNormally`
- `StoppedByUser`
- `ReconnectExhausted { detail }`
- `DiskGuard { detail }`

The runner always indexes and persists usable media before mapping the terminal reason to a step state. `EndedNormally` and `StoppedByUser` may continue to downstream automatic steps. `ReconnectExhausted` and `DiskGuard` leave ingest failed and stop automatic execution while preserving media for inspection or retry.

The runner marks a job as actively recording before expensive sidecar probing and clears that marker through a guard on every exit path. Only active live ingest accepts a stop request or causes window close-to-hide behavior.

## Transcription and SRT Merge

Each segment execution has a single result boundary that persists `Succeeded` or `Failed` for every error path, including ffmpeg extraction and whisper process startup.

Merge validates that every selected segment succeeded and has a readable text artifact. SRT offsets use ffprobe media duration when available. The final subtitle timestamp is a documented fallback only when duration probing fails. Missing SRT still advances the media-duration offset, and a merge that produces no SRT removes any previous merged SRT.

## Configuration Transaction

`save_config` follows a candidate transaction:

1. Clone active config while holding the mutex briefly.
2. Apply request fields to the candidate.
3. Preserve eligible existing keys and validate IDs, defaults, paths, numeric bounds, and provider protocols.
4. Create or validate the candidate workspace.
5. Atomically persist the candidate.
6. Reacquire the mutex and replace active config.

Workspace changes are rejected while any job is running. Long HTTP, archive, probe, and update operations clone the required values and release the mutex before I/O.

## Sidecar Resolution

Application setup resolves a bundled sidecar root from Tauri resources, with the executable directory as a development fallback. The root is stored in `AppState` and passed to every `resolve_all` call. Resolution order remains:

1. Bundled resource candidate.
2. Explicit configured path.
3. `PATH` lookup.

The same resolver is used by the runner, settings probe, and update check.

## Summary and Secret Boundary

Provider requests remain blocking work executed outside shared locks and outside the UI thread. Anthropic response parsing concatenates all text blocks in order.

Redaction occurs before logs or error details are persisted, not only at individual call sites. The redactor receives configured API keys and sensitive extra-header values, removes authorization tokens and signed-query credentials, and truncates only after secret replacement where truncation could split a token.

## Export Transaction

Export rejects a running job and any destination within the source job directory. It writes a unique temporary ZIP outside the job tree, copies files with `std::io::copy`, omits credential-bearing files/content, finalizes the archive, then renames it to the requested output. Failure removes the temporary file.

## Frontend Synchronization

The frontend registers one `job-updated` listener for the application lifetime. The listener uses refs for the current selection and handles asynchronous registration cleanup under React StrictMode.

`refreshSelectedJob` uses a monotonically increasing request token. It clears stale display state when selection changes and loads job, log, transcript, and summary independently with `Promise.allSettled`. Results are committed only if the token and selected job still match. Terminal and relevant step events refresh generated artifacts; explicit global refresh also refreshes the selected detail.

Deleting a task updates the list locally after backend success. If it was selected, request generations are invalidated and the selected job, log, transcript, and summary states are cleared so an older in-flight response cannot restore deleted detail.

## Persistence and Startup Recovery

Job and config writes use a temporary file in the destination directory, flush it, and atomically replace the destination where the platform permits. Job mutation is serialized per job through runner ownership or a workspace-level lock so progress callbacks cannot overwrite unrelated fields.

Before loading configuration or recovering jobs, the process acquires an exclusive lock file in the application config directory and retains it for the application lifetime. A second process exits before touching a workspace, so startup recovery cannot misclassify another instance's active jobs or create concurrent sidecar writers.

At startup, persisted `running` jobs have no surviving process ownership. They are marked `failed`, their active capture/current-step markers are cleared, and they receive an interrupted/retryable explanation before the first frontend list response. Switching to an existing workspace applies the same recovery after the current process has confirmed that it owns the application lock and has no running jobs.

## Compatibility and Migration

All new serialized fields use Serde defaults so existing job/config JSON remains loadable. Existing stale artifact files may remain on disk during migration, but references and step states are invalidated before display or export. No workspace migration is required.

## Rollback

Changes are grouped by contract boundary. If a stabilization phase regresses behavior, revert that phase without removing already persisted media or transcript segment files. Config and job schema changes must remain backward-compatible even when their consuming behavior is rolled back.

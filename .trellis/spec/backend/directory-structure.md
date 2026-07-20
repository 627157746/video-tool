# Rust Backend Directory Structure

> Module ownership for the Tauri application core.

## Current Layout

```text
src-tauri/src/
├── main.rs                 # Thin executable entry point
├── lib.rs                  # Tauri assembly, plugins, state, commands, tray
├── error.rs                # AppError and AppResult
├── storage.rs              # Atomic byte/JSON replacement
├── commands/mod.rs         # Tauri IPC handlers and AppState
├── config/mod.rs           # Config schema, validation, persistence, public view
├── models/
│   ├── mod.rs
│   └── job.rs              # Job domain model, statuses, request DTOs
├── workspace/mod.rs        # Job directory CRUD and interrupted-run recovery
├── sidecar/mod.rs          # Binary resolution and version probing
└── pipeline/
    ├── mod.rs
    ├── runner.rs           # Execution coordination, persistence, events
    ├── paths.rs            # Artifact paths, media discovery, cleanup
    ├── logs.rs             # Step logs and redaction
    ├── download.rs         # yt-dlp and local import
    ├── record.rs           # streamlink/ffmpeg live recording
    ├── transcribe.rs       # Audio extraction, whisper, transcript merge
    ├── summarize.rs        # Templates and Provider HTTP clients
    └── export.rs           # Redacted streaming ZIP export
```

## Ownership Rules

### Application assembly

`lib.rs` initializes plugins, the single-instance lock, shared state, command
registration, tray behavior, and window close handling. Keep business pipeline
logic out of this file.

### Command boundary

`commands/mod.rs` parses typed IPC input, performs boundary validation and lock
decisions, snapshots configuration, and delegates to workspace/pipeline code.
It must not duplicate sidecar or artifact implementation logic.

### Domain model

`models/job.rs` owns persisted Job state, serialized enums, step requirements,
derived status, and dependency invalidation. Avoid putting unrelated config
request DTOs here; the current `SaveConfigRequest` coupling is legacy, not a
pattern to expand.

### Persistence

`workspace` owns validated Job directory construction and Job CRUD. `storage`
owns atomic replacement. No other module should join an unvalidated external
Job ID or implement a second JSON-write scheme.

### Pipeline

`runner` owns sequencing, same-Job exclusion, state persistence, and
`job-updated` emission. Individual modules own tool arguments and artifact
production. `paths` owns stable layout helpers and downstream cleanup.

### Configuration and sidecars

`config` owns defaults, candidate updates, validation, disk storage, API key
resolution, and public redaction. `sidecar` owns bundled/configured/PATH lookup
and version probing; pipeline modules consume resolved executables.

## Dependency Direction

- Tauri assembly -> commands -> domain/workspace/pipeline/config.
- Runner -> individual pipeline modules and workspace persistence.
- Pipeline modules -> path/log/storage helpers as needed.
- Frontend never accesses workspace files directly; it uses Tauri commands and
  opener/dialog plugins.
- Lower-level storage/path helpers do not depend on UI or Tauri window state.

## Naming

- Rust modules, functions, and fields use snake_case.
- Types and enum variants use PascalCase.
- Functions use verb phrases; data values use descriptive noun phrases.
- Persisted enum values use `#[serde(rename_all = "snake_case")]`.
- Job artifact paths are relative to the validated Job directory when stored.

## Avoid

- Raw `workspace.join(job_id)` with an IPC-provided ID.
- Sidecar process construction in `commands/mod.rs` or `lib.rs`.
- UI-specific labels in Rust domain enums.
- New catch-all modules that mix command transport, domain transitions, file
  layout, and tool invocation.
- Treating current large `commands`, `config`, or `models/job` files as a reason
  to add unrelated responsibilities there.

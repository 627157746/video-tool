# Rust Backend Development Guidelines

> Project-specific guidance for the Tauri 2 and Rust application core.

## Scope and Sources

The backend owns application configuration, Job persistence and state,
workspace files, sidecar resolution and process execution, pipeline
orchestration, exports, and Tauri command/event boundaries.

`docs/PRODUCT_SPEC.md` owns product behavior. `src-tauri/Cargo.toml` and
`src-tauri/src/` are the evidence for current implementation patterns.

## Pre-Development Checklist

- [ ] Identify the owning module before adding command, pipeline, path, config,
      or serialization logic.
- [ ] Read `job-and-pipeline.md` before changing Job steps, status, progress,
      retries, segment selection, or downstream artifacts.
- [ ] Read `database-guidelines.md` before touching config, `source.json`, or
      workspace paths; this project has file persistence, not a database.
- [ ] Read `configuration-and-security.md` before handling Provider settings,
      keys, headers, proxy URLs, exports, or public config views.
- [ ] Read `sidecar-guidelines.md` before adding a process or changing binary
      resolution, output handling, timeout, cancellation, or update behavior.
- [ ] For a Tauri command or shared type, load `../tauri-ipc/` and synchronize
      Rust, registration, TypeScript wrapper, and caller.
- [ ] Decide whether a new persisted field needs a Serde default or migration.
- [ ] Search existing helpers before constructing Job paths, writing JSON,
      redacting logs, or spawning commands.

## Guidelines Index

| Guide | Project-specific focus |
|-------|------------------------|
| [Directory Structure](./directory-structure.md) | Rust module ownership and dependencies |
| [Workspace and Persistence](./database-guidelines.md) | File-backed config, Jobs, artifacts, and atomic writes |
| [Job and Pipeline](./job-and-pipeline.md) | State derivation, execution, retry, and invalidation |
| [Configuration and Security](./configuration-and-security.md) | Candidate config, public views, secrets, and trust boundaries |
| [Sidecar Guidelines](./sidecar-guidelines.md) | Binary resolution and subprocess behavior |
| [Error Handling](./error-handling.md) | `AppResult`, user-facing failures, and recovery |
| [Logging Guidelines](./logging-guidelines.md) | Per-step logs, privacy, redaction, and export |
| [Quality Guidelines](./quality-guidelines.md) | Rust checks, tests, cross-layer review, and smoke tests |
| [Tauri IPC Guidelines](../tauri-ipc/index.md) | Commands, data mirrors, and events |

## Quality Check

- [ ] `cargo +stable fmt --manifest-path "src-tauri/Cargo.toml" --all -- --check`
- [ ] `cargo +stable test --manifest-path "src-tauri/Cargo.toml"`
- [ ] `cargo +stable clippy --manifest-path "src-tauri/Cargo.toml" --all-targets -- -D warnings`
- [ ] `pnpm typecheck` and `pnpm build` when IPC/shared data changed.
- [ ] Relevant Tauri runtime, sidecar, Provider, or installer behavior is checked
      separately from pure unit-test results.
- [ ] Error/log/export paths have been reviewed for secrets and private content.
- [ ] Validation claims list commands actually run, not historical counts.

**Language**: Spec files are written in English. User-facing errors and product
copy are Simplified Chinese by default.

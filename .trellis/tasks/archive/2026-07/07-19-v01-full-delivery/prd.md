# PRD: v0.1 full delivery

## Goal

Deliver the `PRODUCT_SPEC.md` v0.1 workflow as a reliable self-use desktop application: ingest media, record live streams, transcribe locally, summarize through a configured provider, retry failed work, inspect artifacts, and export a safe job package.

## Background

- The worktree already contains a broad implementation of the v0.1 pipeline, but the interrupted prior session left this task in `planning` and did not complete its design, execution plan, or quality gate.
- The existing `07-19-download-runner-ui` work supplies the download/import runner and task-center baseline. This task owns the remaining v0.1 integration and the final cross-layer acceptance pass.
- Current evidence confirms `pnpm typecheck`, `pnpm build`, and eight Rust unit tests pass with the installed `stable` toolchain. Rust formatting currently fails, and the Tauri installer build previously stopped while downloading WiX after the release executable compiled.
- Static cross-layer review found release-blocking issues in direct IPC parameter naming, runner status derivation, partial live-record outcomes, export memory use, config mutation, bundled sidecar resolution, and frontend refresh/listener behavior. Existing code is therefore implementation input, not an accepted baseline.

## Requirements

### R1. Job and IPC integrity

- All frontend-to-Tauri command arguments must follow one verified naming convention.
- Job identifiers must be validated at the backend path boundary before accessing a job directory.
- Deleting a job must remove only its validated `jobs/<uuid>/` directory and must be rejected while that job is running.
- Job status must be derived from the required step states. Retrying one step or one transcript segment must not mark unrelated failed or stale downstream steps as successful.
- Re-running ingest, transcription, or segment selection must invalidate dependent transcript/summary artifacts and step states.
- Job and config JSON writes must use same-directory temporary files followed by atomic replacement where supported.

### R2. Live recording lifecycle

- Record a live source into ordered ffmpeg segments, optionally resolving the stream through streamlink.
- Support explicit user stop, disk guard, bounded reconnect, and post-record segment merge.
- Persist every usable partial segment before reporting reconnect exhaustion, disk guard activation, merge failure, or another terminal error.
- Treat user stop with usable media as a normal recording completion; treat exhausted reconnect and disk guard as failed ingest while preserving artifacts for inspection/retry.
- Tray close-to-hide behavior and the stop command must apply only while the live ingest process is active.

### R3. Local transcription and merge

- Transcribe ordered media segments with configurable `whisper-cli`, GGML model, language, and ffmpeg audio extraction.
- Persist a failed status and readable detail for every segment-level failure path.
- Support per-segment retry and user-selected segment ranges.
- Refuse to merge selected segments that are not successfully transcribed.
- Produce ordered plain text, raw JSON, and SRT when available; SRT offsets must use media duration when available and must not retain a stale prior SRT.

### R4. Provider summary and secret handling

- Support OpenAI-compatible and Anthropic message protocols, configured templates, proxy settings, extra headers, and a provider connectivity check.
- Reject input over the configured local context guard without truncating it.
- Read all Anthropic text content blocks in a successful response.
- Redact configured API keys, authorization headers, signed URL credentials, and sensitive header values before persisting logs or error details.
- Public config responses and export packages must not expose API keys.

### R5. Configuration and sidecars

- Validate a complete candidate configuration before replacing the active configuration.
- Reject duplicate provider/template IDs and dangling default IDs.
- Preserve an existing API key only when the matching provider ID remains unchanged and the incoming key is intentionally blank.
- Do not hold the config mutex while performing HTTP requests, archive creation, sidecar probes, or sidecar updates.
- Reject workspace changes while a job is running; after a successful workspace change, reload the frontend task state from the new workspace.
- Resolve sidecars in the documented priority order: bundled application resources, explicit user configuration, then `PATH`.

### R6. Export safety

- Export the complete job directory without config credentials.
- Stream file contents into a temporary ZIP instead of loading media files into memory.
- Reject destinations inside the source job directory and reject export while that job is running.
- Atomically publish the completed archive and remove temporary output after failure.

### R7. Frontend task-center behavior

- Keep the selected job, logs, transcript, summary, and task list synchronized with backend events and explicit refresh.
- Allow permanent deletion from the task list only after explicit confirmation, and clear selected-job artifacts when the selected job is deleted.
- Prevent old asynchronous responses from overwriting a newly selected job or log.
- Register exactly one leak-safe `job-updated` listener under React StrictMode.
- Independently load optional artifacts so one missing file cannot preserve unrelated stale content.
- Show live stop only during live ingest, clear stale downstream artifacts after invalidating actions, and report actionable backend errors.
- Provide baseline keyboard and screen-reader semantics for status messages, progress, log tabs, segment selection, and the create/settings dialogs.

### R8. Pipeline, retry, and lifecycle recovery

- Run configured automatic steps in dependency order and support explicit step retry.
- Emit and persist observable progress without allowing concurrent runs of the same job.
- On application startup, convert persisted `running` jobs that have no active runner into an interrupted failure state with retry guidance.

### R9. Verification and product documentation

- Cover pure status derivation, dependency invalidation, recording outcome mapping, merge selection, SRT offset fallback, template rendering, provider response parsing, redaction, path validation, atomic persistence helpers, sidecar priority, and export guards with focused Rust tests.
- Keep `docs/PRODUCT_SPEC.md` section 13 unchecked for any capability that still has a confirmed release blocker.
- Record automated and environment-dependent validation separately.

## Acceptance Criteria

1. `cargo +stable fmt --manifest-path "src-tauri/Cargo.toml" --all -- --check` passes.
2. `cargo +stable test --manifest-path "src-tauri/Cargo.toml"` passes with focused tests for the pure logic listed in R9.
3. `cargo +stable clippy --manifest-path "src-tauri/Cargo.toml" --all-targets -- -D warnings` passes.
4. `pnpm typecheck` and `pnpm build` pass.
5. A Tauri development smoke test confirms task detail, stop recording, open directory, transcript, and summary IPC calls receive their expected arguments.
6. A mocked or controlled run confirms step retry cannot hide failed downstream work and segment selection invalidates merged transcript and summary artifacts.
7. A controlled recording test confirms user stop preserves media, while reconnect exhaustion and disk guard preserve partial media and leave ingest failed.
8. An export test uses a file larger than the in-memory copy buffer, rejects a destination inside the job directory, and produces a ZIP without secrets.
9. Config tests confirm failed validation does not mutate active state, duplicate/dangling IDs are rejected, and long operations do not hold the config lock.
10. Sidecar resolution tests confirm bundled, configured, and `PATH` priority.
11. `docs/PRODUCT_SPEC.md` accurately distinguishes automated success from sidecar/provider/network checks that remain environment-dependent.
12. A deletion test confirms a completed job directory is removed, invalid IDs cannot escape the workspace, and running jobs are rejected by the command boundary.

## Out of Scope

- Shipping third-party sidecar binaries, a whisper model, API credentials, or a network proxy.
- Guaranteeing compatibility with every streaming site or codec beyond the documented ffmpeg/streamlink command strategy.
- Resuming an external ffmpeg or whisper process after an application crash; v0.1 only marks orphaned persisted runs as interrupted and retryable.
- Adding a new frontend test framework solely for this task; frontend behavior may use targeted Tauri smoke checks unless an existing lightweight harness is sufficient.
- Treating installer-tool download availability as an application correctness requirement. A release executable must compile, while MSI/NSIS packaging failures caused solely by unavailable external tooling must be reported separately.

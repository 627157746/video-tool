# Implementation Plan: v0.1 full delivery stabilization

## Baseline

- Preserve all user and prior-agent work already present in the dirty worktree.
- Do not mark `PRODUCT_SPEC.md` checklist items complete until the corresponding acceptance checks pass.
- Apply Rust formatting before reviewing final diffs, then avoid unrelated formatting or UI rewrites.

## Phase 1: Restore task and build hygiene

- [ ] Curate the implementation and check context manifests.
- [ ] Activate this task after artifact review.
- [ ] Run `cargo +stable fmt --manifest-path "src-tauri/Cargo.toml" --all`.
- [ ] Re-run the existing frontend and Rust checks to establish a clean baseline.

Validation:

```bash
pnpm typecheck
pnpm build
cargo +stable test --manifest-path "src-tauri/Cargo.toml"
cargo +stable fmt --manifest-path "src-tauri/Cargo.toml" --all -- --check
```

Rollback point: formatting-only changes can be reviewed separately from behavioral fixes.

## Phase 2: Repair cross-layer command and persistence contracts

- [ ] Fix direct Tauri invocation arguments to use `jobId`; leave nested request fields in `snake_case`.
- [ ] Add a reusable UUID-validating job-directory helper and route all job commands/export through it.
- [ ] Add atomic JSON write helpers for config and job persistence.
- [ ] Add startup recovery for persisted jobs left in `running` state.
- [ ] Add focused tests for path rejection, atomic helper behavior, and interrupted-state recovery.

Validation:

- Unit tests for valid UUID paths and traversal rejection.
- Tauri development smoke test for detail, open directory, stop, transcript, and summary commands.

Rollback point: keep command naming and path validation in one focused diff before runner changes.

## Phase 3: Make runner and recording outcomes truthful

- [ ] Implement required-step status derivation and dependency invalidation helpers in the model layer.
- [ ] Use derived status after full runs, step retry, and segment retry.
- [ ] Invalidate downstream artifacts before rerunning an upstream step or changing segment selection.
- [ ] Replace the live recording success boolean with an explicit terminal outcome.
- [ ] Persist partial media before mapping reconnect exhaustion, disk guard, or merge errors to failed ingest.
- [ ] Register live-record lifecycle before probing and accept stop only for active live ingest.
- [ ] Add pure tests for status derivation, invalidation, and recording outcome mapping.

Validation:

- Controlled fake-sidecar or short local stream scenarios for normal end, user stop, reconnect exhaustion, and disk guard.
- Confirm downstream automatic steps do not run after a failed recording outcome.

Rollback point: retain generated media regardless of state-mapping rollback.

## Phase 4: Stabilize transcription and summary artifacts

- [ ] Persist segment failure status for audio extraction, process spawn, process exit, and output-read failures.
- [ ] Reject merge when a selected segment is not successful.
- [ ] Probe media duration for SRT offsets, retain timestamp fallback, and remove stale merged SRT output.
- [ ] Invalidate merged transcript and summary after segment retry or selection changes.
- [ ] Parse and join all Anthropic text blocks.
- [ ] Centralize persisted log/error redaction for configured keys, sensitive headers, bearer tokens, and signed URL queries.
- [ ] Add tests for failed selected segments, missing SRT, duration-based offset, Anthropic content blocks, and redaction.

Validation:

- Run pure Rust tests without requiring a model or network.
- With configured tools, transcribe two short segments and verify plain text/SRT order after changing the selection.

## Phase 5: Make config, sidecar, and export operations safe

- [ ] Refactor config save into validate/persist/swap candidate semantics.
- [ ] Validate duplicate IDs, dangling defaults, protocols, and numeric/path bounds.
- [ ] Reject workspace changes while any job runs.
- [ ] Clone config values before long-running provider, sidecar, update, and export operations.
- [ ] Resolve and store the bundled sidecar root in `AppState`; pass it to every sidecar resolution path.
- [ ] Add sidecar priority tests.
- [ ] Stream ZIP entries, reject self-containing destinations/running jobs, use temporary output, and clean up on failure.
- [ ] Add export guard and no-secret tests.

Validation:

- Config failure leaves both in-memory and on-disk config unchanged.
- Provider timeout does not block an active recording stop request.
- Export a large sparse/test file without proportional process-memory growth.

Rollback point: config schema remains unchanged; behavior can revert without migrating existing config files.

## Phase 6: Synchronize and harden the frontend

- [ ] Add a request-token-based selected-job refresh that independently loads optional artifacts.
- [ ] Clear stale logs/transcript/summary on selection and invalidating actions.
- [ ] Add confirmed task-list deletion, reject running jobs, and clear selected detail after successful deletion.
- [ ] Register one StrictMode-safe event listener and refresh artifacts on relevant updates.
- [ ] Include current detail in explicit refresh behavior.
- [ ] Show stop only for live ingest and use operation-specific busy state.
- [ ] Normalize auto-summary/auto-transcribe controls to communicate their dependency.
- [ ] Prevent Provider connectivity tests from silently testing unsaved drafts.
- [ ] Report yt-dlp update semantics and non-zero exits accurately.
- [ ] Add baseline dialog, live-region, progressbar, tab, checkbox-label, keyboard escape, and focus-visible accessibility behavior.

Validation:

- Rapidly switch jobs and log tabs while jobs emit updates; old responses must not overwrite the current selection.
- Run under React StrictMode and confirm each backend event produces one UI refresh.
- Keyboard-only smoke test for create/settings dialogs and segment selection.

## Phase 7: Final quality gate and documentation

- [ ] Run the complete automated quality gate.
- [ ] Review the full cross-layer flow: persisted job -> IPC -> frontend -> command -> pipeline -> persisted artifacts -> event -> refreshed UI.
- [ ] Confirm no secrets appear in config public view, logs, persisted errors, or an export fixture.
- [ ] Update `docs/PRODUCT_SPEC.md` section 13 only for capabilities whose blockers are resolved.
- [ ] Record the WiX/network packaging result separately from application compilation.

Final commands:

```bash
cargo +stable fmt --manifest-path "src-tauri/Cargo.toml" --all -- --check
cargo +stable test --manifest-path "src-tauri/Cargo.toml"
cargo +stable clippy --manifest-path "src-tauri/Cargo.toml" --all-targets -- -D warnings
pnpm typecheck
pnpm build
pnpm tauri:build
```

`pnpm tauri:build` may require external WiX/installer downloads. If only that download fails after the release executable compiles, report it as an environment blocker and do not describe installer packaging as successful.

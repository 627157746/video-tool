# Rust Backend Quality Guidelines

> Compile, lint, test, cross-layer review, and environment validation rules.

## Automated Quality Gate

Run from the repository root:

```bash
cargo +stable fmt --manifest-path "src-tauri/Cargo.toml" --all -- --check
cargo +stable test --manifest-path "src-tauri/Cargo.toml"
cargo +stable clippy --manifest-path "src-tauri/Cargo.toml" --all-targets -- -D warnings
```

Use `cargo check --manifest-path "src-tauri/Cargo.toml"` as a fast local check,
not as a replacement for the full gate. Run `pnpm typecheck` and `pnpm build`
for command, event, or serialized data changes.

## Test Style

Current Rust tests are colocated in `#[cfg(test)] mod tests` blocks. They use
temporary directories with UUID isolation and directly exercise pure logic and
filesystem boundaries.

Add focused tests when changing:

- Required steps, derived Job status, or downstream invalidation.
- Config defaults, validation, key preservation, and public redaction.
- UUID/path containment, atomic replacement, delete, export, or recovery.
- Secret redaction and bounded text behavior.
- Endpoint/template parsing and Provider response extraction.
- Segment ordering, selection, transcript merge, or recording termination.

Do not add tests that only repeat implementation assignments. Prefer boundary,
failure, restart, compatibility, and privacy regressions.

## Cross-Layer Review

For a Tauri contract change, verify:

1. Rust request/response type and Serde representation.
2. `#[tauri::command]` handler signature.
3. `generate_handler!` registration in `lib.rs`.
4. `src/api.ts` wrapper and direct-vs-envelope naming.
5. `src/types.ts` mirror and all UI mappings.
6. Event payload consumers and persisted schema compatibility.

See `../tauri-ipc/` for the exact contract.

## Side-Effect Review

Every command/process addition must state:

- Whether it blocks, spawns background work, or returns completion.
- Files, network endpoints, environment variables, and child processes used.
- Lock scope and same-Job/cross-Job concurrency behavior.
- Timeout, cancellation, output draining, and process cleanup behavior.
- Secret/private data entering errors, logs, or exports.
- Rollback or retained-partial-output behavior on failure.

An operation that checks for an update must not silently execute an updater.
Names and confirmation UI must reflect remote or modifying side effects.

## Forbidden Patterns

- Shell command-string interpolation for sidecars; use `Command` argv.
- Joining IPC-provided Job IDs without UUID validation.
- Direct metadata `fs::write` where atomic replacement is required.
- New persisted fields without a compatibility/default decision.
- `unwrap`/`expect` on user, network, process, path, or runtime lock failures.
- Logging config/provider `Debug` output containing secrets.
- Unbounded child output, background threads, retries, or cross-Job concurrency
  without an explicit resource decision.
- Declaring external tool or Provider behavior verified based only on unit tests.

## Manual and Environment Validation

Automated tests do not prove the installed environment. Depending on scope,
check configured/bundled/PATH sidecar resolution, real download, live stop and
reconnect, whisper model execution, Provider protocols, proxy behavior, tray
close-to-hide, release packaging, and installer output.

Report these separately from pure checks. Packaging download failures or absent
sidecars must not be misreported as source compilation failures, and successful
compilation must not be misreported as real media workflow validation.

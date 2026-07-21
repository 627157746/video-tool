# Implement plan — v0.2 P0 foundation

## Order

1. **Backend status + config**
   - `JobStatus::Queued`
   - config concurrency fields + public/save/validate/defaults
   - recover queued → pending
2. **Backend queue scheduler** in `pipeline/runner.rs`
   - enqueue API used by all spawn entry points
   - pump on end
   - expose queue positions for list_jobs / get_job
3. **Backend health** in `workspace` (+ commands)
4. **Frontend types/api/labels**
5. **Frontend split** (extract files; App composes)
6. **Settings**: concurrency + 诊断 panel
7. **Validate** + product/spec notes

## Validation

```bash
cargo +stable fmt --manifest-path "src-tauri/Cargo.toml" --all -- --check
cargo +stable test --manifest-path "src-tauri/Cargo.toml"
cargo +stable clippy --manifest-path "src-tauri/Cargo.toml" --all-targets -- -D warnings
pnpm typecheck
pnpm build
```

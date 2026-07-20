# Tauri IPC and Data Contract Guidelines

> Cross-layer rules for the React webview and Rust application core.

## Boundary Map

```text
React component
  -> src/api.ts wrapper
  -> Tauri invoke serialization
  -> #[tauri::command] in src-tauri/src/commands/mod.rs
  -> Rust domain/config/workspace/pipeline
  -> persisted state + full job-updated event
  -> React guarded snapshot merge
```

Neither `src/types.ts` nor Rust alone provides a generated cross-language
contract. Every change must be traced across both sides.

## Pre-Development Checklist

- [ ] Identify command name, direct parameters or request envelope, result type,
      error behavior, blocking/background semantics, and side effects.
- [ ] Read the Rust Serde model and the TypeScript mirror together.
- [ ] Check persistence compatibility when the response is also stored on disk.
- [ ] Check whether the value reaches `job-updated` and snapshot merge logic.
- [ ] Review public/private config separation and log/export redaction.
- [ ] Verify Tauri plugin package, Rust initialization, and capability permission
      when using dialog/opener or a new plugin.

## Guides

| Guide | Focus |
|-------|-------|
| [Command Contracts](./command-contracts.md) | Handler, registration, wrapper, arguments, and side effects |
| [Data Contracts](./data-contracts.md) | Serde naming, TypeScript mirrors, optionality, compatibility, privacy |
| [Events and Consistency](./events-and-consistency.md) | Full Job events, polling, request generations, and snapshot ordering |

## Quality Check

- [ ] Rust handler and `generate_handler!` registration agree.
- [ ] `src/api.ts` uses the correct direct/envelope parameter shape.
- [ ] `src/types.ts` matches serialized names, enums, nullability, and public
      secret boundaries.
- [ ] `pnpm typecheck`, `pnpm build`, and relevant Rust checks pass.
- [ ] Desktop runtime smoke confirms the command/event/plugin capability.
- [ ] Older `config.json`/`source.json` behavior is explicitly considered.
- [ ] No stale response or older refresh can overwrite a newer Job snapshot.

## Source References

- `src/api.ts`
- `src/types.ts`
- `src/App.tsx`
- `src-tauri/src/commands/mod.rs`
- `src-tauri/src/models/job.rs`
- `src-tauri/src/config/mod.rs`
- `src-tauri/src/pipeline/runner.rs`
- `src-tauri/src/lib.rs`

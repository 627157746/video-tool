# Frontend Development Guidelines

> Project-specific guidance for the React 19, TypeScript, Vite, and Tauri UI.

## Scope and Sources

The frontend is a single-window desktop UI under `src/`. Product behavior is
owned by `docs/PRODUCT_SPEC.md`; current implementation evidence comes from
`src/`, `package.json`, `tsconfig.json`, and the matching Rust IPC code.

The current UI is intentionally dependency-light: it uses React built-in state
and effects, plain CSS, and Tauri APIs. Do not assume React Router, a global
state library, a query library, a CSS framework, or a frontend test runner.

## Pre-Development Checklist

- [ ] Read the relevant product contract in `docs/PRODUCT_SPEC.md`.
- [ ] Identify whether the change is UI-only, Tauri platform integration, or a
      cross-layer IPC/data-contract change.
- [ ] For application commands, read `src/api.ts`, `src/types.ts`, the Rust
      command/model, and `src-tauri/src/lib.rs` registration together.
- [ ] Search all enum labels, dynamic CSS classes, and branch logic before
      adding a Job kind, status, step, or segment status.
- [ ] Preserve request-version guards when changing overlapping async loads.
- [ ] Preserve Provider/template reference integrity when editing settings.
- [ ] Check keyboard, focus, and announcement behavior for interactive UI.
- [ ] Use `pnpm`; do not create npm or Yarn lock files.

## Guidelines Index

| Guide | Project-specific focus |
|-------|------------------------|
| [Directory Structure](./directory-structure.md) | Current flat layout and file ownership |
| [Component Guidelines](./component-guidelines.md) | Functional components, forms, CSS, and accessibility |
| [Hook Guidelines](./hook-guidelines.md) | Effects, event cleanup, polling, and stale-response guards |
| [State Management](./state-management.md) | Draft config, backend-owned Job state, and concurrency |
| [Type Safety](./type-safety.md) | Strict TypeScript and Rust/TypeScript mirrors |
| [Quality Guidelines](./quality-guidelines.md) | Real validation commands and review checks |
| [Tauri IPC Guidelines](../tauri-ipc/index.md) | Commands, serialization, events, and privacy boundaries |

## Quality Check

- [ ] `pnpm typecheck` passes.
- [ ] `pnpm build` passes.
- [ ] Rust checks are run when IPC contracts or Tauri integration changed.
- [ ] Tauri runtime behavior is checked with `pnpm tauri:dev` when relevant;
      browser-only `pnpm dev` is not treated as IPC validation.
- [ ] Rapid Job/log switching does not show stale details or artifacts.
- [ ] React StrictMode does not leave duplicate listeners or timers.
- [ ] Dialogs, tabs, progress, errors, and success messages remain keyboard and
      screen-reader usable.
- [ ] No API key, sensitive header, or proxy credential is added to public UI
      types, Job metadata, logs, or exported content.

**Language**: Spec files are written in English. User-visible product copy is
Simplified Chinese unless a product requirement says otherwise.

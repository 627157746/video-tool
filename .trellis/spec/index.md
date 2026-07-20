# video-tool Specification Index

> Navigation and evidence rules for project-specific development guidance.

## Technology Profile

- Desktop shell: Tauri 2.
- Core: Rust 2021.
- Frontend: React 19, TypeScript 5.8, Vite 7.
- Package manager: pnpm 10 (`pnpm-lock.yaml` is authoritative).
- Persistence: JSON and filesystem workspace; no database.
- External tools: yt-dlp, streamlink, ffmpeg/ffprobe, whisper.cpp.
- Cloud summary: blocking reqwest client for OpenAI-compatible or Anthropic
  protocols; media is not uploaded by the summary pipeline.
- Primary target: Windows 10/11 x64, with path-resolution structure reserved for
  other platforms but no v0.1 delivery guarantee.

## Evidence Priority

Use these sources for different questions:

1. `docs/PRODUCT_SPEC.md` - locked product behavior and scope.
2. `package.json`, `src-tauri/Cargo.toml`, `src/`, `src-tauri/src/` - current
   implementation and dependencies.
3. `.trellis/spec/` - coding and change-management contracts derived from that
   implementation.
4. `README.md` - onboarding entry point, not authoritative implementation
   completion evidence when it conflicts with source or product spec.

Do not copy historical test counts or completion statements into new specs.
Validation is established by commands run for the current change.

## Package and Layer Specs

| Area | Entry point | Owns |
|------|-------------|------|
| Frontend | [frontend/index.md](./frontend/index.md) | React state, components, effects, CSS, TypeScript quality |
| Rust backend | [backend/index.md](./backend/index.md) | Tauri core, Job pipeline, workspace, config, sidecars, errors |
| Tauri IPC | [tauri-ipc/index.md](./tauri-ipc/index.md) | Commands, Rust/TypeScript data mirrors, events, consistency |
| Thinking guides | [guides/index.md](./guides/index.md) | Change-impact and cross-layer review prompts |

## Project-Wide Invariants

- Use pnpm; do not add npm/Yarn lock files.
- Rust owns final validation, Job state, persistence, secrets, and side effects.
- Frontend application commands go through `src/api.ts`.
- Rust Serde and `src/types.ts` are a manually synchronized contract.
- All source kinds converge on one Job workspace layout and pipeline.
- Summary consumes merged transcript text and does not upload video.
- API keys stay out of Job metadata, public config, logs, and exports.
- External Job IDs are UUID-validated before path construction.
- Config and Job JSON use atomic replacement helpers.
- Sidecar arguments use process argv, never shell interpolation.
- User-visible copy and actionable errors default to Simplified Chinese.
- Real sidecar, Provider, Tauri runtime, and installer behavior is reported
  separately from compile/unit-test results.

## Choosing Context Before a Change

- UI-only change: load `frontend/`.
- Rust-only pure/domain change: load `backend/`.
- Command, event, Job/config field, enum, plugin, or public view change: load
  `frontend/`, `backend/`, and `tauri-ipc/`.
- Source/step/artifact/config changes: also load both thinking guides because
  they have repository-specific impact checklists.

## Spec Maintenance

Update these specs when a stable project convention changes, a bug reveals a
missing prevention rule, or a new layer/package is introduced. Every rule should
reference a real file, symbol, test, or product contract. Remove stale guidance
rather than accumulating contradictory history.

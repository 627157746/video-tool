# Frontend Quality Guidelines

> Validation and review requirements that match the repository's actual tools.

## Automated Checks

Run from the repository root:

```bash
pnpm typecheck
pnpm build
```

`pnpm build` performs TypeScript compilation and a Vite production build.
`pnpm dev` is useful for browser layout work but cannot validate Tauri IPC.
Use `pnpm tauri:dev` for command, event, dialog, opener, tray, and filesystem
smoke checks.

The project currently has no frontend unit test runner and no ESLint, Prettier,
or Biome script. Do not claim `pnpm test` or `pnpm lint` was run. Add automated
coverage only when a focused regression test is valuable and the required test
infrastructure is an intentional project decision.

## Required Review Checks

- Strict TypeScript remains clean without suppressing real errors.
- Application commands are called through `src/api.ts`.
- IPC enum/field changes are synchronized with Rust and serialization rules.
- Optional artifacts clear on Job/workspace changes and cannot show stale data.
- Repeated requests, events, and polling preserve newest-snapshot ordering.
- Listener, interval, focus, and inert cleanup works under StrictMode.
- Provider/template ID edits preserve references before save and remain
  backend-validated.
- Destructive actions have confirmation and scoped in-flight protection.
- Keyboard navigation, focus return, status announcements, and progress
  semantics remain usable.
- User-visible copy is Simplified Chinese and gives actionable failure context.
- No public type, state snapshot, error banner, log, or export contains secrets.

## Forbidden Shortcuts

- `npm install`, `yarn`, `package-lock.json`, or `yarn.lock`.
- Direct application `invoke` calls scattered through components.
- `any`/broad assertions used to mask Rust/TypeScript drift.
- Parsing natural-language errors as machine-readable status.
- Async effects/listeners/timers without cleanup.
- Unprotected overlapping requests that can update the wrong Job or log.
- Introducing a framework or dependency when an existing local pattern solves
  the problem clearly.
- Treating current `App.tsx` size or global CSS concentration as a reason to add
  unrelated responsibilities there.

## Manual Smoke Matrix

Choose the smallest relevant subset:

1. Create each affected Job kind and inspect pending/running/final UI states.
2. Switch Jobs and log tabs rapidly while requests are in flight.
3. Delete the selected Job during or after list refresh.
4. Save, rename, and delete Provider/template IDs and verify default references.
5. Open/close dialogs by keyboard and verify focus returns.
6. Verify Tauri file dialog and opener capabilities in the desktop runtime.
7. Verify `job-updated` plus running-Job polling does not duplicate or regress
   snapshots.
8. Confirm no complete API key or sensitive header appears in rendered output.
9. When UI/theme changed: toggle system/light/dark and each accent; reload and
   confirm preference persistence; open Settings fullscreen and confirm the
   panel spans the content width without a large empty side gutter.

## Reporting

Report exact commands and smoke paths actually executed. Historical statements
in README or product documents are not current validation evidence.

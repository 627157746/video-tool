# State Management

> State ownership and concurrency rules for the task center.

## State Categories

### Local interaction state

Dialog visibility, active tabs, search text, focus targets, transient feedback,
and operation-specific busy flags belong to the nearest UI owner.

### Settings drafts

Provider profiles, templates, defaults, sidecar paths, model settings, and
pipeline defaults are editable drafts. They may diverge from the last accepted
configuration until the user saves.

`AppConfigPublic` is the persisted public snapshot used to repopulate drafts
and detect unsaved changes. Replace that snapshot only after the Rust backend
accepts and persists the complete candidate.

### Backend-owned Job state

Jobs, steps, segments, logs, transcript, and summary are backend-owned. The UI
caches snapshots for rendering but must refresh or merge them from Tauri
commands and `job-updated` events. Do not derive a second independent Job state
machine in React.

## Update Patterns

- Use functional setters when the next value depends on current state.
- Use immutable `map`/`filter` updates for Provider and template collections.
- Copy a `Set` before adding/removing IDs used for per-Job operation tracking.
- Keep derived display data in `useMemo` when it is expensive or used as a
  stable dependency; do not persist values that can be cheaply recomputed.
- Treat backend timestamps as UTC ISO strings; current snapshot ordering relies
  on their lexical ordering.

## Snapshot Consistency

`mergeJobListSnapshots` demonstrates the merge rules:

- Prefer the Job snapshot with the newer `updated_at`.
- Keep list ordering by `created_at` descending.
- Respect deletion tombstones so an older in-flight refresh cannot reinsert a
  Job deleted by the user.
- Use request generations for list, detail, and log requests.
- Clear selected artifacts when deleting a selected Job or switching workspace.

The selected running Job may be polled every three seconds in addition to event
updates. Polling is a recovery path, not a competing source of truth.

## Settings Reference Integrity

The frontend improves editing ergonomics, but Rust remains the final validation
boundary. Rust must continue rejecting duplicate IDs and dangling default
Provider/template references.

For draft collections referenced by another field:

1. Propagate an ID rename to the default reference in the same action.
2. Select a valid remaining ID after deletion.
3. Trim IDs and reconcile references again at the save boundary.
4. Submit the complete candidate config, not a set of unsynchronized patches.
5. Replace drafts with the public config returned after a successful save.

An HTML `<select>` may visually show its first option while its controlled value
matches no option. Never treat visible selection as proof of valid state.

## Global State

There is currently no global state library. Keep state local while the
application remains a single mounted shell. Introduce shared context/store only
when multiple independently mounted feature boundaries need the same mutable
state and lifting it to their nearest owner is demonstrably worse.

Do not add a store merely to move the existing `App` state without clarifying
which state remains backend-owned, which is a settings draft, and which is
purely local interaction state.

## Avoid

- Mutating arrays, objects, or sets already stored in React state.
- Letting stale async responses update current selection.
- Treating event delivery as guaranteed or polling as the sole update channel.
- Silently accepting unknown enum/protocol values by mapping them to a default.
- Parsing user-facing backend error strings as stable business codes.
- Saving API keys into Job or general UI snapshots beyond the current input.

# State Management

> How state is managed in this project.

---

## Overview

The React task center keeps editable settings as local draft state. Persisted
configuration remains owned by the Rust backend and is refreshed after a
successful save.

---

## State Categories

- **Settings drafts**: Provider profiles, summary templates, defaults, and
  sidecar paths may diverge from the last persisted config while editing.
- **Persisted config snapshot**: `AppConfigPublic` is used for dirty checks and
  is replaced only after the backend accepts the complete candidate config.
- **Task server state**: Jobs and artifacts are refreshed from Tauri commands
  and `job-updated` events with request-generation guards.

---

## When to Use Global State

<!-- Criteria for promoting state to global -->

(To be filled by the team)

---

## Server State

The frontend may improve editing ergonomics, but the backend remains the final
validation boundary. In particular, Rust must continue rejecting duplicate IDs
and dangling default Provider/template references.

Before submitting related settings drafts, normalize identifier whitespace and
ensure every default ID references an entity in the same outgoing candidate.
If an entity ID is renamed or a referenced entity is deleted, update the
corresponding default draft in the same UI action.

---

## Common Mistakes

### Controlled selects with dangling values

An HTML `<select>` can visually display its first option even when React's
controlled `value` does not match any option. Never treat the displayed option
as proof that the state is valid. This occurred when `example-openai` remained
in `settingsDefaultProviderId` after the Provider was renamed or deleted; the
save request then correctly failed backend validation.

For collections referenced by another draft field:

1. Propagate ID renames to the reference field.
2. Select a valid remaining ID after deletion.
3. Reconcile references again at the save boundary as a final guard.
4. Keep backend referential-integrity validation enabled.

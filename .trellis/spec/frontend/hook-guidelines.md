# Hook and Effect Guidelines

> Lifecycle and asynchronous coordination rules for the current React UI.

## Current Baseline

The project uses React built-in hooks directly in `src/App.tsx`. There is no
custom hook directory, query library, or external state library. Do not invent
an abstraction merely to match a generic React layout.

Extract a custom hook when one lifecycle unit has a clear input/output contract
and would otherwise repeat or obscure listener, polling, focus, or request
coordination. Custom hooks use the `use...` naming convention and must not hide
cross-layer side effects from their callers.

## Tauri Command Requests

Overlapping requests must not allow an older response to replace newer state.
The current reference pattern uses request-generation refs:

- `detailRequestVersionRef` for Job detail and artifacts.
- `logRequestVersionRef` for log tab changes.
- `refreshRequestVersionRef` for list refreshes.
- `selectedJobIdRef` and `logNameRef` for current selection checks.

At response and error boundaries, verify both the captured generation and the
current selected identity before updating state. Reset optional artifacts when
selection changes so a failed load cannot leave content from another Job.

Use `Promise.allSettled` when logs, transcript, and summary are independent
optional artifacts. One missing artifact must not block the others.

## Events and Polling

`job-updated` is the primary real-time Job channel. A three-second poll is used
only while the selected Job is running as recovery against missed/delayed
events.

Effects that register a Tauri listener must:

1. Keep the unlisten function.
2. Handle cleanup occurring before the listener Promise resolves.
3. Unregister on cleanup.
4. Avoid duplicate listeners under React StrictMode.
5. Reuse the event payload or perform a guarded refresh; do not start an
   unbounded refresh loop.

Intervals and timeouts must be cleared in cleanup. Poll only while the state
requires it, not for the lifetime of the application.

## Dependencies and Refs

- Use `useCallback` for async functions consumed by effects or handlers when a
  stable identity prevents unnecessary resubscription.
- Include real reactive dependencies; do not silence dependency problems with
  comments or stale closures.
- Use refs for mutable coordination that must not trigger rendering, such as
  request generations, current selected IDs, deletion tombstones, focus return
  elements, and in-flight guards.
- Do not mirror every state value into a ref; refs are not a second state store.

## Avoid

- An async effect body that returns a Promise directly.
- A listener or timer without cleanup.
- A request guard that checks only Job ID but not request generation when the
  same Job can be requested repeatedly.
- `Promise.all` for independently optional artifacts.
- Polling all Jobs continuously when `job-updated` already carries a full Job.
- Adding React Query/SWR solely to wrap the current small IPC surface.

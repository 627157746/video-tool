# Error Handling

> Error propagation, user-facing failures, persistence, and restart recovery.

## Current Error Type

Backend functions return `AppResult<T>` from `src-tauri/src/error.rs`.
`AppError` currently wraps message, I/O, and JSON failures. Tauri serialization
returns `Display` text as a string, so the webview does not receive a structured
error code or retryability field.

Use `?` for errors that already have sufficient context. Wrap low-level errors
with an actionable message when the operation, Job step, path category, or
sidecar cannot otherwise be inferred.

User-facing errors are Simplified Chinese and should answer:

- What operation failed?
- Which dependency or input was involved?
- Was partial work retained?
- Which retry, setting, log, or path can help next?

## Command Boundary

- Validate cheap structural constraints before starting background work.
- Return command-start acceptance separately from background completion. Job
  completion is reported by persisted Job state and `job-updated`.
- Keep Rust as the final validation boundary even when the UI pre-validates.
- Do not expose API keys, Authorization headers, proxy credentials, or full
  Provider response bodies in command errors.
- The frontend must not parse natural-language text as an error category.

If behavior requires machine-readable categories, introduce a versioned error
envelope across Rust, Serde, TypeScript, and UI instead of adding message-text
matching.

## Pipeline Failures

The runner owns the Job failure transition:

1. Mark the step running and persist before side effects.
2. Execute the step.
3. On failure, redact the message before storing it in Job state/logs.
4. Mark the active step failed and derive the Job status.
5. Persist the final state and notify the UI.
6. Preserve useful media or completed segments unless the product contract
   explicitly requires cleanup.

Do not discard the original operation error if secondary logging, cleanup, or
notification also fails. Persistence failure is critical; event notification
failure should not retroactively redefine a successfully persisted business
operation as failed.

## Lock and Thread Failures

Avoid adding new `.expect(...)` calls on runtime mutexes. A poisoned lock or
background panic should become an actionable application failure and must not
leave a Job permanently marked running in memory.

Background work needs cleanup that removes the Job from `RunnerState` even on
early return. For new runner paths, prefer an explicit guard/cleanup boundary
over detached logic with many return points.

## Recovery

`workspace::recover_interrupted_jobs` converts persisted running Jobs to failed
on startup. Preserve this visibility: interruption is not success, and the user
must be able to inspect logs and retry.

Live recording has explicit termination reasons in `RecordTermination`. Use a
descriptive enum when an operation has several materially different outcomes;
do not compress normal end, user stop, disk guard, retry exhaustion, and process
failure into a boolean.

## Avoid

- `unwrap`/`expect` for user input, filesystem, process, network, or lock paths.
- `let _ = ...` for required persistence or cleanup without a documented
  best-effort reason.
- Returning raw Provider/sidecar output before redaction.
- Treating a missing optional artifact and an empty artifact as equivalent in a
  new contract without making that decision explicit.
- Deleting old useful output before a retry unless the state/rollback behavior
  is understood and accepted.

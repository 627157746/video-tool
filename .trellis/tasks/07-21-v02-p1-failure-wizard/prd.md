# v0.2 P1 failure recovery wizard + error_code

## Goal

When a Job fails, show a recovery card driven by a stable `error_code` plus step context, with one-click actions (retry, logs, settings, segments, cookies guidance).

## Requirements

- Persist optional `error_code` on Job (snake_case strings).
- Classify failures from error text / step (heuristic MVP); keep human-readable `error_message`.
- Clear `error_code` when a new run starts successfully.
- UI card only when status is failed (or step failed with message).
- Non-goals: silent auto-fix, ML decision trees.

## Codes (MVP)

`SIDECAR_MISSING`, `AUTH_REQUIRED`, `CONTEXT_TOO_LONG`, `DISK_GUARD`, `NETWORK`, `DOWNLOAD_FAILED`, `TRANSCRIBE_FAILED`, `SUMMARIZE_FAILED`, `INTERRUPTED`, `UNKNOWN`.

## Acceptance

- [x] Failed jobs get a non-empty `error_code` when classified.
- [x] Detail shows recovery suggestions with actions.
- [x] Tests for classifier; typecheck/clippy pass.

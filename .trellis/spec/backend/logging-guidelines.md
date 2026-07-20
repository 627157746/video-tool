# Logging Guidelines

> Per-Job diagnostic logging and privacy rules.

## Current Log Model

The project uses text files under `jobs/<job_id>/logs/`, not a structured
logging library. Each pipeline step owns a stable log file:

- `download.log`
- `record.log`
- `transcribe.log`
- `merge_transcript.log`
- `summarize.log`

`pipeline::logs::append_log` appends tool and application output. The command
boundary exposes only whitelisted log names and `read_log` returns a bounded
tail for the UI.

Do not claim support for log levels, rotation, retention, or fully structured
records; those are not implemented today.

## What to Record

- Step start/end and the operation being attempted.
- Resolved tool path/version where it aids reproducibility.
- Progress, retry, reconnect, disk guard, and heartbeat diagnostics.
- Exit status and redacted stderr/stdout needed to troubleshoot a sidecar.
- Provider protocol/model/endpoint category without credentials.
- Whether partial media, transcripts, or summaries were retained.

Keep output streaming for long-running tools. Do not buffer unbounded process
output merely to write it after process exit.

## Sensitive and Private Data

The workspace is private user-content storage, not a harmless cache. Logs may
contain source URLs, local paths, transcript excerpts, titles, and sidecar
output. Minimize them even when they are not credentials.

Never persist:

- Complete API keys or bearer tokens.
- `Authorization`, `x-api-key`, or sensitive extra-header values.
- Proxy usernames/passwords.
- Signed URL query parameters when they can be redacted.
- Full Provider request/response bodies by default.
- Full prompts or transcripts solely for debugging.

Use `redact_secrets` before a message reaches Job state, logs, error responses,
or export metadata. Pass explicit configured secret values when available in
addition to heuristic regex redaction. Heuristics alone are not proof that a
new secret format is covered.

## Prompt and URL Logging

The summarizer currently records a truncated prompt preview for diagnostics.
Treat it as private content and keep the preview bounded and redacted. New code
must not increase that scope without a product/privacy decision.

Source URLs can include account or signature query data. Prefer a redacted form
or origin/path context when the complete URL is not necessary to reproduce the
failure.

## Export

`pipeline::export` streams binary files and redacts text metadata before adding
it to the ZIP. Export must never include application config or API keys. Review
new text artifact types against `redact_secrets` before adding them.

## Reliability

- A required Job-state write must not be ignored as a logging concern.
- Best-effort logging failure should not replace the original operation error.
- Large log reads should remain bounded in memory and response size; the current
  implementation bounds response text but still reads the whole file, so avoid
  extending that pattern to larger files.
- Use explicit UTC timestamps for application-generated heartbeat/status lines.

## Avoid

- Logging `Debug` representations of config/provider types that contain keys.
- Assuming a redacted public config makes all nested URLs safe.
- Compiling new regexes on every high-frequency log call when a reusable helper
  can own them.
- Adding arbitrary user-provided log filenames.
- Reporting a logging write as successful without checking the actual result
  when the log is required for the product's diagnostics contract.

# Sidecar and Process Guidelines

> Resolution and subprocess rules for yt-dlp, streamlink, ffmpeg/ffprobe, and
> whisper.cpp executables.

## Resolution Order

`sidecar` resolves tools in this order:

1. Bundled resource paths.
2. User-configured executable paths.
3. System `PATH`.
4. Missing.

Preserve source/path/version information in `SidecarStatus` so the settings UI
can show the executable actually used. whisper.cpp aliases currently include
`whisper-cli`, `whisper-cpp`, and `main`.

Code support for a bundled location does not prove the Tauri bundle contains the
binary. Keep `tauri.conf.json` resource/externalBin declarations and packaged
artifact verification aligned before claiming a tool is bundled.

## Process Construction

- Use `std::process::Command` with separate argv values.
- Never build a shell command string from URLs, paths, model names, templates,
  or other user/config input.
- On Windows, use the existing no-console behavior where appropriate.
- Keep tool-specific argument construction in the owning pipeline module.
- Record the resolved tool and version without exposing credentials.

## Output Handling

Long-running or verbose children must drain stdout/stderr concurrently so pipe
buffers cannot deadlock the process. Stream useful output into the step log and
parse only bounded progress data in memory.

Check exit status explicitly. A non-empty first output line is not proof that a
version probe or command succeeded. Include redacted stderr context in failures
without returning unbounded tool output.

## Timeout, Cancellation, and Cleanup

Every new subprocess integration must make these decisions explicit:

- Startup and total timeout.
- User cancellation support.
- Child kill and wait/reap behavior.
- Pipe-reader thread completion.
- Temporary file cleanup.
- Partial artifact retention.
- Application shutdown behavior.

Live recording currently has a stop flag and bounded reconnect handling. Most
other tools do not have end-to-end cancellation or timeout. Do not expose a stop
button or claim cancellation until the child process, Job state, runner cleanup,
and UI event flow all support it.

## Trust and Environment

Configured paths and `PATH` entries are executable trust boundaries. Before
executing a newly resolved path, verify at least that it is the expected file
type and report its source. Do not treat existence alone as identity or safety.

Children currently inherit the application environment, which may contain API
keys. New process integrations should pass the smallest necessary environment
where practical, especially for user-configured binaries.

## Network and Update Side Effects

Distinguish read-only probing from download/update operations. The existing
`check_yt_dlp_update` command executes `yt-dlp -U` and can modify a binary; it is
an update action despite its name. New commands and UI copy must disclose:

- Network access.
- Binary or file modification.
- Required permissions.
- Whether a bundled or user-configured executable can be changed.

Do not trigger updates as part of ordinary version probing or app startup.

## Pipeline-Specific Contracts

- Download consumes yt-dlp output while parsing bounded progress.
- Live recording uses streamlink to resolve a stream and ffmpeg to segment it;
  reconnect count, heartbeat, disk guard, and user stop must remain distinct.
- Transcription extracts 16 kHz mono audio with ffmpeg and then runs the
  configured whisper executable/model/language.
- Transcript media merge and duration probing use ffmpeg/ffprobe.
- Summary uses blocking HTTP through reqwest, not a sidecar.

Do not add platform-specific source parsing to downstream transcription or
summary modules. All ingest paths must converge on the shared Job media layout.

## Verification Checklist

- [ ] Bundled/configured/PATH precedence is covered by focused tests.
- [ ] Packaged resources are checked when bundled behavior changes.
- [ ] Arguments never pass through a shell.
- [ ] stdout/stderr are drained and bounded.
- [ ] Exit status, timeout, cancellation, and cleanup are explicit.
- [ ] Child environment and logged output are reviewed for secrets.
- [ ] Real target-environment execution is reported separately from unit tests.

# Job State and Pipeline Guidelines

> Contracts for Job state derivation, step execution, retries, segments, and
> downstream artifacts.

## Job Is a Shared Schema

`models::job::Job` is simultaneously:

- The in-memory domain model.
- The `source.json` persistence schema.
- A Tauri command response.
- The full payload of the `job-updated` event.

A field change therefore requires persistence compatibility, Rust logic,
TypeScript mirrors, event consumers, and UI rendering to be reviewed together.

## Serialized Enums

`JobKind`, `JobStatus`, `StepStatus`, `JobStep`, `SegmentStatus`, and
`MediaSaveMode` serialize as snake_case. Keep TypeScript unions and exhaustive
label/style maps in sync. Do not use display labels as persisted values.

### MediaSaveMode (ingest product)

`JobSource.media_save_mode` is an **exclusive** final-product choice:

- `video` (default) — keep a video container under `media/` (may still contain
  an audio track inside the container).
- `audio` — keep only a standalone audio file; never leave a full video as the
  final `media/` artifact.

Do **not** model this as two combinable booleans. Omitted field on older
`source.json` must deserialize as `video` (`#[serde(default)]` +
`Default::default()`).

Audio-mode ingest must not “download full video to `media/`, then convert”:

| Path | Audio-mode approach |
|------|---------------------|
| yt-dlp | Direct audio format (`-f ba/b`, `-x`, `--audio-format m4a`) |
| Douyin | Same video `play_url` into ffmpeg with `-vn` (and explicit muxer when needed) |
| Live | ffmpeg maps audio only (`0:a:0`), audio segment extension |

Transcribe’s temporary 16 kHz WAV extract is an internal pipeline step, not the
user-facing “save audio” product.

Some existing variants such as `Cancelled` and `Skipped` have limited or no
production paths. Do not invent behavior from the enum name; trace actual
transitions before using one, and complete the transition contract if new code
starts producing it.

## Required Steps and Derived Status

The pipeline order is:

```text
ingest
  -> transcribe (when auto_transcribe or auto_summarize)
  -> merge_transcript
  -> summarize (when auto_summarize)
```

`Job::required_steps()` owns which steps count for the configured pipeline.
`Job::derived_status()` owns aggregate status:

1. Any required running step -> running.
2. Otherwise any required failed step -> failed.
3. All required steps succeeded or skipped -> succeeded.
4. Otherwise -> pending.

Do not assign top-level status independently from step state except at a clearly
defined recovery boundary. Keep these methods as the single transition table.

`progress` currently represents the active step's percentage, not whole-pipeline
progress. UI or backend changes must not silently reinterpret it.

## Runner Contract

`pipeline::runner::RunnerState` provides:

1. Same-Job exclusion (`running_job_ids`).
2. A global FIFO wait queue for work that cannot start yet.
3. Concurrency limits from config: `max_concurrent_jobs` (default 2) and
   `max_live_records` (default 1). Live-record full-run / ingest work also
   claims a live slot for the whole run (`live_slot_holders`).

When no free slot exists, the scheduler sets `JobStatus::Queued`, persists,
emits `job-updated`, and enqueues FIFO. On each job end, `pump_queue` starts
as many runnable queued jobs as slots allow. Create, run, step retry, and
segment retry all enter this path.

`Queued` is scheduler-owned; `Job::derived_status()` never invents it. On app
startup, `recover_interrupted_jobs` maps stale `Running` → `Failed` and
`Queued` → `Pending` (in-memory queue is not durable).

List items may include 1-based `queue_position` when status is queued and the
job is present in the in-memory queue.

Starting a background command means the run was **accepted** (running or
queued), not completed. The runner persists state and emits `job-updated` as
work progresses.

Use a cloned config snapshot for a run so Provider, sidecar, and workspace
settings do not change halfway through a step. Workspace switching must remain
blocked while Jobs are running.

## Step Execution

For every step:

1. Validate prerequisites and identify the owned log/artifact paths.
2. Invalidate current/downstream state using the model contract.
3. Persist running state before expensive side effects.
4. Remove only artifacts made invalid by the transition.
5. Execute the owning pipeline module.
6. Persist success or a redacted actionable failure.
7. Emit a full latest Job snapshot.

Keep sidecar arguments in the step module; keep state sequencing in the runner.

## Retry and Invalidation

`Job::invalidate_after_step()` and `pipeline::paths::remove_downstream_artifacts`
must remain aligned. Typical dependencies are:

- Ingest invalidates transcribe, merged transcript, and summary outputs.
- Transcribe invalidates merged transcript and summary outputs.
- Merge transcript invalidates summary output.
- Summarize has no downstream step.

A retry currently may remove old downstream results before new work succeeds.
Do not expand this behavior casually. If old usable artifacts must survive a
failed retry, design a publish/rollback boundary instead of changing only Job
flags.

## Segments

- Transcript segments have stable IDs and explicit status.
- Merge sorts by segment index and uses selected segment IDs in timeline order.
- Merge must reject selected segments that did not transcribe successfully.
- A selection change invalidates merged transcript and summary state and files.
- Summarization reads the merged `transcript/plain.txt`; it does not summarize
  each segment independently.
- Context overflow fails with an actionable message; do not silently truncate or
  introduce map-reduce without changing the product contract.

## Live Recording

Use `RecordTermination`-style descriptive outcomes for normal end, user stop,
reconnect exhaustion, disk protection, and tool failure. Preserve completed
segments on interruption. User stop with captured media and operational failure
are not automatically the same state.

When `media_save_mode` is `audio`, record audio-only segments (e.g.
`segment_%03d.m4a`) by mapping the first audio stream; do not capture full video
segments then post-convert. Merge output should match the segment family (e.g.
`merged.m4a` for audio-only segments).

Only live recording currently has a stop flag. Do not present download,
transcribe, merge, or summarize as cancellable until process and state cleanup
are implemented end to end.

## Verification Checklist

- [ ] Required step and derived-status unit tests cover the new transition.
- [ ] State invalidation and filesystem cleanup change together.
- [ ] Retry failure behavior for existing artifacts is understood.
- [ ] Event payload and TypeScript labels/styles include new enum values.
- [ ] Startup recovery handles any new running state.
- [ ] Partial media/transcript behavior is documented and tested.

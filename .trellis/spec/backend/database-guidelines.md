# Workspace and Persistence Guidelines

> This project does not use a database, ORM, or migration framework. Persistence
> is JSON plus media/text artifacts in the application config and workspace.

## Sources of Truth

- System config directory: `video-tool/config.json` stores `AppConfig`.
- Workspace: `workspace/jobs/<uuid>/source.json` stores the complete `Job`.
- Job artifact directories store media, transcript, summary, and step logs.
- In-memory config and UI snapshots are caches; disk data is the restart source.

The filename `source.json` is historical: it contains source metadata, pipeline
options, steps, status, progress, tool metadata, artifact references, errors,
and timestamps.

## Job Directory Contract

```text
workspace/jobs/<job_id>/
├── source.json
├── media/
├── transcript/
│   ├── segments/
│   ├── plain.txt
│   ├── raw.json
│   └── srt.srt
├── summary/
│   ├── summary.md
│   └── meta.json
└── logs/
```

Use `WorkspacePaths` and helpers in `workspace`/`pipeline::paths`; do not
duplicate these names or invent a parallel layout for a new source type.

## Path Safety

- External `job_id` values must pass `Uuid::parse_str` through
  `workspace::validated_job_dir` before joining paths.
- Keep stored artifact references relative to the validated Job directory.
- For any newly consumed path read from `source.json`, normalize/canonicalize it
  and prove it remains inside the Job directory before reading or deleting.
- Reject export destinations inside the source Job directory.
- Do not use user strings as log filenames; keep the command-layer whitelist.

## Atomic Writes

Use `storage::write_json_atomically` for config and Job JSON. It serializes
pretty JSON and delegates to same-directory atomic replacement with flush and
sync behavior, including the Windows `ReplaceFileW` path.

Do not replace this with direct `fs::write` for metadata that must survive an
interrupted update. Also do not overstate the guarantee: the helper is not a
multi-file transaction, schema migration system, checksum, backup, or
multi-process compare-and-swap mechanism.

Transcript, summary, and log artifacts are currently written directly. Changes
requiring crash-safe publication must design a temp-file/rename boundary rather
than assuming the JSON helper automatically covers every artifact.

## Compatibility and Schema Changes

There is no schema version or general migration framework. Therefore every new
persisted field needs an explicit compatibility decision:

1. Can old files omit it? Add a Serde default or custom deserializer.
2. Can new files be read by the current UI mirror?
3. Does the field contain a path or secret requiring extra validation?
4. Does loading an old config/Job preserve behavior or need a one-time rewrite?
5. Does export include or redact it?

Do not rely on Serde ignoring unknown fields as a complete migration strategy.

## Listing, Delete, and Recovery

- Job listing reads each `source.json` and sorts by creation time descending.
- Invalid Job files are currently skipped after diagnostic output; new code must
  not silently turn recoverable corruption into permanent data loss.
- Job deletion must refuse active Jobs and remove only a validated Job tree.
- Startup recovery marks persisted running Jobs as failed so interrupted work is
  visible and retryable.
- Keep Job metadata and artifact cleanup ordering explicit; file persistence is
  not transactional across multiple paths.

## Avoid

- Describing file operations as database transactions.
- Introducing an ORM/migration abstraction without a real database decision.
- Storing absolute Job-internal artifact paths when relative paths suffice.
- Writing API keys into `source.json`, summary metadata, or exported Job data.
- Deleting downstream artifacts without matching Job-state invalidation.

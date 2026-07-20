# PRD: Download executor + job runner + polished UI

## Goal

Move from skeleton (job directory only) toward PRODUCT_SPEC v0.1 milestone 2 start:
real ingest for download/import, job runner, logs, retry, and a polished task-center UI.

## In scope

- yt-dlp download executor writing into `jobs/<id>/media/`
- Local import: copy/link media into job media dir
- Job runner: update status/step/logs/source.json
- IPC: run job, retry ingest, read logs, open dir, save config basics
- Frontend: modern task center, job detail, create flow, settings (editable basics)
- Live record remains stub (create job + clear “not implemented” failure path)

## Out of scope (this task)

- Live segment recording implementation
- Real local transcription / merge / summarize HTTP
- Tray keep-alive / export package

## Acceptance

- Create download job → run → media file + download.log + status succeeded/failed
- Create import job → run → media present
- Failed download shows readable error + log path context
- UI: list/search/detail/retry/logs; settings show sidecars and editable workspace defaults
- `pnpm typecheck` and `cargo check` pass

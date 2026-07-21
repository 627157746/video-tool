# v0.2 P3 multi-template + cross-job search

## Goal

- Summarize with an ordered list of templates; primary → `summary/summary.md`, others → `summary/by_template/<id>.md`.
- Local full-text search over transcripts and summaries under `workspace/index/`.

## Acceptance

- [x] `template_ids` on Job (compat with single `template_id`)
- [x] Multi-template summarize + meta.json; partial failure keeps successes
- [x] SQLite FTS index rebuild / incremental upsert; search IPC
- [x] UI: multi-template pick + topbar search
- [x] PRODUCT_SPEC P3; tests / typecheck / clippy

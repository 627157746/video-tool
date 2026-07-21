# v0.2 P1 batch URL + batch_id

## Goal

Implement PRODUCT_SPEC §14.3.1: multi-line URL create for downloads — one Job per URL, shared create options, optional `batch_id`, execution only via global queue.

## Requirements

- Download create accepts multi-line paste (one URL / entry per line when multiple URL-like lines).
- Preserve single multi-line Douyin share paste as **one** job when only one URL-like line exists.
- Each URL becomes an independent Job; shared group / pipeline / auto_start / title optional prefix.
- Generate one `batch_id` (UUID) for the batch; store on each Job; list filterable by batch.
- Create only; global queue runs them (no unbounded parallel spawn).
- Single-job failure does not block siblings.
- Non-goals: playlist deep parse, parent/child Jobs, batch entity store.

## Acceptance Criteria

- [x] Multi-line with 2+ URLs creates N jobs sharing `batch_id`.
- [x] Single URL / single share paste still creates one job (`batch_id=null`).
- [x] Jobs enter queue when concurrent slots full (reuse global queue).
- [x] List can filter/search by batch.
- [x] `pnpm typecheck` / `pnpm build` / Rust tests (49) + clippy pass.

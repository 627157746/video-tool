# Code Reuse and Change Impact Guide

> Search for the existing owner before introducing a second implementation.

## Project Owners to Reuse

| Concern | Existing owner |
|---------|----------------|
| Tauri command names and payload construction | `src/api.ts` |
| Cross-layer TypeScript data mirrors | `src/types.ts` |
| UI labels for Job enums | `src/App.tsx` exhaustive `Record` maps |
| Job required steps and aggregate state | `Job::required_steps`, `Job::derived_status` |
| Downstream state invalidation | `Job::invalidate_after_step` |
| Downstream file cleanup | `pipeline::paths::remove_downstream_artifacts` |
| Validated Job directory construction | `workspace::validated_job_dir` |
| Atomic JSON publication | `storage::write_json_atomically` |
| Step log names and secret redaction | `pipeline::logs` |
| Sidecar lookup precedence | `sidecar` |
| Pipeline sequencing and events | `pipeline::runner` |
| Config candidate validation/public views | `config` |

## Search Before Adding

Search the repository before adding:

- A command wrapper, request DTO, enum variant, or label.
- A Job/artifact path or log filename.
- A JSON write helper or UUID/path check.
- A redaction regex or secret list.
- Sidecar resolution, Windows console suppression, process-output handling, or
  endpoint construction.
- Dialog focus, stale-request, listener cleanup, or settings-ID reconciliation.

The correct result may be reuse, extension, or a new owner. Do not force a
shared abstraction when one clear use is simpler, but do not create a third
copy of non-trivial behavior.

## Paired Mechanisms

Some duplicated concepts intentionally exist in different layers and must be
kept synchronized rather than merged:

- Rust Serde types <-> `src/types.ts`.
- Rust direct argument names <-> `src/api.ts` camelCase invoke keys.
- Job enum values <-> UI label and CSS state maps.
- State invalidation <-> artifact cleanup.
- Tauri plugin package <-> Rust plugin initialization <-> capability permission.
- Bundled sidecar lookup <-> Tauri bundle resources <-> packaged output.
- Config private model <-> public webview view.

For these pairs, add a cross-layer test/checklist rather than pretending one
language or build step automatically updates the other.

## Abstraction Threshold

Extract when at least one is true:

- Complex behavior is already repeated.
- A contract has several consumers and one owner prevents drift.
- Cleanup/error/privacy behavior must be uniform.
- A component region has its own props and lifecycle.

Keep local when all are true:

- It has one use.
- It is simple and self-explanatory.
- Extraction would hide ownership or side effects.
- No contract or invariants need sharing.

## High-Risk Change Searches

### New Job enum or step

Search Rust matches, required steps, derived status, invalidation, artifact
cleanup, TypeScript unions, labels, action visibility, logs, and CSS classes.

### New config field

Search defaults, `SaveConfigRequest`, candidate update, validation, Serde
compatibility, public view, frontend draft, save payload, dirty check, logs,
exports, and product spec.

### New sidecar or process

Search resolver status, configured paths, version probe, settings UI, process
argv, output draining, Windows behavior, timeout/cancel, logs, packaging, and
real-environment validation.

### New artifact or log

Search layout creation, Job relative reference, invalidation/cleanup, read
command whitelist, UI tab, export inclusion/redaction, delete, and recovery.

## Completion Checklist

- [ ] Existing owner and all references were searched.
- [ ] One module owns each new invariant.
- [ ] Intentional cross-language mirrors were updated together.
- [ ] State transition and filesystem cleanup remain paired.
- [ ] Repeated error/redaction/process/UI lifecycle logic was not copied.
- [ ] Validation covers the contract rather than only the new helper.

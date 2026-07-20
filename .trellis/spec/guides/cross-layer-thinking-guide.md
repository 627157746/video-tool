# Cross-Layer Tauri Flow Guide

> Map data, side effects, persistence, notification, and UI consumption before
> changing a feature that crosses the webview/Rust boundary.

## Canonical Flows

### Job creation and execution

```text
React form draft
  -> src/api.ts request
  -> Tauri command boundary
  -> create Job + workspace layout
  -> optional RunnerState background start
  -> step sidecars/providers + artifacts
  -> atomic source.json state
  -> full job-updated event
  -> guarded React snapshot merge
```

### Settings update

```text
React settings draft
  -> SaveConfigRequest
  -> clone current AppConfig
  -> apply + preserve secrets + validate candidate
  -> prepare workspace + atomic config.json
  -> AppConfigPublic
  -> replace frontend persisted snapshot and drafts
```

### Summary

```text
selected successful transcript segments
  -> transcript/plain.txt
  -> template expansion + context limit
  -> Provider protocol request (text only)
  -> summary.md + meta.json
  -> Job state + event
```

## Boundary Questions

For every arrow, answer:

1. What exact serialized shape crosses it?
2. Which layer validates syntax, references, paths, and business rules?
3. Is the operation synchronous, background, cancellable, or retryable?
4. What persists before and after the side effect?
5. What happens after crash, event loss, stale response, or partial output?
6. What private data enters files, processes, network, logs, errors, or export?

## Change Matrices

### Command change

- Rust request/response and handler.
- `lib.rs` registration.
- `src/api.ts` command string and direct/envelope casing.
- `src/types.ts` mirror.
- UI busy/error/stale-request behavior.
- Runtime command smoke.

### Job field or enum change

- Serde representation and old `source.json` compatibility.
- State derivation, invalidation, recovery, and tests.
- Event full-payload contract.
- TypeScript union/interface, labels, actions, and CSS.
- Export/redaction impact.

### Config field change

- Default, candidate update, validation, persistence compatibility.
- Public/private view and API-key semantics.
- Frontend draft, dirty check, save payload, and reference reconciliation.
- Provider/proxy/process/log/export privacy impact.

### Pipeline/artifact change

- Shared Job layout and relative path containment.
- Required step, retry, downstream invalidation, and cleanup.
- Log whitelist and UI artifact access.
- Export inclusion/redaction.
- Startup recovery and partial-output behavior.

### Tauri plugin change

- JavaScript dependency and import.
- Rust plugin dependency and initialization.
- Capability permission and target window.
- Desktop runtime and packaged application smoke.

## Consistency Failure Modes

- TypeScript compiles but command was not registered.
- Direct camelCase argument and nested snake_case DTO are confused.
- Rust enum changes but UI labels/CSS silently default.
- Job state is invalidated but old artifact remains, or file is deleted while
  state still advertises it.
- Event snapshot is newer than a delayed refresh and gets overwritten.
- Provider/template ID is renamed while a default reference stays stale.
- A field is added without Serde default and old Jobs disappear from listing.
- Public config redacts API key but leaks a proxy credential or sensitive URL.
- Bundled resolution works in source but the installer does not contain the tool.
- Unit tests pass while the real sidecar/Provider/capability remains unverified.

## Validation Layers

- Frontend validation: immediate usability feedback and draft consistency.
- Rust command/config/domain validation: authoritative typed/business boundary.
- Workspace/path validation: UUID and containment before filesystem access.
- Process/network validation: executable/endpoint status, timeout, exit/HTTP
  result, and redacted diagnostics.
- Persistence verification: restart compatibility and partial-write behavior.
- UI consistency verification: events, polling, request generations, and focus.

Do not remove backend validation because the UI currently supplies controlled
inputs. Do not duplicate Job transition rules in the UI.

## Final Cross-Layer Checklist

- [ ] Complete data/side-effect flow is mapped.
- [ ] One owner is named for each invariant.
- [ ] Rust/TypeScript and state/file mirrors are updated together.
- [ ] Optional/null/default and old-file behavior are explicit.
- [ ] Stale response, lost event, crash, retry, and partial output are considered.
- [ ] Secret/private data paths are reviewed.
- [ ] Automated checks and real runtime checks are reported separately.

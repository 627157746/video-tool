# video-tool Thinking Guides

> Short project-specific checklists for changes with broad or hidden impact.

## Available Guides

| Guide | Use when |
|-------|----------|
| [Code Reuse and Change Impact](./code-reuse-thinking-guide.md) | Adding helpers, constants, fields, steps, paths, log kinds, or similar UI/process logic |
| [Cross-Layer Tauri Flow](./cross-layer-thinking-guide.md) | Changing commands, events, Rust/TypeScript data, config, Job state, artifacts, or plugins |

## Trigger: Search and Reuse

Load the reuse guide when:

- A new path, log filename, label map, sidecar argument helper, or redaction rule
  resembles existing code.
- A serialized enum/config field must be updated in multiple files.
- State transition or cleanup logic appears in more than one place.
- A component starts repeating async error, focus, dialog, or request-guard code.

## Trigger: Cross-Layer Flow

Load the cross-layer guide when:

- A Tauri command or plugin capability changes.
- A Job/config field or enum changes.
- A pipeline step changes an artifact path or status.
- `job-updated`, list refresh, polling, or detail loads change.
- Provider, proxy, API key, URL, local path, log, or export behavior changes.

## Review Evidence Rule

Verify every finding against the real data source and trust boundary:

- Webview/user/config input is not the same as an app-generated internal value.
- A locally editable `source.json` should not automatically be treated as
  cryptographically trusted.
- A bundled-path resolver does not prove the binary is packaged.
- TypeScript compile success does not prove Rust/Serde payload compatibility.
- Unit tests do not prove sidecar, Provider, Tauri runtime, or installer behavior.
- Existing implementation debt is not automatically the preferred convention.

## Before Any Broad Change

1. Search for the value, enum, command name, path, and serialized field.
2. Identify one owner for the contract.
3. List all mirrors and consumers.
4. Decide persistence and privacy impact.
5. Run the smallest reliable automated and runtime checks.

Add a new guide only for a recurring project-specific class of mistake. Keep
Trellis framework internals and examples from unrelated repositories out of
this directory.

# Configuration and Security Guidelines

> Configuration updates, public views, secret handling, and trust boundaries.

## Storage and Ownership

`config::AppConfig` is stored in the system config directory as `config.json`.
The workspace is stored separately so exporting or backing up media does not
implicitly include Provider keys.

The backend is the final owner of defaults, validation, persistence, and secret
resolution. The UI edits a complete candidate through `SaveConfigRequest` and
receives `AppConfigPublic` after success.

## Candidate Update Pattern

Configuration writes follow this sequence:

1. Clone the current in-memory config.
2. Apply the request to the candidate.
3. Preserve existing Provider secrets according to the current request contract.
4. Validate the complete candidate and all cross-references.
5. Prepare/recover the candidate workspace if required.
6. Atomically persist the candidate.
7. Replace in-memory config and return a public view.

Do not mutate live config before validation and persistence succeed. Do not split
related Provider/template/default updates into independently inconsistent writes.

## Validation Boundary

Keep validation in Rust for:

- Workspace and configured path constraints.
- Numeric ranges for recording, retries, and context length.
- Provider/template non-emptiness and unique IDs.
- Default IDs referencing members of the same candidate.
- Provider protocol and base URL.
- Proxy URL scheme.
- HTTP extra-header names and values.
- Pipeline implications such as summarize requiring transcript production.

Frontend constraints improve UX but are never authoritative for filesystem,
network, subprocess, or persisted data.

## API Key Resolution

`resolve_api_key` gives a configured environment variable precedence over the
stored key. Keep this precedence stable unless product configuration changes.

The save contract currently preserves an existing key when the same Provider
ID is submitted without a replacement. Absence, null, and empty string do not
form a complete three-state clear/retain/set API. Before adding a "clear key"
feature, define that behavior explicitly in Rust and TypeScript.

## Public Views

`AppConfigPublic` and `ProviderProfilePublic` are security boundaries:

- Never return stored `api_key` values to the webview.
- Expose `has_api_key` for UI state.
- Redact sensitive extra-header values.
- Review proxy URLs for embedded credentials before returning or logging them.
- Do not derive public views with blanket serialization of private config.

Job `source.json`, summary metadata, logs, errors, and export ZIPs must not carry
API keys or sensitive headers.

## Provider Network Boundary

Summarization sends text, not media, but may include title, source information,
system prompt, user template, and transcript. Treat all of it as private user
content.

Custom Provider base URLs and proxies are powerful settings. If plain HTTP is
allowed for local gateways, distinguish loopback use from remote cleartext risk
and provide an explicit warning rather than assuming credentials are protected.

Do not send a value from an arbitrary environment variable to a custom endpoint
without preserving the project's trusted-local-user threat model. Any change
that makes webview/config input less trusted requires a broader security design.

## Tauri and Webview Capabilities

- Keep plugin dependencies, Rust plugin initialization, and capability
  permissions synchronized.
- Grant only permissions used by the main window.
- `app.security.csp` is currently `null`; treat that as a known security gap,
  not a recommended Tauri configuration.
- New webview/network features must assess CSP and command exposure together.

## Local Secrets at Rest

Stored API keys and sensitive headers currently reside in plaintext
`config.json`; no OS keychain integration is implemented. Never describe them as
encrypted. Avoid widening access, copying them into workspace data, or printing
config `Debug` output. A keychain migration is a separate product/storage design
with compatibility and fallback requirements.

## Review Checklist

- [ ] Public views contain no complete key, auth header, or proxy password.
- [ ] Environment-key precedence and retain/set/clear semantics are explicit.
- [ ] Candidate validation runs before persistence and in-memory replacement.
- [ ] Provider/template defaults remain referentially valid.
- [ ] HTTP/proxy endpoint changes account for private transcript data.
- [ ] Tauri plugin package, initialization, and capability permissions align.
- [ ] Logs, errors, Job metadata, and exports are reviewed for new secrets.

# Cross-Layer Data Contracts

> Serde and TypeScript mirror rules for commands, events, and persisted data.

## Source and Mirror

Rust Serde types define the runtime serialized shape. `src/types.ts` is a
hand-maintained TypeScript mirror for the webview. There is no code generation
or runtime TypeScript schema validation, so edits require deliberate search and
cross-layer verification.

`Job` has especially broad impact because the same shape is persisted to
`source.json`, returned by commands, and emitted as an event.

## Naming

- Rust struct fields serialize as snake_case unless annotated otherwise.
- Job-related enums use `#[serde(rename_all = "snake_case")]`.
- TypeScript serialized object fields preserve snake_case.
- TypeScript function names, local variables, and direct Tauri argument names
  use camelCase.
- Display labels are UI mappings, never serialized domain values.

## Closed Values

Represent Rust enums as TypeScript string unions and use exhaustive mappings for
labels and visual states.

`MediaSaveMode` is `"video" | "audio"` (snake_case serialization). It is exclusive
(not two booleans). Create requests may omit it (default `video`); `Job.source`
should expose it for detail UI. When adding a variant, search:

- Rust pattern matches and state derivation.
- Serde tests/defaults.
- TypeScript unions.
- `Record<Union, ...>` label maps.
- Conditional rendering and action availability.
- Dynamic CSS classes.
- Persisted old-file compatibility.

Do not silently map an unknown Provider protocol or Job enum to an existing
default. That hides contract drift.

## Optionality and Null

Rust `Option<T>` does not automatically provide distinct absent, null, empty,
retain, and clear meanings. For every optional request field, document:

- Omitted input behavior.
- Explicit null behavior.
- Empty string/collection behavior.
- Existing persisted value behavior.

Keep TypeScript `?` and `| null` aligned with actual Serde and update logic.
Avoid truthiness checks when zero or empty values are valid domain inputs.

## Persistence Compatibility

For a new Job/config field:

1. Decide whether old JSON can omit it.
2. Add `#[serde(default)]`, a default function, or a migration when needed.
3. Confirm the default preserves product semantics.
4. Confirm TypeScript can render both old/default and new values.
5. Review export and redaction behavior.

There is currently no schema version or complete migration framework. Unknown
fields being ignored is not sufficient proof of forward/backward compatibility.

## Public and Private Shapes

Private config and public IPC types are intentionally separate:

- `AppConfig`/`ProviderProfile` may contain stored secrets.
- `AppConfigPublic`/`ProviderProfilePublic` must not expose them.
- `has_api_key` communicates presence without revealing value.
- Sensitive extra-header values remain redacted.
- Input types carry only values intentionally submitted for the current update.

Never simplify this boundary by returning persistence types directly.

## Dates and Snapshot Ordering

Rust emits Chrono UTC timestamps as ISO strings. Frontend list and event merging
currently compare these strings lexically. Keep one canonical UTC serialization
format or change all ordering logic together.

## Errors

`AppError` currently serializes as a string. This is a known contract limitation,
not a structured API. If clients need categories, introduce a typed envelope
with stable code/category/message/retryability fields across both languages.

## Verification Checklist

- [ ] Rust serialization and TypeScript mirror match field-for-field.
- [ ] Enum values, labels, branches, and styles are exhaustive.
- [ ] Optional/null/empty semantics are tested at the command boundary.
- [ ] Old persisted JSON behavior is covered by default/migration tests.
- [ ] Public responses contain no private config fields.
- [ ] Timestamp representation preserves snapshot ordering.
- [ ] `pnpm typecheck`, Rust tests, and a Tauri round-trip smoke are run.

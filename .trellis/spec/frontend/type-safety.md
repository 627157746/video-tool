# Type Safety

> TypeScript conventions and the manual Rust/TypeScript contract boundary.

## Compiler Baseline

`tsconfig.json` uses strict TypeScript with `noUnusedLocals`,
`noUnusedParameters`, `noFallthroughCasesInSwitch`, `isolatedModules`, and
`noEmit`. Do not weaken these options to make a change compile.

The repository does not currently use a runtime schema library. Rust command
deserialization and validation are the final runtime boundary; TypeScript still
must accurately model the data it consumes.

## Type Organization

- Put persisted/domain/IPC mirrors in `src/types.ts`.
- Keep component props and one-file view models beside their component.
- Use descriptive interfaces for object shapes and string unions for closed
  serialized enums.
- Use `Record<Union, Value>` for labels and other exhaustive enum mappings, as
  demonstrated by Job kind/status/step label maps in `src/App.tsx`.
- Keep UI identifiers and function parameters camelCase.
- Preserve Rust/Serde snake_case inside nested request DTOs and serialized
  domain objects. See `../tauri-ipc/data-contracts.md`.

## Public and Input Types

Secrets require separate read and write shapes:

- `ProviderProfilePublic` never carries the stored `api_key`; it exposes
  `has_api_key` instead.
- `ProviderProfileInput` carries only a key entered for the current save and
  supports the backend's key-preservation semantics.
- Public extra headers must remain redacted when sensitive.

Do not merge public and persistence types for convenience. A type that reaches
the webview is part of the privacy boundary.

## Optional and Nullable Values

Before changing `field?: T | null`, confirm the Rust `Option<T>` semantics and
the intended distinction among omitted, null, empty string, and retained prior
value. Current configuration updates do not support every three-state meaning;
document or redesign the command before relying on one.

Use explicit null checks for domain absence. Avoid truthiness when `0`, an empty
string, or an empty collection is a meaningful input.

## Error Values

Tauri currently rejects application commands with serialized strings. Convert
unknown caught values for display, but do not infer error categories from
Chinese message text. If UI behavior needs error categories, first define a
stable cross-layer error envelope.

## Avoid

- `any`, broad type assertions, or `as unknown as ...` to bypass an IPC mismatch.
- Inline command payload types duplicated in multiple components.
- Local casts of the same event or response fields in multiple consumers.
- A catch-all default branch for a closed Rust enum that would hide a new value.
- Returning stored secrets in public types.
- Assuming TypeScript types validate JSON at runtime.

## Cross-Layer Change Rule

For any field or enum change, check the Rust model, Serde naming/defaults,
Tauri command signature, command registration, `src/api.ts`, `src/types.ts`, UI
label mappings, state merging, and dynamic CSS classes. TypeScript compilation
alone cannot prove the hand-maintained Rust mirror is correct.

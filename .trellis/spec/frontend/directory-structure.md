# Frontend Directory Structure

> Current ownership boundaries for the React/Tauri frontend.

## Current Layout

```text
src/
├── main.tsx       # React root and StrictMode
├── App.tsx        # Application shell, task center, settings, dialogs
├── App.css        # Global design tokens, layout, states, responsive rules
├── api.ts         # Typed wrappers for application-specific Tauri commands
├── types.ts       # TypeScript mirrors of Rust IPC and persisted domain data
├── vite-env.d.ts  # Vite ambient types
└── assets/        # Bundled frontend assets, when needed
```

Related boundaries:

- `package.json` owns pnpm scripts and frontend dependencies.
- `vite.config.ts` owns Tauri-aware Vite host, port, and HMR behavior.
- `src-tauri/tauri.conf.json` owns the matching dev URL and build commands.
- `src-tauri/capabilities/default.json` owns plugin permissions.

## File Ownership

### `main.tsx`

Keep this file limited to application mounting and root providers. The current
entry point uses `ReactDOM.createRoot` and `React.StrictMode`.

### `api.ts`

All application-specific `invoke` command names and payload construction live
here. Components call exported wrappers instead of repeating command strings.
Tauri plugin APIs such as dialog, event, and opener may be imported directly at
the UI integration boundary.

### `types.ts`

Cross-feature domain and IPC types live here. Component-only props and local
view models stay beside their component. Any edit to an IPC type must be
checked against the Rust Serde source described in `../tauri-ipc/`.

### `App.tsx`

`App.tsx` currently contains most product UI and asynchronous coordination. It
is a historical concentration point, not evidence that every new feature
should be appended there. Keep a small, local change in place when extraction
would add indirection; extract a named component or hook when a region has its
own props, lifecycle, repeated behavior, or independently testable purpose.

`PathPickerField` is the current example of a local reusable component with a
typed props interface.

### `App.css`

Styles are currently global. Reuse root variables and existing state/variant
classes. When extracting a substantial component, make style ownership clear
instead of adding unrelated selectors to arbitrary sections of `App.css`.

## Naming and Imports

- Components and component files use PascalCase.
- Hooks, handlers, helpers, and values use camelCase.
- CSS class names use kebab-case; domain variants use stable serialized values
  such as `status-running` or `kind-live_record`.
- TypeScript source uses ES modules and the automatic JSX transform.
- Prefer direct relative imports in the current small tree; no path alias is
  configured.

## Do Not Invent Structure Prematurely

The repository currently has no `pages/`, `features/`, `stores/`, `services/`,
or shared component hierarchy. Do not introduce a broad directory architecture
for a one-file change. If a feature requires several components, state logic,
and styles, document and apply one coherent feature boundary rather than
creating multiple empty layers.

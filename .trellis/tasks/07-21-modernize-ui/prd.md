# PRD: Modernize app UI visual design

## Goal

Upgrade the desktop task-center UI from a dense, dated dark theme into a polished modern product surface while preserving all existing functionality, accessibility contracts, and IPC behavior.

## Scope

- Visual redesign of layout density, color system, surfaces, buttons, cards, forms, modals, and status chips
- Typography hierarchy and spacing consistency
- Keep React logic, Tauri commands, and data contracts unchanged unless markup needs minor structural wrappers for styling

## Out of scope

- New product features
- CSS frameworks or component libraries
- Routing / state library changes
- Backend / IPC changes

## Acceptance criteria

1. App retains jobs + settings flows and create-job dialogs
2. Existing class names remain usable or are updated consistently in TSX + CSS
3. Focus-visible, dialog, alert/status, progress, and tab accessibility remain intact
4. `pnpm typecheck` passes
5. UI looks modern: clearer hierarchy, refined surfaces, better spacing, less visual noise
6. Theme mode supports system / light / dark, persisted in localStorage
7. Accent color supports multiple palettes and updates UI chrome immediately
8. First paint avoids theme flash via early `index.html` bootstrap script

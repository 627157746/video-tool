# Component Guidelines

> React component, form, styling, and accessibility conventions used by the
> desktop task center.

## Component Shape

- Use function components and the automatic JSX transform; do not introduce
  `React.FC` solely for typing.
- Define props with descriptive interfaces near a component when they are
  local. Move only genuinely shared domain types to `src/types.ts`.
- Destructure props at the component boundary.
- Keep command invocation in event handlers or hooks and use `src/api.ts`
  wrappers; presentation markup must not repeat Tauri command strings.
- Use `void` for intentionally unawaited async UI handlers so fire-and-forget
  intent is explicit.

Reference: `PathPickerField` and `PathPickerFieldProps` in `src/App.tsx`.

## Forms and Collections

- Use controlled inputs for settings and Job creation.
- Preserve draft values separately from the last accepted backend config.
- Update arrays and sets immutably with `map`, `filter`, spreads, or a copied
  `Set`; do not mutate React state in place.
- Normalize identifiers at the save boundary and keep default Provider/template
  references valid during rename and delete actions.
- HTML constraints such as `required` and `min` improve interaction but do not
  replace Rust validation.

## Destructive and Busy Actions

- Destructive operations such as deleting a Job require explicit confirmation.
- Disable or scope actions while the same operation is in flight. Use a local
  boolean or an ID `Set` when operations can run independently; do not expand a
  single global busy flag to block unrelated UI.
- Restore disabled/loading state in `finally` blocks.
- Surface failures through the existing alert region and successes through the
  polite status region.

## Accessibility Contract

Interactive additions must preserve the behaviors demonstrated in `App.tsx`:

- Errors use `role="alert"`.
- Non-urgent success feedback uses `role="status"` and `aria-live="polite"`.
- Progress exposes `role="progressbar"` and min/max/current values.
- Tab sets use `tablist`, `tab`, and `tabpanel` relationships, roving
  `tabIndex`, ArrowLeft/ArrowRight, Home, and End handling.
- Dialogs expose `role="dialog"`, `aria-modal`, and a labelled title.
- Opening a dialog moves focus to a meaningful control; Escape closes it; Tab
  remains inside; closing restores focus; background content is inert.
- Icon-only and ambiguous actions receive contextual `aria-label` text.
- Interactive controls keep visible `:focus-visible` styling.

Reuse behavior, not copied focus-trap code. If a second complex dialog appears,
extract shared dialog behavior rather than duplicating the existing effect.

## Styling

- Reuse CSS variables from `src/App.css` for colors, borders, shadows, and
  typography. Theme surfaces are resolved by `html[data-theme="light"|"dark"]`
  and accent hues by `html[data-accent="..."]` (see `src/theme.ts`).
- Prefer semantic tokens (`--text`, `--panel`, `--accent-soft`, `--btn-primary-to`)
  over hard-coded light/dark hex values in component rules.
- Use kebab-case classes and existing variant composition such as
  `.btn.secondary`.
- Derive state classes from stable domain values only when every enum member has
  a corresponding visual treatment.
- Inline styles are reserved for runtime values that cannot live in CSS (progress
  width, accent swatch color from option data), not general component styling.
- Settings layout should fill the content area on wide windows (`max-width: none`
  on `.panel.settings`); do not reintroduce a fixed settings max-width that
  leaves large empty side gutters fullscreen.
- Settings uses a fixed chrome layout: app topbar + settings title stay put;
  only `.settings-main` (and the left nav when tall) scrolls. Save actions live in
  a fixed bottom-right `.settings-fab`, not in the settings header.
- Respect the current desktop minimum window and responsive breakpoints.

## Avoid

- Parsing error text to drive component behavior.
- Showing a stale log, transcript, or summary from the previously selected Job.
- Treating a visually selected `<option>` as proof that its controlled value is
  still valid.
- Adding a component library, CSS framework, or form library for a single use.
- Copying the current large `App.tsx` or global CSS concentration as a desired
  architecture for new independent features.

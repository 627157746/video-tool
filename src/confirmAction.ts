import { ask } from "@tauri-apps/plugin-dialog";

export interface ConfirmActionOptions {
  /** Dialog window title. Defaults to a generic confirmation title. */
  title?: string;
  /**
   * Visual kind for the native dialog.
   * Destructive actions should use `warning`.
   */
  kind?: "info" | "warning" | "error";
  /** Label for the affirmative button. Defaults to system/localized Yes. */
  okLabel?: string;
  /** Label for the dismiss button. Defaults to system/localized No. */
  cancelLabel?: string;
}

/**
 * Ask the user to confirm a high-impact or destructive action.
 *
 * Uses Tauri's native dialog plugin. Browser `window.confirm` is unreliable in
 * the WebView (often returns without showing UI), so it is only a last resort
 * when the plugin is unavailable (e.g. plain Vite browser preview).
 */
export async function confirmAction(
  message: string,
  options: ConfirmActionOptions = {},
): Promise<boolean> {
  const title = options.title?.trim() || "请确认";
  const kind = options.kind ?? "warning";

  try {
    return await ask(message, {
      title,
      kind,
      okLabel: options.okLabel?.trim() || "确定",
      cancelLabel: options.cancelLabel?.trim() || "取消",
    });
  } catch (error) {
    // Outside the Tauri shell (or if dialog permission is missing), fall back
    // so development still works; prefer native dialog in production.
    console.warn(
      "native confirm dialog failed, falling back to window.confirm",
      error,
    );
    return window.confirm(message);
  }
}

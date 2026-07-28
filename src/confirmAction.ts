export interface ConfirmActionOptions {
  /** Dialog title. Defaults to “请确认”. */
  title?: string;
  /**
   * Visual severity. `warning` / `error` use the destructive confirm button
   * styling so delete/purge actions read as high impact.
   */
  kind?: "info" | "warning" | "error";
  /** Affirmative button label. Defaults to “确定”. */
  okLabel?: string;
  /** Dismiss button label. Defaults to “取消”. */
  cancelLabel?: string;
}

export interface ConfirmRequest {
  message: string;
  options: ConfirmActionOptions;
  resolve: (confirmed: boolean) => void;
}

type ConfirmHostHandler = (request: ConfirmRequest) => void;

let confirmHostHandler: ConfirmHostHandler | null = null;

/**
 * Register the in-app confirm dialog host. Called by `ConfirmDialogHost` on
 * mount/unmount. Only one host is active at a time.
 */
export function registerConfirmHost(handler: ConfirmHostHandler | null): void {
  confirmHostHandler = handler;
}

/**
 * Ask the user to confirm a high-impact or destructive action.
 * Renders through the in-app modal host so styling matches the task center UI.
 * Falls back to `window.confirm` only when no host is mounted (e.g. tests).
 */
export function confirmAction(
  message: string,
  options: ConfirmActionOptions = {},
): Promise<boolean> {
  if (confirmHostHandler) {
    return new Promise<boolean>((resolve) => {
      confirmHostHandler?.({
        message,
        options,
        resolve,
      });
    });
  }

  return Promise.resolve(window.confirm(message));
}

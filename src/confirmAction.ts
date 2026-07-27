/**
 * Ask the user to confirm a high-impact or destructive action.
 * Returns true when the user accepts.
 */
export function confirmAction(message: string): boolean {
  return window.confirm(message);
}

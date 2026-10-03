import { getCurrentWebview } from "@tauri-apps/api/webview";

export const ACCOUNT_SETTINGS_OPEN_EVENT = "gitru:open-account-settings";

/** A UI hint only: the main webview performs every credential operation itself. */
export function requestAccountSettings(): Promise<void> {
  return getCurrentWebview().emitTo(
    { kind: "Webview", label: "main" },
    ACCOUNT_SETTINGS_OPEN_EVENT,
  );
}

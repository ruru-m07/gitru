/**
 * Helper to emit navigation events to embedded tab webviews
 */

import { emit } from "@tauri-apps/api/event";
import {
  createWebviewLabel,
  WEBVIEW_NAVIGATION_EVENT,
  type WebviewNavigationEventPayload,
} from "./navigation-events";

export async function emitWebviewNavigation(
  tabId: string,
  path: string,
  type: "back" | "forward" = "back",
): Promise<void> {
  try {
    const label = createWebviewLabel(tabId);
    const payload: WebviewNavigationEventPayload = {
      type,
      path,
      targetTabId: tabId,
    };

    console.log(`[WebviewNav] Emitting ${type} event to ${label}:`, payload);

    await emit(WEBVIEW_NAVIGATION_EVENT, payload);
  } catch (error) {
    console.error(
      `[WebviewNav] Failed to emit navigation event to tab ${tabId}:`,
      error,
    );
  }
}

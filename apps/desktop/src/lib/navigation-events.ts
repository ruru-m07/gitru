/**
 * Inter-webview navigation event types and constants
 * Used to communicate navigation actions from the main shell to embedded tab webviews
 */

export const WEBVIEW_NAVIGATION_EVENT = "webview-navigation";

export type WebviewNavigationEventPayload = {
  type: "back" | "forward";
  path: string;
  targetTabId: string;
};

export const createWebviewLabel = (tabId: string): string => {
  // Match the format used in WebviewTabHost.tsx
  const sanitized = tabId.replace(/[^a-zA-Z0-9\-/:_]/g, "_");
  return `tab-webview:${sanitized}`;
};

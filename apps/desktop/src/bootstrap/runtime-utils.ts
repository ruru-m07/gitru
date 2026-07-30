import { getCurrentWebview } from "@tauri-apps/api/webview";

import {
  GitViewState,
  RepoFileSelectionState,
  WorkspaceSessionSnapshot,
} from "@/types/store";

export const TAB_RUNTIME_STATE_EVENT = "gitru:tab-runtime-state";
export const TAB_RUNTIME_READY_EVENT = "gitru:tab-runtime-ready";
export const TAB_RUNTIME_REQUEST_SYNC_EVENT = "gitru:tab-runtime-request-sync";
export const TAB_WEBVIEW_LABEL_PREFIX = "tab-webview:";
export const HOST_SHELL_ROUTE = "/app";
export const SNAPSHOT_EMIT_DEBOUNCE_MS = 180;

const DEFAULT_STASH_STATUS_FILTERS: GitViewState["stashStatusFilters"] = {
  modified: true,
  renamed: true,
  deleted: true,
  conflicted: true,
  untracked: true,
};

export const isEmbeddedRuntime = () => {
  if (typeof window === "undefined") {
    return false;
  }

  const searchParams = new URLSearchParams(window.location.search);
  const hasEmbeddedFlag =
    searchParams.get("embedded") === "1" ||
    searchParams.get("embedded") === "true";

  if (hasEmbeddedFlag) {
    return true;
  }

  try {
    const label = getCurrentWebview().label;
    return label.startsWith("tab-webview:");
  } catch {
    return false;
  }
};

export const isDesktopHostRuntime = () => !isEmbeddedRuntime();

export type TabRuntimeStatePayload = {
  tabId: string;
  routePath?: string;
  repositoryId?: string | null;
  title?: string;
  snapshot?: WorkspaceSessionSnapshot | null;
};

export type TabRuntimeReadyPayload = {
  tabId?: string;
};

export const cloneRuntimeSelectionState = (
  value: RepoFileSelectionState | null | undefined,
): RepoFileSelectionState => ({
  worktree: value?.worktree ?? null,
  stashByReference: { ...(value?.stashByReference ?? {}) },
  historyByCommit: { ...(value?.historyByCommit ?? {}) },
});

export const cloneRuntimeGitViewState = (
  value: GitViewState | null | undefined,
): GitViewState => ({
  leftPanelView:
    value?.leftPanelView === "stash" || value?.leftPanelView === "history"
      ? value.leftPanelView
      : "changes",
  changesTab: value?.changesTab === "history" ? "history" : "changes",
  stashViewMode: value?.stashViewMode === "all" ? "all" : "branch",
  selectedStashReference: value?.selectedStashReference ?? null,
  selectedHistoryCommitHash: value?.selectedHistoryCommitHash ?? null,
  stashStatusFilters: {
    ...DEFAULT_STASH_STATUS_FILTERS,
    ...(value?.stashStatusFilters ?? {}),
  },
});

export const sanitizeTabWebviewLabel = (tabId: string) =>
  `${TAB_WEBVIEW_LABEL_PREFIX}${tabId.replace(/[^a-zA-Z0-9\-/:_]/g, "_")}`;

export const getRoutePathname = (routePath: string) => {
  try {
    const origin =
      typeof window !== "undefined"
        ? window.location.origin
        : "http://localhost";
    return new URL(routePath, origin).pathname;
  } catch {
    return routePath.split("?")[0].split("#")[0];
  }
};

export const normalizeWorkspaceRoutePath = (routePath: string) => {
  const pathname = getRoutePathname(routePath);

  if (pathname === "/app" || pathname === "/app/") {
    return "/app/git";
  }

  return routePath;
};

export const stripEmbeddedQueryFromRoutePath = (routePath: string) => {
  try {
    const url = new URL(routePath, window.location.origin);
    url.searchParams.delete("embedded");
    const query = url.searchParams.toString();
    return `${url.pathname}${query ? `?${query}` : ""}${url.hash}`;
  } catch {
    const [pathPart, rawQuery = ""] = routePath.split("?");
    const params = new URLSearchParams(rawQuery);
    params.delete("embedded");
    const query = params.toString();
    return `${pathPart}${query ? `?${query}` : ""}`;
  }
};

export const getTabTitleFromRoute = (routePath: string) => {
  const pathname = getRoutePathname(routePath);

  if (pathname.startsWith("/app/")) {
    const segment = pathname.slice("/app/".length).split("/")[0];
    if (segment) {
      return segment.charAt(0).toUpperCase() + segment.slice(1);
    }
  }

  return "Workspace";
};

export const isGitRoutePath = (routePath: string) =>
  getRoutePathname(routePath).startsWith("/app/git");

export const getEmbeddedTabId = () => {
  if (!isEmbeddedRuntime()) {
    return null;
  }

  try {
    const label = getCurrentWebview().label;
    return label.startsWith(TAB_WEBVIEW_LABEL_PREFIX)
      ? label.slice(TAB_WEBVIEW_LABEL_PREFIX.length)
      : null;
  } catch {
    return null;
  }
};

export const enableDevDiagnostics = () =>
  import.meta.env.DEV && isEmbeddedRuntime();

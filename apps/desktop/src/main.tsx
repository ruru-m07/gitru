import { CommandManagerProvider } from "@gitru/ui/components/command";
import { createRouter, RouterProvider } from "@tanstack/react-router";
import { ThemeProvider as NextThemesProvider } from "next-themes";
import { StrictMode, useCallback, useEffect, useMemo, useRef } from "react";
import ReactDOM from "react-dom/client";
import { scan } from "react-scan";
import { Toaster } from "sonner";

import { colorKeyList } from "./lib/colors.ts";
import {
  TAB_SWITCH_SHORTCUT_EVENT,
  type TabSwitchShortcutPayload,
} from "./lib/tabSwitching";
import { routeTree } from "./routeTree.gen";
import { useLastPageStore } from "./store/useLastPageStore.ts";
import "./app.css";

import { QueryClientProvider } from "@tanstack/react-query";
import { ReactQueryDevtools } from "@tanstack/react-query-devtools";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { TabContextProvider } from "./context/TabContextProvider";
import { appState } from "./state";
import { initializeQueryFocusBridge } from "./state/core/StateManager";
import {
  type GitViewState,
  type RepoFileSelectionState,
  selectActiveRepositoryPath,
  selectActiveSessionRepoKey,
  useAppStore,
  type WorkspaceSessionSnapshot,
} from "./store/useAppStore";

const router = createRouter({
  routeTree,
  context: {},
  defaultPreload: "intent",
  scrollRestoration: true,
  defaultStructuralSharing: true,
  defaultPreloadStaleTime: 0,
});

const isEmbeddedRuntime = () => {
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

const enableDevDiagnostics = import.meta.env.DEV && isEmbeddedRuntime();

const TAB_RUNTIME_STATE_EVENT = "gitru:tab-runtime-state";
const TAB_RUNTIME_READY_EVENT = "gitru:tab-runtime-ready";
const TAB_RUNTIME_REQUEST_SYNC_EVENT = "gitru:tab-runtime-request-sync";
const TAB_WEBVIEW_LABEL_PREFIX = "tab-webview:";
const HOST_SHELL_ROUTE = "/app";
const SNAPSHOT_EMIT_DEBOUNCE_MS = 180;

const DEFAULT_STASH_STATUS_FILTERS: GitViewState["stashStatusFilters"] = {
  modified: true,
  renamed: true,
  deleted: true,
  conflicted: true,
  untracked: true,
};

const cloneRuntimeSelectionState = (
  value: RepoFileSelectionState | null | undefined,
): RepoFileSelectionState => ({
  worktree: value?.worktree ?? null,
  stashByReference: { ...(value?.stashByReference ?? {}) },
  historyByCommit: { ...(value?.historyByCommit ?? {}) },
});

const cloneRuntimeGitViewState = (
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

const sanitizeTabWebviewLabel = (tabId: string) =>
  `${TAB_WEBVIEW_LABEL_PREFIX}${tabId.replace(/[^a-zA-Z0-9\-/:_]/g, "_")}`;

const isDesktopHostRuntime = () => !isEmbeddedRuntime();

type TabRuntimeStatePayload = {
  tabId: string;
  routePath?: string;
  repositoryId?: string | null;
  title?: string;
  snapshot?: WorkspaceSessionSnapshot | null;
};

type TabRuntimeReadyPayload = {
  tabId?: string;
};

const getRoutePathname = (routePath: string) => {
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

const normalizeWorkspaceRoutePath = (routePath: string) => {
  const pathname = getRoutePathname(routePath);

  if (pathname === "/app" || pathname === "/app/") {
    return "/app/git";
  }

  return routePath;
};

const stripEmbeddedQueryFromRoutePath = (routePath: string) => {
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

const getTabTitleFromRoute = (routePath: string) => {
  const pathname = getRoutePathname(routePath);

  if (pathname.startsWith("/app/")) {
    const segment = pathname.slice("/app/".length).split("/")[0];
    if (segment) {
      return segment.charAt(0).toUpperCase() + segment.slice(1);
    }
  }

  return "Workspace";
};

const isGitRoutePath = (routePath: string) =>
  getRoutePathname(routePath).startsWith("/app/git");

const getEmbeddedTabId = () => {
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

if (!isEmbeddedRuntime()) {
  router.subscribe("onResolved", (state) => {
    if (
      isDesktopHostRuntime() &&
      getRoutePathname(state.toLocation.href).startsWith("/app")
    ) {
      useLastPageStore.getState().setLastPage(HOST_SHELL_ROUTE);
      return;
    }

    useLastPageStore.getState().setLastPage(state.toLocation.href);
  });
}

declare module "@tanstack/react-router" {
  interface Register {
    router: typeof router;
  }
}

async function redirectToLastPage() {
  if (isEmbeddedRuntime()) return;

  if (isDesktopHostRuntime()) {
    const currentPathIsApp = window.location.pathname.startsWith("/app");
    const alreadyAtHostShell =
      window.location.pathname === HOST_SHELL_ROUTE &&
      window.location.search.length === 0 &&
      window.location.hash.length === 0;

    if (currentPathIsApp && !alreadyAtHostShell) {
      await router.navigate({ to: HOST_SHELL_ROUTE });
      return;
    }
  }

  const { lastPage } = useLastPageStore.getState();
  if (!lastPage) return;
  if (lastPage === "/") return;

  const hasEmbeddedFlag = (() => {
    try {
      const url = new URL(lastPage, window.location.origin);
      const embedded = url.searchParams.get("embedded");
      return embedded === "1" || embedded === "true";
    } catch {
      return (
        lastPage.includes("embedded=1") || lastPage.includes("embedded=true")
      );
    }
  })();

  if (hasEmbeddedFlag) {
    return;
  }

  if (isDesktopHostRuntime() && getRoutePathname(lastPage).startsWith("/app")) {
    if (
      window.location.pathname !== HOST_SHELL_ROUTE ||
      window.location.search.length > 0 ||
      window.location.hash.length > 0
    ) {
      await router.navigate({ to: HOST_SHELL_ROUTE });
    }
    return;
  }

  if (window.location.pathname + window.location.search !== lastPage) {
    await router.navigate({ to: lastPage });
  }
}

await redirectToLastPage();
initializeQueryFocusBridge();

const rootElement = document.getElementById("root");
if (rootElement && !rootElement.innerHTML) {
  const root = ReactDOM.createRoot(rootElement);

  const AppRouter = () => {
    const selectedRepository = useAppStore((state) => state.selectedRepository);
    const setEmbeddedRuntimeSession = useAppStore(
      (state) => state.setEmbeddedRuntimeSession,
    );
    const syncTabMetadata = useAppStore((state) => state.syncTabMetadata);
    const activeTabId = useAppStore((state) => state.activeTabId);
    const activeRuntimeId = useAppStore(
      (state) => state.activeSessionId ?? state.activeTabId,
    );
    const activeRepositoryPath = useAppStore(selectActiveRepositoryPath);
    const activeRepoStateKey = useAppStore(selectActiveSessionRepoKey);
    const activeGitViewState = useAppStore((state) =>
      activeRepoStateKey ? state.gitViewByRepo[activeRepoStateKey] : null,
    );
    const activeSelectionState = useAppStore((state) =>
      activeRepoStateKey ? state.selectionByRepo[activeRepoStateKey] : null,
    );
    const mainWindowView = useAppStore((state) => state.mainWindowView);
    const captureActiveSessionSnapshot = useAppStore(
      (state) => state.captureActiveSessionSnapshot,
    );

    // Compute derived values once
    const embeddedRuntime = useMemo(() => isEmbeddedRuntime(), []);
    const embeddedTabId = useMemo(() => getEmbeddedTabId(), []);

    const embeddedRuntimeSessionExists = useAppStore((state) => {
      if (!embeddedTabId) {
        return false;
      }

      return (
        Boolean(state.sessionsById[embeddedTabId]) ||
        state.tabs.some((tab) => tab.id === embeddedTabId)
      );
    });
    const isEmbeddedRuntimeBound = useAppStore((state) => {
      if (!embeddedTabId) {
        return false;
      }

      const runtimeId = state.activeSessionId ?? state.activeTabId;
      return runtimeId === embeddedTabId;
    });
    const emitRuntimeStateRef = useRef<((href: string) => void) | null>(null);
    const snapshotEmitTimerRef = useRef<number | null>(null);

    // Memoize snapshot capture function to prevent recreating on every render
    const captureSnapshot = useCallback(() => {
      captureActiveSessionSnapshot();
    }, [captureActiveSessionSnapshot]);

    // Effect 1: Set embedded runtime session
    useEffect(() => {
      if (
        !embeddedRuntime ||
        !embeddedTabId ||
        !embeddedRuntimeSessionExists ||
        isEmbeddedRuntimeBound
      ) {
        return;
      }

      setEmbeddedRuntimeSession(embeddedTabId);
    }, [
      embeddedRuntime,
      embeddedTabId,
      embeddedRuntimeSessionExists,
      isEmbeddedRuntimeBound,
      setEmbeddedRuntimeSession,
    ]);

    // Effect 2: Handle keyboard shortcuts in embedded runtime
    useEffect(() => {
      if (!embeddedRuntime) {
        return;
      }

      const emitTabSwitchShortcut = (payload: TabSwitchShortcutPayload) => {
        void getCurrentWebview().emit(TAB_SWITCH_SHORTCUT_EVENT, payload);
      };

      const handleKeyDown = (event: KeyboardEvent) => {
        if (!(event.ctrlKey || event.metaKey) || event.key !== "Tab") {
          return;
        }

        event.preventDefault();

        emitTabSwitchShortcut({
          phase: "advance",
          backward: event.shiftKey,
          modifier: event.metaKey ? "Meta" : "Control",
        });
      };

      const handleKeyUp = (event: KeyboardEvent) => {
        if (event.key !== "Control" && event.key !== "Meta") {
          return;
        }

        emitTabSwitchShortcut({
          phase: "commit",
        });
      };

      const handleWindowBlur = () => {
        emitTabSwitchShortcut({
          phase: "commit",
        });
      };

      window.addEventListener("keydown", handleKeyDown, true);
      window.addEventListener("keyup", handleKeyUp, true);
      window.addEventListener("blur", handleWindowBlur);

      return () => {
        window.removeEventListener("keydown", handleKeyDown, true);
        window.removeEventListener("keyup", handleKeyUp, true);
        window.removeEventListener("blur", handleWindowBlur);
      };
    }, [embeddedRuntime]);

    // Effect 3: Listen for navigation events from main shell (embedded runtime only)
    useEffect(() => {
      if (!embeddedRuntime) {
        return;
      }

      let unlistenNavigation: (() => void) | undefined;

      const setupNavigationListener = async () => {
        try {
          const { listen } = await import("@tauri-apps/api/event");
          const { WEBVIEW_NAVIGATION_EVENT } = await import(
            "@/lib/navigationEvents"
          );

          unlistenNavigation = await listen<{
            type: "back" | "forward";
            path: string;
            targetTabId: string;
          }>(WEBVIEW_NAVIGATION_EVENT, (event) => {
            const { targetTabId, path } = event.payload;
            console.log("[EmbeddedNav] Received navigation event:", {
              targetTabId,
              embeddedTabId,
              path,
            });

            // Verify this event is intended for this webview
            if (targetTabId !== embeddedTabId) {
              console.log(
                "[EmbeddedNav] Ignoring event - wrong target. Expected:",
                embeddedTabId,
                "Got:",
                targetTabId,
              );
              return;
            }

            // Verify this tab is currently active
            const currentActiveTabId = useAppStore.getState().activeTabId;
            if (embeddedTabId !== currentActiveTabId) {
              console.log(
                "[EmbeddedNav] Ignoring event - not active tab. Current:",
                embeddedTabId,
                "Active:",
                currentActiveTabId,
              );
              return;
            }

            if (path) {
              console.log("[EmbeddedNav] Applying navigation to path:", path);
              void router.navigate({ to: path });
            }
          });

          console.log("[EmbeddedNav] Navigation listener setup complete");
        } catch (error) {
          console.error(
            "[EmbeddedNav] Failed to setup navigation listener:",
            error,
          );
        }
      };

      void setupNavigationListener();

      return () => {
        if (unlistenNavigation) {
          unlistenNavigation();
        }
      };
    }, [embeddedRuntime, embeddedTabId]);

    // Effect 4: Capture state changes only for main window (not embedded)
    useEffect(() => {
      if (embeddedRuntime) {
        return;
      }

      // Capture on page visibility/unload events
      const handleVisibilityChange = () => {
        if (document.visibilityState === "hidden") {
          captureSnapshot();
        }
      };

      window.addEventListener("beforeunload", captureSnapshot);
      window.addEventListener("pagehide", captureSnapshot);
      document.addEventListener("visibilitychange", handleVisibilityChange);

      return () => {
        window.removeEventListener("beforeunload", captureSnapshot);
        window.removeEventListener("pagehide", captureSnapshot);
        document.removeEventListener(
          "visibilitychange",
          handleVisibilityChange,
        );
      };
    }, [captureSnapshot, embeddedRuntime]);

    // Effect 4: Consolidated embedded tab runtime state management and sync
    useEffect(() => {
      if (!embeddedTabId || !embeddedRuntimeSessionExists) {
        emitRuntimeStateRef.current = null;
        return;
      }

      const emitRuntimeState = (href: string) => {
        const runtimeState = useAppStore.getState();
        const runtimeId =
          runtimeState.activeSessionId ?? runtimeState.activeTabId;

        if (runtimeId !== embeddedTabId) {
          return;
        }

        const runtimeSession = runtimeState.sessionsById[runtimeId] ?? null;
        const runtimeRepositoryId = runtimeId
          ? (runtimeSession?.repositoryId ?? null)
          : null;
        const runtimeRepository = runtimeRepositoryId
          ? (runtimeState.repositories.find(
              (repo) => repo.id === runtimeRepositoryId,
            ) ?? null)
          : null;
        const runtimeRepoStateKey = selectActiveSessionRepoKey(runtimeState);
        const runtimeGitViewState = runtimeRepoStateKey
          ? (runtimeState.gitViewByRepo[runtimeRepoStateKey] ?? null)
          : null;
        const runtimeSelectionState = runtimeRepoStateKey
          ? (runtimeState.selectionByRepo[runtimeRepoStateKey] ?? null)
          : null;

        const normalizedRoutePath = normalizeWorkspaceRoutePath(
          stripEmbeddedQueryFromRoutePath(href),
        );
        const isGitTabRoute = isGitRoutePath(normalizedRoutePath);
        const snapshot: WorkspaceSessionSnapshot = {
          repositoryPath:
            runtimeRepository?.path ??
            runtimeSession?.snapshot?.repositoryPath ??
            null,
          mainWindowView: runtimeState.mainWindowView,
          fileSelection: cloneRuntimeSelectionState(runtimeSelectionState),
          gitViewState: cloneRuntimeGitViewState(runtimeGitViewState),
          capturedAt: Date.now(),
        };

        const payload: TabRuntimeStatePayload = {
          tabId: embeddedTabId,
          routePath: normalizedRoutePath,
          snapshot,
          ...(isGitTabRoute
            ? {
                repositoryId: runtimeRepositoryId,
                title:
                  runtimeRepository?.name ??
                  getTabTitleFromRoute(normalizedRoutePath),
              }
            : {
                title: getTabTitleFromRoute(normalizedRoutePath),
              }),
        };

        void getCurrentWebview().emit(TAB_RUNTIME_STATE_EVENT, payload);
      };

      emitRuntimeStateRef.current = emitRuntimeState;

      let unlistenSyncRequest: (() => void) | undefined;

      const registerSyncRequestListener = async () => {
        unlistenSyncRequest =
          await getCurrentWebview().listen<TabRuntimeReadyPayload>(
            TAB_RUNTIME_REQUEST_SYNC_EVENT,
            ({ payload }) => {
              if (payload?.tabId && payload.tabId !== embeddedTabId) {
                return;
              }

              emitRuntimeState(router.state.location.href);
            },
          );
      };

      void registerSyncRequestListener();
      void getCurrentWebview().emit(TAB_RUNTIME_READY_EVENT, {
        tabId: embeddedTabId,
      });

      emitRuntimeState(router.state.location.href);

      const unsubscribe = router.subscribe("onResolved", (state) => {
        emitRuntimeState(state.toLocation.href);
      });

      return () => {
        if (snapshotEmitTimerRef.current !== null) {
          window.clearTimeout(snapshotEmitTimerRef.current);
          snapshotEmitTimerRef.current = null;
        }

        emitRuntimeStateRef.current = null;

        if (unlistenSyncRequest) {
          unlistenSyncRequest();
        }

        if (typeof unsubscribe === "function") {
          unsubscribe();
        }
      };
    }, [embeddedTabId, embeddedRuntimeSessionExists]);

    // Effect 5: Debounced snapshot emit for embedded tabs
    useEffect(() => {
      if (
        !embeddedRuntime ||
        !embeddedTabId ||
        !embeddedRuntimeSessionExists ||
        !isEmbeddedRuntimeBound
      ) {
        return;
      }

      if (snapshotEmitTimerRef.current !== null) {
        window.clearTimeout(snapshotEmitTimerRef.current);
      }

      snapshotEmitTimerRef.current = window.setTimeout(() => {
        const emitRuntimeState = emitRuntimeStateRef.current;
        if (emitRuntimeState) {
          emitRuntimeState(router.state.location.href);
        }
        snapshotEmitTimerRef.current = null;
      }, SNAPSHOT_EMIT_DEBOUNCE_MS);

      return () => {
        if (snapshotEmitTimerRef.current !== null) {
          window.clearTimeout(snapshotEmitTimerRef.current);
          snapshotEmitTimerRef.current = null;
        }
      };
    }, [
      activeGitViewState,
      activeRepositoryPath,
      activeSelectionState,
      embeddedRuntime,
      embeddedTabId,
      mainWindowView,
      selectedRepository?.id,
      selectedRepository?.name,
      selectedRepository?.path,
      embeddedRuntimeSessionExists,
      isEmbeddedRuntimeBound,
    ]);

    // Effect 6: Listen for tab runtime events in main window
    useEffect(() => {
      if (isEmbeddedRuntime()) {
        return;
      }

      let unlistenRuntimeState: (() => void) | undefined;
      let unlistenRuntimeReady: (() => void) | undefined;

      const register = async () => {
        unlistenRuntimeState =
          await getCurrentWebview().listen<TabRuntimeStatePayload>(
            TAB_RUNTIME_STATE_EVENT,
            ({ payload }) => {
              if (
                !payload ||
                typeof payload.tabId !== "string" ||
                !payload.tabId
              ) {
                return;
              }

              const nextRoutePath =
                typeof payload.routePath === "string"
                  ? normalizeWorkspaceRoutePath(
                      stripEmbeddedQueryFromRoutePath(payload.routePath),
                    )
                  : undefined;

              syncTabMetadata(payload.tabId, {
                routePath: nextRoutePath,
                repositoryId: payload.repositoryId,
                title: payload.title,
                snapshot: payload.snapshot,
              });
            },
          );

        unlistenRuntimeReady =
          await getCurrentWebview().listen<TabRuntimeReadyPayload>(
            TAB_RUNTIME_READY_EVENT,
            ({ payload }) => {
              const tabId = payload?.tabId;

              if (!tabId) {
                return;
              }

              void getCurrentWebview().emitTo(
                sanitizeTabWebviewLabel(tabId),
                TAB_RUNTIME_REQUEST_SYNC_EVENT,
                { tabId },
              );
            },
          );

        const currentTabs = useAppStore.getState().tabs;

        for (const tab of currentTabs) {
          void getCurrentWebview().emitTo(
            sanitizeTabWebviewLabel(tab.id),
            TAB_RUNTIME_REQUEST_SYNC_EVENT,
            { tabId: tab.id },
          );
        }
      };

      void register();

      return () => {
        if (unlistenRuntimeState) {
          unlistenRuntimeState();
        }

        if (unlistenRuntimeReady) {
          unlistenRuntimeReady();
        }
      };
    }, [syncTabMetadata]);

    const tabScopeId =
      embeddedRuntime && embeddedTabId
        ? embeddedTabId
        : (activeRuntimeId ?? activeTabId ?? "tab-main");

    return (
      <TabContextProvider scopeId={tabScopeId}>
        <RouterProvider router={router} />
      </TabContextProvider>
    );
  };

  root.render(
    <StrictMode>
      <QueryClientProvider client={appState.queryClient}>
        <NextThemesProvider
          disableTransitionOnChange
          defaultTheme="light"
          enableColorScheme
          themes={colorKeyList}
        >
          <CommandManagerProvider initialViewId="root">
            <AppRouter />
            <Toaster />
            {enableDevDiagnostics && (
              <ReactQueryDevtools
                buttonPosition="top-right"
                initialIsOpen={false}
              />
            )}
          </CommandManagerProvider>
        </NextThemesProvider>
      </QueryClientProvider>
    </StrictMode>,
  );
}

if (enableDevDiagnostics) {
  scan({
    enabled: true,
  });
}

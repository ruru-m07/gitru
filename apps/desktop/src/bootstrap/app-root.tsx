import { CommandManagerProvider } from "@gitru/ui/components/command";
import { QueryClientProvider } from "@tanstack/react-query";
import { ReactQueryDevtools } from "@tanstack/react-query-devtools";
import { RouterProvider } from "@tanstack/react-router";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { ThemeProvider as NextThemesProvider } from "next-themes";
import { StrictMode, useCallback, useEffect, useMemo, useRef } from "react";
import { Toaster } from "sonner";
import { WorkspaceSessionSnapshot } from "@/types/store";
import { TabContextProvider } from "../context/tab-context-provider";
import { colorKeyList } from "../lib/colors";
import {
  TAB_SWITCH_SHORTCUT_EVENT,
  type TabSwitchShortcutPayload,
} from "../lib/tab-switching";
import { appState } from "../state";
import {
  selectActiveRepositoryPath,
  selectActiveSessionRepoKey,
  useAppStore,
} from "../store/use-app-store";
import { router } from "./create-router";
import {
  cloneRuntimeGitViewState,
  cloneRuntimeSelectionState,
  enableDevDiagnostics,
  getEmbeddedTabId,
  getTabTitleFromRoute,
  isEmbeddedRuntime,
  isGitRoutePath,
  normalizeWorkspaceRoutePath,
  SNAPSHOT_EMIT_DEBOUNCE_MS,
  sanitizeTabWebviewLabel,
  stripEmbeddedQueryFromRoutePath,
  TAB_RUNTIME_READY_EVENT,
  TAB_RUNTIME_REQUEST_SYNC_EVENT,
  TAB_RUNTIME_STATE_EVENT,
  type TabRuntimeReadyPayload,
  type TabRuntimeStatePayload,
} from "./runtime-utils";

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

  const captureSnapshot = useCallback(() => {
    captureActiveSessionSnapshot();
  }, [captureActiveSessionSnapshot]);

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

  useEffect(() => {
    if (!embeddedRuntime) {
      return;
    }

    let unlistenNavigation: (() => void) | undefined;

    const setupNavigationListener = async () => {
      try {
        const { listen } = await import("@tauri-apps/api/event");
        const { WEBVIEW_NAVIGATION_EVENT } = await import(
          "@/lib/navigation-events"
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

          if (targetTabId !== embeddedTabId) {
            console.log(
              "[EmbeddedNav] Ignoring event - wrong target. Expected:",
              embeddedTabId,
              "Got:",
              targetTabId,
            );
            return;
          }

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

  useEffect(() => {
    if (embeddedRuntime) {
      return;
    }

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
      document.removeEventListener("visibilitychange", handleVisibilityChange);
    };
  }, [captureSnapshot, embeddedRuntime]);

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

export function AppRoot() {
  const showDevDiagnostics = enableDevDiagnostics();

  return (
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
            {showDevDiagnostics && (
              <ReactQueryDevtools
                buttonPosition="top-right"
                initialIsOpen={false}
              />
            )}
          </CommandManagerProvider>
        </NextThemesProvider>
      </QueryClientProvider>
    </StrictMode>
  );
}

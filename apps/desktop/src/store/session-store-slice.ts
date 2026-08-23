import type { AppState } from "@/types/store";
import type { AppStoreGet, AppStoreSet } from "./store-helpers";
import {
  captureSessionSnapshot,
  cloneSessionSnapshot,
  createDefaultTab,
  createSessionFromTab,
  createWorkspaceTab,
  DEFAULT_TAB_ID,
  DEFAULT_TAB_ROUTE,
  freezeSessionWithSnapshot,
  getRepositoryById,
  isSameSessionSnapshot,
  normalizeRuntimeSnapshot,
  normalizeWorkspaceRoutePath,
  restoreSessionUiState,
} from "./store-helpers";

export const createSessionSlice = (set: AppStoreSet, get: AppStoreGet) => ({
  tabs: [createDefaultTab()],
  activeTabId: DEFAULT_TAB_ID,
  sessionsById: {
    [DEFAULT_TAB_ID]: createSessionFromTab(createDefaultTab(), "active"),
  },
  activeSessionId: DEFAULT_TAB_ID,

  ensureActiveTab: (payload?: {
    routePath?: string;
    repositoryId?: string | null;
    title?: string;
  }) => {
    set((state) => {
      const hasActiveTab =
        !!state.activeTabId &&
        state.tabs.some((tab) => tab.id === state.activeTabId);

      const ensureSessionsForTabs = (
        tabs: AppState["tabs"],
        activeId: string | null,
      ) => {
        const now = Date.now();
        const sessionsById = { ...state.sessionsById };

        for (const tab of tabs) {
          const lifecycle = tab.id === activeId ? "active" : "frozen";
          const existing = sessionsById[tab.id];

          if (!existing) {
            sessionsById[tab.id] = createSessionFromTab(tab, lifecycle);
            continue;
          }

          sessionsById[tab.id] = {
            ...existing,
            id: tab.id,
            repositoryId: tab.repositoryId,
            routePath: tab.routePath,
            title: tab.title,
            lifecycle,
            frozenAt:
              lifecycle === "frozen"
                ? existing.lifecycle === "frozen"
                  ? existing.frozenAt
                  : now
                : null,
            updatedAt: Math.max(tab.updatedAt, existing.updatedAt),
          };
        }

        for (const sessionId of Object.keys(sessionsById)) {
          if (!tabs.some((tab) => tab.id === sessionId)) {
            delete sessionsById[sessionId];
          }
        }

        return sessionsById;
      };

      if (state.tabs.length > 0 && hasActiveTab) {
        const nextSessionsById = ensureSessionsForTabs(
          state.tabs,
          state.activeTabId,
        );

        const activeRepositoryId =
          state.tabs.find((tab) => tab.id === state.activeTabId)
            ?.repositoryId ?? null;
        const nextSelectedRepository = getRepositoryById(
          state.repositories,
          activeRepositoryId,
        );

        const hasSessionShapeChange =
          Object.keys(nextSessionsById).length !==
            Object.keys(state.sessionsById).length ||
          Object.keys(nextSessionsById).some(
            (sessionId) =>
              state.sessionsById[sessionId] !== nextSessionsById[sessionId],
          );

        const hasSelectedRepositoryChange =
          (state.selectedRepository?.id ?? null) !==
          (nextSelectedRepository?.id ?? null);

        if (
          !hasSessionShapeChange &&
          !hasSelectedRepositoryChange &&
          (state.activeSessionId ?? null) === (state.activeTabId ?? null)
        ) {
          return state;
        }

        return {
          sessionsById: nextSessionsById,
          activeSessionId: state.activeTabId,
          selectedRepository: nextSelectedRepository,
          repoSelectIsOpen: state.activeTabId
            ? Boolean(state.repoSelectIsOpenBySession[state.activeTabId])
            : false,
        };
      }

      if (state.tabs.length > 0) {
        const nextActiveTabId = state.tabs[0]?.id ?? null;
        const nextSessionsById = ensureSessionsForTabs(
          state.tabs,
          nextActiveTabId,
        );
        const activeRepositoryId =
          state.tabs.find((tab) => tab.id === nextActiveTabId)?.repositoryId ??
          null;
        const nextSelectedRepository = getRepositoryById(
          state.repositories,
          activeRepositoryId,
        );

        return {
          activeTabId: nextActiveTabId,
          activeSessionId: nextActiveTabId,
          sessionsById: nextSessionsById,
          selectedRepository: nextSelectedRepository,
          repoSelectIsOpen: nextActiveTabId
            ? Boolean(state.repoSelectIsOpenBySession[nextActiveTabId])
            : false,
        };
      }

      const fallbackTab = createWorkspaceTab({
        routePath: payload?.routePath,
        repositoryId: payload?.repositoryId ?? null,
        title: payload?.title,
      });

      const fallbackSession = createSessionFromTab(fallbackTab, "active");
      const nextSelectedRepository = getRepositoryById(
        state.repositories,
        fallbackTab.repositoryId,
      );

      return {
        tabs: [fallbackTab],
        activeTabId: fallbackTab.id,
        sessionsById: {
          [fallbackSession.id]: fallbackSession,
        },
        activeSessionId: fallbackTab.id,
        selectedRepository: nextSelectedRepository,
        repoSelectIsOpen: Boolean(
          state.repoSelectIsOpenBySession[fallbackTab.id],
        ),
      };
    });
  },

  createTab: (payload?: {
    routePath?: string;
    repositoryId?: string | null;
    title?: string;
  }) => {
    const newTab = createWorkspaceTab({
      routePath: payload?.routePath,
      repositoryId: payload?.repositoryId ?? null,
      title: payload?.title,
    });

    set((state) => {
      const now = Date.now();
      const previousActiveSessionId =
        state.activeSessionId ?? state.activeTabId;
      let nextSessionsById = {
        ...state.sessionsById,
      };

      if (previousActiveSessionId) {
        nextSessionsById = freezeSessionWithSnapshot({
          sessionsById: nextSessionsById,
          sessionId: previousActiveSessionId,
          repositories: state.repositories,
          selectionByRepo: state.selectionByRepo,
          gitViewByRepo: state.gitViewByRepo,
          mainWindowView: state.mainWindowView,
          at: now,
        });
      }

      const inheritedSnapshot = previousActiveSessionId
        ? cloneSessionSnapshot(
            nextSessionsById[previousActiveSessionId]?.snapshot ?? null,
          )
        : null;

      nextSessionsById[newTab.id] = {
        ...createSessionFromTab(newTab, "active"),
        snapshot: inheritedSnapshot,
      };

      const restoredUiState = restoreSessionUiState({
        session: nextSessionsById[newTab.id],
        repositories: state.repositories,
        selectionByRepo: state.selectionByRepo,
        gitViewByRepo: state.gitViewByRepo,
        mainWindowView: state.mainWindowView,
      });

      const nextSelectedRepository = getRepositoryById(
        state.repositories,
        newTab.repositoryId,
      );

      return {
        tabs: [...state.tabs, newTab],
        activeTabId: newTab.id,
        activeSessionId: newTab.id,
        sessionsById: nextSessionsById,
        repoSelectIsOpen: Boolean(state.repoSelectIsOpenBySession[newTab.id]),
        selectionByRepo: restoredUiState.selectionByRepo,
        gitViewByRepo: restoredUiState.gitViewByRepo,
        mainWindowView: restoredUiState.mainWindowView,
        selectedRepository: nextSelectedRepository,
      };
    });

    return newTab;
  },

  activateTab: (tabId: string) => {
    set((state) => {
      if (state.activeTabId === tabId) {
        return state;
      }

      const exists = state.tabs.some((tab) => tab.id === tabId);

      if (!exists) {
        return state;
      }

      const targetTab = state.tabs.find((tab) => tab.id === tabId) ?? null;
      if (!targetTab) {
        return state;
      }

      const now = Date.now();
      const previousActiveSessionId =
        state.activeSessionId ?? state.activeTabId;
      let nextSessionsById = {
        ...state.sessionsById,
      };

      if (previousActiveSessionId && previousActiveSessionId !== tabId) {
        nextSessionsById = freezeSessionWithSnapshot({
          sessionsById: nextSessionsById,
          sessionId: previousActiveSessionId,
          repositories: state.repositories,
          selectionByRepo: state.selectionByRepo,
          gitViewByRepo: state.gitViewByRepo,
          mainWindowView: state.mainWindowView,
          at: now,
        });
      }

      const nextTargetSession = nextSessionsById[tabId]
        ? {
            ...nextSessionsById[tabId],
            repositoryId: targetTab.repositoryId,
            routePath: targetTab.routePath,
            title: targetTab.title,
            lifecycle: "active" as const,
            frozenAt: null,
            updatedAt: now,
          }
        : createSessionFromTab(targetTab, "active");

      nextSessionsById[tabId] = nextTargetSession;

      const restoredUiState = restoreSessionUiState({
        session: nextTargetSession,
        repositories: state.repositories,
        selectionByRepo: state.selectionByRepo,
        gitViewByRepo: state.gitViewByRepo,
        mainWindowView: state.mainWindowView,
      });

      const nextSelectedRepository = getRepositoryById(
        state.repositories,
        targetTab.repositoryId,
      );

      return {
        activeTabId: tabId,
        activeSessionId: tabId,
        sessionsById: nextSessionsById,
        repoSelectIsOpen: Boolean(state.repoSelectIsOpenBySession[tabId]),
        selectionByRepo: restoredUiState.selectionByRepo,
        gitViewByRepo: restoredUiState.gitViewByRepo,
        mainWindowView: restoredUiState.mainWindowView,
        selectedRepository: nextSelectedRepository,
      };
    });
  },

  setEmbeddedRuntimeSession: (sessionId: string) => {
    set((state) => {
      if (!sessionId) {
        return state;
      }

      const targetSession = state.sessionsById[sessionId] ?? null;
      const targetTab = state.tabs.find((tab) => tab.id === sessionId) ?? null;

      if (!targetSession && !targetTab) {
        return state;
      }

      const now = Date.now();
      const resolvedRepositoryId =
        targetSession?.repositoryId ?? targetTab?.repositoryId ?? null;
      const resolvedRoutePath =
        targetTab?.routePath ?? targetSession?.routePath;
      const resolvedTitle = targetTab?.title ?? targetSession?.title;

      const nextSession = targetSession
        ? {
            ...targetSession,
            repositoryId: resolvedRepositoryId,
            routePath: resolvedRoutePath ?? targetSession.routePath,
            title: resolvedTitle ?? targetSession.title,
            lifecycle: "active" as const,
            frozenAt: null,
            updatedAt: Math.max(targetSession.updatedAt, now),
          }
        : createSessionFromTab(
            targetTab ??
              createWorkspaceTab({
                id: sessionId,
                routePath: DEFAULT_TAB_ROUTE,
                title: "Git",
                repositoryId: resolvedRepositoryId,
              }),
            "active",
          );

      const restoredUiState = restoreSessionUiState({
        session: nextSession,
        repositories: state.repositories,
        selectionByRepo: state.selectionByRepo,
        gitViewByRepo: state.gitViewByRepo,
        mainWindowView: state.mainWindowView,
      });

      const nextSelectedRepository = getRepositoryById(
        state.repositories,
        resolvedRepositoryId,
      );
      const nextRepoSelectIsOpen = Boolean(
        state.repoSelectIsOpenBySession[sessionId],
      );
      const hasPointerChange =
        state.activeSessionId !== sessionId ||
        (targetTab ? state.activeTabId !== sessionId : false);
      const hasSelectedRepositoryChange =
        (state.selectedRepository?.id ?? null) !==
        (nextSelectedRepository?.id ?? null);
      const hasUiStateChange =
        restoredUiState.selectionByRepo !== state.selectionByRepo ||
        restoredUiState.gitViewByRepo !== state.gitViewByRepo ||
        restoredUiState.mainWindowView !== state.mainWindowView;
      const hasRepoSelectChange =
        state.repoSelectIsOpen !== nextRepoSelectIsOpen;
      const hasSessionWrite =
        !targetSession || state.sessionsById[sessionId] !== nextSession;

      if (
        !hasPointerChange &&
        !hasSelectedRepositoryChange &&
        !hasUiStateChange &&
        !hasRepoSelectChange &&
        !hasSessionWrite
      ) {
        return state;
      }

      return {
        activeSessionId: sessionId,
        activeTabId: targetTab ? sessionId : state.activeTabId,
        sessionsById: {
          ...state.sessionsById,
          [sessionId]: nextSession,
        },
        selectedRepository: nextSelectedRepository,
        repoSelectIsOpen: nextRepoSelectIsOpen,
        selectionByRepo: restoredUiState.selectionByRepo,
        gitViewByRepo: restoredUiState.gitViewByRepo,
        mainWindowView: restoredUiState.mainWindowView,
      };
    });
  },

  activateSession: (sessionId: string) => {
    get().activateTab(sessionId);
  },

  freezeSession: (sessionId?: string) => {
    set((state) => {
      const targetSessionId =
        sessionId ?? state.activeSessionId ?? state.activeTabId;
      if (!targetSessionId) {
        return state;
      }

      const session = state.sessionsById[targetSessionId];
      if (!session || session.lifecycle === "frozen") {
        return state;
      }

      const nextSessionsById = freezeSessionWithSnapshot({
        sessionsById: state.sessionsById,
        sessionId: targetSessionId,
        repositories: state.repositories,
        selectionByRepo: state.selectionByRepo,
        gitViewByRepo: state.gitViewByRepo,
        mainWindowView: state.mainWindowView,
        at: Date.now(),
      });

      return {
        sessionsById: nextSessionsById,
      };
    });
  },

  disposeSession: (sessionId: string) => {
    set((state) => {
      if (!state.sessionsById[sessionId]) {
        return state;
      }

      const nextSessionsById = {
        ...state.sessionsById,
      };
      delete nextSessionsById[sessionId];
      const nextRepoSelectIsOpenBySession = {
        ...state.repoSelectIsOpenBySession,
      };
      delete nextRepoSelectIsOpenBySession[sessionId];

      const nextActiveSessionId =
        state.activeSessionId === sessionId
          ? state.activeTabId
          : state.activeSessionId;

      return {
        sessionsById: nextSessionsById,
        activeSessionId: nextActiveSessionId,
        repoSelectIsOpenBySession: nextRepoSelectIsOpenBySession,
        repoSelectIsOpen: nextActiveSessionId
          ? Boolean(nextRepoSelectIsOpenBySession[nextActiveSessionId])
          : false,
      };
    });
  },

  captureActiveSessionSnapshot: () => {
    set((state) => {
      const targetSessionId = state.activeSessionId ?? state.activeTabId;

      if (!targetSessionId || !state.sessionsById[targetSessionId]) {
        return state;
      }

      return {
        sessionsById: captureSessionSnapshot({
          sessionsById: state.sessionsById,
          sessionId: targetSessionId,
          repositories: state.repositories,
          selectionByRepo: state.selectionByRepo,
          gitViewByRepo: state.gitViewByRepo,
          mainWindowView: state.mainWindowView,
          at: Date.now(),
        }),
      };
    });
  },

  closeTab: (tabId: string) => {
    set((state) => {
      if (state.tabs.length <= 1) {
        return state;
      }

      const closingIndex = state.tabs.findIndex((tab) => tab.id === tabId);

      if (closingIndex === -1) {
        return state;
      }

      const nextTabs = state.tabs.filter((tab) => tab.id !== tabId);
      const isClosingActiveTab = state.activeTabId === tabId;
      let nextActiveTabId = state.activeTabId;
      let nextActiveSessionId = state.activeSessionId;

      const nextSessionsById = {
        ...state.sessionsById,
      };
      delete nextSessionsById[tabId];
      const nextRepoSelectIsOpenBySession = {
        ...state.repoSelectIsOpenBySession,
      };
      delete nextRepoSelectIsOpenBySession[tabId];

      if (isClosingActiveTab) {
        const fallbackIndex = Math.max(0, closingIndex - 1);
        nextActiveTabId =
          nextTabs[fallbackIndex]?.id ?? nextTabs[0]?.id ?? null;
        nextActiveSessionId = nextActiveTabId;
      } else if (
        nextActiveTabId &&
        !nextTabs.some((tab) => tab.id === nextActiveTabId)
      ) {
        nextActiveTabId = nextTabs[0]?.id ?? null;
        nextActiveSessionId = nextActiveTabId;
      }

      if (nextActiveSessionId && nextSessionsById[nextActiveSessionId]) {
        nextSessionsById[nextActiveSessionId] = {
          ...nextSessionsById[nextActiveSessionId],
          lifecycle: "active",
          frozenAt: null,
          updatedAt: Date.now(),
        };
      }

      const nextActiveTab =
        nextTabs.find((tab) => tab.id === nextActiveTabId) ?? null;
      const nextSelectedRepository = isClosingActiveTab
        ? getRepositoryById(
            state.repositories,
            nextActiveTab?.repositoryId ?? null,
          )
        : state.selectedRepository;

      const restoredUiState = restoreSessionUiState({
        session:
          isClosingActiveTab && nextActiveSessionId
            ? (nextSessionsById[nextActiveSessionId] ?? null)
            : null,
        repositories: state.repositories,
        selectionByRepo: state.selectionByRepo,
        gitViewByRepo: state.gitViewByRepo,
        mainWindowView: state.mainWindowView,
      });

      return {
        tabs: nextTabs,
        activeTabId: nextActiveTabId,
        activeSessionId: nextActiveSessionId,
        sessionsById: nextSessionsById,
        repoSelectIsOpenBySession: nextRepoSelectIsOpenBySession,
        repoSelectIsOpen: nextActiveSessionId
          ? Boolean(nextRepoSelectIsOpenBySession[nextActiveSessionId])
          : false,
        selectionByRepo: restoredUiState.selectionByRepo,
        gitViewByRepo: restoredUiState.gitViewByRepo,
        mainWindowView: restoredUiState.mainWindowView,
        selectedRepository: nextSelectedRepository,
      };
    });
  },

  reorderTab: (tabId: string, targetIndex: number) => {
    set((state) => {
      if (state.tabs.length <= 1) {
        return state;
      }

      const sourceIndex = state.tabs.findIndex((tab) => tab.id === tabId);

      if (sourceIndex === -1) {
        return state;
      }

      const clampedTargetIndex = Math.max(
        0,
        Math.min(targetIndex, state.tabs.length - 1),
      );

      if (clampedTargetIndex === sourceIndex) {
        return state;
      }

      const nextTabs = [...state.tabs];
      const [movedTab] = nextTabs.splice(sourceIndex, 1);

      if (!movedTab) {
        return state;
      }

      nextTabs.splice(clampedTargetIndex, 0, movedTab);

      return {
        tabs: nextTabs,
      };
    });
  },

  syncActiveTab: (payload: {
    routePath?: string;
    repositoryId?: string | null;
    title?: string;
  }) => {
    const activeTabId = get().activeTabId;

    if (!activeTabId) {
      return;
    }

    get().syncTabMetadata(activeTabId, payload);
  },

  syncTabMetadata: (
    tabId: string,
    payload: {
      routePath?: string;
      repositoryId?: string | null;
      title?: string;
      snapshot?: AppState["sessionsById"][string]["snapshot"];
    },
  ) => {
    set((state) => {
      const activeIndex = state.tabs.findIndex((tab) => tab.id === tabId);

      if (activeIndex === -1) {
        return state;
      }

      const activeTab = state.tabs[activeIndex];
      const nextRoutePath = normalizeWorkspaceRoutePath(
        payload.routePath ?? activeTab.routePath,
      );
      const nextRepositoryId =
        payload.repositoryId === undefined
          ? activeTab.repositoryId
          : payload.repositoryId;
      const nextTitle = payload.title?.trim() || activeTab.title;
      const targetRepositoryPath =
        getRepositoryById(state.repositories, nextRepositoryId)?.path ?? null;
      const hasSnapshotPayload = payload.snapshot !== undefined;
      const normalizedSnapshot = hasSnapshotPayload
        ? normalizeRuntimeSnapshot({
            snapshot: payload.snapshot ?? null,
            repositoryPathFallback: targetRepositoryPath,
          })
        : null;

      const didMetadataChange =
        nextRoutePath !== activeTab.routePath ||
        nextRepositoryId !== activeTab.repositoryId ||
        nextTitle !== activeTab.title;

      const nextSessionsById = {
        ...state.sessionsById,
      };
      const activeSession = nextSessionsById[tabId];
      const nextSnapshot = hasSnapshotPayload
        ? normalizedSnapshot
        : (activeSession?.snapshot ?? null);
      const didSnapshotChange = hasSnapshotPayload
        ? !isSameSessionSnapshot(activeSession?.snapshot ?? null, nextSnapshot)
        : false;

      if (!didMetadataChange && !didSnapshotChange) {
        return state;
      }

      const now = Date.now();

      const nextTabs = [...state.tabs];
      nextTabs[activeIndex] = {
        ...activeTab,
        routePath: nextRoutePath,
        repositoryId: nextRepositoryId,
        title: nextTitle,
        updatedAt: now,
      };

      if (activeSession) {
        const activeRuntimeId = state.activeSessionId ?? state.activeTabId;
        const isTargetActiveSession = activeRuntimeId === tabId;

        nextSessionsById[tabId] = {
          ...activeSession,
          routePath: nextRoutePath,
          repositoryId: nextRepositoryId,
          title: nextTitle,
          lifecycle: isTargetActiveSession ? "active" : activeSession.lifecycle,
          frozenAt: isTargetActiveSession ? null : activeSession.frozenAt,
          snapshot: nextSnapshot,
          updatedAt: now,
        };
      } else {
        const activeRuntimeId = state.activeSessionId ?? state.activeTabId;
        const isTargetActiveSession = activeRuntimeId === tabId;

        nextSessionsById[tabId] = {
          ...createSessionFromTab(
            nextTabs[activeIndex],
            isTargetActiveSession ? "active" : "frozen",
          ),
          snapshot: nextSnapshot,
          frozenAt: isTargetActiveSession ? null : now,
          updatedAt: now,
        };
      }

      const isActiveTab = state.activeTabId === tabId;
      const nextSelectedRepository = isActiveTab
        ? getRepositoryById(state.repositories, nextRepositoryId)
        : state.selectedRepository;

      return {
        tabs: nextTabs,
        sessionsById: nextSessionsById,
        ...(isActiveTab
          ? {
              selectedRepository: nextSelectedRepository,
            }
          : {}),
      };
    });
  },
});

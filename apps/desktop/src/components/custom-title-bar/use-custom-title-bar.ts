import {
  type DragCancelEvent,
  type DragEndEvent,
  type DragOverEvent,
  type DragStartEvent,
  PointerSensor,
  useSensor,
  useSensors,
} from "@dnd-kit/core";
import { arrayMove } from "@dnd-kit/sortable";
import { type RepositoryInfo } from "@gitru/commands";
import { useNavigate, useRouterState } from "@tanstack/react-router";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { useEffect, useRef, useState } from "react";

import { useRepositories } from "@/hooks/use-repositories";
import { useSessionNavigation } from "@/hooks/use-session-navigation";
import {
  resolveTabManagementShortcut,
  TAB_SWITCH_CYCLE_MODE,
  TAB_SWITCH_SHORTCUT_EVENT,
  type TabSwitchShortcutPayload,
} from "@/lib/tab-switching";
import { appState } from "@/state";
import { repoContextRegistry } from "@/state/core/repo-context-registry";
import { useAppStore } from "@/store/use-app-store";

import {
  DEFAULT_TAB_ROUTE,
  MAIN_HEADER_HEIGHT_CSS_VAR,
  TAB_SWITCHER_HEADER_HEIGHT_PX,
  type TabSwitchModifier,
} from "./constants";
import {
  areSameTabOrder,
  getRoutePathname,
  getTitleFromRoute,
  isEmbeddedRuntime,
  isGitRoute,
  normalizeTabRoutePath,
} from "./utils";

export function useCustomTitleBar() {
  const routerState = useRouterState();
  const navigate = useNavigate();
  const pathname = routerState.location.pathname;
  const routePath = routerState.location.href;

  const activeTabId = useAppStore((state) => state.activeTabId);
  const { navigationState, goBack, goForward, pushToHistory } =
    useSessionNavigation(activeTabId);

  const selectedRepository = useAppStore((state) => state.selectedRepository);
  const { repositories } = useRepositories();

  const tabs = useAppStore((state) => state.tabs);
  const [hoveredTabId, setHoveredTabId] = useState<string | null>(null);
  const ensureActiveTab = useAppStore((state) => state.ensureActiveTab);
  const createTab = useAppStore((state) => state.createTab);
  const activateTab = useAppStore((state) => state.activateTab);
  const reorderTab = useAppStore((state) => state.reorderTab);
  const syncActiveTab = useAppStore((state) => state.syncActiveTab);
  const suppressClickTabIdRef = useRef<string | null>(null);
  const visualTabOrderRef = useRef<string[] | null>(null);
  const mruTabIdsRef = useRef<string[]>([]);
  const tabSwitchModifierRef = useRef<TabSwitchModifier | null>(null);
  const tabsRef = useRef(tabs);
  const activeTabIdRef = useRef(activeTabId);
  const [draggingTabId, setDraggingTabId] = useState<string | null>(null);
  const [visualTabOrder, setVisualTabOrder] = useState<string[] | null>(null);
  const [isTabSwitcherOpen, setIsTabSwitcherOpen] = useState(false);
  const [tabSwitcherTabIds, setTabSwitcherTabIds] = useState<string[]>([]);
  const [tabSwitcherIndex, setTabSwitcherIndex] = useState(0);
  const tabSwitcherOpenRef = useRef(false);
  const tabSwitcherTabIdsRef = useRef<string[]>([]);
  const tabSwitcherIndexRef = useRef(0);
  const handleTabSwitchAdvanceRef = useRef(
    (_backward: boolean, _modifier: TabSwitchModifier) => {},
  );
  const handleTabSwitchCommitRef = useRef(() => {});
  const handleCreateTabRef = useRef(() => {});
  const handleCloseActiveTabRef = useRef(() => {});
  const sensors = useSensors(
    useSensor(PointerSensor, {
      activationConstraint: {
        distance: 6,
      },
    }),
  );

  tabsRef.current = tabs;
  activeTabIdRef.current = activeTabId;

  const activeTab = tabs.find((tab) => tab.id === activeTabId) ?? null;
  const isRootShellMode = !isEmbeddedRuntime();
  const effectiveRoutePath = isRootShellMode
    ? normalizeTabRoutePath(activeTab?.routePath)
    : routePath;
  const isGitEffectiveRoute = isGitRoute(effectiveRoutePath);
  const effectiveTitle = isGitEffectiveRoute
    ? (selectedRepository?.name ?? getTitleFromRoute(effectiveRoutePath))
    : getTitleFromRoute(effectiveRoutePath);
  const tabById = new Map(tabs.map((tab) => [tab.id, tab] as const));
  const orderedTabIds = visualTabOrder ?? tabs.map((tab) => tab.id);
  const orderedTabs = orderedTabIds
    .map((tabId) => tabById.get(tabId))
    .filter((tab): tab is (typeof tabs)[number] => tab !== undefined);
  const isTabDragInProgress = draggingTabId !== null;
  const shouldReserveTabSwitcherSpace =
    isRootShellMode && TAB_SWITCH_CYCLE_MODE === "MRU" && isTabSwitcherOpen;

  const disposeRuntimeForTab = (
    tabId: string,
    repositoryId: string | null | undefined,
  ) => {
    const contextEntry = repoContextRegistry.getScopeContext(tabId);

    if (repositoryId && contextEntry?.contextId) {
      const repository =
        useAppStore
          .getState()
          .repositories.find((repo) => repo.id === repositoryId) ?? null;

      if (repository?.path) {
        void appState.repositories.dispose(
          repository.path,
          contextEntry.contextId,
        );
      }
    }

    void repoContextRegistry.disposeScope(tabId);
  };

  useEffect(() => {
    ensureActiveTab({
      routePath: effectiveRoutePath,
      repositoryId: selectedRepository?.id ?? null,
      title: effectiveTitle,
    });
  }, [
    effectiveRoutePath,
    effectiveTitle,
    ensureActiveTab,
    selectedRepository?.id,
  ]);

  useEffect(() => {
    if (!activeTabId) {
      return;
    }

    if (
      isRootShellMode &&
      activeTab &&
      getRoutePathname(activeTab.routePath) === "/app"
    ) {
      syncActiveTab({
        routePath: DEFAULT_TAB_ROUTE,
        repositoryId: selectedRepository?.id ?? null,
        title: selectedRepository?.name ?? getTitleFromRoute(DEFAULT_TAB_ROUTE),
      });
      return;
    }

    if (isRootShellMode) {
      return;
    }

    syncActiveTab({
      routePath: effectiveRoutePath,
      repositoryId: selectedRepository?.id ?? null,
      title: effectiveTitle,
    });
  }, [
    activeTabId,
    activeTab,
    effectiveRoutePath,
    effectiveTitle,
    isRootShellMode,
    selectedRepository?.id,
    syncActiveTab,
  ]);

  useEffect(() => {
    if (!activeTabId || !effectiveRoutePath) {
      console.log(
        "[RouteTracking] Skipping - activeTabId:",
        activeTabId,
        "effectiveRoutePath:",
        effectiveRoutePath,
      );
      return;
    }

    console.log(
      "[RouteTracking] Pushing to history. Session:",
      activeTabId,
      "Path:",
      effectiveRoutePath,
    );
    void pushToHistory(effectiveRoutePath);
  }, [activeTabId, effectiveRoutePath, pushToHistory]);

  useEffect(() => {
    tabSwitcherOpenRef.current = isTabSwitcherOpen;
  }, [isTabSwitcherOpen]);

  useEffect(() => {
    tabSwitcherTabIdsRef.current = tabSwitcherTabIds;
  }, [tabSwitcherTabIds]);

  useEffect(() => {
    tabSwitcherIndexRef.current = tabSwitcherIndex;
  }, [tabSwitcherIndex]);

  useEffect(() => {
    const validTabIds = new Set(tabs.map((tab) => tab.id));
    const prunedOrder = mruTabIdsRef.current.filter((tabId) =>
      validTabIds.has(tabId),
    );

    if (!activeTabId || !validTabIds.has(activeTabId)) {
      mruTabIdsRef.current = prunedOrder;
      return;
    }

    mruTabIdsRef.current = [
      activeTabId,
      ...prunedOrder.filter((tabId) => tabId !== activeTabId),
    ];
  }, [activeTabId, tabs]);

  const closeTabSwitcher = () => {
    tabSwitcherOpenRef.current = false;
    tabSwitcherTabIdsRef.current = [];
    tabSwitcherIndexRef.current = 0;
    tabSwitchModifierRef.current = null;

    setIsTabSwitcherOpen(false);
    setTabSwitcherTabIds([]);
    setTabSwitcherIndex(0);
  };

  useEffect(() => {
    if (!isTabSwitcherOpen) {
      return;
    }

    const currentTabIds = tabs.map((tab) => tab.id);
    const nextSwitcherTabIds = tabSwitcherTabIdsRef.current.filter((tabId) =>
      currentTabIds.includes(tabId),
    );

    if (nextSwitcherTabIds.length === 0) {
      closeTabSwitcher();
      return;
    }

    if (!areSameTabOrder(nextSwitcherTabIds, tabSwitcherTabIdsRef.current)) {
      tabSwitcherTabIdsRef.current = nextSwitcherTabIds;
      setTabSwitcherTabIds(nextSwitcherTabIds);
    }

    const clampedIndex = Math.min(
      tabSwitcherIndexRef.current,
      nextSwitcherTabIds.length - 1,
    );

    if (clampedIndex !== tabSwitcherIndexRef.current) {
      tabSwitcherIndexRef.current = clampedIndex;
      setTabSwitcherIndex(clampedIndex);
    }
  }, [isTabSwitcherOpen, tabs]);

  const handleActivateTab = async (tabId: string) => {
    if (!tabs.some((item) => item.id === tabId)) {
      return;
    }

    activateTab(tabId);
  };

  const handleCreateTab = async () => {
    const sourceRoutePath = isRootShellMode
      ? normalizeTabRoutePath(activeTab?.routePath)
      : routePath;

    createTab({
      routePath: sourceRoutePath,
      repositoryId: null,
      title: getTitleFromRoute(sourceRoutePath),
    });
  };

  const handleCloseTab = async (tabId: string) => {
    const currentState = useAppStore.getState();
    const tab = currentState.tabs.find((item) => item.id === tabId);

    if (currentState.tabs.length <= 1 || !tab) {
      return;
    }

    currentState.closeTab(tabId);
    disposeRuntimeForTab(tabId, tab.repositoryId);
  };

  const getMruCandidateTabIds = () => {
    const currentTabs = tabsRef.current;
    const activeId = activeTabIdRef.current;
    const orderedIds = currentTabs.map((tab) => tab.id);
    const mruTabIds = mruTabIdsRef.current.filter((tabId) =>
      orderedIds.includes(tabId),
    );
    const remainingTabIds = orderedIds.filter(
      (tabId) => !mruTabIds.includes(tabId),
    );
    const mergedTabIds = [...mruTabIds, ...remainingTabIds];

    if (!activeId) {
      return mergedTabIds;
    }

    return mergedTabIds.filter((tabId) => tabId !== activeId);
  };

  const getSequentialTargetTabId = (backward: boolean) => {
    const currentTabs = tabsRef.current;
    const activeId = activeTabIdRef.current;

    if (currentTabs.length <= 1) {
      return null;
    }

    if (!activeId) {
      return currentTabs[0]?.id ?? null;
    }

    const activeIndex = currentTabs.findIndex((tab) => tab.id === activeId);

    if (activeIndex === -1) {
      return currentTabs[0]?.id ?? null;
    }

    const nextIndex = backward
      ? (activeIndex - 1 + currentTabs.length) % currentTabs.length
      : (activeIndex + 1) % currentTabs.length;

    return currentTabs[nextIndex]?.id ?? null;
  };

  const commitTabSwitcherSelection = () => {
    const targetTabId =
      tabSwitcherTabIdsRef.current[tabSwitcherIndexRef.current] ?? null;

    closeTabSwitcher();

    if (targetTabId && targetTabId !== activeTabIdRef.current) {
      activateTab(targetTabId);
    }
  };

  const advanceMruTabSwitcher = (
    backward: boolean,
    modifier: TabSwitchModifier,
  ) => {
    const candidates = getMruCandidateTabIds();

    if (candidates.length === 0) {
      return;
    }

    const hasSameCandidateSet = areSameTabOrder(
      candidates,
      tabSwitcherTabIdsRef.current,
    );
    const shouldCycleCurrentList =
      tabSwitcherOpenRef.current && hasSameCandidateSet;

    const nextIndex = shouldCycleCurrentList
      ? backward
        ? (tabSwitcherIndexRef.current - 1 + candidates.length) %
          candidates.length
        : (tabSwitcherIndexRef.current + 1) % candidates.length
      : backward
        ? candidates.length - 1
        : 0;

    tabSwitchModifierRef.current = modifier;
    tabSwitcherOpenRef.current = true;
    tabSwitcherTabIdsRef.current = candidates;
    tabSwitcherIndexRef.current = nextIndex;

    setIsTabSwitcherOpen(true);
    setTabSwitcherTabIds(candidates);
    setTabSwitcherIndex(nextIndex);
  };

  const handleTabSwitchAdvance = (
    backward: boolean,
    modifier: TabSwitchModifier,
  ) => {
    if (TAB_SWITCH_CYCLE_MODE === "Sequential") {
      closeTabSwitcher();

      const targetTabId = getSequentialTargetTabId(backward);

      if (targetTabId && targetTabId !== activeTabIdRef.current) {
        activateTab(targetTabId);
      }

      return;
    }

    advanceMruTabSwitcher(backward, modifier);
  };

  const handleTabSwitchCommit = () => {
    if (TAB_SWITCH_CYCLE_MODE !== "MRU" || !tabSwitcherOpenRef.current) {
      return;
    }

    commitTabSwitcherSelection();
  };

  useEffect(() => {
    handleTabSwitchAdvanceRef.current = handleTabSwitchAdvance;
    handleTabSwitchCommitRef.current = handleTabSwitchCommit;
    handleCreateTabRef.current = () => {
      void handleCreateTab();
    };
    handleCloseActiveTabRef.current = () => {
      const currentActiveTabId = useAppStore.getState().activeTabId;

      if (currentActiveTabId) {
        void handleCloseTab(currentActiveTabId);
      }
    };
  });

  useEffect(() => {
    if (!isRootShellMode) {
      return;
    }

    const rootStyle = document.documentElement.style;

    if (shouldReserveTabSwitcherSpace) {
      rootStyle.setProperty(
        MAIN_HEADER_HEIGHT_CSS_VAR,
        `${TAB_SWITCHER_HEADER_HEIGHT_PX}px`,
      );
    } else {
      rootStyle.removeProperty(MAIN_HEADER_HEIGHT_CSS_VAR);
    }

    return () => {
      rootStyle.removeProperty(MAIN_HEADER_HEIGHT_CSS_VAR);
    };
  }, [isRootShellMode, shouldReserveTabSwitcherSpace]);

  useEffect(() => {
    if (!isRootShellMode) {
      return;
    }

    const handleKeyDown = (event: KeyboardEvent) => {
      const managementShortcut = resolveTabManagementShortcut(event);

      if (managementShortcut) {
        event.preventDefault();
        event.stopPropagation();

        if (managementShortcut === "create") {
          handleCreateTabRef.current();
        } else {
          handleCloseActiveTabRef.current();
        }

        return;
      }

      if (
        TAB_SWITCH_CYCLE_MODE === "MRU" &&
        event.key === "Escape" &&
        tabSwitcherOpenRef.current
      ) {
        event.preventDefault();
        closeTabSwitcher();
        return;
      }

      if (!(event.ctrlKey || event.metaKey) || event.key !== "Tab") {
        return;
      }

      event.preventDefault();

      handleTabSwitchAdvanceRef.current(
        Boolean(event.shiftKey),
        event.metaKey ? "Meta" : "Control",
      );
    };

    const handleKeyUp = (event: KeyboardEvent) => {
      const activeModifier = tabSwitchModifierRef.current;

      if (!activeModifier || event.key !== activeModifier) {
        return;
      }

      event.preventDefault();
      handleTabSwitchCommitRef.current();
    };

    const handleWindowBlur = () => {
      handleTabSwitchCommitRef.current();
    };

    window.addEventListener("keydown", handleKeyDown, true);
    window.addEventListener("keyup", handleKeyUp, true);
    window.addEventListener("blur", handleWindowBlur);

    let disposed = false;
    let unlistenTabSwitchShortcut: (() => void) | undefined;

    const registerTabSwitchShortcutListener = async () => {
      const unlisten =
        await getCurrentWebview().listen<TabSwitchShortcutPayload>(
          TAB_SWITCH_SHORTCUT_EVENT,
          ({ payload }) => {
            if (!payload) {
              return;
            }

            if (payload.phase === "advance") {
              handleTabSwitchAdvanceRef.current(
                Boolean(payload.backward),
                payload.modifier,
              );
              return;
            }

            if (payload.phase === "commit") {
              handleTabSwitchCommitRef.current();
              return;
            }

            if (payload.phase === "create") {
              handleCreateTabRef.current();
              return;
            }

            if (payload.phase === "close") {
              handleCloseActiveTabRef.current();
            }
          },
        );

      if (disposed) {
        unlisten();
        return;
      }

      unlistenTabSwitchShortcut = unlisten;
    };

    void registerTabSwitchShortcutListener();

    return () => {
      disposed = true;
      window.removeEventListener("keydown", handleKeyDown, true);
      window.removeEventListener("keyup", handleKeyUp, true);
      window.removeEventListener("blur", handleWindowBlur);

      if (unlistenTabSwitchShortcut) {
        unlistenTabSwitchShortcut();
      }
    };
  }, [isRootShellMode]);

  useEffect(() => {
    if (!draggingTabId) {
      setVisualTabOrder(null);
      visualTabOrderRef.current = null;
      return;
    }

    const currentTabIds = tabs.map((tab) => tab.id);

    if (!currentTabIds.some((tabId) => tabId === draggingTabId)) {
      setDraggingTabId(null);
      setVisualTabOrder(null);
      visualTabOrderRef.current = null;
      return;
    }

    setVisualTabOrder((currentOrder) => {
      const baseOrder = currentOrder ?? currentTabIds;
      const nextOrder = baseOrder.filter((tabId) =>
        currentTabIds.includes(tabId),
      );

      for (const tabId of currentTabIds) {
        if (!nextOrder.includes(tabId)) {
          nextOrder.push(tabId);
        }
      }

      return areSameTabOrder(baseOrder, nextOrder) ? baseOrder : nextOrder;
    });
  }, [draggingTabId, tabs]);

  useEffect(() => {
    visualTabOrderRef.current = visualTabOrder;
  }, [visualTabOrder]);

  const finalizeDrag = (
    activeTabIdForDrag: string | null,
    shouldCommit: boolean,
  ) => {
    if (shouldCommit && activeTabIdForDrag) {
      const finalOrder = visualTabOrderRef.current ?? tabs.map((tab) => tab.id);
      const finalIndex = finalOrder.indexOf(activeTabIdForDrag);

      if (finalIndex !== -1) {
        reorderTab(activeTabIdForDrag, finalIndex);
      }

      suppressClickTabIdRef.current = activeTabIdForDrag;
    }

    setHoveredTabId(null);
    setDraggingTabId(null);
    setVisualTabOrder(null);
    visualTabOrderRef.current = null;
  };

  const handleDragStart = (event: DragStartEvent) => {
    const activeTabForDrag = String(event.active.id);

    setDraggingTabId(activeTabForDrag);
    setHoveredTabId(activeTabForDrag);
    suppressClickTabIdRef.current = null;
    setVisualTabOrder(
      (currentOrder) => currentOrder ?? tabs.map((tab) => tab.id),
    );
  };

  const handleDragOver = (event: DragOverEvent) => {
    const activeTabForDrag = String(event.active.id);
    const overTab = event.over?.id ? String(event.over.id) : null;

    if (!overTab || activeTabForDrag === overTab) {
      return;
    }

    setVisualTabOrder((currentOrder) => {
      const baseOrder = currentOrder ?? tabs.map((tab) => tab.id);
      const activeIndex = baseOrder.indexOf(activeTabForDrag);
      const overIndex = baseOrder.indexOf(overTab);

      if (activeIndex === -1 || overIndex === -1 || activeIndex === overIndex) {
        return baseOrder;
      }

      return arrayMove(baseOrder, activeIndex, overIndex);
    });
  };

  const handleDragEnd = (event: DragEndEvent) => {
    finalizeDrag(String(event.active.id), true);
  };

  const handleDragCancel = (_event: DragCancelEvent) => {
    finalizeDrag(draggingTabId, false);
  };

  return {
    pathname,
    activeTabId,
    navigationState,
    goBack,
    goForward,
    navigate,
    repositories: repositories as RepositoryInfo[],
    isRootShellMode,
    sensors,
    orderedTabIds,
    orderedTabs,
    tabById,
    draggingTabId,
    hoveredTabId,
    setHoveredTabId,
    isTabDragInProgress,
    suppressClickTabIdRef,
    handleActivateTab,
    handleCreateTab,
    handleCloseTab,
    handleDragStart,
    handleDragOver,
    handleDragEnd,
    handleDragCancel,
    isTabSwitcherOpen,
    tabSwitcherTabIds,
    tabSwitcherIndex,
  };
}

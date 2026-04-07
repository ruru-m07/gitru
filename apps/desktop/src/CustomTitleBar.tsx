import {
  closestCenter,
  DndContext,
  type DragCancelEvent,
  type DragEndEvent,
  type DragOverEvent,
  type DragStartEvent,
  PointerSensor,
  useSensor,
  useSensors,
} from "@dnd-kit/core";
import {
  restrictToHorizontalAxis,
  restrictToParentElement,
} from "@dnd-kit/modifiers";
import {
  arrayMove,
  horizontalListSortingStrategy,
  SortableContext,
  useSortable,
} from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import { type RepositoryInfo } from "@gitru/commands";
import { Inbox } from "@gitru/icon";
import {
  Avatar,
  AvatarFallback,
  AvatarImage,
} from "@gitru/ui/components/avatar";
import { Button } from "@gitru/ui/components/button";
import { cn } from "@gitru/ui/lib/utils";
import { useNavigate, useRouterState } from "@tanstack/react-router";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import {
  ArrowLeft,
  ArrowRight,
  ChevronLeft,
  ChevronRight,
  Plus,
  X,
} from "lucide-react";
import {
  type MouseEvent,
  type ReactNode,
  useEffect,
  useRef,
  useState,
} from "react";
import { useSessionNavigation } from "./hooks/useSessionNavigation";
import { emitWebviewNavigation } from "./lib/emitWebviewNavigation";
import { getAvatarByProvider } from "./lib/getAvatarByGitProvider";
import { parseOrigin } from "./lib/parseOrigin";
import {
  TAB_SWITCH_CYCLE_MODE,
  TAB_SWITCH_SHORTCUT_EVENT,
  type TabSwitchShortcutPayload,
} from "./lib/tabSwitching";
import { appState } from "./state";
import { repoContextRegistry } from "./state/core/RepoContextRegistry";
import { useAppStore } from "./store/useAppStore";

type CustomTitleBarProps = {
  restrictedPaths: string[];
};

const DEFAULT_TAB_ROUTE = "/app/git";
const DND_MODIFIERS = [restrictToHorizontalAxis, restrictToParentElement];
const SORTABLE_TAB_TRANSITION = {
  duration: 170,
  easing: "cubic-bezier(0.22, 1, 0.36, 1)",
};
const TAB_SWITCHER_HEADER_HEIGHT_PX = 240;
const MAIN_HEADER_HEIGHT_CSS_VAR = "--main-custom-header-height";

const areSameTabOrder = (left: string[], right: string[]) =>
  left.length === right.length &&
  left.every((value, index) => value === right[index]);

type SortableTabShellProps = {
  id: string;
  className?: string;
  children: ReactNode;
};

type TabSwitchModifier = "Control" | "Meta";

const SortableTabShell = ({
  id,
  className,
  children,
}: SortableTabShellProps) => {
  const {
    attributes,
    listeners,
    setNodeRef,
    transform,
    transition,
    isDragging,
  } = useSortable({
    id,
    transition: SORTABLE_TAB_TRANSITION,
  });

  return (
    <div
      ref={setNodeRef}
      style={{
        transform: CSS.Transform.toString(transform),
        transition,
        zIndex: isDragging ? 30 : undefined,
      }}
      className={className}
      {...attributes}
      {...listeners}
    >
      {children}
    </div>
  );
};

const isTauriRuntime = () =>
  typeof window !== "undefined" &&
  typeof (window as Window & { __TAURI_INTERNALS__?: unknown })
    .__TAURI_INTERNALS__ !== "undefined";

const isEmbeddedRuntime = () => {
  if (!isTauriRuntime()) {
    return false;
  }

  try {
    return window.location.search.includes("embedded=1") ||
      window.location.search.includes("embedded=true")
      ? true
      : ((
          window as Window & {
            __TAURI_INTERNALS__?: {
              metadata?: { currentWebview?: { label?: string } };
            };
          }
        ).__TAURI_INTERNALS__?.metadata?.currentWebview?.label?.startsWith(
          "tab-webview:",
        ) ?? false);
  } catch {
    return false;
  }
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

const getTitleFromRoute = (routePath: string) => {
  const pathname = getRoutePathname(routePath);

  if (pathname.startsWith("/app/")) {
    const segment = pathname.slice("/app/".length).split("/")[0];

    if (segment) {
      return segment.charAt(0).toUpperCase() + segment.slice(1);
    }
  }

  return "Workspace";
};

const isGitRoute = (routePath: string) =>
  getRoutePathname(routePath).startsWith("/app/git");

const normalizeTabRoutePath = (routePath: string | null | undefined) => {
  if (!routePath) {
    return DEFAULT_TAB_ROUTE;
  }

  const pathname = getRoutePathname(routePath);
  if (pathname === "/app" || pathname === "/app/") {
    return DEFAULT_TAB_ROUTE;
  }

  return routePath;
};

const renderTitleForGitPage = ({
  repository,
  isActive,
}: {
  repository: RepositoryInfo | null;
  isActive: boolean;
}) => {
  if (!repository) {
    return <span className="truncate font-medium">Git</span>;
  }

  const origin = parseOrigin(repository.origin);

  if (!origin) {
    return (
      <span className="truncate font-medium">
        {repository.name}
        {repository.current_branch ? ` > ${repository.current_branch}` : ""}
      </span>
    );
  }

  const providerIcon = getAvatarByProvider(
    origin.provider,
    cn("size-2.5 rounded-full", isActive ? "" : "bg-secondary"),
  );
  const textClass = isActive ? "text-foreground" : "text-muted-foreground";

  return (
    <div className="flex min-w-0 items-center">
      <div className="relative shrink-0">
        <Avatar className="size-4.5 rounded-sm">
          <AvatarImage alt={origin.owner} src={origin.avatarUrl} />
          <AvatarFallback>
            {origin.owner.charAt(0).toUpperCase()}
          </AvatarFallback>
        </Avatar>
        {providerIcon ? (
          <span
            className={cn(
              "absolute -inset-e-1 -bottom-1 rounded-full p-0.5",
              isActive ? "bg-background" : "bg-secondary",
            )}
          >
            {providerIcon}
          </span>
        ) : null}
      </div>
      {/* <div className="relative shrink-0">
        {providerIcon ? <span className="size-4.5">{providerIcon}</span> : null}
        <Avatar className="rounded-sm absolute ring ring-background -inset-e-0.5 -bottom-0.5 bg-background p-0.5">
          <AvatarImage alt={origin.owner} src={origin.avatarUrl} />
          <AvatarFallback>
            {origin.owner.charAt(0).toUpperCase()}
          </AvatarFallback>
        </Avatar>
      </div> */}

      <span className="ml-2 flex min-w-0 items-center gap-1 text-sm">
        <span className="truncate text-muted-foreground">{origin.owner}</span>
        <span className="text-muted-foreground">/</span>
        <span className={cn("truncate font-medium", textClass)}>
          {origin.repo}
        </span>
        <span className="text-muted-foreground mx-1">&gt;</span>
        <span className={cn("flex min-w-0 items-center gap-1", textClass)}>
          <span className="truncate">
            {repository.current_branch ?? "detached"}
          </span>
        </span>
      </span>
    </div>
  );
};

const CustomTitleBar = ({ restrictedPaths = [] }: CustomTitleBarProps) => {
  const routerState = useRouterState();
  const navigate = useNavigate();
  const pathname = routerState.location.pathname;
  const routePath = routerState.location.href;

  const activeTabId = useAppStore((state) => state.activeTabId);
  const { navigationState, goBack, goForward, pushToHistory } =
    useSessionNavigation(activeTabId);

  const selectedRepository = useAppStore((state) => state.selectedRepository);
  const repositories = useAppStore((state) => state.repositories);

  const tabs = useAppStore((state) => state.tabs);
  const [hoveredTabId, setHoveredTabId] = useState<string | null>(null);
  const ensureActiveTab = useAppStore((state) => state.ensureActiveTab);
  const createTab = useAppStore((state) => state.createTab);
  const activateTab = useAppStore((state) => state.activateTab);
  const closeTab = useAppStore((state) => state.closeTab);
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
  const isRootShellMode = isTauriRuntime() && !isEmbeddedRuntime();
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

  const disposeRuntimeForTab = (tabId: string) => {
    const tab = tabs.find((item) => item.id === tabId);
    const contextEntry = repoContextRegistry.getScopeContext(tabId);

    if (tab?.repositoryId && contextEntry?.contextId) {
      const repository =
        repositories.find((repo) => repo.id === tab.repositoryId) ?? null;

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

    // In root shell mode, route changes happen inside child webviews.
    // Avoid overwriting the active tab route with the shell route (`/app`).
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

  // Track route changes and push to session history
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
    const isGitSourceRoute = isGitRoute(sourceRoutePath);

    createTab({
      routePath: sourceRoutePath,
      repositoryId: selectedRepository?.id ?? null,
      title: isGitSourceRoute
        ? (selectedRepository?.name ?? getTitleFromRoute(sourceRoutePath))
        : getTitleFromRoute(sourceRoutePath),
    });
  };

  const handleCloseTab = async (
    tabId: string,
    event: MouseEvent<HTMLButtonElement>,
  ) => {
    event.stopPropagation();

    if (tabs.length <= 1) {
      return;
    }

    const wasActive = tabId === activeTabId;
    closeTab(tabId);
    disposeRuntimeForTab(tabId);

    if (!wasActive) {
      return;
    }
  };

  const getMruCandidateTabIds = () => {
    const currentTabs = tabsRef.current;
    const activeId = activeTabIdRef.current;
    const orderedTabIds = currentTabs.map((tab) => tab.id);
    const mruTabIds = mruTabIdsRef.current.filter((tabId) =>
      orderedTabIds.includes(tabId),
    );
    const remainingTabIds = orderedTabIds.filter(
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

  const closeTabSwitcher = () => {
    tabSwitcherOpenRef.current = false;
    tabSwitcherTabIdsRef.current = [];
    tabSwitcherIndexRef.current = 0;
    tabSwitchModifierRef.current = null;

    setIsTabSwitcherOpen(false);
    setTabSwitcherTabIds([]);
    setTabSwitcherIndex(0);
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
  }, [handleTabSwitchAdvance, handleTabSwitchCommit]);

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

    let unlistenTabSwitchShortcut: (() => void) | undefined;

    const registerTabSwitchShortcutListener = async () => {
      if (!isTauriRuntime()) {
        return;
      }

      unlistenTabSwitchShortcut =
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

            handleTabSwitchCommitRef.current();
          },
        );
    };

    void registerTabSwitchShortcutListener();

    return () => {
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

  const finalizeDrag = (activeTab: string | null, shouldCommit: boolean) => {
    if (shouldCommit && activeTab) {
      const finalOrder = visualTabOrderRef.current ?? tabs.map((tab) => tab.id);
      const finalIndex = finalOrder.indexOf(activeTab);

      if (finalIndex !== -1) {
        reorderTab(activeTab, finalIndex);
      }

      suppressClickTabIdRef.current = activeTab;
    }

    setHoveredTabId(null);
    setDraggingTabId(null);
    setVisualTabOrder(null);
    visualTabOrderRef.current = null;
  };

  const handleDragStart = (event: DragStartEvent) => {
    const activeTab = String(event.active.id);

    setDraggingTabId(activeTab);
    setHoveredTabId(activeTab);
    suppressClickTabIdRef.current = null;
    setVisualTabOrder(
      (currentOrder) => currentOrder ?? tabs.map((tab) => tab.id),
    );
  };

  const handleDragOver = (event: DragOverEvent) => {
    const activeTab = String(event.active.id);
    const overTab = event.over?.id ? String(event.over.id) : null;

    if (!overTab || activeTab === overTab) {
      return;
    }

    setVisualTabOrder((currentOrder) => {
      const baseOrder = currentOrder ?? tabs.map((tab) => tab.id);
      const activeIndex = baseOrder.indexOf(activeTab);
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

  if (restrictedPaths.includes(pathname)) {
    return null;
  }

  return (
    <div
      className="h-(--main-custom-header-height) relative mr-1 flex items-center pl-4 select-none z-10"
      data-tauri-drag-region
      style={{
        // @ts-expect-error - ¯\_(ツ)_/¯
        WebkitAppRegion: "drag",
      }}
    >
      <div
        className="absolute flex w-fit items-center pr-4"
        style={{
          // @ts-expect-error - ¯\_(ツ)_/¯
          WebkitAppRegion: "no-drag",
          paddingLeft: "70px",
        }}
      >
        <div className="flex items-center mr-3 translate-y-px">
          <Button
            onClick={() => {
              console.log(
                "[NavButton] Back clicked. Session:",
                activeTabId,
                "canGoBack:",
                navigationState?.can_go_back,
              );
              void goBack().then((state) => {
                console.log("[NavButton] goBack returned:", state);
                if (state?.current_path && activeTabId) {
                  console.log("[NavButton] Navigating to:", state.current_path);
                  if (isRootShellMode) {
                    // In root shell mode, emit event to embedded webview
                    void emitWebviewNavigation(
                      activeTabId,
                      state.current_path,
                      "back",
                    );
                  } else {
                    // In embedded mode, use local router
                    void navigate({ to: state.current_path });
                  }
                }
              });
            }}
            disabled={!navigationState?.can_go_back}
            size={"icon-sm"}
            className="hover:bg-foreground/7! [&_svg]:size-4.75! [&_svg]:stroke-[1.5]"
            variant="ghost"
          >
            <ChevronLeft size={16} aria-hidden="true" />
          </Button>
          <Button
            onClick={() => {
              console.log(
                "[NavButton] Forward clicked. Session:",
                activeTabId,
                "canGoForward:",
                navigationState?.can_go_forward,
              );
              void goForward().then((state) => {
                console.log("[NavButton] goForward returned:", state);
                if (state?.current_path && activeTabId) {
                  console.log("[NavButton] Navigating to:", state.current_path);
                  if (isRootShellMode) {
                    // In root shell mode, emit event to embedded webview
                    void emitWebviewNavigation(
                      activeTabId,
                      state.current_path,
                      "forward",
                    );
                  } else {
                    // In embedded mode, use local router
                    void navigate({ to: state.current_path });
                  }
                }
              });
            }}
            disabled={!navigationState?.can_go_forward}
            size={"icon-sm"}
            className="hover:bg-foreground/7! [&_svg]:size-4.75! [&_svg]:stroke-[1.5]"
            variant="ghost"
          >
            <ChevronRight aria-hidden="true" />
          </Button>
        </div>
        <div className="-translate-x-3 flex w-fit items-center h-[calc(var(--main-custom-header-height)-0px)]">
          <div className="relative ml-2 flex min-w-0 flex-1 items-center gap-1 h-full pt-1">
            <DndContext
              sensors={sensors}
              collisionDetection={closestCenter}
              modifiers={DND_MODIFIERS}
              onDragStart={handleDragStart}
              onDragOver={handleDragOver}
              onDragEnd={handleDragEnd}
              onDragCancel={handleDragCancel}
            >
              <SortableContext
                items={orderedTabIds}
                strategy={horizontalListSortingStrategy}
              >
                {orderedTabs.map((tab, index) => {
                  const isActive = tab.id === activeTabId;
                  const isDraggingTab = tab.id === draggingTabId;
                  const isHovered =
                    tab.id === hoveredTabId &&
                    (!isTabDragInProgress || isDraggingTab);
                  const tabRepository = tab.repositoryId
                    ? (repositories.find(
                        (repo) => repo.id === tab.repositoryId,
                      ) ?? null)
                    : null;
                  const showGitTitle = isGitRoute(tab.routePath);
                  const nextTab = orderedTabs[index + 1] ?? null;
                  const shouldHideSeparator =
                    !!nextTab &&
                    (isActive ||
                      isHovered ||
                      nextTab.id === activeTabId ||
                      nextTab.id === hoveredTabId);

                  return (
                    <SortableTabShell
                      key={tab.id}
                      id={tab.id}
                      className="relative h-full"
                    >
                      <div className="relative flex items-end h-full">
                        {isActive && (
                          <svg
                            width="15"
                            height="15"
                            viewBox="0 0 15 15"
                            fill="none"
                            xmlns="http://www.w3.org/2000/svg"
                            className="filter-[drop-shadow(-1px_-1px_1px_#00000011)] absolute -left-3.75 bottom-0"
                          >
                            <path
                              d="M15 15H0C8.28427 15 15 8.28427 15 0V15Z"
                              fill="var(--background)"
                            />
                          </svg>
                        )}
                        <div
                          role="button"
                          tabIndex={0}
                          onMouseDown={() => {
                            if (suppressClickTabIdRef.current === tab.id) {
                              suppressClickTabIdRef.current = null;
                              return;
                            }

                            void handleActivateTab(tab.id);
                          }}
                          onMouseEnter={() => {
                            if (isTabDragInProgress && !isDraggingTab) {
                              return;
                            }

                            setHoveredTabId(tab.id);
                          }}
                          onMouseLeave={() => {
                            if (isTabDragInProgress && !isDraggingTab) {
                              return;
                            }

                            setHoveredTabId((currentHoveredTabId) =>
                              currentHoveredTabId === tab.id
                                ? null
                                : currentHoveredTabId,
                            );
                          }}
                          onKeyDown={(event) => {
                            if (event.key === "Enter" || event.key === " ") {
                              event.preventDefault();
                              void handleActivateTab(tab.id);
                            }
                          }}
                          className={cn(
                            "group flex h-full min-w-44 max-w-72 shrink-0 items-center gap-1 text-sm rounded-t-2xl pb-1 touch-none",
                            isActive
                              ? "bg-background flex items-center [box-shadow:-1px_-1px_1px_0px_#00000011,1px_-1px_1px_0px_#00000011]"
                              : "bg-transparent text-muted-foreground",
                          )}
                        >
                          <div
                            className={cn(
                              "flex max-w-full w-full items-center gap-1 h-full pl-1.5 pr-1 rounded-[12px]",
                              !isActive &&
                                (!isTabDragInProgress || isDraggingTab) &&
                                "hover:bg-foreground/10",
                            )}
                          >
                            {showGitTitle ? (
                              renderTitleForGitPage({
                                repository: tabRepository,
                                isActive,
                              })
                            ) : tab.routePath === "/app/inbox" ? (
                              <div className="flex items-center gap-1.5">
                                <Inbox className={"size-4"} />
                                <span>
                                  Inbox{" "}
                                  <span className="text-muted-foreground/70">
                                    (5)
                                  </span>
                                </span>
                              </div>
                            ) : (
                              <span className="truncate font-medium">
                                {tab.title}
                              </span>
                            )}
                            <Button
                              size={"icon-xs"}
                              variant={"ghost"}
                              data-tab-close-button="true"
                              className={cn(
                                "ms-auto h-5 w-5 rounded-full",
                                isActive
                                  ? "text-muted-foreground hover:text-foreground"
                                  : "text-muted-foreground/70",
                              )}
                              onPointerDownCapture={(event) => {
                                event.stopPropagation();
                              }}
                              onClick={(event) => {
                                void handleCloseTab(tab.id, event);
                              }}
                            >
                              <X size={12} aria-hidden="true" />
                            </Button>
                          </div>
                        </div>
                        {isActive && (
                          <svg
                            width="15"
                            height="15"
                            viewBox="0 0 15 15"
                            fill="none"
                            xmlns="http://www.w3.org/2000/svg"
                            className="filter-[drop-shadow(1px_-1px_1px_#00000011)] absolute -right-3.75 bottom-0"
                          >
                            <path
                              d="M0 15L6.5568e-07 0C2.93563e-07 8.28427 6.71573 15 15 15L0 15Z"
                              fill="var(--background)"
                            />
                          </svg>
                        )}
                      </div>
                      {nextTab ? (
                        <div
                          aria-hidden="true"
                          className={cn(
                            "absolute -right-px bottom-2",
                            "w-0.5 h-4 bg-foreground/5 mb-1 transition-opacity",
                            shouldHideSeparator && "opacity-0",
                          )}
                        />
                      ) : null}
                    </SortableTabShell>
                  );
                })}
              </SortableContext>
            </DndContext>

            {TAB_SWITCH_CYCLE_MODE === "MRU" &&
            isTabSwitcherOpen &&
            tabSwitcherTabIds.length > 0 ? (
              <div className="pointer-events-none absolute left-1/2 top-12 z-50 -translate-x-1/2">
                <div className="min-w-80 max-w-136 overflow-hidden rounded-xl border border-border/60 bg-background/95 p-1.5 shadow-2xl backdrop-blur">
                  <div className="border-b border-border/50 px-2 py-1 text-[11px] font-medium uppercase tracking-wide text-muted-foreground">
                    Recent Tabs
                  </div>
                  <div className="flex max-h-44 flex-col gap-0.5 overflow-auto p-1">
                    {tabSwitcherTabIds.map((tabId, index) => {
                      const switcherTab = tabById.get(tabId);

                      if (!switcherTab) {
                        return null;
                      }

                      const switcherTabRepository = switcherTab.repositoryId
                        ? (repositories.find(
                            (repo) => repo.id === switcherTab.repositoryId,
                          ) ?? null)
                        : null;
                      const title = isGitRoute(switcherTab.routePath)
                        ? (switcherTabRepository?.name ?? "Git")
                        : switcherTab.title;
                      const subtitle = getRoutePathname(switcherTab.routePath);
                      const isSelected = index === tabSwitcherIndex;

                      return (
                        <div
                          key={tabId}
                          className={cn(
                            "flex min-w-0 items-center gap-3 rounded-md px-2 py-1.5 text-sm",
                            isSelected
                              ? "bg-foreground/10 text-foreground"
                              : "text-muted-foreground",
                          )}
                        >
                          <span className="min-w-0 flex-1 truncate font-medium">
                            {title}
                          </span>
                          <span className="max-w-56 truncate text-xs text-muted-foreground">
                            {subtitle}
                          </span>
                        </div>
                      );
                    })}
                  </div>
                </div>
              </div>
            ) : null}

            <div
              aria-hidden="true"
              className={cn(
                "w-0.5 h-4 bg-foreground/5 mb-1 transition-opacity",
              )}
            />

            <Button
              size={"icon-sm"}
              variant={"ghost"}
              className="mb-1 text-muted-foreground/70"
              onClick={() => {
                void handleCreateTab();
              }}
            >
              <Plus size={16} aria-hidden="true" />
            </Button>
          </div>
        </div>
      </div>
    </div>
  );
};

export default CustomTitleBar;

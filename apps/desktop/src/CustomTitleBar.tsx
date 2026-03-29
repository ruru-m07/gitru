import { type RepositoryInfo } from "@gitru/commands";
import {
  Avatar,
  AvatarFallback,
  AvatarImage,
} from "@gitru/ui/components/avatar";
import { Button } from "@gitru/ui/components/button";
import { cn } from "@gitru/ui/lib/utils";
import {
  useCanGoBack,
  useRouterState,
} from "@tanstack/react-router";
import { ArrowLeft, ArrowRight, Plus, X } from "lucide-react";
import { type MouseEvent, useEffect } from "react";
import { getAvatarByProvider } from "./lib/getAvatarByGitProvider";
import { parseOrigin } from "./lib/parseOrigin";
import { appState } from "./state";
import { repoContextRegistry } from "./state/core/RepoContextRegistry";
import { useAppStore } from "./store/useAppStore";

type CustomTitleBarProps = {
  restrictedPaths: string[];
};

const DEFAULT_TAB_ROUTE = "/app/git";

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
      : (window as Window & { __TAURI_INTERNALS__?: { metadata?: { currentWebview?: { label?: string } } } })
          .__TAURI_INTERNALS__?.metadata?.currentWebview?.label
          ?.startsWith("tab-webview:") ?? false;
  } catch {
    return false;
  }
};

const getRoutePathname = (routePath: string) => {
  try {
    const origin =
      typeof window !== "undefined" ? window.location.origin : "http://localhost";
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
  const pathname = routerState.location.pathname;
  const routePath = routerState.location.href;

  const canGoBack = useCanGoBack();

  const selectedRepository = useAppStore((state) => state.selectedRepository);
  const repositories = useAppStore((state) => state.repositories);

  const tabs = useAppStore((state) => state.tabs);
  const activeTabId = useAppStore((state) => state.activeTabId);
  const ensureActiveTab = useAppStore((state) => state.ensureActiveTab);
  const createTab = useAppStore((state) => state.createTab);
  const activateTab = useAppStore((state) => state.activateTab);
  const closeTab = useAppStore((state) => state.closeTab);
  const syncActiveTab = useAppStore((state) => state.syncActiveTab);

  const activeTab = tabs.find((tab) => tab.id === activeTabId) ?? null;
  const isRootShellMode = isTauriRuntime() && !isEmbeddedRuntime();
  const effectiveRoutePath = isRootShellMode
    ? normalizeTabRoutePath(activeTab?.routePath)
    : routePath;
  const isGitEffectiveRoute = isGitRoute(effectiveRoutePath);
  const effectiveTitle = isGitEffectiveRoute
    ? (selectedRepository?.name ?? getTitleFromRoute(effectiveRoutePath))
    : getTitleFromRoute(effectiveRoutePath);

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

  if (restrictedPaths.includes(pathname)) {
    return null;
  }

  return (
    <div
      className="h-(--main-custom-header-height) relative mr-1 flex items-center pl-4 select-none"
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
        <div className="flex items-center mr-3 translate-y-0.5">
          <Button
            onClick={() => window.history.back()}
            disabled={!canGoBack}
            size={"icon"}
            className="size-7"
            variant="ghost"
          >
            <ArrowLeft size={16} aria-hidden="true" />
          </Button>
          <Button
            onClick={() => window.history.forward()}
            disabled={true}
            size={"icon"}
            className="size-7"
            variant="ghost"
          >
            <ArrowRight size={16} aria-hidden="true" />
          </Button>
        </div>
        <div className="-translate-x-2 flex w-fit items-center h-[calc(var(--main-custom-header-height)-0px)]">
          <div className="ml-2 flex min-w-0 flex-1 items-center gap-1 h-full pt-1">
            {tabs.map((tab) => {
              const isActive = tab.id === activeTabId;
              const tabRepository = tab.repositoryId
                ? (repositories.find((repo) => repo.id === tab.repositoryId) ??
                  null)
                : null;
              const showGitTitle = isGitRoute(tab.routePath);

              return (
                <div key={tab.id} className="relative flex items-end h-full">
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
                    onClick={() => {
                      void handleActivateTab(tab.id);
                    }}
                    onKeyDown={(event) => {
                      if (event.key === "Enter" || event.key === " ") {
                        event.preventDefault();
                        void handleActivateTab(tab.id);
                      }
                    }}
                    className={cn(
                      "group flex h-full min-w-44 max-w-72 shrink-0 items-center gap-1 pl-2.5 pr-1 text-sm rounded-t-2xl",
                      isActive
                        ? "bg-background flex items-center [box-shadow:-1px_-1px_1px_0px_#00000011,1px_-1px_1px_0px_#00000011]"
                        : "bg-transparent hover:bg-foreground/10 text-muted-foreground rounded-xl",
                    )}
                  >
                    {showGitTitle ? (
                      renderTitleForGitPage({
                        repository: tabRepository,
                        isActive,
                      })
                    ) : (
                      <span className="truncate font-medium">{tab.title}</span>
                    )}
                    <Button
                      size={"icon-xs"}
                      variant={"ghost"}
                      className={cn(
                        "ms-auto h-5 w-5 rounded-full",
                        isActive
                          ? "text-muted-foreground hover:text-foreground"
                          : "text-muted-foreground/70",
                      )}
                      onClick={(event) => {
                        void handleCloseTab(tab.id, event);
                      }}
                    >
                      <X size={12} aria-hidden="true" />
                    </Button>
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
              );
            })}
            <div className="w-0.5 h-4 bg-foreground/10" />

            <Button
              size={"icon-sm"}
              variant={"ghost"}
              className="_-translate-y-0.5"
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

import {
  closestCenter,
  DndContext,
  type DragCancelEvent,
  type DragEndEvent,
  type DragOverEvent,
  type DragStartEvent,
  type SensorDescriptor,
  type SensorOptions,
} from "@dnd-kit/core";
import {
  horizontalListSortingStrategy,
  SortableContext,
} from "@dnd-kit/sortable";
import { type RepositoryInfo } from "@gitru/commands";
import { Inbox } from "@gitru/icon";
import { Button } from "@gitru/ui/components/button";
import { cn } from "@gitru/ui/lib/utils";
import { Plus, X } from "lucide-react";
import { type MouseEvent, type RefObject } from "react";

import { TAB_SWITCH_CYCLE_MODE } from "@/lib/tab-switching";

import { DND_MODIFIERS } from "./constants";
import { GitTabTitle } from "./git-tab-title";
import { SortableTabShell } from "./sortable-tab-shell";
import { getRoutePathname, isGitRoute } from "./utils";

type TabRecord = {
  id: string;
  routePath: string;
  title: string;
  repositoryId?: string | null;
};

type TabListProps = {
  sensors: SensorDescriptor<SensorOptions>[];
  orderedTabIds: string[];
  orderedTabs: TabRecord[];
  tabById: Map<string, TabRecord>;
  activeTabId: string | null;
  draggingTabId: string | null;
  hoveredTabId: string | null;
  setHoveredTabId: (
    id: string | null | ((current: string | null) => string | null),
  ) => void;
  isTabDragInProgress: boolean;
  suppressClickTabIdRef: RefObject<string | null>;
  repositories: RepositoryInfo[];
  handleActivateTab: (tabId: string) => Promise<void>;
  handleCreateTab: () => Promise<void>;
  handleCloseTab: (
    tabId: string,
    event: MouseEvent<HTMLButtonElement>,
  ) => Promise<void>;
  handleDragStart: (event: DragStartEvent) => void;
  handleDragOver: (event: DragOverEvent) => void;
  handleDragEnd: (event: DragEndEvent) => void;
  handleDragCancel: (event: DragCancelEvent) => void;
  isTabSwitcherOpen: boolean;
  tabSwitcherTabIds: string[];
  tabSwitcherIndex: number;
};

export const TabList = ({
  sensors,
  orderedTabIds,
  orderedTabs,
  tabById,
  activeTabId,
  draggingTabId,
  hoveredTabId,
  setHoveredTabId,
  isTabDragInProgress,
  suppressClickTabIdRef,
  repositories,
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
}: TabListProps) => (
  <div
    data-tab-list="true"
    className="relative ml-2 flex h-full min-w-0 flex-1 items-center gap-1 pt-1"
  >
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
            tab.id === hoveredTabId && (!isTabDragInProgress || isDraggingTab);
          const tabRepository = tab.repositoryId
            ? (repositories.find((repo) => repo.id === tab.repositoryId) ??
              null)
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
              className="workspace-tab-shell title-bar-no-drag relative h-full min-w-0 max-w-72 flex-[1_1_18rem]"
            >
              <div className="relative flex h-full min-w-0 items-end">
                {isActive && (
                  <svg
                    width="15"
                    height="15"
                    viewBox="0 0 15 15"
                    fill="none"
                    xmlns="http://www.w3.org/2000/svg"
                    className="filter-[drop-shadow(-1.2px_-0.5px_1px_#0000001A)] absolute -left-3.75 bottom-0"
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
                    "workspace-tab group flex h-full w-full min-w-0 items-center gap-1 rounded-t-2xl pb-1 text-sm touch-none",
                    isActive
                      ? "bg-background flex items-center [box-shadow:-1px_-1px_1px_0.1px_#0000001A,1px_-1px_1px_0.1px_#0000001A]"
                      : "bg-transparent text-muted-foreground",
                  )}
                >
                  <div
                    className={cn(
                      "flex h-full w-full min-w-0 max-w-full items-center gap-1 rounded-[12px] pr-1 pl-1.5",
                      !isActive &&
                        (!isTabDragInProgress || isDraggingTab) &&
                        "hover:bg-foreground/10",
                    )}
                  >
                    {showGitTitle ? (
                      <GitTabTitle
                        repository={tabRepository}
                        isActive={isActive}
                      />
                    ) : tab.routePath === "/app/inbox" ? (
                      <div className="flex min-w-0 items-center gap-1.5">
                        <Inbox className="size-4 shrink-0" />
                        <span className="truncate">
                          Inbox{" "}
                          <span className="text-muted-foreground/70">(5)</span>
                        </span>
                      </div>
                    ) : (
                      <span className="truncate font-medium">{tab.title}</span>
                    )}
                    <Button
                      size={"icon-xs"}
                      variant={"ghost"}
                      data-tab-close-button="true"
                      data-active={isActive}
                      aria-label={`Close ${tab.title}`}
                      className={cn(
                        "workspace-tab-close ms-auto h-5 w-5 shrink-0 rounded-full",
                        isActive
                          ? "text-muted-foreground hover:text-foreground"
                          : "text-muted-foreground/70",
                      )}
                      onMouseDown={(e) => {
                        e.preventDefault();
                        e.stopPropagation();
                      }}
                      onPointerDownCapture={(event) => {
                        event.stopPropagation();
                      }}
                      onClick={(event) => {
                        event.preventDefault();
                        event.stopPropagation();
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
                    className="filter-[drop-shadow(1.2px_-0.5px_1px_#0000001A)] absolute -right-3.75 bottom-0"
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
        "mb-1 h-4 w-0.5 shrink-0 bg-foreground/5 transition-opacity",
      )}
    />

    <Button
      size={"icon-sm"}
      variant={"ghost"}
      aria-label="New tab"
      className="title-bar-no-drag mb-1 shrink-0 text-muted-foreground/70"
      onClick={() => {
        void handleCreateTab();
      }}
    >
      <Plus size={16} aria-hidden="true" />
    </Button>
  </div>
);

import type { GetStatusResponse } from "@gitru/commands";
import { Stashed } from "@gitru/icon";
import { Button } from "@gitru/ui/components/button";
import { Group, GroupSeparator } from "@gitru/ui/components/group";
import { Input } from "@gitru/ui/components/input";
import {
  InputGroup,
  InputGroupAddon,
  InputGroupInput,
} from "@gitru/ui/components/input-group";
import {
  Menu,
  MenuCheckboxItem,
  MenuGroup,
  MenuGroupLabel,
  MenuPopup,
  MenuTrigger,
} from "@gitru/ui/components/menu";
import { Tabs, TabsList, TabsPanel, TabsTab } from "@gitru/ui/components/tabs";
import {
  ChevronsRight,
  GitBranch,
  History,
  ListFilterPlus,
  SearchIcon,
} from "lucide-react";
import { useMemo, useState } from "react";
import { useFileSelectionStore } from "@/components/diff/useFileSelectionStore";
import { getStatusIcon } from "@/components/getStatusIcon";
import { VirtualizedFileList } from "@/components/VirtualizedFileList";
import {
  useGetCurrentBranchStash,
  useGetStatus,
  useGitAdd,
  useGitDiscard,
  useGitHistoryGraph,
  useGitUnstage,
} from "@/hooks";
import { formatNumber } from "@/lib/formatNumber";
import { resolveFileSelection } from "@/lib/gitSelectionResolver";
import { selectActiveSessionRepoKey, useAppStore } from "@/store/useAppStore";
import {
  DEFAULT_STATUS_FILTERS,
  type FileStatusFilter,
  hasActiveStatusFilters,
  matchesStatusFilters,
} from "../lib/file-status-filters";
import { matchesSearchQuery } from "../lib/matches-search-query";
import { getVisibleFilePaths } from "../lib/visible-file-paths";
import { DiscardChangesDialog } from "./discard-changes-dialog";
import { HistoryCommitInfiniteList } from "./history-commit-infinite-list";
import { WriteCommitBox } from "./write-commit-box";

export function ListFileChanges({
  activeTab,
  onTabChange,
  onOpenHistoryView,
  onOpenStashView,
}: {
  activeTab: "changes" | "history";
  onTabChange: (tab: "changes" | "history") => void;
  onOpenHistoryView: (commitHash: string) => void;
  onOpenStashView: (stashReference: string | null) => void;
}) {
  const [changesQuery, setChangesQuery] = useState("");
  const [historySearchInput, setHistorySearchInput] = useState("");
  const [statusFilters, setStatusFilters] = useState<
    Record<FileStatusFilter, boolean>
  >(DEFAULT_STATUS_FILTERS);

  const repoStateKey = useAppStore(selectActiveSessionRepoKey);
  const setWorktreeSelectionForRepo = useAppStore(
    (state) => state.setWorktreeSelectionForRepo,
  );
  const clearWorktreeSelectionForRepo = useAppStore(
    (state) => state.clearWorktreeSelectionForRepo,
  );
  const repoSelectionState = useAppStore((state) =>
    repoStateKey ? state.selectionByRepo[repoStateKey] : undefined,
  );
  const setMainWindowView = useAppStore((state) => state.setMainWindowView);

  const { data: status, isLoading: isStatusLoading } = useGetStatus();

  const { mutateAsync: addFile } = useGitAdd();
  const { mutateAsync: unstageFile } = useGitUnstage();
  const { mutateAsync: discardChanges } = useGitDiscard();

  const { data: currentBranchStash } = useGetCurrentBranchStash();

  const handleFileClick = useFileSelectionStore(
    (state) => state.handleFileClick,
  );

  const historySearch = historySearchInput.trim();
  const historyQuery = useMemo(
    () => ({
      limit: 50,
      search: historySearch.length > 0 ? historySearch : undefined,
      include_local: true,
      include_remotes: false,
      include_tags: false,
      include_stash: false,
    }),
    [historySearch],
  );
  const {
    data: historyData,
    fetchNextPage,
    hasNextPage,
    isFetchingNextPage,
    isLoading: isHistoryLoading,
  } = useGitHistoryGraph(historyQuery);
  const historyRows = useMemo(
    () =>
      (historyData?.pages.flatMap((page) => page.rows) ?? []).filter(
        (row) => row.type === "Commit",
      ),
    [historyData],
  );
  const worktreeSelection = repoSelectionState?.worktree ?? null;
  const resolvedWorktreeSelection = resolveFileSelection({
    selection: worktreeSelection,
    files: status?.files ?? [],
    context: {
      source: "worktree",
    },
  });
  const selectedFileForList =
    resolvedWorktreeSelection.state === "valid"
      ? resolvedWorktreeSelection.identity
      : worktreeSelection;

  const hasFilterSelection = hasActiveStatusFilters(statusFilters);
  const toggleStatusFilter = (filter: FileStatusFilter, checked: boolean) => {
    setStatusFilters((prev) => ({
      ...prev,
      [filter]: checked,
    }));
  };

  const stagedChanges: GetStatusResponse["files"] = (
    status?.files ?? []
  ).filter(
    (file) =>
      file.status.some((s) => s.startsWith("Index")) &&
      matchesStatusFilters(file, statusFilters) &&
      matchesSearchQuery(file.path, changesQuery),
  );
  const unstagedChanges: GetStatusResponse["files"] = (
    status?.files ?? []
  ).filter(
    (file) =>
      file.status.some((s) => s.startsWith("Worktree")) &&
      matchesStatusFilters(file, statusFilters) &&
      matchesSearchQuery(file.path, changesQuery),
  );
  const conflictedChanges: GetStatusResponse["files"] = (
    status?.files ?? []
  ).filter(
    (file) =>
      file.status.some((s) => s.includes("Conflicted")) &&
      matchesStatusFilters(file, statusFilters) &&
      matchesSearchQuery(file.path, changesQuery),
  );
  const visibleConflictPaths = useMemo(
    () => getVisibleFilePaths(conflictedChanges),
    [conflictedChanges],
  );
  const visibleStagedPaths = useMemo(
    () => getVisibleFilePaths(stagedChanges),
    [stagedChanges],
  );
  const visibleUnstagedPaths = useMemo(
    () => getVisibleFilePaths(unstagedChanges),
    [unstagedChanges],
  );
  const visibleAddablePaths = useMemo(
    () =>
      Array.from(new Set([...visibleConflictPaths, ...visibleUnstagedPaths])),
    [visibleConflictPaths, visibleUnstagedPaths],
  );

  return (
    <Tabs
      value={activeTab === "changes" ? "tab-1" : "tab-2"}
      onValueChange={(value) => {
        const nextTab = value === "tab-2" ? "history" : "changes";

        if (nextTab === activeTab) {
          return;
        }

        onTabChange(nextTab);
      }}
      className={"gap-0 h-full flex flex-col"}
    >
      <TabsList
        className={
          "select-none rounded-none bg-background w-full shrink-0 border-b *:data-[slot=tab-indicator]:bg-secondary *:data-[slot=tab-indicator]:transition-none"
        }
      >
        <TabsTab className={"rounded-none!"} value="tab-1">
          Changes
        </TabsTab>
        <TabsTab className={"rounded-none!"} value="tab-2">
          History
        </TabsTab>
      </TabsList>
      <TabsPanel
        value="tab-1"
        className={"flex-1 flex flex-col min-h-0"}
        tabIndex={-1}
      >
        <div className="p-1.5 max-h-10 min-h-10 border-b">
          <Group aria-label="Subscription actions" className="w-full">
            <InputGroup>
              <InputGroupInput
                aria-label="Filter files"
                placeholder="Filter files..."
                className={"rounded-l-md! border-border! w-full"}
                size={"sm"}
                value={changesQuery}
                onChange={(e) => setChangesQuery(e.target.value)}
              />
              <InputGroupAddon>
                <SearchIcon
                  className="opacity-50 -translate-x-0.5"
                  aria-hidden="true"
                />
              </InputGroupAddon>
            </InputGroup>

            <GroupSeparator />
            <Menu>
              <MenuTrigger
                render={
                  <Button
                    aria-label="Copy options"
                    size="icon-sm"
                    variant={"outline"}
                    className="relative"
                  />
                }
              >
                <ListFilterPlus aria-hidden="true" className="size-4" />
                {hasFilterSelection && (
                  <span className="absolute top-1 right-1 h-1.5 w-1.5 rounded-full bg-primary" />
                )}
              </MenuTrigger>
              <MenuPopup align="end" className={"w-45"}>
                <MenuGroup>
                  <MenuGroupLabel className={"justify-between flex"}>
                    <span>Filter by status</span>
                    {hasFilterSelection && (
                      <span
                        className="font-normal hover:underline cursor-pointer"
                        onClick={() => {
                          setStatusFilters(DEFAULT_STATUS_FILTERS);
                        }}
                      >
                        clear
                      </span>
                    )}
                  </MenuGroupLabel>
                  <MenuCheckboxItem
                    checked={statusFilters.untracked}
                    onCheckedChange={(checked) =>
                      toggleStatusFilter("untracked", Boolean(checked))
                    }
                    variant="switch"
                  >
                    <span className="flex gap-1.5">
                      {getStatusIcon(["WorktreeNew"], 18)}
                      Untracked
                    </span>
                  </MenuCheckboxItem>
                  <MenuCheckboxItem
                    checked={statusFilters.modified}
                    onCheckedChange={(checked) =>
                      toggleStatusFilter("modified", Boolean(checked))
                    }
                    variant="switch"
                  >
                    <span className="flex gap-1.5">
                      {getStatusIcon(["IndexModified"], 18)}
                      Modified
                    </span>
                  </MenuCheckboxItem>
                  <MenuCheckboxItem
                    checked={statusFilters.renamed}
                    onCheckedChange={(checked) =>
                      toggleStatusFilter("renamed", Boolean(checked))
                    }
                    variant="switch"
                  >
                    <span className="flex gap-1.5">
                      {getStatusIcon(["IndexRenamed"], 18)}
                      Renamed
                    </span>
                  </MenuCheckboxItem>
                  <MenuCheckboxItem
                    checked={statusFilters.deleted}
                    onCheckedChange={(checked) =>
                      toggleStatusFilter("deleted", Boolean(checked))
                    }
                    variant="switch"
                  >
                    <span className="flex gap-1.5">
                      {getStatusIcon(["IndexDeleted"], 18)}
                      Deleted
                    </span>
                  </MenuCheckboxItem>
                  <MenuCheckboxItem
                    checked={statusFilters.conflicted}
                    onCheckedChange={(checked) =>
                      toggleStatusFilter("conflicted", Boolean(checked))
                    }
                    variant="switch"
                  >
                    <span className="flex gap-1.5">
                      {getStatusIcon(["Conflicted"], 18)}
                      Conflicted
                    </span>
                  </MenuCheckboxItem>
                </MenuGroup>
              </MenuPopup>
            </Menu>
          </Group>
        </div>
        <div className="flex-1 overflow-y-auto custom-scroll">
          {isStatusLoading ? (
            <>
              <span>Loading...</span>
            </>
          ) : (
            <>
              {status &&
              ((stagedChanges && stagedChanges?.length > 0) ||
                (conflictedChanges && conflictedChanges?.length > 0) ||
                (unstagedChanges && unstagedChanges?.length > 0)) ? (
                <VirtualizedFileList
                  searchQuery={changesQuery}
                  sections={[
                    {
                      id: "conflicted",
                      name: "Conflicted",
                      type: "conflicted",
                      files: conflictedChanges || [],
                      actions: {
                        onAddAll: async () => {
                          await addFile(visibleConflictPaths);
                        },
                        renderDiscardAll: () => (
                          <DiscardChangesDialog
                            filePaths={visibleConflictPaths}
                            label={`${visibleConflictPaths.length} visible conflicted files`}
                          />
                        ),
                      },
                    },
                    {
                      id: "staged",
                      name: "Staged Changes",
                      type: "staged",
                      files: stagedChanges || [],
                      actions: {
                        onUnstageAll: async () => {
                          await unstageFile(visibleStagedPaths);
                        },
                      },
                    },
                    {
                      id: "unstaged",
                      name: "Changes",
                      type: "changes",
                      files: unstagedChanges || [],
                      actions: {
                        onAddAll: async () => {
                          await addFile(visibleUnstagedPaths);
                        },
                        renderDiscardAll: () => (
                          <DiscardChangesDialog
                            filePaths={visibleUnstagedPaths}
                            label={`${visibleUnstagedPaths.length} visible files`}
                          />
                        ),
                      },
                    },
                  ]}
                  onFileClick={handleFileClick}
                  onAdd={addFile}
                  onUnstage={unstageFile}
                  onDiscard={(filePath) => discardChanges({ filePath })}
                  renderDiscard={(filePath) => (
                    <DiscardChangesDialog
                      filePaths={
                        Array.isArray(filePath) ? filePath : [filePath]
                      }
                    />
                  )}
                  setSelectedFilePath={setWorktreeSelectionForRepo}
                  selectedFilePath={
                    selectedFileForList
                      ? {
                          path: selectedFileForList.filePath,
                          newPath: selectedFileForList.fileNewPath,
                          scope:
                            selectedFileForList.worktreeScope === "unstaged"
                              ? "changes"
                              : selectedFileForList.worktreeScope,
                        }
                      : undefined
                  }
                  defaultExpandedSections={["staged", "unstaged"]}
                  className="h-full"
                />
              ) : (
                <div className="w-full h-full flex flex-col items-center justify-center">
                  <div className="relative">
                    <GitBranch className="opacity-50 size-12 mb-4 text-muted-foreground" />
                  </div>
                  <span className="flex justify-center text-sm text-muted-foreground">
                    No changes, Look in!
                  </span>
                </div>
              )}
            </>
          )}
        </div>
        {currentBranchStash ? (
          <Button
            variant={"secondary"}
            className="border-0 border-t! py-4 pl-2.5 ring-0! flex items-center justify-between rounded-none relative overflow-hidden [--glow-end:0.09] dark:[--glow-end:0.09]"
            style={{
              backgroundColor: "var(--color-background)",
              backgroundImage: `
              linear-gradient(
                to right,
                rgba(234, 179, 8, var(--glow-end)) 0%,
                rgba(234, 179, 8, 0.03) 55%,
                transparent 76%
              )
            `,
            }}
            onClick={() =>
              onOpenStashView(currentBranchStash?.reference ?? null)
            }
          >
            <div
              className="absolute inset-0 pointer-events-none [--stripe-alpha:0.15] dark:[--stripe-alpha:0.1]"
              style={{
                background: `
                repeating-linear-gradient(
                  45deg,
                  rgba(234, 179, 8, var(--stripe-alpha)) 0px,
                  rgba(234, 179, 8, var(--stripe-alpha)) 4px,
                  transparent 4px,
                  transparent 10px
                )
              `,
                maskImage:
                  "linear-gradient(to left, transparent 20%, black 80%)",
                WebkitMaskImage:
                  "linear-gradient(to left, transparent 20%, black 80%)",
              }}
            />
            <div className="flex items-center justify-between w-full gap-2 relative z-1">
              <div className="flex items-center gap-2 ">
                <Stashed
                  style={{
                    width: "20px",
                    height: "20px",
                  }}
                />
                <span className="text-sm">Stashed changes</span>
                <span className="tabular-nums flex items-center gap-1 text-muted-foreground font-normal">
                  ({formatNumber(currentBranchStash?.files_changed || 0)})
                </span>
              </div>
              <div className="text-xs flex items-center">
                <span className="flex gap-2">
                  <span className="text-green-600 tabular-nums font-normal">
                    +{formatNumber(currentBranchStash?.insertions || 0)}
                  </span>
                  <span className="text-red-600 tabular-nums font-normal">
                    -{formatNumber(currentBranchStash?.deletions || 0)}
                  </span>
                </span>
              </div>
            </div>
            <div className="relative z-1">
              <ChevronsRight className="size-6 text-muted-foreground/50" />
            </div>
          </Button>
        ) : null}
        <WriteCommitBox visibleAddablePaths={visibleAddablePaths} />
      </TabsPanel>
      <TabsPanel
        value="tab-2"
        className={"flex-1 flex flex-col min-h-0"}
        tabIndex={-1}
      >
        <div className="border-b max-h-10 min-h-10 p-1.5">
          <Group aria-label="Subscription actions" className="w-full">
            <Input
              aria-label="Filter Commit"
              placeholder="Filter commits..."
              className={"rounded-l-md! border-border! w-full"}
              size={"sm"}
              value={historySearchInput}
              onChange={(e) => setHistorySearchInput(e.target.value)}
            />
            <GroupSeparator />
            <Button
              aria-label="Copy options"
              size="icon-sm"
              variant={"secondary"}
              className="rounded-r-md! border-border"
              onClick={() => {
                setMainWindowView("HistoryGraph");
                clearWorktreeSelectionForRepo();
              }}
            >
              <History />
            </Button>
          </Group>
        </div>
        <HistoryCommitInfiniteList
          rows={historyRows}
          onOpenCommit={onOpenHistoryView}
          fetchNextPage={fetchNextPage}
          hasNextPage={hasNextPage}
          isFetchingNextPage={isFetchingNextPage}
          isLoading={isHistoryLoading}
        />
      </TabsPanel>
    </Tabs>
  );
}

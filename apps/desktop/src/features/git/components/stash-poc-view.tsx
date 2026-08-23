import {
  AlertDialog,
  AlertDialogClose,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogPopup,
  AlertDialogTitle,
  AlertDialogTrigger,
} from "@gitru/ui/components/alert-dialog";
import { Button } from "@gitru/ui/components/button";
import { Group, GroupSeparator } from "@gitru/ui/components/group";
import { Input } from "@gitru/ui/components/input";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
  Menu,
  MenuCheckboxItem,
  MenuGroup,
  MenuGroupLabel,
  MenuPopup,
  MenuTrigger,
} from "@gitru/ui/components/menu";
import {
  ChevronDownIcon,
  ChevronLeftIcon,
  ListFilterPlus,
  Undo2,
} from "lucide-react";
import { memo, useEffect, useState } from "react";
import { toast } from "sonner";
import { useFileSelectionStore } from "@/components/diff/use-file-selection-store";
import { getStatusIcon } from "@/components/get-status-icon";
import { VirtualizedFileList } from "@/components/virtualized-file-list";
import {
  DEFAULT_STATUS_FILTERS,
  type FileStatusFilter,
  hasActiveStatusFilters,
  matchesStatusFilters,
} from "@/features/git/lib/file-status-filters";
import { matchesSearchQuery } from "@/features/git/lib/matches-search-query";
import {
  useGetCurrentBranchStash,
  useStashDrop,
  useStashList,
  useStashPop,
  useStashRestoreFile,
  useStashShow,
} from "@/hooks";
import { resolveFileSelection } from "@/lib/git-selection-resolver";
import {
  selectActiveRepository,
  selectActiveSessionRepoKey,
  useAppStore,
} from "@/store/use-app-store";

export const StashPocView = memo(function StashPocView({
  onBack,
  mode,
}: {
  onBack: () => void;
  mode: "branch" | "all";
}) {
  const [query, setQuery] = useState("");
  const [statusFilters, setStatusFilters] = useState<
    Record<FileStatusFilter, boolean>
  >(DEFAULT_STATUS_FILTERS);

  const activeRepository = useAppStore(selectActiveRepository);
  const repoStateKey = useAppStore(selectActiveSessionRepoKey);
  const repoPath = activeRepository?.path ?? "";
  const repoSelectionState = useAppStore((state) =>
    repoStateKey ? state.selectionByRepo[repoStateKey] : undefined,
  );
  const setStashSelectionForRepo = useAppStore(
    (state) => state.setStashSelectionForRepo,
  );
  const clearStashSelectionForRepo = useAppStore(
    (state) => state.clearStashSelectionForRepo,
  );
  const pruneStashSelectionsForRepo = useAppStore(
    (state) => state.pruneStashSelectionsForRepo,
  );
  const repoGitViewState = useAppStore((state) =>
    repoStateKey ? state.gitViewByRepo[repoStateKey] : undefined,
  );
  const setGitViewStateForRepo = useAppStore(
    (state) => state.setGitViewStateForRepo,
  );

  const handleFileClick = useFileSelectionStore(
    (state) => state.handleFileClick,
  );
  const { data: currentBranchStash } = useGetCurrentBranchStash();
  const { data: stashes, isLoading: isStashesLoading } = useStashList();
  const { mutateAsync: popStash, isPending: isRestoreAllPending } =
    useStashPop();
  const { mutateAsync: dropStash, isPending: isDiscardAllPending } =
    useStashDrop();
  const { mutateAsync: restoreStashFile } = useStashRestoreFile();

  const persistedSelectedReference =
    repoGitViewState?.selectedStashReference ?? null;
  const selectedReference =
    mode === "branch"
      ? (currentBranchStash?.reference ?? null)
      : persistedSelectedReference;

  useEffect(() => {
    if (mode !== "all") {
      return;
    }

    if (!stashes || stashes.length === 0) {
      setGitViewStateForRepo(
        { selectedStashReference: null },
        repoStateKey ?? repoPath,
      );
      return;
    }

    if (
      selectedReference &&
      stashes.some((stash) => stash.reference === selectedReference)
    ) {
      return;
    }

    setGitViewStateForRepo(
      { selectedStashReference: stashes[0]?.reference ?? null },
      repoStateKey ?? repoPath,
    );
  }, [
    mode,
    repoPath,
    repoStateKey,
    selectedReference,
    setGitViewStateForRepo,
    stashes,
  ]);

  useEffect(() => {
    const targetRepoKey = repoStateKey ?? repoPath;
    if (!targetRepoKey) {
      return;
    }

    const references = (stashes ?? []).map((stash) => stash.reference);
    pruneStashSelectionsForRepo(targetRepoKey, references);
  }, [pruneStashSelectionsForRepo, repoPath, repoStateKey, stashes]);

  const { data: stashShow, isLoading: isStashShowLoading } =
    useStashShow(selectedReference);

  const hasFilterSelection = hasActiveStatusFilters(statusFilters);
  const filteredFiles = (stashShow?.files ?? []).filter(
    (file) =>
      matchesStatusFilters(file, statusFilters) &&
      matchesSearchQuery(file.path, query),
  );

  const selectedFileForCurrentRepo = selectedReference
    ? (repoSelectionState?.stashByReference[selectedReference] ?? null)
    : null;
  const stashReferences = (stashes ?? []).map((stash) => stash.reference);
  const resolvedStashSelection = resolveFileSelection({
    selection: selectedFileForCurrentRepo,
    files: stashShow?.files ?? [],
    context: {
      source: "stash",
      stashReference: selectedReference,
      availableStashReferences: stashReferences,
    },
  });
  const selectedStashFileForList =
    resolvedStashSelection.state === "valid"
      ? resolvedStashSelection.identity
      : selectedFileForCurrentRepo;

  return (
    <div className="h-full flex flex-col">
      <div className="p-1.5 min-h-10 border-b flex items-center gap-2">
        <Button size="icon-sm" variant="outline" onClick={onBack}>
          <ChevronLeftIcon className="size-4" />
        </Button>
        <Group aria-label="Stash actions" className="w-full">
          <Input
            aria-label="Filter stashed files"
            placeholder="Filter files..."
            size={"sm"}
            value={query}
            onChange={(e) => setQuery(e.target.value)}
          />
          <GroupSeparator />
          {mode === "all" ? (
            <>
              <DropdownMenu>
                <DropdownMenuTrigger
                  render={
                    <Button
                      size={"sm"}
                      variant={"outline"}
                      className="max-w-44"
                    />
                  }
                >
                  <span className="truncate">
                    {selectedReference || "Select stash"}
                  </span>
                  <ChevronDownIcon
                    className="-me-1 opacity-60"
                    size={16}
                    aria-hidden="true"
                  />
                </DropdownMenuTrigger>
                <DropdownMenuContent align="end" className="w-90 max-h-72">
                  {(stashes ?? []).length > 0 ? (
                    (stashes ?? []).map((stash) => (
                      <DropdownMenuItem
                        key={stash.reference}
                        onClick={() =>
                          setGitViewStateForRepo(
                            { selectedStashReference: stash.reference },
                            repoPath,
                          )
                        }
                        className="flex flex-col items-start"
                      >
                        <span className="truncate w-full">
                          {stash.reference}
                        </span>
                        <span className="text-xs text-muted-foreground truncate w-full">
                          {stash.message}
                        </span>
                      </DropdownMenuItem>
                    ))
                  ) : (
                    <DropdownMenuItem disabled>No stash found</DropdownMenuItem>
                  )}
                </DropdownMenuContent>
              </DropdownMenu>
              <GroupSeparator />
            </>
          ) : null}
          <Menu>
            <MenuTrigger
              render={
                <Button
                  aria-label="Filter stash files by status"
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
                    setStatusFilters((prev) => ({
                      ...prev,
                      untracked: Boolean(checked),
                    }))
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
                    setStatusFilters((prev) => ({
                      ...prev,
                      modified: Boolean(checked),
                    }))
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
                    setStatusFilters((prev) => ({
                      ...prev,
                      renamed: Boolean(checked),
                    }))
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
                    setStatusFilters((prev) => ({
                      ...prev,
                      deleted: Boolean(checked),
                    }))
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
                    setStatusFilters((prev) => ({
                      ...prev,
                      conflicted: Boolean(checked),
                    }))
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

      <div className="flex-1 min-h-0 overflow-y-auto custom-scroll **:data-[slot=file-row]:mr-2!">
        {isStashesLoading || isStashShowLoading ? (
          <div className="w-full h-full flex items-center justify-center">
            <span className="text-sm text-muted-foreground">Loading...</span>
          </div>
        ) : mode === "branch" && !selectedReference ? (
          <div className="w-full h-full flex items-center justify-center">
            <span className="text-sm text-muted-foreground">
              No !!Gitru stash for this branch
            </span>
          </div>
        ) : !selectedReference ? (
          <div className="w-full h-full flex items-center justify-center">
            <span className="text-sm text-muted-foreground">
              No stashed changes
            </span>
          </div>
        ) : filteredFiles.length > 0 ? (
          <VirtualizedFileList
            sectionMode="flat"
            searchQuery={query}
            sections={[
              {
                id: "stash",
                name: "Stashed Changes",
                type: "stash",
                files: filteredFiles,
              },
            ]}
            onFileClick={handleFileClick}
            setSelectedFilePath={(file) => {
              if (!file) {
                if (selectedReference) {
                  clearStashSelectionForRepo(selectedReference);
                }
                return;
              }

              if (selectedReference) {
                setStashSelectionForRepo(selectedReference, file);
              }
            }}
            getContextActions={({ file }) =>
              selectedReference
                ? [
                    {
                      id: "stash-restore-file",
                      label: "Restore File",
                      icon: <Undo2 size={16} />,
                      onSelect: async () => {
                        await restoreStashFile({
                          reference: selectedReference,
                          filePath: file.path,
                        });
                        toast.success(`Restored ${file.path}`);
                      },
                    },
                  ]
                : []
            }
            selectedFilePath={
              selectedStashFileForList
                ? {
                    path: selectedStashFileForList.filePath,
                    newPath: selectedStashFileForList.fileNewPath,
                  }
                : undefined
            }
            className="h-full"
          />
        ) : (
          <div className="w-full h-full flex items-center justify-center">
            <span className="text-sm text-muted-foreground">
              No matching stashed files
            </span>
          </div>
        )}
      </div>
      <div className="gap-2 flex px-2 py-2 border-t">
        <AlertDialog>
          <AlertDialogTrigger
            render={
              <Button
                disabled={!selectedReference || isDiscardAllPending}
                className="flex-1"
                variant="destructive-outline"
              />
            }
          >
            Discard
          </AlertDialogTrigger>
          <AlertDialogPopup>
            <AlertDialogHeader>
              <AlertDialogTitle>
                Are you sure you want to discard this stash?
              </AlertDialogTitle>
              <AlertDialogDescription>
                This action cannot be undone. All stashed changes will be lost.
              </AlertDialogDescription>
            </AlertDialogHeader>
            <AlertDialogFooter className="px-4 py-3">
              <AlertDialogClose render={<Button variant="outline" />}>
                Cancel
              </AlertDialogClose>
              <AlertDialogClose
                render={
                  <Button
                    variant="destructive"
                    onClick={async () => {
                      if (!selectedReference) return;
                      await dropStash(selectedReference);
                      if (
                        selectedFileForCurrentRepo?.stashReference ===
                        selectedReference
                      ) {
                        clearStashSelectionForRepo(selectedReference);
                      }
                      toast.success("Discarded stash");
                    }}
                  />
                }
              >
                Discard
              </AlertDialogClose>
            </AlertDialogFooter>
          </AlertDialogPopup>
        </AlertDialog>
        <Button
          disabled={!selectedReference || isRestoreAllPending}
          className="flex-1"
          onClick={async () => {
            if (!selectedReference) return;
            await popStash({ reference: selectedReference });
            if (
              selectedFileForCurrentRepo?.stashReference === selectedReference
            ) {
              clearStashSelectionForRepo(selectedReference);
            }
            onBack();
            toast.success("Restored changes");
          }}
        >
          Restore
        </Button>
      </div>
    </div>
  );
});

import { GetStatusResponse } from "@gitru/commands";
import { Stashed } from "@gitru/icon";
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
import {
  Avatar,
  AvatarFallback,
  AvatarImage,
} from "@gitru/ui/components/avatar";
import { Button } from "@gitru/ui/components/button";
import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@gitru/ui/components/dialog";
import { Group, GroupSeparator } from "@gitru/ui/components/group";
import { Input } from "@gitru/ui/components/input";
import {
  InputGroup,
  InputGroupAddon,
  InputGroupInput,
  InputGroupTextarea,
} from "@gitru/ui/components/input-group";
import { Kbd } from "@gitru/ui/components/kbd";
import { Label } from "@gitru/ui/components/label";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
  Menu,
  MenuCheckboxItem,
  MenuGroup,
  MenuGroupLabel,
  MenuItem,
  MenuPopup,
  MenuTrigger,
} from "@gitru/ui/components/menu";
import {
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup,
} from "@gitru/ui/components/resizable";
import { ScrollArea } from "@gitru/ui/components/scroll-area";
import { Tabs, TabsList, TabsPanel, TabsTab } from "@gitru/ui/components/tabs";
import {
  Tooltip,
  TooltipPopup,
  TooltipProvider,
  TooltipTrigger,
} from "@gitru/ui/components/tooltip";
import { cn } from "@gitru/ui/lib/utils";
import { createFileRoute, Outlet } from "@tanstack/react-router";
import { open } from "@tauri-apps/plugin-dialog";
import {
  BadgeQuestionMark,
  ChevronDown,
  ChevronDownIcon,
  ChevronLeftIcon,
  ChevronsRight,
  ChevronUp,
  CircleAlertIcon,
  GitBranch,
  History,
  ListFilterPlus,
  Loader2,
  SearchIcon,
  Sparkles,
  Undo2,
  UserPlus,
} from "lucide-react";
import { AnimatePresence, motion, useReducedMotion } from "motion/react";
import { memo, useCallback, useEffect, useState } from "react";
import { useDefaultLayout } from "react-resizable-panels";
import { toast } from "sonner";
import z from "zod";
import { useFileSelectionStore } from "@/components/diff/useFileSelectionStore";
import { getStatusIcon } from "@/components/getStatusIcon";
import PageLayout from "@/components/pageLayout";
import { RepositoryListItem } from "@/components/RepositoryListItem";
import StatusBar from "@/components/statusBar";
import { VirtualizedFileList } from "@/components/VirtualizedFileList";
import {
  useCreateCommit,
  useGetCommitHistory,
  useGetCurrentBranch,
  useGetCurrentBranchStash,
  useGetStatus,
  useGitAdd,
  useGitDiscard,
  useGitUnstage,
  useStashDrop,
  useStashList,
  useStashPop,
  useStashRestoreFile,
  useStashShow,
} from "@/hooks";
import { useRepositories } from "@/hooks/useRepositories";
import { formatNumber } from "@/lib/formatNumber";
import { getAvatarByProvider } from "@/lib/getAvatarByGitProvider";
import { parseOrigin } from "@/lib/parseOrigin";
import {
  formatUnixSecondsToDateTime,
  timeAgoFromUnixSeconds,
} from "@/lib/time";
import { useAppStore } from "@/store/useAppStore";
import { GIT_PROVIDERS } from "@/type";
import { resolveFileSelection } from "../../../lib/gitSelectionResolver";

const CoAuthers = z.array(z.tuple([z.string(), z.string()]));
type CoAuthers = z.infer<typeof CoAuthers>;
type FileStatusFilter =
  | "modified"
  | "renamed"
  | "conflicted"
  | "deleted"
  | "untracked";

const DEFAULT_STATUS_FILTERS: Record<FileStatusFilter, boolean> = {
  modified: true,
  renamed: true,
  deleted: true,
  conflicted: true,
  untracked: true,
};

const hasActiveStatusFilters = (filters: Record<FileStatusFilter, boolean>) =>
  !filters.modified ||
  !filters.renamed ||
  !filters.deleted ||
  !filters.conflicted ||
  !filters.untracked;

const matchesStatusFilters = (
  file: GetStatusResponse["files"][number],
  filters: Record<FileStatusFilter, boolean>,
) => {
  const isModified = file.status.some((s) => s.includes("Modified"));
  const isRenamed = file.status.some((s) => s.includes("Renamed"));
  const isDeleted = file.status.some((s) => s.includes("Deleted"));
  const isConflicted = file.status.some((s) => s.includes("Conflicted"));
  const isUntracked = file.status.some((s) => s.includes("New"));

  return (
    (filters.modified && isModified) ||
    (filters.renamed && isRenamed) ||
    (filters.deleted && isDeleted) ||
    (filters.conflicted && isConflicted) ||
    (filters.untracked && isUntracked)
  );
};

const matchesSearchQuery = (value: string, query: string) => {
  const normalizedQuery = query.trim();
  if (!normalizedQuery) return true;
  if (normalizedQuery === "*") return true;

  if (value.toLowerCase().includes(normalizedQuery.toLowerCase())) {
    return true;
  }

  try {
    if (normalizedQuery.startsWith("/") && normalizedQuery.length > 1) {
      const lastSlashIndex = normalizedQuery.lastIndexOf("/");
      if (lastSlashIndex > 0) {
        const pattern = normalizedQuery.slice(1, lastSlashIndex);
        const flags = normalizedQuery.slice(lastSlashIndex + 1) || "i";
        return new RegExp(pattern, flags).test(value);
      }
    }

    return new RegExp(normalizedQuery, "i").test(value);
  } catch {
    try {
      const escapedGlob = normalizedQuery
        .replace(/[.+^${}()|[\]\\]/g, "\\$&")
        .replace(/\*/g, ".*")
        .replace(/\?/g, ".");
      return new RegExp(escapedGlob, "i").test(value);
    } catch {
      return false;
    }
  }
};

export const Route = createFileRoute("/app/git")({
  component: GitPageLayout,
});

function GitPageLayout() {
  return (
    <PageLayout className="flex-col flex justify-between">
      <ResizableArea />
      <StatusBar />
    </PageLayout>
  );
}

const ResizableArea = () => {
  const repoSelectIsOpen = useAppStore((state) => state.repoSelectIsOpen);
  const setRepoSelectIsOpen = useAppStore((state) => state.setRepoSelectIsOpen);
  const selectedRepository = useAppStore((state) => state.selectedRepository);
  const gitViewByRepo = useAppStore((state) => state.gitViewByRepo);
  const setGitViewStateForRepo = useAppStore(
    (state) => state.setGitViewStateForRepo,
  );

  const shouldReduceMotion = useReducedMotion();
  const [panelDirection, setPanelDirection] = useState<1 | -1>(1);
  const repoPath = selectedRepository?.path;
  const gitViewState: {
    leftPanelView: "changes" | "stash";
    stashViewMode: "branch" | "all";
    selectedStashReference: string | null;
    stashStatusFilters: Record<FileStatusFilter, boolean>;
  } = repoPath
    ? (gitViewByRepo[repoPath] ?? {
        leftPanelView: "changes",
        stashViewMode: "branch",
        selectedStashReference: null,
        stashStatusFilters: DEFAULT_STATUS_FILTERS,
      })
    : {
        leftPanelView: "changes",
        stashViewMode: "branch",
        selectedStashReference: null,
        stashStatusFilters: DEFAULT_STATUS_FILTERS,
      };

  const panelSlideVariants = {
    initial: (direction: 1 | -1) => ({
      x: shouldReduceMotion ? 0 : direction > 0 ? 10 : -10,
      opacity: shouldReduceMotion ? 1 : 0.94,
    }),
    animate: {
      x: 0,
      opacity: 1,
    },
    exit: (direction: 1 | -1) => ({
      x: shouldReduceMotion ? 0 : direction > 0 ? -4 : 4,
      opacity: shouldReduceMotion ? 1 : 0.97,
    }),
  };

  const panelTransition = shouldReduceMotion
    ? { duration: 0.06, ease: "linear" as const }
    : { duration: 0.14, ease: [0.32, 0.72, 0, 1] as const };

  const { defaultLayout, onLayoutChanged } = useDefaultLayout({
    id: "git-page-layout",
    storage: localStorage,
  });

  return (
    <div className="flex h-full">
      <ResizablePanelGroup
        defaultLayout={defaultLayout}
        onLayoutChanged={onLayoutChanged}
      >
        <ResizablePanel
          defaultSize={320}
          minSize={270}
          maxSize={800}
          id="left"
          className="flex flex-col h-full"
        >
          <ToggelPanelButton />
          <div className="h-full border-t max-h-[calc(100vh-calc(var(--spacing)*31.5))] relative overflow-hidden">
            {repoSelectIsOpen ? (
              <div className="absolute inset-0 bg-background">
                <ListRepositories />
              </div>
            ) : (
              <AnimatePresence
                mode="sync"
                initial={false}
                custom={panelDirection}
              >
                {gitViewState.leftPanelView === "stash" ? (
                  <motion.div
                    key="stash"
                    className="absolute inset-0 bg-background will-change-transform"
                    custom={panelDirection}
                    variants={panelSlideVariants}
                    initial="initial"
                    animate="animate"
                    exit="exit"
                    transition={panelTransition}
                  >
                    <StashPocView
                      mode={gitViewState.stashViewMode}
                      onBack={() => {
                        setPanelDirection(-1);
                        setGitViewStateForRepo(
                          {
                            leftPanelView: "changes",
                          },
                          repoPath,
                        );
                      }}
                    />
                  </motion.div>
                ) : (
                  <motion.div
                    key="changes"
                    className="absolute inset-0 bg-background will-change-transform"
                    custom={panelDirection}
                    variants={panelSlideVariants}
                    initial="initial"
                    animate="animate"
                    exit="exit"
                    transition={panelTransition}
                  >
                    <ListFileChanges
                      onOpenStashView={(stashReference) => {
                        setPanelDirection(1);
                        setGitViewStateForRepo(
                          {
                            leftPanelView: "stash",
                            stashViewMode: "branch",
                            selectedStashReference: stashReference,
                          },
                          repoPath,
                        );
                      }}
                    />
                  </motion.div>
                )}
              </AnimatePresence>
            )}
          </div>
        </ResizablePanel>
        <ResizableHandle withHandle />
        <ResizablePanel
          className={cn("relative", repoSelectIsOpen && "cursor-pointer")}
          onClick={() => setRepoSelectIsOpen(false)}
          id="right"
        >
          {repoSelectIsOpen && (
            <div className="absolute inset-0 bg-background/40 z-10 w-full h-full backdrop-blur-[2px]"></div>
          )}
          <Outlet />
        </ResizablePanel>
      </ResizablePanelGroup>
    </div>
  );
};

const DiscardChangesDialog = memo(function DiscardChangesDialog({
  fileName,
}: {
  fileName: string;
}) {
  const [open, setOpen] = useState(false);
  const [isDeleteLoading, setIsDeleteLoading] = useState(false);

  const { mutateAsync: discardChanges } = useGitDiscard();

  const handleOpenChange = useCallback((newOpen: boolean) => {
    setOpen(newOpen);
  }, []);

  return (
    <Dialog open={open} onOpenChange={handleOpenChange}>
      <DialogTrigger
        render={
          <Button
            onClick={(e) => {
              e.stopPropagation();
              setOpen(true);
            }}
            size={"icon-sm"}
            variant={"ghost"}
          />
        }
      >
        <Undo2 size={20} strokeWidth={1.25} />
      </DialogTrigger>
      <DialogContent className="min-w-150">
        <div className="flex flex-col items-center gap-2">
          <div
            className="flex size-9 shrink-0 items-center justify-center rounded-full border"
            aria-hidden="true"
          >
            <CircleAlertIcon
              className="opacity-80 text-destructive"
              size={30}
            />
          </div>
          <DialogHeader>
            <DialogTitle className="sm:text-center">
              Discard changes to{" "}
              <span className="font-semibold text-destructive">
                {fileName === "." ? "All" : fileName.split("/").pop()}
              </span>
              ?
            </DialogTitle>
            <DialogDescription className="sm:text-center">
              This action cannot be undone. All modifications to this file will
              be permanently lost.
            </DialogDescription>
          </DialogHeader>
        </div>

        <DialogFooter className="grid grid-cols-2 p-3!">
          <DialogClose
            render={
              <Button
                type="button"
                variant="outline"
                className="flex-1"
                size={"lg"}
                onClick={(e) => {
                  e.stopPropagation();
                }}
                disabled={isDeleteLoading}
              />
            }
          >
            Cancel
            <Kbd>Esc</Kbd>
          </DialogClose>
          <Button
            type="button"
            className="flex-1"
            variant={"destructive"}
            disabled={isDeleteLoading}
            size={"lg"}
            onClick={async (e) => {
              e.stopPropagation();
              setIsDeleteLoading(true);

              try {
                await discardChanges({
                  filePath: fileName,
                });
              } catch (error) {
                toast.error("Unable to discard changes");
              } finally {
                setIsDeleteLoading(false);
                setOpen(false);
              }
            }}
          >
            Discard
            <Kbd className="bg-background/30 text-background">
              {/* <CornerDownLeft /> */}↵
            </Kbd>
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
});

const WriteCommitBox = memo(function WriteCommitBox() {
  const [title, setTitle] = useState("");
  const [description, setDescription] = useState("");
  const [co_authors, setCoAuthors] = useState<CoAuthers>([]);

  const { data: currentBranch } = useGetCurrentBranch();
  const { data: status } = useGetStatus();
  const { mutateAsync: gitAdd, isPending: isAdding } = useGitAdd();
  const { mutateAsync: createCommit, isPending: isCreatingCommit } =
    useCreateCommit();

  const nothingToCommit =
    status?.files.filter((file) =>
      file.status.some((s) => s.startsWith("Index")),
    ).length === 0;

  const handelCommit = useCallback(async () => {
    if (nothingToCommit) {
      await gitAdd(".");
    }

    const data = await createCommit({
      commitMeta: {
        title,
        description,
        co_authors,
      },
      allowEmpty: false,
    });
    if (data) {
      setTitle("");
      setDescription("");
      setCoAuthors([]);
      toast.success("Commit created successfully");
    }
  }, [createCommit, title, description, co_authors]);

  return (
    <div>
      <div className="shrink-0 flex flex-col gap-2 justify-between items-center border-t px-2 py-2 ">
        <InputGroup>
          <InputGroupInput
            placeholder="Summary (required)"
            className="h-8"
            value={title}
            onChange={(e) => setTitle(e.target.value)}
          />
          <InputGroupAddon align="inline-end">
            <Button
              aria-label="Password requirements"
              size="icon-xs"
              variant="ghost"
              className="opacity-50 hover:opacity-100"
            >
              <Sparkles size={16} />
            </Button>
          </InputGroupAddon>
        </InputGroup>
        <InputGroup>
          <InputGroupTextarea
            placeholder="Description"
            value={description}
            onChange={(e) => setDescription(e.target.value)}
          />
          <InputGroupAddon align="block-end">
            <Button
              variant="ghost"
              size="icon-xs"
              className="opacity-50 hover:opacity-100"
              aria-label="Add Co Authors"
            >
              <UserPlus size={16} />
            </Button>
          </InputGroupAddon>
        </InputGroup>
        <div className="w-full flex items-center gap-2">
          <Group aria-label="Subscription actions" className="w-full">
            <Button
              onClick={handelCommit}
              className="flex-1 truncate"
              disabled={isAdding || isCreatingCommit}
            >
              {isAdding || isCreatingCommit ? (
                <>
                  <Loader2 className="size-4 animate-spin" />
                  Committing...
                </>
              ) : (
                <>
                  {nothingToCommit ? (
                    <span>Add all & Commit</span>
                  ) : (
                    <span className="truncate">
                      Commit to <span>{currentBranch?.name}</span>
                    </span>
                  )}
                </>
              )}
            </Button>
            <GroupSeparator className="bg-primary/72" />
            <Menu>
              <MenuTrigger
                render={
                  <Button
                    aria-label="Copy options"
                    size="icon"
                    className="rounded-r-lg!"
                  />
                }
              >
                <ChevronDownIcon className="size-4" />
              </MenuTrigger>
              <MenuPopup align="end" className={"w-full"}>
                <MenuItem>Empty Commit</MenuItem>
                <MenuItem>Amend Last Commit</MenuItem>
              </MenuPopup>
            </Menu>
          </Group>
        </div>
      </div>
    </div>
  );
});

const ToggelPanelButton = () => {
  const repoSelectIsOpen = useAppStore((state) => state.repoSelectIsOpen);
  const setRepoSelectIsOpen = useAppStore((state) => state.setRepoSelectIsOpen);
  const selectedRepository = useAppStore((state) => state.selectedRepository);

  return (
    <Button
      onClick={() => {
        setRepoSelectIsOpen(!repoSelectIsOpen);
      }}
      className={cn(
        "rounded-none justify-between min-h-13.75 max-h-13.75",
        repoSelectIsOpen && "bg-accent",
      )}
      variant={"ghost"}
    >
      <div className="flex-col flex items-start">
        <span className="text-xs text-muted-foreground">
          Current Repository
        </span>
        <span>{selectedRepository?.name || "No repository selected"}</span>
      </div>
      {repoSelectIsOpen ? <ChevronUp size={18} /> : <ChevronDown size={18} />}
    </Button>
  );
};

const ListFileChanges = ({
  onOpenStashView,
}: {
  onOpenStashView: (stashReference: string | null) => void;
}) => {
  const [query, setQuery] = useState("");
  const [statusFilters, setStatusFilters] = useState<
    Record<FileStatusFilter, boolean>
  >(DEFAULT_STATUS_FILTERS);

  const selectedRepository = useAppStore((state) => state.selectedRepository);
  const setWorktreeSelectionForRepo = useAppStore(
    (state) => state.setWorktreeSelectionForRepo,
  );
  const clearWorktreeSelectionForRepo = useAppStore(
    (state) => state.clearWorktreeSelectionForRepo,
  );
  const selectionByRepo = useAppStore((state) => state.selectionByRepo);
  const setMainWindowView = useAppStore((state) => state.setMainWindowView);

  const { data: status, isLoading: isStatusLoading } = useGetStatus();

  const { mutateAsync: addFile } = useGitAdd();
  const { mutateAsync: unstageFile } = useGitUnstage();

  const { data: currentBranchStash } = useGetCurrentBranchStash();

  const { handleFileClick } = useFileSelectionStore();

  const { data: commitHistory } = useGetCommitHistory();
  const repoPath = selectedRepository?.path ?? "";
  const worktreeSelection = selectionByRepo[repoPath]?.worktree ?? null;
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
      matchesSearchQuery(file.path, query),
  );
  const unstagedChanges: GetStatusResponse["files"] = (
    status?.files ?? []
  ).filter(
    (file) =>
      file.status.some((s) => s.startsWith("Worktree")) &&
      matchesStatusFilters(file, statusFilters) &&
      matchesSearchQuery(file.path, query),
  );
  const conflictedChanges: GetStatusResponse["files"] = (
    status?.files ?? []
  ).filter(
    (file) =>
      file.status.some((s) => s.includes("Conflicted")) &&
      matchesStatusFilters(file, statusFilters) &&
      matchesSearchQuery(file.path, query),
  );

  return (
    <Tabs defaultValue="tab-1" className={"gap-0 h-full flex flex-col"}>
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
            <Input
              aria-label="Filter files"
              placeholder="Filter files..."
              className={"rounded-l-md! border-border! w-full"}
              size={"sm"}
              value={query}
              onChange={(e) => setQuery(e.target.value)}
            />
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
                  searchQuery={query}
                  sections={[
                    {
                      id: "conflicted",
                      name: "Conflicted",
                      type: "conflicted",
                      files: conflictedChanges || [],
                      actions: {
                        onAddAll: async () => {
                          await addFile(".");
                        },
                        renderDiscardAll: () => (
                          <DiscardChangesDialog fileName="." />
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
                          await unstageFile(".");
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
                          await addFile(".");
                        },
                        renderDiscardAll: () => (
                          <DiscardChangesDialog fileName="." />
                        ),
                      },
                    },
                  ]}
                  onFileClick={handleFileClick}
                  onAdd={addFile}
                  onUnstage={unstageFile}
                  renderDiscard={(filePath) => (
                    <DiscardChangesDialog fileName={filePath} />
                  )}
                  setSelectedFilePath={setWorktreeSelectionForRepo}
                  selectedFilePath={
                    selectedFileForList
                      ? {
                          path: selectedFileForList.filePath,
                          newPath: selectedFileForList.fileNewPath,
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
        <WriteCommitBox />
      </TabsPanel>
      <TabsPanel value="tab-2" className={"h-full"} tabIndex={-1}>
        <div className="border-b max-h-10 min-h-10 p-1.5">
          <Group aria-label="Subscription actions" className="w-full">
            <Input
              aria-label="Filter Commit"
              placeholder="Filter commits..."
              className={"rounded-l-md! border-border! w-full"}
              size={"sm"}
              value={query}
              onChange={(e) => setQuery(e.target.value)}
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
        <ScrollArea className="flex-1 h-full" tabIndex={-1}>
          {commitHistory?.map((commit) => (
            <div
              className="w-full p-2 border-b hover:bg-accent cursor-pointer"
              key={commit.id}
            >
              <p className="truncate text-sm">{commit.summary}</p>
              <div className="flex mt-1 items-center justify-between w-full">
                <TooltipProvider>
                  <div className="flex items-center">
                    <div className="flex -mt-0.5 group">
                      <Tooltip>
                        <TooltipTrigger
                          style={{
                            zIndex: commit.authors.co_authors.length + 1,
                          }}
                        >
                          <Avatar className="ring-2 ring-background rounded-sm size-4">
                            <AvatarImage
                              alt={commit.authors.author.name}
                              src={`https://avatars.githubusercontent.com/u/e?email=${commit.authors.author.email}&s=64`}
                            />
                            <AvatarFallback>
                              {commit.authors.author.name
                                .split(" ")
                                .map((n) => n[0])
                                .join("")
                                .toUpperCase()}
                            </AvatarFallback>
                          </Avatar>
                        </TooltipTrigger>
                        <TooltipPopup side="bottom">
                          {commit.authors.author.name}
                        </TooltipPopup>
                      </Tooltip>
                      {commit.authors.co_authors.map((coAuthor, idx) => (
                        <Tooltip key={`${idx}-tooltip-coauthor`}>
                          <TooltipTrigger
                            style={{
                              zIndex: commit.authors.co_authors.length - idx,
                            }}
                            key={`${idx}-tooltip-trigger-coauthor`}
                          >
                            <Avatar className="ring-2 ring-background rounded-sm size-4 -ml-[0.2rem] group-hover:ml-0.5 transition-all duration-100">
                              <AvatarImage
                                alt="U1"
                                src={`https://avatars.githubusercontent.com/u/e?email=${coAuthor.email}&s=64`}
                              />
                              <AvatarFallback>
                                {coAuthor.name
                                  .split(" ")
                                  .map((n) => n[0])
                                  .join("")
                                  .toUpperCase()}
                              </AvatarFallback>
                            </Avatar>
                          </TooltipTrigger>
                          <TooltipPopup side="bottom">
                            {coAuthor.name}
                          </TooltipPopup>
                        </Tooltip>
                      ))}
                    </div>
                    <Label className="ml-1 text-muted-foreground text-xs font-light">
                      <Tooltip>
                        <TooltipTrigger>
                          {commit.authors.author.name}
                        </TooltipTrigger>
                        <TooltipPopup side="bottom">
                          {commit.authors.author.email}
                        </TooltipPopup>
                      </Tooltip>
                      <span className="-mx-1">{" • "}</span>
                      <Tooltip>
                        <TooltipTrigger>
                          {timeAgoFromUnixSeconds(commit.timestamp)}{" "}
                        </TooltipTrigger>
                        <TooltipPopup side="bottom">
                          {formatUnixSecondsToDateTime(commit.timestamp)}
                        </TooltipPopup>
                      </Tooltip>
                    </Label>
                  </div>
                </TooltipProvider>
              </div>
            </div>
          ))}
          <div className="h-20" />
        </ScrollArea>
      </TabsPanel>
    </Tabs>
  );
};

const ListRepositories = memo(() => {
  const selectedRepository = useAppStore((state) => state.selectedRepository);
  const setSelectedRepository = useAppStore(
    (state) => state.setSelectedRepository,
  );
  const setRepoSelectIsOpen = useAppStore((state) => state.setRepoSelectIsOpen);

  const { repositories, addRepo, removeRepo } = useRepositories();
  const [query, setQuery] = useState("");

  const normalizedQuery = query.trim();
  const filteredRepositories = normalizedQuery
    ? repositories.filter((repo) => {
        const name = repo.name ?? "";
        const path = repo.path ?? "";
        return (
          matchesSearchQuery(name, normalizedQuery) ||
          matchesSearchQuery(path, normalizedQuery)
        );
      })
    : repositories;

  /* we are grouping repositories by owners */
  // const origin = parseOrigin(item.origin || "");
  // const icon = getAvatarByProvider(origin?.provider);
  const groupedByOwner = filteredRepositories.reduce(
    (acc, repo) => {
      const origin = parseOrigin(repo.origin || "");
      const owner = origin?.owner || "unknown";
      const provider = origin?.provider || "unknown";
      const key = `${provider}/${owner}`;

      if (!acc[key]) {
        acc[key] = {
          owner,
          provider,
          avatarUrl: origin?.avatarUrl,
          repos: [],
        };
      }
      acc[key].repos.push(repo);
      return acc;
    },
    {} as Record<
      string,
      {
        owner: string;
        provider: GIT_PROVIDERS;
        avatarUrl?: string;
        repos: typeof repositories;
      }
    >,
  );

  return (
    <div>
      <div className="w-full p-2 border-b flex justify-between items-center gap-2">
        <InputGroup>
          <InputGroupInput
            aria-label="Search"
            placeholder="Search"
            type="search"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
          />
          <InputGroupAddon>
            <SearchIcon aria-hidden="true" />
          </InputGroupAddon>
        </InputGroup>

        <DropdownMenu>
          <DropdownMenuTrigger render={<Button size={"sm"} />}>
            Add
            <ChevronDownIcon
              className="-me-1 opacity-60"
              size={16}
              aria-hidden="true"
            />
          </DropdownMenuTrigger>
          <DropdownMenuContent align="end">
            <DropdownMenuItem
              onClick={async () => {
                console.log(repositories);
              }}
            >
              Clone repository...
            </DropdownMenuItem>
            <DropdownMenuItem
              onClick={async () => {
                const folder = await open({
                  directory: true,
                  multiple: false,
                });

                if (folder) {
                  if (repositories.find((r) => r.path === folder)) {
                    toast.error("Repository already added");
                    return;
                  }

                  try {
                    const repo = await addRepo(folder);
                    if (repo) {
                      setSelectedRepository(repo);
                      setRepoSelectIsOpen(false);
                      toast.success("Repository added successfully!");
                    }
                  } catch (error) {
                    // Error already handled by the hook
                  }
                }
              }}
            >
              Add local repository
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
      </div>
      <div className="max-h-full _flex-1">
        <div className="">
          {groupedByOwner &&
            Object.entries(groupedByOwner)
              .filter(([, repos]) => repos.repos.length > 0)
              .map(([owner, repos]) => {
                const icon = getAvatarByProvider(repos?.provider || undefined);

                return (
                  <div key={owner} className="border-b">
                    <div className="text-muted-foreground flex items-center px-2 py-1">
                      <div className="size-3.5 text-lg text-foreground mr-1">
                        {icon || <BadgeQuestionMark className="size-3.5" />}
                      </div>
                      {origin ? (
                        <div>
                          <span>/</span>
                          <Avatar className="rounded-sm size-4 -translate-y-px mx-1">
                            <AvatarImage alt="User" src={repos.avatarUrl} />
                            <AvatarFallback>
                              {owner.charAt(0).toUpperCase()}
                            </AvatarFallback>
                          </Avatar>
                          <span>{repos.owner}</span>
                        </div>
                      ) : (
                        <div>
                          <span className="text-foreground">{owner}</span>
                        </div>
                      )}
                    </div>
                    {repos.repos.map((repo) => (
                      <RepositoryListItem
                        key={repo.id}
                        repo={repo}
                        isSelected={selectedRepository?.id === repo.id}
                        onSelect={() => {
                          setSelectedRepository(repo);
                          setRepoSelectIsOpen(false);
                        }}
                        onRemove={() => {
                          removeRepo(repo.id);
                          if (selectedRepository?.id === repo.id) {
                            setSelectedRepository(null);
                          }
                        }}
                      />
                    ))}
                  </div>
                );
              })}
        </div>
      </div>
    </div>
  );
});

const StashPocView = memo(function StashPocView({
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

  const selectedRepository = useAppStore((state) => state.selectedRepository);
  const selectionByRepo = useAppStore((state) => state.selectionByRepo);
  const setStashSelectionForRepo = useAppStore(
    (state) => state.setStashSelectionForRepo,
  );
  const clearStashSelectionForRepo = useAppStore(
    (state) => state.clearStashSelectionForRepo,
  );
  const pruneStashSelectionsForRepo = useAppStore(
    (state) => state.pruneStashSelectionsForRepo,
  );
  const gitViewByRepo = useAppStore((state) => state.gitViewByRepo);
  const setGitViewStateForRepo = useAppStore(
    (state) => state.setGitViewStateForRepo,
  );

  const { handleFileClick } = useFileSelectionStore();
  const { data: currentBranchStash } = useGetCurrentBranchStash();
  const { data: stashes, isLoading: isStashesLoading } = useStashList();
  const { mutateAsync: popStash, isPending: isRestoreAllPending } =
    useStashPop();
  const { mutateAsync: dropStash, isPending: isDiscardAllPending } =
    useStashDrop();
  const { mutateAsync: restoreStashFile } = useStashRestoreFile();

  const repoPath = selectedRepository?.path ?? "";
  const persistedSelectedReference =
    gitViewByRepo[repoPath]?.selectedStashReference ?? null;
  const selectedReference =
    mode === "branch"
      ? (currentBranchStash?.reference ?? null)
      : persistedSelectedReference;

  useEffect(() => {
    if (mode !== "all") {
      return;
    }

    if (!stashes || stashes.length === 0) {
      setGitViewStateForRepo({ selectedStashReference: null }, repoPath);
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
      repoPath,
    );
  }, [mode, repoPath, selectedReference, setGitViewStateForRepo, stashes]);

  useEffect(() => {
    if (!repoPath) {
      return;
    }

    const references = (stashes ?? []).map((stash) => stash.reference);
    pruneStashSelectionsForRepo(repoPath, references);
  }, [pruneStashSelectionsForRepo, repoPath, stashes]);

  const { data: stashShow, isLoading: isStashShowLoading } =
    useStashShow(selectedReference);

  const hasFilterSelection = hasActiveStatusFilters(statusFilters);
  const filteredFiles = (stashShow?.files ?? []).filter(
    (file) =>
      matchesStatusFilters(file, statusFilters) &&
      matchesSearchQuery(file.path, query),
  );

  const selectedFileForCurrentRepo = selectedReference
    ? (selectionByRepo[repoPath]?.stashByReference[selectedReference] ?? null)
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

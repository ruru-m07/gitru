import { GetStatusResponse, GraphRow } from "@gitru/commands";
import { Stashed } from "@gitru/icon";
import { Mascot } from "@gitru/mascot";
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
import { Badge } from "@gitru/ui/components/badge";
import { Button } from "@gitru/ui/components/button";
import { useCommandNavigation } from "@gitru/ui/components/command";
import { CopyButton } from "@gitru/ui/components/copy-button";
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
import { Separator } from "@gitru/ui/components/separator";
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
  BookCopy,
  ChevronDown,
  ChevronDownIcon,
  ChevronLeftIcon,
  ChevronsRight,
  ChevronUp,
  CircleAlertIcon,
  CircleDashed,
  CopyPlus,
  Files,
  GitBranch,
  History,
  ListFilterPlus,
  Loader2,
  SearchIcon,
  Sparkles,
  Tags,
  Undo2,
  UserPlus,
} from "lucide-react";
import { AnimatePresence, motion, useReducedMotion } from "motion/react";
import { memo, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useOnInView } from "react-intersection-observer";
import { toast } from "sonner";
import z from "zod";
import { useFileSelectionStore } from "@/components/diff/useFileSelectionStore";
import { getStatusIcon } from "@/components/getStatusIcon";
import Logo from "@/components/logo";
import PageLayout from "@/components/pageLayout";
import { RepositoryListItem } from "@/components/RepositoryListItem";
import { ResizableLayout } from "@/components/resizableLayout";
import StatusBar from "@/components/statusBar";
import { VirtualizedFileList } from "@/components/VirtualizedFileList";
import {
  useCreateCommit,
  useGetCommitById,
  useGetCurrentBranch,
  useGetCurrentBranchStash,
  useGetStatus,
  useGitAdd,
  useGitDiscard,
  useGitHistoryGraph,
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
import {
  selectActiveRepoSelectIsOpen,
  selectActiveRepository,
  selectActiveSessionRepoKey,
  useAppStore,
} from "@/store/useAppStore";
import { GIT_PROVIDERS } from "@/types/app";
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

const getVisibleFilePaths = (files: GetStatusResponse["files"]) =>
  Array.from(
    new Set(
      files.flatMap((file) =>
        file.new_path ? [file.path, file.new_path] : [file.path],
      ),
    ),
  );

export const Route = createFileRoute("/app/git")({
  component: GitPageLayout,
});

function GitPageLayout() {
  const activeRepository = useAppStore(selectActiveRepository);
  const setSelectedRepository = useAppStore(
    (state) => state.setSelectedRepository,
  );
  const setRepoSelectIsOpen = useAppStore((state) => state.setRepoSelectIsOpen);
  const { repositories, addRepo } = useRepositories();
  const navigation = useCommandNavigation();

  if (!activeRepository) {
    return (
      <PageLayout className="flex-col flex justify-center items-center gap-4">
        <div className="flex flex-col gap-4 justify-center">
          <span className="flex items-center gap-3 px-[calc(--spacing(3)-1px)]">
            <div className="relative cursor-pointer **:data-[name='mascot-svg']:size-8 **:data-[name='heart-svg']:scale-50">
              <Mascot
                particles={{
                  offset: {
                    x: -0.1,
                    y: -0.5,
                  },
                }}
                transition={{
                  duration: 0.3,
                }}
              />
            </div>
            {/* <h1 className="text-3xl font-[350]">Add your first repository</h1> */}
            <span className="text-3xl">Gitru</span>
          </span>
          <div className="flex justify-between items-end px-[calc(--spacing(3)-1px)]">
            <h1 className="text-muted-foreground font-normal">
              {repositories?.length === 0 ? "Add" : "Select"} repositorys to get
              start!
            </h1>
            <a
              href="https://gitru.app/docs"
              className="text-sm font-normal text-muted-foreground hover:underline opacity-70 hover:opacity-100 transition-opacity"
              target="_blank"
            >
              Learn more ↗
            </a>
          </div>
          <div className="grid grid-cols-3 gap-4 px-[calc(--spacing(3)-1px)]">
            <Button
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
              className="flex-col group h-fit! items-start pt-4 pb-2 gap-2 w-64"
            >
              <BookCopy className="size-5.5" />
              <span className="text-lg font-[450]">
                Import Local Repository
              </span>
            </Button>
            <Button
              variant="secondary"
              className="flex-col group h-fit! items-start pt-4 pb-2 gap-2 w-64"
              onClick={() => {
                navigation.setOpen(true);
                navigation.push("clone-repository");
              }}
            >
              <span className="flex items-center gap-2 pl-1">
                <span className="-rotate-20 -mr-2">
                  {getAvatarByProvider("bitbucket", "size-5.5")}
                </span>

                <span className="z-10">
                  {getAvatarByProvider("github", "size-5.5")}
                </span>

                <span className="rotate-20 -ml-2">
                  {getAvatarByProvider("gitlab", "size-5.5")}
                </span>
              </span>
              <span className="text-lg font-normal">Clone from Remote</span>
            </Button>
            <Button
              variant={"secondary"}
              className="flex-col group h-fit! items-start pt-4 pb-2 gap-2 w-64"
              onClick={() => {
                navigation.setOpen(true);
                navigation.push("init-repository");
              }}
            >
              <CopyPlus className="size-5.5" />
              <span className="text-lg font-normal">Create New Repository</span>
            </Button>
          </div>
          {repositories && repositories.length > 0 ? (
            <div className="flex flex-col w-full">
              <div className="px-[calc(--spacing(3)-1px)] w-full">
                <div className="relative my-4">
                  <Separator />
                  <span className="font-normal absolute -top-3 left-6 -translate-x-1/2 bg-background px-2 text-sm text-muted-foreground">
                    Recent
                  </span>
                </div>
              </div>
              <div className="flex flex-col w-full">
                {repositories.map((repo) => {
                  const origin = parseOrigin(repo.origin || "");

                  return (
                    <Button
                      variant={"ghost"}
                      className={`py-4 px-[calc(--spacing(3)-1px)]`}
                      size={"lg"}
                      onClick={() => {
                        setSelectedRepository(repo);
                        setRepoSelectIsOpen(false);
                      }}
                    >
                      <div className="flex w-full justify-between items-center gap-2 overflow-hidden">
                        <div className="flex items-center gap-1 flex-1">
                          <div className="text-muted-foreground flex items-center">
                            {origin ? (
                              <div>
                                <Avatar className="rounded-sm size-4 -translate-y-px">
                                  <AvatarImage
                                    alt="User"
                                    src={origin.avatarUrl}
                                  />
                                  <AvatarFallback>{repo.origin}</AvatarFallback>
                                </Avatar>
                                <span className="ml-1.5">{origin?.owner}</span>
                                <span className="ml-1">/</span>
                              </div>
                            ) : (
                              <div>
                                <span className="text-foreground">
                                  {repo.origin}
                                </span>
                              </div>
                            )}
                          </div>
                          <span className="font-medium text-sm text-left text-nowrap leading-none">
                            {repo.name}
                          </span>
                          {repo?.has_uncommitted_changes && (
                            <Tooltip>
                              <TooltipTrigger
                                render={
                                  <Badge variant={"warning"} className="ml-1" />
                                }
                              >
                                <CircleDashed className="size-3" />
                              </TooltipTrigger>
                              <TooltipPopup className={"dark"}>
                                Uncommitted changes
                              </TooltipPopup>
                            </Tooltip>
                          )}
                        </div>
                        <div className="flex items-center gap-1 ml-1 min-w-0">
                          {(repo.ahead_behind?.[0] || 0) > 0 ? (
                            <Badge
                              variant={"error"}
                              className="ml-1 font-normal tabular-nums"
                            >
                              <span className="translate-y-px">
                                <span className="ml-0.5 mr-0.75 h-fit">↑</span>
                                {repo.ahead_behind?.[0] || 0}
                              </span>
                            </Badge>
                          ) : null}
                          {(repo.ahead_behind?.[1] || 0) > 0 ? (
                            <Badge
                              variant={"warning"}
                              className="ml-1 font-normal flex items-center tabular-nums"
                            >
                              <span className="translate-y-px">
                                <span className="ml-0.5 mr-0.75 h-fit">↓</span>
                                {repo.ahead_behind?.[1] || 0}
                              </span>
                            </Badge>
                          ) : null}
                          <Badge
                            variant={"info"}
                            className="flex items-center min-w-0 flex-1"
                          >
                            <span className="ml-0.5 mr-px h-fit">
                              <GitBranch strokeWidth={2.5} className="size-3" />
                            </span>
                            <span className="truncate max-w-full min-w-0 font-[450]">
                              {repo.current_branch}
                            </span>
                          </Badge>
                        </div>
                      </div>
                    </Button>
                  );
                })}
              </div>
            </div>
          ) : null}
        </div>
      </PageLayout>
    );
  }

  return (
    <PageLayout className="flex-col flex justify-between">
      <ResizableArea />
      <StatusBar />
    </PageLayout>
  );
}

const ResizableArea = () => {
  const repoSelectIsOpen = useAppStore(selectActiveRepoSelectIsOpen);
  const setRepoSelectIsOpen = useAppStore((state) => state.setRepoSelectIsOpen);
  const activeRepository = useAppStore(selectActiveRepository);
  const repoStateKey = useAppStore(selectActiveSessionRepoKey);
  const setGitViewStateForRepo = useAppStore(
    (state) => state.setGitViewStateForRepo,
  );

  const shouldReduceMotion = useReducedMotion();
  const [panelDirection, setPanelDirection] = useState<1 | -1>(1);
  const repoPath = activeRepository?.path ?? "";
  const repoGitViewState = useAppStore((state) =>
    repoStateKey ? state.gitViewByRepo[repoStateKey] : undefined,
  );
  const gitViewState: {
    leftPanelView: "changes" | "stash" | "history";
    changesTab: "changes" | "history";
    stashViewMode: "branch" | "all";
    selectedStashReference: string | null;
    selectedHistoryCommitHash: string | null;
    stashStatusFilters: Record<FileStatusFilter, boolean>;
  } = repoPath
    ? (repoGitViewState ?? {
        leftPanelView: "changes",
        changesTab: "changes",
        stashViewMode: "branch",
        selectedStashReference: null,
        selectedHistoryCommitHash: null,
        stashStatusFilters: DEFAULT_STATUS_FILTERS,
      })
    : {
        leftPanelView: "changes",
        changesTab: "changes",
        stashViewMode: "branch",
        selectedStashReference: null,
        selectedHistoryCommitHash: null,
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

  return (
    <div className="flex h-full">
      <ResizableLayout id="local-git-layout" minWidth={350} maxWidth={800}>
        <div className="flex flex-col h-full">
          <ToggelPanelButton />
          <div className="h-full border-t max-h-[calc(var(--layout-height)---spacing(13.75))] relative overflow-hidden">
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
                            changesTab: "changes",
                          },
                          repoPath,
                        );
                      }}
                    />
                  </motion.div>
                ) : gitViewState.leftPanelView === "history" ? (
                  <motion.div
                    key="history"
                    className="absolute inset-0 bg-background will-change-transform"
                    custom={panelDirection}
                    variants={panelSlideVariants}
                    initial="initial"
                    animate="animate"
                    exit="exit"
                    transition={panelTransition}
                  >
                    <HistoryDetailView
                      onBack={() => {
                        setPanelDirection(-1);
                        setGitViewStateForRepo(
                          {
                            leftPanelView: "changes",
                            changesTab: "history",
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
                      activeTab={gitViewState.changesTab}
                      onTabChange={(nextTab) => {
                        setGitViewStateForRepo(
                          { changesTab: nextTab },
                          repoPath,
                        );
                      }}
                      onOpenHistoryView={(commitHash) => {
                        setPanelDirection(1);
                        setGitViewStateForRepo(
                          {
                            leftPanelView: "history",
                            changesTab: "history",
                            selectedHistoryCommitHash: commitHash,
                          },
                          repoPath,
                        );
                      }}
                      onOpenStashView={(stashReference) => {
                        setPanelDirection(1);
                        setGitViewStateForRepo(
                          {
                            leftPanelView: "stash",
                            changesTab: "changes",
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
        </div>
        <div
          className={cn(
            "relative w-(--right-width) h-(--layout-height)",
            repoSelectIsOpen && "cursor-pointer",
          )}
          onClick={() => setRepoSelectIsOpen(false)}
        >
          {repoSelectIsOpen && (
            <div className="absolute inset-0 bg-background/40 z-10 w-full h-full backdrop-blur-[2px]"></div>
          )}
          <Outlet />
        </div>
      </ResizableLayout>
    </div>
  );
};

const DiscardChangesDialog = memo(function DiscardChangesDialog({
  filePaths,
  label,
}: {
  filePaths: string[];
  label?: string;
}) {
  const [open, setOpen] = useState(false);
  const [isDeleteLoading, setIsDeleteLoading] = useState(false);

  const { mutateAsync: discardChanges } = useGitDiscard();

  const isBulkDiscard = filePaths.length > 1;
  const titleLabel =
    label ??
    (isBulkDiscard
      ? `${filePaths.length} visible files`
      : (filePaths[0]?.split("/").pop() ?? "selected changes"));

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
                {titleLabel}
              </span>
              ?
            </DialogTitle>
            <DialogDescription className="sm:text-center">
              This action cannot be undone. Only the currently visible selected
              changes will be permanently lost.
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
                  filePath: isBulkDiscard ? filePaths : (filePaths[0] ?? ""),
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

const WriteCommitBox = memo(function WriteCommitBox({
  visibleAddablePaths,
}: {
  visibleAddablePaths: string[];
}) {
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
      if (visibleAddablePaths.length === 0) {
        toast.error("No visible changes to add");
        return;
      }

      await gitAdd(visibleAddablePaths);
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
  }, [
    co_authors,
    createCommit,
    description,
    gitAdd,
    nothingToCommit,
    title,
    visibleAddablePaths,
  ]);

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
                    <span>Add visible & Commit</span>
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
  const repoSelectIsOpen = useAppStore(selectActiveRepoSelectIsOpen);
  const setRepoSelectIsOpen = useAppStore((state) => state.setRepoSelectIsOpen);
  const activeRepository = useAppStore(selectActiveRepository);

  return (
    <Button
      onClick={() => {
        setRepoSelectIsOpen(!repoSelectIsOpen);
      }}
      className={cn(
        "rounded-none justify-between min-h-13.75 max-h-13.75 pl-2",
        repoSelectIsOpen && "bg-accent",
      )}
      variant={"ghost"}
    >
      <div className="flex-col flex items-start">
        <span className="text-xs text-muted-foreground font-[430]">
          Current Repository
        </span>
        <span>{activeRepository?.name || "No repository selected"}</span>
      </div>
      {repoSelectIsOpen ? <ChevronUp size={18} /> : <ChevronDown size={18} />}
    </Button>
  );
};

const ListFileChanges = ({
  activeTab,
  onTabChange,
  onOpenHistoryView,
  onOpenStashView,
}: {
  activeTab: "changes" | "history";
  onTabChange: (tab: "changes" | "history") => void;
  onOpenHistoryView: (commitHash: string) => void;
  onOpenStashView: (stashReference: string | null) => void;
}) => {
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
            <Input
              aria-label="Filter files"
              placeholder="Filter files..."
              className={"rounded-l-md! border-border! w-full"}
              size={"sm"}
              value={changesQuery}
              onChange={(e) => setChangesQuery(e.target.value)}
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
};

const HistoryCommitInfiniteList = ({
  rows,
  onOpenCommit,
  fetchNextPage,
  hasNextPage,
  isFetchingNextPage,
  isLoading,
}: {
  rows: GraphRow[];
  onOpenCommit: (commitHash: string) => void;
  fetchNextPage: () => void;
  hasNextPage: boolean;
  isFetchingNextPage: boolean;
  isLoading: boolean;
}) => {
  const scrollRef = useRef<HTMLDivElement>(null);
  const bottomRef = useOnInView(
    (inView) => {
      if (inView && hasNextPage && !isFetchingNextPage) {
        fetchNextPage();
      }
    },
    {
      root: scrollRef.current,
      threshold: 0,
      rootMargin: "500px",
    },
  );

  if (isLoading) {
    return (
      <div className="w-full h-full flex items-center justify-center">
        <span className="text-sm text-muted-foreground">Loading...</span>
      </div>
    );
  }

  return (
    <div ref={scrollRef} className="flex-1 min-h-0 overflow-y-auto">
      {rows.map((row) => (
        <div
          className="w-full p-2 border-b hover:bg-accent cursor-pointer"
          key={row.oid}
          onClick={() => onOpenCommit(row.commit.id)}
        >
          <p className="truncate text-sm">{row.commit.summary}</p>
          <div className="flex mt-1 items-center justify-between w-full">
            <TooltipProvider>
              <div className="flex items-center">
                <div className="flex -mt-0.5 group">
                  <Tooltip>
                    <TooltipTrigger
                      style={{
                        zIndex: row.commit.authors.co_authors.length + 1,
                      }}
                    >
                      <Avatar className="ring-2 ring-background rounded-sm size-4">
                        <AvatarImage
                          alt={row.commit.authors.author.name}
                          src={`https://avatars.githubusercontent.com/u/e?email=${row.commit.authors.author.email}&s=64`}
                        />
                        <AvatarFallback>
                          {row.commit.authors.author.name
                            .split(" ")
                            .map((n) => n[0])
                            .join("")
                            .toUpperCase()}
                        </AvatarFallback>
                      </Avatar>
                    </TooltipTrigger>
                    <TooltipPopup side="bottom">
                      {row.commit.authors.author.name}
                    </TooltipPopup>
                  </Tooltip>
                  {row.commit.authors.co_authors.map((coAuthor, idx) => (
                    <Tooltip key={`${row.oid}-${idx}-tooltip-coauthor`}>
                      <TooltipTrigger
                        style={{
                          zIndex: row.commit.authors.co_authors.length - idx,
                        }}
                        key={`${row.oid}-${idx}-tooltip-trigger-coauthor`}
                      >
                        <Avatar className="ring-2 ring-background rounded-sm size-4 -ml-[0.2rem] group-hover:ml-0.5 transition-all duration-100">
                          <AvatarImage
                            alt={coAuthor.name}
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
                      <TooltipPopup side="bottom">{coAuthor.name}</TooltipPopup>
                    </Tooltip>
                  ))}
                </div>
                <Label className="ml-1 text-muted-foreground text-xs font-light">
                  <Tooltip>
                    <TooltipTrigger>
                      {row.commit.authors.author.name}
                    </TooltipTrigger>
                    <TooltipPopup side="bottom">
                      {row.commit.authors.author.email}
                    </TooltipPopup>
                  </Tooltip>
                  <span className="-mx-1">{" • "}</span>
                  <Tooltip>
                    <TooltipTrigger>
                      {timeAgoFromUnixSeconds(row.commit.timestamp)}{" "}
                    </TooltipTrigger>
                    <TooltipPopup side="bottom">
                      {formatUnixSecondsToDateTime(row.commit.timestamp)}
                    </TooltipPopup>
                  </Tooltip>
                </Label>
              </div>
              <div className="flex gap-1 items-center">
                {row.tags.length > 0 && (
                  <>
                    <Tooltip>
                      <TooltipTrigger className={"flex gap-0.5"}>
                        <Tags
                          className="size-3.5 text-muted-foreground"
                          aria-label={`${row.tags.length} tags`}
                        />
                        <span className="text-xs text-muted-foreground tabular-nums font-normal">
                          {row.tags.length}
                        </span>
                      </TooltipTrigger>
                      <TooltipPopup>
                        {row.tags.map((tag) => (
                          <Badge key={tag.name}>{tag.name}</Badge>
                        ))}
                      </TooltipPopup>
                    </Tooltip>
                    <span className="text-xs text-muted-foreground">/</span>
                  </>
                )}

                <span className="text-xs text-muted-foreground flex items-center gap-1">
                  <Files className="size-3.5" />
                  {row?.commit.stats?.files_changed ?? 0}
                </span>

                <span className="text-xs text-muted-foreground">/</span>

                <span className="text-xs text-green-600 tabular-nums font-normal">
                  +{row?.commit.stats?.insertions ?? 0}
                </span>
                <span className="text-xs text-red-600 tabular-nums font-normal">
                  -{row?.commit.stats?.deletions ?? 0}
                </span>
              </div>
            </TooltipProvider>
          </div>
        </div>
      ))}
      <div className="w-full flex justify-center p-2 text-xs text-muted-foreground">
        {isFetchingNextPage ? "Loading more..." : null}
      </div>
      <div ref={bottomRef} className="h-4" />
    </div>
  );
};

const HistoryDetailView = ({ onBack }: { onBack: () => void }) => {
  const [query, setQuery] = useState("");

  const activeRepository = useAppStore(selectActiveRepository);
  const repoStateKey = useAppStore(selectActiveSessionRepoKey);
  const repoPath = activeRepository?.path ?? "";
  const repoGitViewState = useAppStore((state) =>
    repoStateKey ? state.gitViewByRepo[repoStateKey] : undefined,
  );
  const repoSelectionState = useAppStore((state) =>
    repoStateKey ? state.selectionByRepo[repoStateKey] : undefined,
  );
  const setHistorySelectionForRepo = useAppStore(
    (state) => state.setHistorySelectionForRepo,
  );
  const clearHistorySelectionForRepo = useAppStore(
    (state) => state.clearHistorySelectionForRepo,
  );
  const handleFileClick = useFileSelectionStore(
    (state) => state.handleFileClick,
  );

  const selectedCommitHash =
    repoGitViewState?.selectedHistoryCommitHash ?? null;
  const { data: commitDetails, isLoading: isCommitLoading } = useGetCommitById(
    selectedCommitHash ?? "",
  );

  const selectedFileForCurrentRepo =
    selectedCommitHash && repoPath
      ? (repoSelectionState?.historyByCommit?.[selectedCommitHash] ?? null)
      : null;
  const resolvedHistorySelection = resolveFileSelection({
    selection: selectedFileForCurrentRepo,
    files: commitDetails?.files ?? [],
    context: {
      source: "history",
      historyCommitHash: selectedCommitHash,
    },
  });

  useEffect(() => {
    if (!selectedCommitHash || !commitDetails?.files?.length) {
      return;
    }

    if (resolvedHistorySelection.state === "valid") {
      return;
    }

    const firstFile = commitDetails.files[0];
    setHistorySelectionForRepo(selectedCommitHash, {
      filePath: firstFile.path,
      fileNewPath: firstFile.new_path,
      source: "history",
      historyCommitHash: selectedCommitHash,
      selectedAt: Date.now(),
    });
  }, [
    commitDetails?.files,
    resolvedHistorySelection.state,
    selectedCommitHash,
    setHistorySelectionForRepo,
  ]);

  const selectedHistoryFileForList =
    resolvedHistorySelection.state === "valid"
      ? resolvedHistorySelection.identity
      : selectedFileForCurrentRepo;
  const filteredFiles = (commitDetails?.files ?? []).filter((file) =>
    matchesSearchQuery(file.path, query),
  );

  return (
    <div className="h-full flex flex-col">
      <div className="px-2 py-1 border-b">
        <div className="group flex-1 flex h-6 items-center">
          <span className="text-sm truncate">
            {commitDetails?.summary ?? ""}
          </span>
          <Tooltip>
            <TooltipTrigger
              render={
                <CopyButton
                  size={"xs"}
                  variant="ghost"
                  text={selectedCommitHash ?? ""}
                  className="group-hover:flex hidden"
                />
              }
            ></TooltipTrigger>
            <TooltipPopup>{selectedCommitHash}</TooltipPopup>
          </Tooltip>
        </div>
        <div className="flex justify-between w-full flex-1">
          <div className="flex items-center gap-1 flex-1 mr-1">
            <div className="flex group">
              <Tooltip>
                <TooltipTrigger
                  style={{
                    zIndex: (commitDetails?.authors.co_authors.length || 0) + 1,
                  }}
                >
                  <Avatar className="ring-2 ring-background rounded-sm size-4">
                    <AvatarImage
                      alt={commitDetails?.authors.author.name}
                      src={`https://avatars.githubusercontent.com/u/e?email=${commitDetails?.authors.author.email}&s=64`}
                    />
                    <AvatarFallback>
                      {commitDetails?.authors.author.name
                        .split(" ")
                        .map((n) => n[0])
                        .join("")
                        .toUpperCase()}
                    </AvatarFallback>
                  </Avatar>
                </TooltipTrigger>
                <TooltipPopup side="bottom">
                  {commitDetails?.authors.author.name}
                </TooltipPopup>
              </Tooltip>
              {commitDetails?.authors.co_authors.map((coAuthor, idx) => (
                <Tooltip key={`${idx}-tooltip-coauthor`}>
                  <TooltipTrigger
                    style={{
                      zIndex: commitDetails?.authors.co_authors.length - idx,
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
                  <TooltipPopup side="bottom">{coAuthor.name}</TooltipPopup>
                </Tooltip>
              ))}
            </div>
            <span className="font-normal text-sm text-nowrap">
              {commitDetails?.authors.author.name}
            </span>
            <span className="text-muted-foreground font-light text-xs text-nowrap flex-1 truncate">
              ( {timeAgoFromUnixSeconds(commitDetails?.timestamp || 0)} ) {}
            </span>
          </div>
          <div className="flex gap-1 items-center">
            <span className="text-xs text-muted-foreground flex items-center gap-1">
              <Files className="size-3.5" />
              {commitDetails?.stats?.files_changed ?? 0}
            </span>

            <span className="text-xs text-muted-foreground">/</span>

            <span className="text-xs text-green-600 tabular-nums font-normal">
              +{commitDetails?.stats?.insertions ?? 0}
            </span>
            <span className="text-xs text-red-600 tabular-nums font-normal">
              -{commitDetails?.stats?.deletions ?? 0}
            </span>
          </div>
        </div>
      </div>
      <div className="p-1.5 min-h-10 border-b flex items-center gap-2">
        <Button size="icon-sm" variant="outline" onClick={onBack}>
          <ChevronLeftIcon className="size-4" />
        </Button>
        <Group aria-label="History file actions" className="w-full">
          <Input
            aria-label="Filter history files"
            placeholder="Filter files..."
            size={"sm"}
            value={query}
            onChange={(e) => setQuery(e.target.value)}
          />
        </Group>
      </div>
      <div className="flex-1 min-h-0 overflow-y-auto custom-scroll **:data-[slot=file-row]:mr-2!">
        {isCommitLoading ? (
          <div className="w-full h-full flex items-center justify-center">
            <span className="text-sm text-muted-foreground">Loading...</span>
          </div>
        ) : !selectedCommitHash ? (
          <div className="w-full h-full flex items-center justify-center">
            <span className="text-sm text-muted-foreground">
              No commit selected
            </span>
          </div>
        ) : filteredFiles.length > 0 ? (
          <VirtualizedFileList
            sectionMode="flat"
            searchQuery={query}
            sections={[
              {
                id: "history-files",
                name: "Changed Files",
                type: "custom",
                files: filteredFiles,
              },
            ]}
            onFileClick={handleFileClick}
            setSelectedFilePath={(file) => {
              if (!selectedCommitHash) return;
              if (!file) {
                clearHistorySelectionForRepo(selectedCommitHash);
                return;
              }
              setHistorySelectionForRepo(selectedCommitHash, file);
            }}
            selectedFilePath={
              selectedHistoryFileForList
                ? {
                    path: selectedHistoryFileForList.filePath,
                    newPath: selectedHistoryFileForList.fileNewPath,
                  }
                : undefined
            }
            className="h-full"
          />
        ) : (
          <div className="w-full h-full flex items-center justify-center">
            <span className="text-sm text-muted-foreground">
              No files changed in this commit
            </span>
          </div>
        )}
      </div>
    </div>
  );
};

const ListRepositories = memo(() => {
  const navigation = useCommandNavigation();
  const activeRepository = useAppStore(selectActiveRepository);
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
              onClick={() => {
                navigation.setOpen(true);
                navigation.push("clone-repository");
              }}
            >
              Clone repository...
            </DropdownMenuItem>
            <DropdownMenuItem
              onClick={() => {
                navigation.setOpen(true);
                navigation.push("init-repository");
              }}
            >
              Initialize repository...
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
                const hasKnownOrigin = repos.provider !== "unknown";

                return (
                  <div key={owner} className="border-b">
                    <div className="text-muted-foreground flex items-center px-2 py-1">
                      <div className="size-3.5 text-lg text-foreground mr-1">
                        {icon || <BadgeQuestionMark className="size-3.5" />}
                      </div>
                      {hasKnownOrigin ? (
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
                        dataRepoId={repo.id}
                        isSelected={activeRepository?.id === repo.id}
                        onSelect={() => {
                          setSelectedRepository(repo);
                          setRepoSelectIsOpen(false);
                        }}
                        onRemove={() => {
                          removeRepo(repo.id);
                          if (activeRepository?.id === repo.id) {
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

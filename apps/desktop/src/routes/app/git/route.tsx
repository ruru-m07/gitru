import {
  Accordion,
  AccordionContent,
  AccordionItem,
  AccordionTrigger,
} from "@gitru/ui/components/accordion";
import {
  Avatar,
  AvatarFallback,
  AvatarImage,
} from "@gitru/ui/components/avatar";
import { Badge } from "@gitru/ui/components/badge";
import { Button, buttonVariants } from "@gitru/ui/components/button";
import * as contextMenu from "@gitru/ui/components/context-menu";
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
import { Input } from "@gitru/ui/components/input";
import {
  InputGroup,
  InputGroupAddon,
  InputGroupTextarea,
} from "@gitru/ui/components/input-group";
import { Kbd } from "@gitru/ui/components/kbd";
import { Label } from "@gitru/ui/components/label";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@gitru/ui/components/menu";
import {
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup,
} from "@gitru/ui/components/resizable";
import { ScrollArea } from "@gitru/ui/components/scroll-area";
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
  AlertTriangle,
  BetweenHorizontalEnd,
  BetweenHorizontalStart,
  BetweenVerticalEnd,
  ChevronDown,
  ChevronDownIcon,
  ChevronUp,
  CircleAlertIcon,
  ClipboardCopy,
  CopyPlus,
  CornerUpRight,
  Diff,
  EyeOff,
  GitBranch,
  GitCommitHorizontal,
  Minus,
  Plus,
  SearchIcon,
  Sparkles,
  SquareDot,
  SquarePlus,
  SquareX,
  Undo2,
  UserPlus,
} from "lucide-react";
import { useEffect, useState } from "react";
import { toast } from "sonner";
import { useDiffViewStore } from "@/components/diff/useDiffViewStore";
import {
  formatUnixSecondsToDateTime,
  timeAgoFromUnixSeconds,
} from "@/lib/time";
import { useRepositoryActions, useStatus } from "@/state/hooks";
import { useAppStore } from "@/store/useAppStore";
import {
  addLocalGitRepo,
  CommitInfo,
  type FileStatus,
  type FileStatusKind,
  history,
} from "@/tauri";

export const Route = createFileRoute("/app/git")({
  component: GitPageLayout,
});

function GitPageLayout() {
  const {
    repoSelectIsOpen,
    setRepoSelectIsOpen,
    setRepositories,
    repositories,
    setSelectedRepository,
    selectedRepository,
  } = useAppStore();

  const { data: status, isLoading: isStatusLoading } = useStatus();
  const actions = useRepositoryActions();

  const [commitHistory, setCommitHistory] = useState<CommitInfo[]>([]);

  const stagedChanges = status?.files.filter((file) =>
    file.status.some((s) => s.startsWith("Index")),
  );
  const unstagedChanges = status?.files.filter((file) =>
    file.status.some((s) => s.startsWith("Worktree")),
  );

  useEffect(() => {
    (async () => {
      if (selectedRepository) {
        const data = await history({
          repoPath: selectedRepository?.path,
          limit: 100,
          skip: 0,
        });
        setCommitHistory(data);
      }
    })();
  }, [selectedRepository]);

  return (
    <ResizablePanelGroup
      className={cn(
        "ml-[var(--main-actual-content-padding)] bg-accent/35 ring-1 ring-inset ring-border h-full w-full rounded-md flex overflow-hidden",
      )}
      // @ts-ignore
      direction="horizontal"
      autoSaveId="git-page-layout"
    >
      {
        // @ts-ignore
        <ResizablePanel
          defaultSize={18}
          minSize={18}
          maxSize={44}
          className="flex flex-col h-full"
        >
          <button
            onClick={() => {
              setRepoSelectIsOpen(!repoSelectIsOpen);
            }}
            className="flex justify-between items-center border-b px-2 pt-2 pb-1 hover:bg-accent/40 cursor-pointer min-h-14 max-h-14"
            type="button"
          >
            <div className="flex-col flex items-start">
              <span className="text-xs text-muted-foreground">
                Current Repository
              </span>
              <span>
                {selectedRepository?.name || "No repository selected"}
              </span>
            </div>
            {repoSelectIsOpen ? (
              <ChevronUp size={18} />
            ) : (
              <ChevronDown size={18} />
            )}
          </button>
          <div className="h-[calc(100vh_-_calc(var(--spacing)_*_14)_-_calc(var(--spacing)_*_9)_-_calc(var(--spacing)_*_3))]">
            {repoSelectIsOpen ? (
              <ScrollArea className="max-h-full _flex-1">
                <div className="w-full p-2 border-b flex justify-between items-center gap-2">
                  <div className="relative">
                    <Input
                      className="peer ps-9 pe-9 h-8 border-border"
                      placeholder="Filter..."
                    />
                    <div className="text-muted-foreground/80 pointer-events-none absolute inset-y-0 start-0 flex items-center justify-center ps-3 peer-disabled:opacity-50">
                      <SearchIcon size={16} />
                    </div>
                  </div>
                  <DropdownMenu>
                    <DropdownMenuTrigger render={<Button size={"sm"} />}>
                      Add
                      <ChevronDownIcon
                        className="-me-1 opacity-60"
                        size={16}
                        aria-hidden="true"
                      />
                    </DropdownMenuTrigger>
                    <DropdownMenuContent className="min-w-(--radix-dropdown-menu-trigger-width)">
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

                            const data = await addLocalGitRepo({
                              repoPath: folder,
                            });

                            if (data.error) {
                              toast.error(data.error);
                              return;
                            }
                            if (data.success) {
                              setRepositories([...repositories, data.success]);
                              setSelectedRepository(data.success);
                              toast.success("Repository added successfully!");
                            }
                          }
                        }}
                      >
                        Add local repository
                      </DropdownMenuItem>
                    </DropdownMenuContent>
                  </DropdownMenu>
                </div>
                <div className="">
                  {repositories.map((repo) => (
                    <button
                      className="py-2 px-2 flex w-full justify-between items-center hover:bg-accent/55 cursor-pointer"
                      key={repo.id}
                      type="button"
                      onClick={() => {
                        setSelectedRepository(repo);
                        setRepoSelectIsOpen(false);
                      }}
                    >
                      <span>{repo.name}</span>
                      <Button
                        variant={"ghost"}
                        size={"icon"}
                        onClick={(e) => {
                          e.stopPropagation();
                          setRepositories(
                            repositories.filter((r) => r.id !== repo.id),
                          );
                          toast.success("Repository removed");

                          if (selectedRepository?.id === repo.id) {
                            setSelectedRepository(null);
                          }
                        }}
                      >
                        <Minus size={16} />
                      </Button>
                    </button>
                  ))}
                </div>
              </ScrollArea>
            ) : (
              <>
                <Tabs
                  defaultValue="tab-1"
                  className={"gap-0 h-full flex flex-col"}
                >
                  <TabsList
                    className={"rounded-none w-full border-l border-b shrink-0"}
                  >
                    <TabsTab className={"rounded-none! ml-0"} value="tab-1">
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
                    <div className="flex-1 overflow-auto [scrollbar-width:none] [-ms-overflow-style:none] [&::-webkit-scrollbar]:hidden">
                      {isStatusLoading ? (
                        <>
                          <span>Loading...</span>
                        </>
                      ) : (
                        <>
                          {status &&
                          ((stagedChanges && stagedChanges?.length > 0) ||
                            (unstagedChanges &&
                              unstagedChanges?.length > 0)) ? (
                            <Accordion
                              defaultValue={["Staged Changes", "Changes"]}
                              className="w-full divide-y"
                              multiple={true}
                            >
                              {(
                                [
                                  {
                                    name: "Staged Changes",
                                    data: stagedChanges,
                                  },
                                  {
                                    name: "Changes",
                                    data: unstagedChanges,
                                  },
                                ] as const
                              ).map((cell) => {
                                if (!cell.data || cell.data.length === 0) {
                                  return null;
                                }
                                return (
                                  <AccordionItem
                                    value={cell.name}
                                    key={cell.name}
                                    className="pt-2 pb-2"
                                  >
                                    <AccordionTrigger
                                      className={cn(
                                        "items-center rounded-none px-3 gap-2 py-0 hover:no-underline [&>svg]:mb-1 [&>svg]:-rotate-90 [&[data-panel-open]>svg]:rotate-0 [&>svg]:-order-1",
                                      )}
                                    >
                                      <div className="flex items-center justify-between w-full">
                                        <span className="text-sm font-medium">
                                          {cell.name}
                                        </span>
                                        <div className="flex items-center gap-1 pointer-events-auto">
                                          {cell.name === "Changes" && (
                                            <>
                                              <DiscardChangesDialog fileName="." />

                                              <div
                                                onClick={async (event) => {
                                                  event.stopPropagation();
                                                  await actions.addAll();
                                                }}
                                                className={cn(
                                                  buttonVariants({
                                                    variant: "ghost",
                                                    className: "h-8 w-8",
                                                  }),
                                                )}
                                              >
                                                <Plus
                                                  size={20}
                                                  strokeWidth={1.25}
                                                />
                                              </div>
                                            </>
                                          )}
                                          {cell.name === "Staged Changes" && (
                                            <div
                                              onClick={async (event) => {
                                                event.stopPropagation();
                                                await actions.removeAll();
                                              }}
                                              className={cn(
                                                buttonVariants({
                                                  variant: "ghost",
                                                  className: "h-8 w-8",
                                                }),
                                              )}
                                            >
                                              <Minus
                                                size={20}
                                                strokeWidth={1.25}
                                              />
                                            </div>
                                          )}
                                          <Badge
                                            variant={"secondary"}
                                            className="tabular-nums font-mono"
                                          >
                                            {cell.data?.length}
                                          </Badge>
                                        </div>
                                      </div>
                                    </AccordionTrigger>
                                    <AccordionContent className="text-muted-foreground pb-1">
                                      {cell.data?.map((v) => (
                                        <EachStatus
                                          key={v.path}
                                          file={v}
                                          type={cell.name}
                                          onAdd={
                                            cell.name === "Changes"
                                              ? actions.add
                                              : undefined
                                          }
                                          onUnstage={
                                            cell.name === "Staged Changes"
                                              ? actions.unstage
                                              : undefined
                                          }
                                        />
                                      ))}
                                    </AccordionContent>
                                  </AccordionItem>
                                );
                              })}
                            </Accordion>
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
                    <div className="shrink-0 border-l border-b rounded-bl-md flex flex-col gap-2 justify-between items-center border-t px-2 py-2 bg-accent dark:bg-accent/10">
                      <Input
                        placeholder="Summary (required)"
                        className="h-8 _border-border dark:bg-background!"
                      />
                      <InputGroup className="dark:bg-background!">
                        <InputGroupTextarea
                          placeholder="Description"
                          className="dark:bg-background!"
                        />
                        <InputGroupAddon align="block-end">
                          <Button
                            variant="ghost"
                            size="icon-xs"
                            className="rounded-full opacity-50 hover:opacity-100"
                            aria-label="Add Co Authors"
                          >
                            <UserPlus size={16} />
                          </Button>
                          <Separator
                            orientation="vertical"
                            className={"h-[80%] mx-0"}
                          />
                          <Button
                            variant="ghost"
                            size="icon-xs"
                            className="rounded-full opacity-50 hover:opacity-100"
                            aria-label="Add files"
                          >
                            <Sparkles size={16} />
                          </Button>
                        </InputGroupAddon>
                      </InputGroup>
                      <Button className="w-full">Commit to main</Button>
                    </div>
                  </TabsPanel>
                  <TabsPanel value="tab-2" className={"h-full"} tabIndex={-1}>
                    <ScrollArea
                      classNameRoot="flex-1 h-full"
                      className="flex-1 h-full"
                      hiddenScrollbar
                      tabIndex={-1}
                    >
                      {commitHistory.map((commit) => (
                        <div
                          className="w-full p-2 border-b hover:bg-accent cursor-pointer hover:border-l-border border-l border-l-transparent"
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
                                        zIndex:
                                          commit.authors.co_authors.length + 1,
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
                                  {commit.authors.co_authors.map(
                                    (coAuthor, idx) => (
                                      <Tooltip key={`${idx}-tooltip-coauthor`}>
                                        <TooltipTrigger
                                          style={{
                                            zIndex:
                                              commit.authors.co_authors.length -
                                              idx,
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
                                    ),
                                  )}
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
                                      {timeAgoFromUnixSeconds(
                                        commit.timestamp,
                                      )}{" "}
                                    </TooltipTrigger>
                                    <TooltipPopup side="bottom">
                                      {formatUnixSecondsToDateTime(
                                        commit.timestamp,
                                      )}
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
              </>
            )}
          </div>
        </ResizablePanel>
      }
      <ResizableHandle className="cursor-col-resize!" withHandle />
      {
        <ResizablePanel
          className={cn(
            "w-full relative",
            repoSelectIsOpen && "_blur-sm cursor-pointer",
          )}
          onClick={() => {
            if (repoSelectIsOpen) {
              setRepoSelectIsOpen(false);
            }
          }}
        >
          {repoSelectIsOpen && (
            <div className="absolute inset-0 bg-black/40 z-10 w-full h-full backdrop-blur-sm border border-l-0"></div>
          )}
          <Outlet />
        </ResizablePanel>
      }
    </ResizablePanelGroup>
  );
}

interface EachStatusProps {
  file: FileStatus;
  type: "Staged Changes" | "Changes";
  onAdd?: (filePath: string) => Promise<boolean>;
  onUnstage?: (filePath: string) => Promise<boolean>;
}

export default function EachStatus({
  file,
  type,
  onAdd,
  onUnstage,
}: EachStatusProps) {
  const { setSelectedFilePath, setSelectedFileStatus, selectedFilePath } =
    useDiffViewStore();

  return (
    <contextMenu.ContextMenu>
      <contextMenu.ContextMenuTrigger
        className={cn(
          `dark:[&[data-state=open]>div]:bg-blue-900/50! [&[data-state=open]>div]:bg-blue-50! [&[data-state=open]>div]:border [&[data-state=open]>div]:border-y-blue-400! [&[data-state=open]>div]:border-dashed! [&[data-state=open]>div]:border-l-border!`,
        )}
      >
        <div
          className={cn(
            "flex relative select-none cursor-pointer hover:bg-muted border border-transparent hover:border-1 hover:border-l-1 hover:border-l-border items-center px-2 py-1",
            selectedFilePath?.path === file.path &&
              // "bg-primary/5! hover:bg-primary/10! border-l-border border border-y-primary/30! border-dashed!",
              "bg-muted-foreground/10! hover:bg-muted-foreground/15!",
          )}
          onClick={() => {
            if (
              file.status.includes("IndexRenamed") ||
              file.status.includes("WorktreeRenamed")
            ) {
              setSelectedFilePath({
                path: file.path,
                newPath: file.new_path,
              });
            } else {
              setSelectedFilePath({
                path: file.path,
              });
            }
            setSelectedFileStatus(file.status);
          }}
        >
          {selectedFilePath?.path === file.path ? (
            <div className="absolute top-1/2 -translate-y-1/2 -left-1 rounded-md w-2 bg-primary h-6"></div>
          ) : null}
          <div className="flex items-center w-full min-w-0">
            <div className="shrink-0">{getStatusIcon(file.status)}</div>
            <div className="flex items-center ml-2 min-w-0 flex-1">
              <Label className="flex cursor-pointer items-center min-w-0 w-full gap-0">
                {file?.path.split("/").slice(0, -1).join("/") && (
                  <>
                    <span className="text-muted-foreground truncate">
                      {file.path.split("/").slice(0, -1).join("/")}
                    </span>
                    <span className="text-muted-foreground">/</span>
                  </>
                )}
                <span className="shrink-0 font-medium text-foreground!">
                  {file?.path.split("/").slice(-1)[0]}
                </span>
              </Label>
            </div>
            {type === "Changes" && onAdd && (
              <div className="flex ml-2 shrink-0">
                <DiscardChangesDialog fileName={file.path} />
                <Button
                  onClick={async (e) => {
                    e.stopPropagation();
                    const success = await onAdd(file.path);
                    if (success) {
                      toast.success("File staged");
                    } else {
                      toast.error("Failed to stage file");
                    }
                  }}
                  className="h-8 w-8"
                  variant={"ghost"}
                >
                  <Plus size={20} strokeWidth={1.25} />
                </Button>
              </div>
            )}
            {type === "Staged Changes" && onUnstage && (
              <div className="flex ml-2 flex-shrink-0">
                <Button
                  onClick={async (e) => {
                    e.stopPropagation();
                    const success = await onUnstage(file.path);
                    if (success) {
                      toast.success("File unstaged");
                    } else {
                      toast.error("Failed to unstage file");
                    }
                  }}
                  className="h-8 w-8"
                  variant={"ghost"}
                >
                  <Minus size={20} strokeWidth={1.25} />
                </Button>
              </div>
            )}
          </div>
        </div>
      </contextMenu.ContextMenuTrigger>
      <contextMenu.ContextMenuContent className="w-52">
        <contextMenu.ContextMenuLabel>
          <div className="flex items-center gap-2">
            {getStatusIcon(file.status)}
            {file.path.split("/").slice(-1)[0]}
          </div>
        </contextMenu.ContextMenuLabel>
        <contextMenu.ContextMenuSeparator />
        <contextMenu.ContextMenuItem>
          <Plus size={16} className="mr-2" />
          Stage Changes
        </contextMenu.ContextMenuItem>
        <contextMenu.ContextMenuItem>
          <Diff size={16} className="mr-2" />
          Open Diff
        </contextMenu.ContextMenuItem>
        <contextMenu.ContextMenuSeparator />
        <contextMenu.ContextMenuItem>
          <ClipboardCopy size={16} className="mr-2" />
          Copy Relative Path
        </contextMenu.ContextMenuItem>
        <contextMenu.ContextMenuItem>
          <BetweenVerticalEnd size={16} className="mr-2" />
          Copy Diff Hunk
        </contextMenu.ContextMenuItem>
        <contextMenu.ContextMenuItem>
          <BetweenHorizontalEnd size={16} className="mr-2" />
          Copy Old File Contents
        </contextMenu.ContextMenuItem>
        <contextMenu.ContextMenuItem>
          <BetweenHorizontalStart size={16} className="mr-2" />
          Copy New File Contents
        </contextMenu.ContextMenuItem>
        <contextMenu.ContextMenuSeparator />
        <contextMenu.ContextMenuItem>
          <GitCommitHorizontal size={16} className="mr-2" />
          Quick Commit
        </contextMenu.ContextMenuItem>
        <contextMenu.ContextMenuItem>
          <CopyPlus size={16} className="mr-2" />
          Amend Commit
        </contextMenu.ContextMenuItem>
        <contextMenu.ContextMenuSeparator />
        <contextMenu.ContextMenuItem className="hover:text-destructive! hover:bg-destructive/10!">
          <Undo2 size={16} className="mr-2" />
          Discard Changes
        </contextMenu.ContextMenuItem>
      </contextMenu.ContextMenuContent>
    </contextMenu.ContextMenu>
  );
}

export function getStatusIcon(type: FileStatusKind[]) {
  // Normalize into a Set for fast lookup
  const kinds = new Set(type);

  // PRIORITY: unreadable > deleted > renamed/typechange > modified > new
  if (kinds.has("WorktreeUnreadable")) {
    return (
      // Unreadable
      <AlertTriangle className="text-orange-500" size={20} />
    );
  }

  if (kinds.has("IndexDeleted") || kinds.has("WorktreeDeleted")) {
    // Deleted
    return <SquareX className="text-red-500" size={20} />;
  }

  if (
    kinds.has("IndexRenamed") ||
    kinds.has("WorktreeRenamed") ||
    kinds.has("IndexTypechange") ||
    kinds.has("WorktreeTypechange")
  ) {
    return (
      // Renamed / Type Changed
      <CornerUpRight className="text-purple-500" size={20} />
    );
  }

  if (kinds.has("IndexModified") || kinds.has("WorktreeModified")) {
    // Modified
    return <SquareDot className="text-yellow-500" size={20} />;
  }

  if (kinds.has("IndexNew") || kinds.has("WorktreeNew")) {
    return (
      // New / Added
      <SquarePlus className="text-green-500" size={20} />
    );
  }

  // If nothing matched, show a neutral icon or nothing
  // Unchanged
  return <EyeOff className="text-gray-400" size={20} />;
}

const DiscardChangesDialog = ({ fileName }: { fileName: string }) => {
  const [open, setOpen] = useState(false);
  const [isDeleteLoading, setIsDeleteLoading] = useState(false);

  const actions = useRepositoryActions();

  return (
    <Dialog
      open={open}
      onOpenChange={(open) => {
        setOpen(open);
      }}
    >
      <DialogTrigger
        render={
          <Button
            onClick={(e) => {
              e.stopPropagation();
              setOpen(true);
            }}
            className="h-8 w-8"
            variant={"ghost"}
          />
        }
      >
        <Undo2 size={20} strokeWidth={1.25} />
      </DialogTrigger>
      <DialogContent className="min-w-[600px]">
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
                await actions.discard(fileName);
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
};

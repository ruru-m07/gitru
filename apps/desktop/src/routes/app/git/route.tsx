import { addLocalGitRepo, GetStatusResponse } from "@gitru/commands";
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
  ChevronDown,
  ChevronDownIcon,
  ChevronUp,
  CircleAlertIcon,
  GitBranch,
  ListFilterPlus,
  Loader2,
  Minus,
  SearchIcon,
  ShareIcon,
  Sparkles,
  Undo2,
  UserPlus,
} from "lucide-react";
import { memo, useCallback, useState } from "react";
import { toast } from "sonner";
import z from "zod";

import { useFileSelectionStore } from "@/components/diff/useFileSelectionStore";
import StatusBar from "@/components/statusBar";
import { VirtualizedFileList } from "@/components/VirtualizedFileList";
import {
  useCreateCommit,
  useGetCommitHistory,
  useGetCurrentBranch,
  useGetStatus,
  useGitAdd,
  useGitDiscard,
  useGitUnstage,
} from "@/hooks";
import {
  formatUnixSecondsToDateTime,
  timeAgoFromUnixSeconds,
} from "@/lib/time";
import { useAppStore } from "@/store/useAppStore";

const CoAuthers = z.array(z.tuple([z.string(), z.string()]));
type CoAuthers = z.infer<typeof CoAuthers>;

export const Route = createFileRoute("/app/git")({
  component: GitPageLayout,
});

function GitPageLayout() {
  const [query, setQuery] = useState("");

  const {
    repoSelectIsOpen,
    setRepoSelectIsOpen,
    setRepositories,
    repositories,
    setSelectedRepository,
    selectedRepository,
    setSelectedFileForRepo,
    selectedFileByRepo,
  } = useAppStore();

  const { data: status, isLoading: isStatusLoading } = useGetStatus();

  const { mutateAsync: addFile } = useGitAdd();
  const { mutateAsync: unstageFile } = useGitUnstage();

  const { handleFileClick } = useFileSelectionStore();

  const { data: commitHistory } = useGetCommitHistory();

  const stagedChanges: GetStatusResponse["files"] = (
    status?.files ?? []
  ).filter(
    (file) =>
      file.status.some((s) => s.startsWith("Index")) &&
      file.path.toLowerCase().includes(query.toLowerCase()),
  );
  const unstagedChanges: GetStatusResponse["files"] = (
    status?.files ?? []
  ).filter(
    (file) =>
      file.status.some((s) => s.startsWith("Worktree")) &&
      file.path.toLowerCase().includes(query.toLowerCase()),
  );

  return (
    <div
      className={cn(
        "ml-(--main-actual-content-padding) bg-accent/35 ring-1 ring-inset ring-border h-full w-full rounded-md flex overflow-hidden",
        "flex flex-col",
      )}
    >
      <ResizablePanelGroup
        // className={cn(
        //   "ml-[var(--main-actual-content-padding)] bg-accent/35 ring-1 ring-inset ring-border h-full w-full rounded-md flex overflow-hidden",
        // )}
        direction="horizontal"
        autoSaveId="git-page-layout"
      >
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
          <div className="h-[calc(100vh-calc(var(--spacing)*14)-calc(var(--spacing)*9)-calc(var(--spacing)*7)-calc(var(--spacing)*3))]">
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
                    <DropdownMenuContent>
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
                              const data = await addLocalGitRepo({
                                repoPath: folder,
                              });
                              if (data) {
                                setRepositories([...repositories, data]);
                                setSelectedRepository(data);
                                setRepoSelectIsOpen(false);
                                toast.success("Repository added successfully!");
                              }
                            } catch (error) {
                              toast.error(
                                error instanceof Error
                                  ? error.message
                                  : String(error),
                              );
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
                  <div className="p-1.5 border-b">
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
                              variant={"secondary"}
                              className="rounded-r-md! border-border"
                            />
                          }
                        >
                          <ListFilterPlus
                            aria-hidden="true"
                            className="size-4"
                          />
                        </MenuTrigger>
                        <MenuPopup align="end">
                          <MenuItem>
                            <ShareIcon aria-hidden="true" />
                            Share link
                          </MenuItem>
                        </MenuPopup>
                      </Menu>
                    </Group>
                  </div>
                  <TabsPanel
                    value="tab-1"
                    className={"flex-1 flex flex-col min-h-0"}
                    tabIndex={-1}
                  >
                    <div className="flex-1 overflow-auto ">
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
                            <VirtualizedFileList
                              sections={[
                                {
                                  id: "staged",
                                  name: "Staged Changes",
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
                              setSelectedFilePath={setSelectedFileForRepo}
                              selectedFilePath={
                                selectedFileByRepo[
                                  selectedRepository?.path || ""
                                ]
                                  ? {
                                      path:
                                        selectedFileByRepo[
                                          selectedRepository?.path || ""
                                        ]?.filePath || "",
                                      newPath:
                                        selectedFileByRepo[
                                          selectedRepository?.path || ""
                                        ]?.fileNewPath,
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
                    <WriteCommitBox />
                  </TabsPanel>
                  <TabsPanel value="tab-2" className={"h-full"} tabIndex={-1}>
                    <ScrollArea className="flex-1 h-full" tabIndex={-1}>
                      {commitHistory?.map((commit) => (
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
        <ResizableHandle className="cursor-col-resize!" withHandle />
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
      </ResizablePanelGroup>
      <StatusBar />
    </div>
  );
}

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
      title,
      description,
      co_authors,
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
      <div className="shrink-0 border-l flex flex-col gap-2 justify-between items-center border-t px-2 py-2 bg-accent dark:bg-accent/10">
        <InputGroup>
          <InputGroupInput
            placeholder="Summary (required)"
            className="h-8 _border-border dark:bg-background!"
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
        <InputGroup className="dark:bg-background!">
          <InputGroupTextarea
            placeholder="Description"
            className="dark:bg-background!"
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
        <Group aria-label="Subscription actions" className="w-full">
          <Button
            onClick={handelCommit}
            className="flex-1 truncate"
            disabled={isAdding || isCreatingCommit || title.trim() === ""}
          >
            {isAdding || isCreatingCommit ? (
              <>
                <Loader2 className="size-4 animate-spin" />
                Committing...
              </>
            ) : (
              <>
                {nothingToCommit ? "Add all & Commit to" : "Commit to"}{" "}
                <span className="truncate -ml-1">{currentBranch?.name}</span>
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
  );
});

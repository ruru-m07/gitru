import { GetStatusResponse } from "@gitru/commands";
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
  BadgeQuestionMark,
  ChevronDown,
  ChevronDownIcon,
  ChevronUp,
  CircleAlertIcon,
  GitBranch,
  ListFilterPlus,
  Loader2,
  SearchIcon,
  ShareIcon,
  Sparkles,
  Undo2,
  UserPlus,
} from "lucide-react";
import { memo, useCallback, useState } from "react";
import { useDefaultLayout } from "react-resizable-panels";
import { toast } from "sonner";
import z from "zod";
import { useFileSelectionStore } from "@/components/diff/useFileSelectionStore";
import { RepositoryListItem } from "@/components/RepositoryListItem";
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
import { useRepositories } from "@/hooks/useRepositories";
import { getAvatarByProvider } from "@/lib/getAvatarByGitProvider";
import { parseOrigin } from "@/lib/parseOrigin";
import {
  formatUnixSecondsToDateTime,
  timeAgoFromUnixSeconds,
} from "@/lib/time";
import { useAppStore } from "@/store/useAppStore";
import { GIT_PROVIDERS } from "@/type";

const CoAuthers = z.array(z.tuple([z.string(), z.string()]));
type CoAuthers = z.infer<typeof CoAuthers>;

export const Route = createFileRoute("/app/git")({
  component: GitPageLayout,
});

function GitPageLayout() {
  const [leftWidth, setLeftWidth] = useState(320);

  const { repoSelectIsOpen, setRepoSelectIsOpen } = useAppStore();

  const { defaultLayout, onLayoutChanged } = useDefaultLayout({
    id: "git-page-layout",
    panelIds: ["left", "right"],
    storage: localStorage,
  });

  return (
    <div
      className={cn(
        "ml-(--main-actual-content-padding) bg-accent/35 ring-1 ring-inset ring-border h-full w-full rounded-md flex overflow-hidden",
        "flex flex-col",
      )}
    >
      <div className="flex">
        <ResizablePanelGroup
          defaultLayout={defaultLayout}
          onLayoutChanged={onLayoutChanged}
          orientation="horizontal"
          className="min-w-94"
          id="git-page-layout"
        >
          <ResizablePanel
            defaultSize={320}
            minSize={270}
            maxSize={700}
            id="left"
            className="flex flex-col h-full"
          >
            <ToggelPanelButton />
            <div className="h-[calc(100vh-calc(var(--spacing)*14)-calc(var(--spacing)*9)-calc(var(--spacing)*7)-calc(var(--spacing)*3))]">
              {repoSelectIsOpen ? <ListRepositories /> : <ListFileChanges />}
            </div>
          </ResizablePanel>
          <ResizableHandle withHandle />
          <ResizablePanel
            className={cn("relative", repoSelectIsOpen && "cursor-pointer")}
            onClick={() => setRepoSelectIsOpen(false)}
            id="right"
          >
            {repoSelectIsOpen && (
              <div className="absolute inset-0 bg-white/50 dark:bg-black/40 z-10 w-full h-full backdrop-blur-[2px] border border-l-0"></div>
            )}
            <Outlet />
          </ResizablePanel>
        </ResizablePanelGroup>
      </div>
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

const ToggelPanelButton = () => {
  const { repoSelectIsOpen, setRepoSelectIsOpen, selectedRepository } =
    useAppStore();

  return (
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
        <span>{selectedRepository?.name || "No repository selected"}</span>
      </div>
      {repoSelectIsOpen ? <ChevronUp size={18} /> : <ChevronDown size={18} />}
    </button>
  );
};

const ListFileChanges = () => {
  const [query, setQuery] = useState("");

  const { selectedRepository, setSelectedFileForRepo, selectedFileByRepo } =
    useAppStore();

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
  const conflictedChanges: GetStatusResponse["files"] = (
    status?.files ?? []
  ).filter(
    (file) =>
      file.status.some((s) => s.includes("Conflicted")) &&
      file.path.toLowerCase().includes(query.toLowerCase()),
  );

  return (
    <Tabs defaultValue="tab-1" className={"gap-0 h-full flex flex-col"}>
      <TabsList className={"rounded-none w-full border-l border-b shrink-0"}>
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
              <ListFilterPlus aria-hidden="true" className="size-4" />
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
                (conflictedChanges && conflictedChanges?.length > 0) ||
                (unstagedChanges && unstagedChanges?.length > 0)) ? (
                <VirtualizedFileList
                  sections={[
                    {
                      id: "conflicted",
                      name: "Conflicted",
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
                    selectedFileByRepo[selectedRepository?.path || ""]
                      ? {
                          path:
                            selectedFileByRepo[selectedRepository?.path || ""]
                              ?.filePath || "",
                          newPath:
                            selectedFileByRepo[selectedRepository?.path || ""]
                              ?.fileNewPath,
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

const ListRepositories = () => {
  const { setRepoSelectIsOpen, setSelectedRepository, selectedRepository } =
    useAppStore();
  const { repositories, addRepo, removeRepo } = useRepositories();
  const [query, setQuery] = useState("");

  const normalizedQuery = query.trim().toLowerCase();
  const filteredRepositories = normalizedQuery
    ? repositories.filter((repo) => {
        const name = repo.name?.toLowerCase() ?? "";
        const path = repo.path?.toLowerCase() ?? "";
        return name.includes(normalizedQuery) || path.includes(normalizedQuery);
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
      <ScrollArea className="max-h-full _flex-1">
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
      </ScrollArea>
    </div>
  );
};

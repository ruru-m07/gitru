import {
  Accordion,
  AccordionContent,
  AccordionItem,
  AccordionTrigger,
} from "@gitru/ui/components/accordion";
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
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@gitru/ui/components/dropdown-menu";
import { Input } from "@gitru/ui/components/input";
import { Label } from "@gitru/ui/components/label";
import {
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup,
} from "@gitru/ui/components/resizable";
import { ScrollArea } from "@gitru/ui/components/scroll-area";
import { Textarea } from "@gitru/ui/components/textarea";
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
  GitCommitHorizontal,
  Minus,
  Plus,
  SearchIcon,
  SquareDot,
  SquarePlus,
  SquareX,
  Undo2,
} from "lucide-react";
import { useState } from "react";
import { toast } from "sonner";
import { useDiffViewStore } from "@/components/diff/useDiffViewStore";
import { useGit } from "@/lib/git";
import { useAppStore } from "@/store/useAppStore";
import { addLocalGitRepo, type FileStatus, type FileStatusKind } from "@/tauri";

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

  const { status, operations } = useGit(
    selectedRepository?.path || null,
    selectedRepository?.name,
  );

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
          className="flex flex-col h-full justify-between"
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
          {repoSelectIsOpen ? (
            <ScrollArea type="scroll" className="max-h-full flex-1">
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
                  <DropdownMenuTrigger asChild>
                    <Button size={"sm"}>
                      Add
                      <ChevronDownIcon
                        className="-me-1 opacity-60"
                        size={16}
                        aria-hidden="true"
                      />
                    </Button>
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
                            repo_path: folder,
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
              <div className="grid grid-cols-2 w-full min-h-9 max-h-9">
                <button
                  type="button"
                  className={cn(
                    buttonVariants({ variant: "ghost" }),
                    "h-full rounded-none border-border _border-b border-l border-l-transparent hover:border-l-border",
                  )}
                >
                  Changes
                </button>
                <button
                  type="button"
                  className={cn(
                    buttonVariants({ variant: "ghost" }),
                    "h-full rounded-none border-l border-b rounded-bl-sm",
                  )}
                >
                  History
                </button>
              </div>
              <div className="max-h-full overflow-auto flex-1 [scrollbar-width:none] [-ms-overflow-style:none] [&::-webkit-scrollbar]:hidden">
                {/* <div className="w-full p-2 border-b flex justify-between items-center">
								<Checkbox />
								<span className="text-sm">{status.length} Changed Files</span>
								<div />
							</div> */}
                {/* <div className="max-w-full">
								{status.map((file) => (
									<EachStatus key={file.path} file={file} type="Changes" />
								))}
							</div> */}
                <Accordion
                  type="multiple"
                  defaultValue={["Staged Changes", "Changes"]}
                  className="w-full divide-y"
                >
                  {(
                    [
                      { name: "Staged Changes", data: status.stagedChanges },
                      { name: "Changes", data: status.unstagedChanges },
                    ] as const
                  ).map((cell) => {
                    if (!cell.data || cell.data.length === 0) {
                      return null;
                    }
                    return (
                      <AccordionItem
                        defaultChecked
                        value={cell.name}
                        key={cell.name}
                        className="pt-2 pb-2"
                      >
                        <AccordionTrigger
                          className={cn(
                            "justify-start sticky top-0 rounded-none px-3 gap-2 py-0 hover:no-underline [&>svg]:-order-1",
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
                                      await operations.addAll();
                                    }}
                                    className={cn(
                                      buttonVariants({
                                        variant: "ghost",
                                        className: "h-8 w-8",
                                      }),
                                    )}
                                  >
                                    <Plus size={20} strokeWidth={1.25} />
                                  </div>
                                </>
                              )}
                              {cell.name === "Staged Changes" && (
                                <div
                                  onClick={async (event) => {
                                    event.stopPropagation();
                                    await operations.removeAll();
                                  }}
                                  className={cn(
                                    buttonVariants({
                                      variant: "ghost",
                                      className: "h-8 w-8",
                                    }),
                                  )}
                                >
                                  <Minus size={20} strokeWidth={1.25} />
                                </div>
                              )}
                              <Badge variant={"secondary"}>
                                {cell.data?.length}
                              </Badge>
                            </div>
                          </div>
                        </AccordionTrigger>
                        <AccordionContent className="text-muted-foreground  pb-1">
                          {cell.data?.map((v) => (
                            <EachStatus
                              key={v.path}
                              file={v}
                              type={cell.name}
                              onAdd={
                                cell.name === "Changes"
                                  ? operations.add
                                  : undefined
                              }
                              onUnstage={
                                cell.name === "Staged Changes"
                                  ? operations.unstage
                                  : undefined
                              }
                            />
                          ))}
                        </AccordionContent>
                      </AccordionItem>
                    );
                  })}
                </Accordion>
              </div>
              <div className="flex flex-col gap-2 justify-between items-center border-t px-2 py-2 bg-accent/40">
                <Input
                  placeholder="Summary (required)"
                  className="h-8 border-border"
                />
                <Textarea placeholder="Description" className="border-border" />
                <Button className="w-full" size={"sm"}>
                  Commit to main
                </Button>
              </div>
            </>
          )}
        </ResizablePanel>
      }
      <ResizableHandle withHandle />
      {
        // @ts-ignore
        <ResizablePanel
          className={cn(
            // 'w-[calc(100vw-(var(--sidebar-width)+var(--inbox-width)+(var(--margin)))+3.5rem)]',
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
            selectedFilePath === file.path &&
              "bg-primary/5! hover:bg-primary/10! border-l-border border border-y-primary/30! border-dashed!",
          )}
          onClick={() => {
            setSelectedFilePath(file.path);
            setSelectedFileStatus(file.status);
          }}
        >
          {selectedFilePath === file.path ? (
            <div className="absolute top-1/2 -translate-y-1/2 -left-1 rounded-md w-2 bg-primary h-6"></div>
          ) : null}
          <div className="flex items-center w-full min-w-0">
            <div className="flex-shrink-0">{getStatusIcon(file.status)}</div>
            <div className="flex items-center ml-2 min-w-0 flex-1">
              <Label className="flex cursor-pointer items-center min-w-0 w-full">
                {file?.path.split("/").slice(0, -1).join("/") && (
                  <>
                    <span className="text-muted-foreground truncate">
                      {file.path.split("/").slice(0, -1).join("/")}
                    </span>
                    <span className="text-muted-foreground flex-shrink-0">
                      /
                    </span>
                  </>
                )}
                <span className="flex-shrink-0 font-medium">
                  {file?.path.split("/").slice(-1)[0]}
                </span>
              </Label>
            </div>
            {type === "Changes" && onAdd && (
              <div className="flex ml-2 flex-shrink-0">
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

  const { selectedRepository } = useAppStore();

  const { operations } = useGit(
    selectedRepository?.path || null,
    selectedRepository?.name,
  );

  return (
    <Dialog
      open={open}
      onOpenChange={(open) => {
        setOpen(open);
      }}
    >
      <DialogTrigger asChild>
        <Button
          onClick={(e) => {
            e.stopPropagation();
            setOpen(true);
          }}
          className="h-8 w-8"
          variant={"ghost"}
        >
          <Undo2 size={20} strokeWidth={1.25} />
        </Button>
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

        <DialogFooter>
          <DialogClose asChild>
            <Button
              type="button"
              variant="outline"
              className="flex-1"
              onClick={(e) => {
                e.stopPropagation();
              }}
              disabled={isDeleteLoading}
            >
              Cancel
            </Button>
          </DialogClose>
          <Button
            type="button"
            className="flex-1"
            variant={"destructive"}
            disabled={isDeleteLoading}
            onClick={async (e) => {
              e.stopPropagation();
              setIsDeleteLoading(true);

              try {
                console.log("gooo");
                await operations.discard(fileName);
              } catch (error) {
                toast.error("Unabel to discard changes");
              } finally {
                setIsDeleteLoading(false);
                setOpen(false);
                console.log("gooo11");
              }
            }}
          >
            Discard
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
};

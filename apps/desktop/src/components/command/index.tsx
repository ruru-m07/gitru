"use client";

import type { BranchInfo, UncommittedChangesStrategy } from "@gitru/commands";
import {
  Avatar,
  AvatarFallback,
  AvatarImage,
} from "@gitru/ui/components/avatar";
import { Badge } from "@gitru/ui/components/badge";
import { Button } from "@gitru/ui/components/button";
import {
  Command,
  CommandCollection,
  CommandDialog,
  CommandDialogPopup,
  CommandEmpty,
  CommandFooter,
  CommandGroup,
  CommandGroupLabel,
  CommandInput,
  CommandItem,
  CommandList,
  CommandPanel,
  CommandSeparator,
  CommandShortcut,
} from "@gitru/ui/components/command";
import { Input } from "@gitru/ui/components/input";
import { Kbd, KbdGroup } from "@gitru/ui/components/kbd";
import { Label } from "@gitru/ui/components/label";
import { RadioGroup, RadioGroupItem } from "@gitru/ui/components/radio-group";
import { useNavigate } from "@tanstack/react-router";
import { useVirtualizer } from "@tanstack/react-virtual";
import {
  ArrowDownIcon,
  ArrowLeft,
  ArrowUpIcon,
  Cloud,
  CornerDownLeftIcon,
  GitBranch,
  GitBranchPlus,
  SearchIcon,
} from "lucide-react";
import * as React from "react";
import { useHotkeys } from "react-hotkeys-hook";
import { toast } from "sonner";
import {
  useGetBranches,
  useGetCurrentBranch,
  useGitCreateBranch,
  useGitSwitchBranch,
  useHasUncommittedChanges,
} from "@/hooks";
import { timeAgoFromUnixSeconds } from "@/lib/time";
import { useActions } from "./actions";
import { goto } from "./goto";
import type { CommandView, Group, Item } from "./type";

export default function CommandBox() {
  const [open, setOpen] = React.useState(false);
  const [waitingForSecondKey, setWaitingForSecondKey] = React.useState(false);
  const [view, setView] = React.useState<CommandView>({ type: "root" });
  const [inputValue, setInputValue] = React.useState("");
  const [newBranchName, setNewBranchName] = React.useState("");
  const [changeStrategy, setChangeStrategy] =
    React.useState<UncommittedChangesStrategy>("BringChanges");

  const navigate = useNavigate();
  const actions = useActions();

  const { data: branches } = useGetBranches("Local");
  const { data: remoteBranches } = useGetBranches("Remote");
  const { data: currentBranch } = useGetCurrentBranch();
  const { data: hasChanges } = useHasUncommittedChanges();
  const { mutateAsync: switchBranch, isPending: isSwitching } =
    useGitSwitchBranch();
  const { mutateAsync: createBranch, isPending: isCreating } =
    useGitCreateBranch();

  // Virtualization ref for branch list
  const branchListRef = React.useRef<HTMLDivElement>(null);

  // Combine all branches for virtualization
  const allBranches = React.useMemo(() => {
    const local = branches?.filter((b) => !b.is_remote) ?? [];
    const remote = remoteBranches?.filter((b) => b.is_remote) ?? [];
    return [...local, ...remote];
  }, [branches, remoteBranches]);

  // Filter branches based on input
  const filteredBranches = React.useMemo(() => {
    if (!inputValue) return allBranches;
    const search = inputValue.toLowerCase();
    return allBranches.filter((b) =>
      b.display_name.toLowerCase().includes(search),
    );
  }, [allBranches, inputValue]);

  const virtualizer = useVirtualizer({
    count: filteredBranches.length,
    getScrollElement: () => branchListRef.current,
    estimateSize: () => 64,
    overscan: 10,
  });

  const handleBranchSelect = React.useCallback(
    async (branch: BranchInfo) => {
      // Skip if already on this branch
      if (branch.is_head) {
        toast.info(`Already on branch ${branch.display_name}`);
        setOpen(false);
        setView({ type: "root" });
        return;
      }

      // Check if there are uncommitted changes
      if (hasChanges) {
        setView({
          type: "switch-branch-confirm",
          targetBranch: branch,
          currentBranch: currentBranch?.display_name ?? "current branch",
          hasChanges: true,
        });
        return;
      }

      // No changes, switch directly
      toast.promise(
        switchBranch({ branchName: branch.name, strategy: "BringChanges" }),
        {
          loading: `Switching to ${branch.display_name}...`,
          success: (result) =>
            result.success
              ? (result.message ?? `Switched to ${branch.display_name}`)
              : (result.message ?? "Switch failed"),
          error: (err) => err ?? "Switch failed",
        },
      );
      setOpen(false);
      setView({ type: "root" });
    },
    [hasChanges, currentBranch, switchBranch],
  );

  const handleSwitchWithStrategy = React.useCallback(async () => {
    if (view.type !== "switch-branch-confirm") return;

    toast.promise(
      switchBranch({
        branchName: view.targetBranch.name,
        strategy: changeStrategy,
      }),
      {
        loading: `Switching to ${view.targetBranch.display_name}...`,
        success: (result) =>
          result.success
            ? (result.message ??
              `Switched to ${view.targetBranch.display_name}`)
            : (result.message ?? "Switch failed"),
        error: (err) => err ?? "Switch failed",
      },
    );
    setOpen(false);
    setView({ type: "root" });
    setChangeStrategy("BringChanges");
  }, [view, changeStrategy, switchBranch]);

  const handleCreateBranch = React.useCallback(async () => {
    if (!newBranchName.trim()) {
      toast.error("Please enter a branch name");
      return;
    }

    toast.promise(
      createBranch({
        branchName: newBranchName.trim(),
        strategy: hasChanges ? changeStrategy : "BringChanges",
      }),
      {
        loading: `Creating branch ${newBranchName}...`,
        success: (result) =>
          result.success
            ? (result.message ?? `Created and switched to ${newBranchName}`)
            : (result.message ?? "Create failed"),
        error: (err) => err ?? "Create failed",
      },
    );
    setOpen(false);
    setView({ type: "root" });
    setNewBranchName("");
    setChangeStrategy("BringChanges");
  }, [newBranchName, hasChanges, changeStrategy, createBranch]);

  const handleItemClick = React.useCallback(
    (item: Item) => {
      if (item.onClick) {
        item.onClick();
      }

      if (item.value === "checkout-branch") {
        setView({ type: "checkout-branch" });
        setInputValue("");
        return;
      }

      if (item.value === "new-branch") {
        setView({ type: "create-branch", hasChanges: hasChanges ?? false });
        setInputValue("");
        setNewBranchName("");
        return;
      }

      if (item.redirect) {
        navigate({
          to: item.redirect,
        });
        setOpen(false);
        return;
      }
    },
    [hasChanges, navigate],
  );

  useHotkeys(
    "meta+k, shift+meta+p",
    () => {
      setOpen((open) => !open);
    },
    {
      enabled: true,
      enableOnFormTags: true,
      enableOnContentEditable: true,
    },
  );

  useHotkeys(
    "g",
    () => {
      setWaitingForSecondKey(true);
      setTimeout(() => setWaitingForSecondKey(false), 1000); // 1 second timeout
    },
    { enabled: !waitingForSecondKey },
  );

  useHotkeys(
    "n, p, i, g",
    (e, h) => {
      if (!waitingForSecondKey) return;

      const secondKey = h.keys?.[0]?.toLowerCase();
      const fullShortcut = `g ${secondKey}`;

      const item = goto.find((item) => {
        if (!item.shortcut) return false;
        return (
          Array.isArray(item.shortcut) &&
          item.shortcut.join(" ").toLowerCase() === fullShortcut
        );
      });

      if (item) {
        e.preventDefault();
        handleItemClick(item);
      }
      setWaitingForSecondKey(false);
    },
    { enabled: waitingForSecondKey },
  );

  const groupedItems = React.useMemo<Group[]>(() => {
    switch (view.type) {
      case "root":
        return [
          { value: "Go to", items: goto },
          { value: "Actions", items: actions },
        ];

      case "checkout-branch":
        // Return empty groups - we'll render a virtualized list instead
        return [];

      case "switch-branch-confirm":
        // Return empty groups - we'll render a custom confirmation UI
        return [];

      case "create-branch":
        // Return empty groups - we'll render a custom form
        return [];
    }
  }, [view]);

  // Render content based on view type
  const renderContent = () => {
    if (view.type === "checkout-branch") {
      return (
        <div className="flex flex-col h-full">
          <div className="px-4 py-3 border-b flex items-center gap-2">
            <Button
              size={"icon-sm"}
              variant={"ghost"}
              onClick={() => {
                setView({ type: "root" });
                setInputValue("");
              }}
            >
              <ArrowLeft className="h-4 w-4" />
            </Button>
            <GitBranch className="h-4 w-4" />
            <span className="font-medium">Switch Branch</span>
          </div>
          <div className="px-4 py-2 border-b">
            <Input
              autoFocus
              value={inputValue}
              onChange={(e) => setInputValue(e.target.value)}
              placeholder="Search branches..."
              className="h-9"
            />
          </div>
          {filteredBranches.length === 0 ? (
            <div className="flex flex-col items-center justify-center py-8 text-muted-foreground">
              <GitBranch className="h-8 w-8 mb-2 opacity-50" />
              <p>No branches found</p>
            </div>
          ) : (
            <div
              ref={branchListRef}
              className="flex-1 overflow-auto"
              style={{ maxHeight: "400px" }}
            >
              <div
                style={{
                  height: `${virtualizer.getTotalSize()}px`,
                  width: "100%",
                  position: "relative",
                }}
              >
                {virtualizer.getVirtualItems().map((virtualRow) => {
                  const branch = filteredBranches[virtualRow.index];
                  return (
                    <div
                      key={virtualRow.key}
                      style={{
                        position: "absolute",
                        top: 0,
                        left: 0,
                        width: "100%",
                        height: `${virtualRow.size}px`,
                        transform: `translateY(${virtualRow.start}px)`,
                      }}
                    >
                      <VirtualBranchItem
                        branch={branch}
                        onClick={() => handleBranchSelect(branch)}
                      />
                    </div>
                  );
                })}
              </div>
            </div>
          )}
          <div className="px-4 py-2 border-t flex items-center justify-between text-xs text-muted-foreground">
            <span>{filteredBranches.length} branches</span>
            <Button
              size="sm"
              variant="ghost"
              onClick={() => {
                setView({
                  type: "create-branch",
                  hasChanges: hasChanges ?? false,
                });
                setNewBranchName("");
              }}
            >
              <GitBranchPlus className="h-4 w-4 mr-1" />
              New Branch
            </Button>
          </div>
        </div>
      );
    }

    if (view.type === "switch-branch-confirm") {
      return (
        <div className="flex flex-col p-4 gap-4">
          <div className="flex items-center gap-2">
            <Button
              size={"icon-sm"}
              variant={"ghost"}
              onClick={() => {
                setView({ type: "checkout-branch" });
                setChangeStrategy("BringChanges");
              }}
            >
              <ArrowLeft className="h-4 w-4" />
            </Button>
            <GitBranch className="h-4 w-4" />
            <span className="font-medium">Switch Branch</span>
          </div>

          <div className="text-sm">
            You have uncommitted changes. What would you like to do with them?
          </div>

          <div className="bg-muted/50 rounded-md p-3 text-sm">
            <div className="flex items-center gap-2 text-muted-foreground">
              <span>Switching from</span>
              <Badge variant="outline" className="font-mono">
                {view.currentBranch}
              </Badge>
              <span>to</span>
              <Badge variant="outline" className="font-mono">
                {view.targetBranch.display_name}
              </Badge>
            </div>
          </div>

          <RadioGroup
            value={changeStrategy}
            onValueChange={(v) =>
              setChangeStrategy(v as UncommittedChangesStrategy)
            }
            className="gap-3"
          >
            <div className="flex items-start space-x-3 p-3 rounded-md border hover:bg-muted/50 cursor-pointer">
              <RadioGroupItem value="StashOnCurrentBranch" id="stash" />
              <div className="flex flex-col gap-1">
                <Label htmlFor="stash" className="cursor-pointer font-medium">
                  Leave my changes on {view.currentBranch}
                </Label>
                <span className="text-xs text-muted-foreground">
                  Your changes will be stashed and can be restored later
                </span>
              </div>
            </div>
            <div className="flex items-start space-x-3 p-3 rounded-md border hover:bg-muted/50 cursor-pointer">
              <RadioGroupItem value="BringChanges" id="bring" />
              <div className="flex flex-col gap-1">
                <Label htmlFor="bring" className="cursor-pointer font-medium">
                  Bring my changes to {view.targetBranch.display_name}
                </Label>
                <span className="text-xs text-muted-foreground">
                  Your uncommitted changes will come with you to the new branch
                </span>
              </div>
            </div>
          </RadioGroup>

          <div className="flex justify-end gap-2 pt-2">
            <Button
              variant="outline"
              onClick={() => {
                setView({ type: "checkout-branch" });
                setChangeStrategy("BringChanges");
              }}
            >
              Cancel
            </Button>
            <Button onClick={handleSwitchWithStrategy} disabled={isSwitching}>
              {isSwitching ? "Switching..." : "Switch Branch"}
            </Button>
          </div>
        </div>
      );
    }

    if (view.type === "create-branch") {
      return (
        <div className="flex flex-col p-4 gap-4">
          <div className="flex items-center gap-2">
            <Button
              size={"icon-sm"}
              variant={"ghost"}
              onClick={() => {
                setView({ type: "root" });
                setNewBranchName("");
                setChangeStrategy("BringChanges");
              }}
            >
              <ArrowLeft className="h-4 w-4" />
            </Button>
            <GitBranchPlus className="h-4 w-4" />
            <span className="font-medium">Create New Branch</span>
          </div>

          <div className="space-y-2">
            <Label htmlFor="branch-name">Branch Name</Label>
            <Input
              id="branch-name"
              autoFocus
              value={newBranchName}
              onChange={(e) => setNewBranchName(e.target.value)}
              placeholder="feature/my-new-branch"
              className="font-mono"
              onKeyDown={(e) => {
                if (e.key === "Enter" && newBranchName.trim()) {
                  handleCreateBranch();
                }
              }}
            />
          </div>

          {view.hasChanges && (
            <>
              <div className="text-sm text-muted-foreground">
                You have uncommitted changes. What would you like to do with
                them?
              </div>

              <RadioGroup
                value={changeStrategy}
                onValueChange={(v) =>
                  setChangeStrategy(v as UncommittedChangesStrategy)
                }
                className="gap-3"
              >
                <div className="flex items-start space-x-3 p-3 rounded-md border hover:bg-muted/50 cursor-pointer">
                  <RadioGroupItem
                    value="StashOnCurrentBranch"
                    id="stash-create"
                  />
                  <div className="flex flex-col gap-1">
                    <Label
                      htmlFor="stash-create"
                      className="cursor-pointer font-medium"
                    >
                      Leave my changes on current branch
                    </Label>
                    <span className="text-xs text-muted-foreground">
                      Your changes will be stashed and can be restored later
                    </span>
                  </div>
                </div>
                <div className="flex items-start space-x-3 p-3 rounded-md border hover:bg-muted/50 cursor-pointer">
                  <RadioGroupItem value="BringChanges" id="bring-create" />
                  <div className="flex flex-col gap-1">
                    <Label
                      htmlFor="bring-create"
                      className="cursor-pointer font-medium"
                    >
                      Bring my changes to new branch
                    </Label>
                    <span className="text-xs text-muted-foreground">
                      Your uncommitted changes will come with you to the new
                      branch
                    </span>
                  </div>
                </div>
              </RadioGroup>
            </>
          )}

          <div className="flex justify-end gap-2 pt-2">
            <Button
              variant="outline"
              onClick={() => {
                setView({ type: "root" });
                setNewBranchName("");
                setChangeStrategy("BringChanges");
              }}
            >
              Cancel
            </Button>
            <Button
              onClick={handleCreateBranch}
              disabled={isCreating || !newBranchName.trim()}
            >
              {isCreating ? "Creating..." : "Create Branch"}
            </Button>
          </div>
        </div>
      );
    }

    // Default: root view
    return (
      <Command key={view.type} items={groupedItems}>
        <CommandInput
          autoFocus
          value={inputValue}
          onChange={(e) => setInputValue(e.target.value)}
          placeholder="Search for apps and commands..."
          startAddon={<SearchIcon className="-translate-x-1" />}
        />
        <CommandPanel>
          <CommandEmpty>No results found.</CommandEmpty>
          <CommandList>
            {(group: Group, _index: number) => (
              <React.Fragment key={group.value}>
                <CommandGroup items={group.items}>
                  <CommandGroupLabel>{group.value}</CommandGroupLabel>
                  <CommandCollection>
                    {(item: Item) => (
                      <>
                        {item.customCommandItem ? (
                          item.customCommandItem
                        ) : (
                          <CommandItem
                            key={item.value}
                            onClick={() => handleItemClick(item)}
                            value={item.value}
                          >
                            {item.icon && (
                              <item.icon className="mr-2 h-4 w-4" />
                            )}
                            <span className="flex-1">{item.label}</span>
                            {item.shortcut && (
                              <CommandShortcut>
                                {Array.isArray(item.shortcut) ? (
                                  <KbdGroup>
                                    {item.shortcut.map((sc, i) => (
                                      <Kbd key={i}>{sc}</Kbd>
                                    ))}
                                  </KbdGroup>
                                ) : (
                                  item.shortcut
                                )}
                              </CommandShortcut>
                            )}
                          </CommandItem>
                        )}
                      </>
                    )}
                  </CommandCollection>
                </CommandGroup>
                <CommandSeparator />
              </React.Fragment>
            )}
          </CommandList>
        </CommandPanel>
        <CommandFooter>
          <div className="flex items-center gap-4">
            <div className="flex items-center gap-2">
              <KbdGroup>
                <Kbd>
                  <ArrowUpIcon />
                </Kbd>
                <Kbd>
                  <ArrowDownIcon />
                </Kbd>
              </KbdGroup>
              <span>Navigate</span>
            </div>
            <div className="flex items-center gap-2">
              <Kbd>
                <CornerDownLeftIcon />
              </Kbd>
              <span>Open</span>
            </div>
          </div>
          <div className="flex items-center gap-2">
            <Kbd>Esc</Kbd>
          </div>
        </CommandFooter>
      </Command>
    );
  };

  return (
    <CommandDialog
      onOpenChange={() => {
        setInputValue("");
        setView({ type: "root" });
        setOpen(false);
        setNewBranchName("");
        setChangeStrategy("BringChanges");
      }}
      open={open}
    >
      <CommandDialogPopup className={"max-h-200 max-w-175"}>
        {renderContent()}
      </CommandDialogPopup>
    </CommandDialog>
  );
}

// Virtualized branch item component
const VirtualBranchItem = ({
  branch,
  onClick,
}: {
  branch: BranchInfo;
  onClick: () => void;
}) => {
  return (
    <div
      onClick={onClick}
      className="flex flex-col gap-1 px-4 py-2 hover:bg-muted/50 cursor-pointer border-b"
    >
      <div className="flex items-center justify-between w-full">
        <div className="flex items-center gap-2">
          {branch.is_head && (
            <Badge variant="default" className="text-[10px] px-1 py-0">
              Current
            </Badge>
          )}
          {branch.is_remote ? (
            <Cloud className="h-4 w-4 text-muted-foreground" />
          ) : (
            <GitBranch className="h-4 w-4 text-muted-foreground" />
          )}
          <span className="font-medium">{branch.display_name}</span>
        </div>
        <div className="flex items-center gap-1">
          {branch.ahead ? (
            <Badge variant={"secondary"} className="text-xs">
              <ArrowUpIcon className="inline-block size-3" />
              {branch.ahead}
            </Badge>
          ) : null}
          {branch.behind ? (
            <Badge variant={"secondary"} className="text-xs">
              <ArrowDownIcon className="inline-block size-3" />
              {branch.behind}
            </Badge>
          ) : null}
        </div>
      </div>
      <div className="flex items-center gap-2 text-xs text-muted-foreground">
        <Avatar className="size-4 rounded-sm">
          <AvatarImage
            alt={branch.commit.authors.author.name}
            src={`https://avatars.githubusercontent.com/u/e?email=${branch.commit.authors.author.email}&s=64`}
          />
          <AvatarFallback className="text-[8px]">
            {branch.commit.authors.author.name
              .split(" ")
              .map((n) => n[0])
              .join("")
              .toUpperCase()}
          </AvatarFallback>
        </Avatar>
        <span>{branch.commit.authors.author.name}</span>
        <span>•</span>
        <span>{timeAgoFromUnixSeconds(branch.commit.timestamp)}</span>
        <span className="truncate max-w-48">• {branch.commit.summary}</span>
      </div>
    </div>
  );
};

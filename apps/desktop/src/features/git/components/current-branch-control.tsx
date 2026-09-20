import type {
  BranchInfo,
  RepoOperation,
  UncommittedChangesStrategy,
} from "@gitru/commands";
import {
  AlertDialog,
  AlertDialogClose,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogPopup,
  AlertDialogTitle,
} from "@gitru/ui/components/alert-dialog";
import { Badge } from "@gitru/ui/components/badge";
import { Button } from "@gitru/ui/components/button";
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuSeparator,
  ContextMenuTrigger,
} from "@gitru/ui/components/context-menu";
import {
  Dialog,
  DialogClose,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogPopup,
  DialogTitle,
} from "@gitru/ui/components/dialog";
import {
  Field,
  FieldDescription,
  FieldLabel,
} from "@gitru/ui/components/field";
import { Group, GroupSeparator } from "@gitru/ui/components/group";
import { Input } from "@gitru/ui/components/input";
import {
  InputGroup,
  InputGroupAddon,
  InputGroupInput,
} from "@gitru/ui/components/input-group";
import {
  Popover,
  PopoverDescription,
  PopoverPopup,
  PopoverTitle,
  PopoverTrigger,
} from "@gitru/ui/components/popover";
import { ScrollArea } from "@gitru/ui/components/scroll-area";
import { Tabs, TabsList, TabsPanel, TabsTab } from "@gitru/ui/components/tabs";
import {
  Tooltip,
  TooltipPopup,
  TooltipTrigger,
} from "@gitru/ui/components/tooltip";
import { cn } from "@gitru/ui/lib/utils";
import { defaultRangeExtractor, useVirtualizer } from "@tanstack/react-virtual";
import {
  Check,
  ChevronDown,
  Cloud,
  GitBranch,
  GitBranchPlus,
  GitCommitVertical,
  HardDrive,
  LaptopMinimal,
  Link2,
  Loader2,
  Pencil,
  Plus,
  Search,
  SearchIcon,
  ShieldCheck,
  Trash2,
  TriangleAlert,
  Unlink,
} from "lucide-react";
import {
  type CSSProperties,
  type FormEvent,
  type KeyboardEvent,
  useMemo,
  useRef,
  useState,
} from "react";
import { toast } from "sonner";
import { branchDeleteBlockReason } from "@/features/git/lib/branch-action-availability";
import {
  useGetBranches,
  useGitCreateBranch,
  useGitDeleteBranch,
  useGitRenameBranch,
  useGitSetBranchUpstream,
  useGitSwitchBranch,
  useGitUnsetBranchUpstream,
  useHasUncommittedChanges,
} from "@/hooks";
import {
  compactTimeAgoFromUnixSeconds,
  formatUnixSecondsToDateTime,
} from "@/lib/time";

type BranchTab = "local" | "remote";
type RepoOperationKind = RepoOperation["kind"];
const EMPTY_BRANCHES: BranchInfo[] = [];
const BRANCH_ROW_HEIGHT = 36;
const BRANCH_SECTION_HEIGHT = 28;
const BRANCH_VIRTUALIZATION_THRESHOLD = 50;

type BranchSectionLabel =
  | "Default Branch"
  | "Other Branches"
  | "Remote Branches";

type BranchListItem =
  | {
      type: "heading";
      key: string;
      label: BranchSectionLabel;
    }
  | {
      type: "branch";
      key: string;
      branch: BranchInfo;
      branchIndex: number;
    };

type BranchDialog =
  | { kind: "create" }
  | { kind: "checkout"; branch: BranchInfo }
  | { kind: "rename"; branch: BranchInfo }
  | { kind: "upstream"; branch: BranchInfo }
  | { kind: "delete"; branch: BranchInfo }
  | null;

export interface CurrentBranchPickerProps {
  currentBranchName?: string;
  currentBranchDisplayName?: string;
  detached: boolean;
  rebasing: boolean;
  rebaseBranch?: string;
  operationKind?: RepoOperationKind;
  localBranches?: BranchInfo[];
  remoteBranches?: BranchInfo[];
  branchesLoading?: boolean;
  hasUncommittedChanges?: boolean;
  worktreeStateLoading?: boolean;
  isMutating?: boolean;
  onSwitchBranch: (
    branchName: string,
    strategy?: UncommittedChangesStrategy,
  ) => Promise<boolean>;
  onCreateBranch: (
    branchName: string,
    strategy?: UncommittedChangesStrategy,
  ) => Promise<boolean>;
  onRenameBranch: (branch: string, newName: string) => Promise<boolean>;
  onDeleteBranch: (
    branch: string,
    isRemote: boolean,
    force: boolean,
  ) => Promise<boolean>;
  onSetUpstream: (branch: string, upstream: string) => Promise<boolean>;
  onUnsetUpstream: (branch: string) => Promise<boolean>;
}

export interface CurrentBranchControlProps {
  currentBranchName?: string;
  currentBranchDisplayName?: string;
  detached: boolean;
  rebasing: boolean;
  rebaseBranch?: string;
  operationKind?: RepoOperationKind;
}

function errorMessage(error: unknown, fallback: string) {
  if (error instanceof Error && error.message) return error.message;
  if (typeof error === "string" && error) return error;
  return fallback;
}

async function runAction(
  action: () => Promise<unknown>,
  successMessage: string,
  failureMessage: string,
  errorHandledByHook = false,
) {
  try {
    const result = await action();
    toast.success(
      typeof result === "string" && result ? result : successMessage,
    );
    return true;
  } catch (error) {
    if (!errorHandledByHook) {
      toast.error(errorMessage(error, failureMessage));
    }
    return false;
  }
}

export function CurrentBranchControl(props: CurrentBranchControlProps) {
  const localBranches = useGetBranches("Local");
  const remoteBranches = useGetBranches("Remote");
  const dirtyWorktree = useHasUncommittedChanges();
  const switchBranch = useGitSwitchBranch();
  const createBranch = useGitCreateBranch();
  const renameBranch = useGitRenameBranch();
  const deleteBranch = useGitDeleteBranch();
  const setUpstream = useGitSetBranchUpstream();
  const unsetUpstream = useGitUnsetBranchUpstream();

  const isMutating =
    switchBranch.isPending ||
    createBranch.isPending ||
    renameBranch.isPending ||
    deleteBranch.isPending ||
    setUpstream.isPending ||
    unsetUpstream.isPending;

  return (
    <CurrentBranchPicker
      {...props}
      localBranches={localBranches.data ?? undefined}
      remoteBranches={remoteBranches.data ?? undefined}
      branchesLoading={localBranches.isLoading || remoteBranches.isLoading}
      hasUncommittedChanges={dirtyWorktree.data ?? undefined}
      worktreeStateLoading={dirtyWorktree.isLoading}
      isMutating={isMutating}
      onSwitchBranch={(branchName, strategy) =>
        runAction(
          () => switchBranch.mutateAsync({ branchName, strategy }),
          `Checked out ${branchName}`,
          "Unable to check out branch",
        )
      }
      onCreateBranch={(branchName, strategy) =>
        runAction(
          () => createBranch.mutateAsync({ branchName, strategy }),
          `Created and checked out ${branchName}`,
          "Unable to create branch",
        )
      }
      onRenameBranch={(branch, newName) =>
        runAction(
          () => renameBranch.mutateAsync({ branch, newName }),
          `Renamed ${branch} to ${newName}`,
          "Unable to rename branch",
          true,
        )
      }
      onDeleteBranch={(branch, isRemote, force) =>
        runAction(
          () => deleteBranch.mutateAsync({ branch, isRemote, force }),
          `Deleted ${branch}`,
          "Unable to delete branch",
          true,
        )
      }
      onSetUpstream={(branch, upstream) =>
        runAction(
          () => setUpstream.mutateAsync({ branch, upstream }),
          `${branch} now tracks ${upstream}`,
          "Unable to set upstream",
          true,
        )
      }
      onUnsetUpstream={(branch) =>
        runAction(
          () => unsetUpstream.mutateAsync({ branch }),
          `Stopped tracking an upstream from ${branch}`,
          "Unable to unset upstream",
          true,
        )
      }
    />
  );
}

export function CurrentBranchPicker({
  currentBranchName,
  currentBranchDisplayName,
  detached,
  rebasing,
  rebaseBranch,
  operationKind,
  localBranches = EMPTY_BRANCHES,
  remoteBranches = EMPTY_BRANCHES,
  branchesLoading = false,
  hasUncommittedChanges,
  worktreeStateLoading = false,
  isMutating = false,
  onSwitchBranch,
  onCreateBranch,
  onRenameBranch,
  onDeleteBranch,
  onSetUpstream,
  onUnsetUpstream,
}: CurrentBranchPickerProps) {
  const [open, setOpen] = useState(false);
  const [tab, setTab] = useState<BranchTab>("local");
  const [query, setQuery] = useState("");
  const [dialog, setDialog] = useState<BranchDialog>(null);
  const [activeBranchIndex, setActiveBranchIndex] = useState(0);
  const [scrollElement, setScrollElement] = useState<HTMLDivElement | null>(
    null,
  );
  const rowRefs = useRef<Array<HTMLButtonElement | null>>([]);
  const pendingFocusIndex = useRef<number | null>(null);

  const operationLocked = Boolean(operationKind && operationKind !== "clean");
  const branchChangesLocked = operationLocked || worktreeStateLoading;
  const currentInfo = localBranches.find(
    (branch) => branch.name === currentBranchName || branch.is_head,
  );
  const sortedBranches = useMemo(() => {
    const source = tab === "local" ? localBranches : remoteBranches;

    return [...source].sort((a, b) => {
      if (tab === "local") {
        if (a.is_protected !== b.is_protected) {
          return a.is_protected ? -1 : 1;
        }
        const aCurrent = a.name === currentBranchName || a.is_head;
        const bCurrent = b.name === currentBranchName || b.is_head;
        if (aCurrent !== bCurrent) return aCurrent ? -1 : 1;
      }
      return a.display_name.localeCompare(b.display_name);
    });
  }, [currentBranchName, localBranches, remoteBranches, tab]);
  const visibleBranches = useMemo(() => {
    const search = query.trim().toLocaleLowerCase();
    if (!search) return sortedBranches;

    return sortedBranches.filter((branch) =>
      `${branch.name} ${branch.display_name}`
        .toLocaleLowerCase()
        .includes(search),
    );
  }, [query, sortedBranches]);
  const branchListItems = useMemo<BranchListItem[]>(() => {
    const items: BranchListItem[] = [];
    let previousLabel: BranchSectionLabel | null = null;

    visibleBranches.forEach((branch, branchIndex) => {
      const label =
        tab === "remote"
          ? "Remote Branches"
          : branch.is_protected
            ? "Default Branch"
            : "Other Branches";

      if (label !== previousLabel) {
        items.push({
          type: "heading",
          key: `heading:${tab}:${label}`,
          label,
        });
        previousLabel = label;
      }

      items.push({
        type: "branch",
        key: `${branch.is_remote ? "remote" : "local"}:${branch.name}`,
        branch,
        branchIndex,
      });
    });

    return items;
  }, [tab, visibleBranches]);
  const branchItemIndexByBranchIndex = useMemo(() => {
    const indexes: number[] = [];
    branchListItems.forEach((item, itemIndex) => {
      if (item.type === "branch") indexes[item.branchIndex] = itemIndex;
    });
    return indexes;
  }, [branchListItems]);
  const shouldVirtualize =
    branchListItems.length > BRANCH_VIRTUALIZATION_THRESHOLD;
  const getBranchScrollElement = () => scrollElement;
  const branchVirtualizer = useVirtualizer({
    count: branchListItems.length,
    enabled: open && shouldVirtualize && Boolean(scrollElement),
    estimateSize: (index) =>
      branchListItems[index]?.type === "heading"
        ? BRANCH_SECTION_HEIGHT
        : BRANCH_ROW_HEIGHT,
    getItemKey: (index) => branchListItems[index]?.key ?? index,
    getScrollElement: getBranchScrollElement,
    overscan: 12,
    paddingEnd: 4,
    paddingStart: 4,
    rangeExtractor: (range) => {
      const indexes = defaultRangeExtractor(range);
      const activeItemIndex = branchItemIndexByBranchIndex[activeBranchIndex];
      if (activeItemIndex === undefined || indexes.includes(activeItemIndex)) {
        return indexes;
      }
      return [...indexes, activeItemIndex].sort((a, b) => a - b);
    },
  });

  const resetBranchScroll = () => {
    setActiveBranchIndex(0);
    pendingFocusIndex.current = null;
    rowRefs.current = [];
    const scrollElement = getBranchScrollElement();
    if (scrollElement) scrollElement.scrollTop = 0;
  };

  const focusBranchAtIndex = (branchIndex: number) => {
    if (branchIndex < 0 || branchIndex >= visibleBranches.length) return;

    setActiveBranchIndex(branchIndex);
    pendingFocusIndex.current = branchIndex;
    if (shouldVirtualize) {
      const itemIndex = branchItemIndexByBranchIndex[branchIndex];
      if (itemIndex !== undefined) {
        branchVirtualizer.scrollToIndex(itemIndex, { align: "auto" });
      }
    }

    const row = rowRefs.current[branchIndex];
    if (row) {
      row.focus();
      pendingFocusIndex.current = null;
    }
  };

  const closePopover = () => {
    resetBranchScroll();
    setOpen(false);
    setQuery("");
  };

  const openDialog = (nextDialog: Exclude<BranchDialog, null>) => {
    closePopover();
    setDialog(nextDialog);
  };

  const closeDialog = () => {
    setDialog(null);
    requestAnimationFrame(() => {
      document
        .querySelector<HTMLButtonElement>("[data-current-branch-trigger]")
        ?.focus();
    });
  };

  const checkout = async (branch: BranchInfo) => {
    if (branch.name === currentBranchName || branch.is_head) {
      closePopover();
      return;
    }
    if (branchChangesLocked || isMutating) {
      return;
    }
    if (hasUncommittedChanges) {
      openDialog({ kind: "checkout", branch });
      return;
    }
    const succeeded = await onSwitchBranch(branch.name);
    if (succeeded) closePopover();
  };

  const focusAdjacentRow = (
    event: KeyboardEvent<HTMLButtonElement>,
    index: number,
  ) => {
    if (
      event.key !== "ArrowDown" &&
      event.key !== "ArrowUp" &&
      event.key !== "Home" &&
      event.key !== "End"
    ) {
      return;
    }
    event.preventDefault();
    const nextIndex =
      event.key === "Home"
        ? 0
        : event.key === "End"
          ? visibleBranches.length - 1
          : (index +
              (event.key === "ArrowDown" ? 1 : -1) +
              visibleBranches.length) %
            visibleBranches.length;
    focusBranchAtIndex(nextIndex);
  };

  const displayLabel = rebasing
    ? "Rebasing"
    : detached
      ? "Detached HEAD"
      : "Current Branch";
  const displayName = rebasing
    ? (rebaseBranch ?? currentBranchDisplayName)
    : currentBranchDisplayName;

  const renderBranchListItem = (
    item: BranchListItem,
    style?: CSSProperties,
    virtualIndex?: number,
  ) => {
    if (item.type === "heading") {
      return (
        <li
          key={item.key}
          aria-level={2}
          className="flex h-7 items-end px-3 pb-1 text-xs font-medium text-muted-foreground"
          data-index={virtualIndex}
          role="heading"
          style={style}
        >
          {item.label}
        </li>
      );
    }

    const { branch, branchIndex } = item;
    return (
      <BranchRow
        key={item.key}
        branch={branch}
        branchIndex={branchIndex}
        branchCount={visibleBranches.length}
        currentBranchName={currentBranchName}
        currentUpstream={currentInfo?.upstream}
        disabled={branchChangesLocked || isMutating}
        tabIndex={branchIndex === activeBranchIndex ? 0 : -1}
        ref={(node) => {
          rowRefs.current[branchIndex] = node;
          if (node && pendingFocusIndex.current === branchIndex) {
            node.focus();
            pendingFocusIndex.current = null;
          }
        }}
        style={style}
        virtualIndex={virtualIndex}
        onCheckout={() => void checkout(branch)}
        onFocus={() => setActiveBranchIndex(branchIndex)}
        onKeyDown={(event) => focusAdjacentRow(event, branchIndex)}
        onOpenDialog={openDialog}
        onUnsetUpstream={async () => {
          const succeeded = await onUnsetUpstream(branch.name);
          if (succeeded) closePopover();
        }}
      />
    );
  };

  const branchListContent = (
    <ScrollArea
      data-current-branch-scroll
      className="h-full w-full [&_[data-slot=scroll-area-content]]:w-full [&_[data-slot=scroll-area-content]]:min-w-0!"
      scrollFade
      viewportRef={setScrollElement}
    >
      {branchesLoading ? (
        <div
          className="flex h-full min-h-32 items-center justify-center gap-2 text-sm text-muted-foreground"
          role="status"
        >
          <Loader2 className="size-4 animate-spin" />
          Loading branches…
        </div>
      ) : visibleBranches.length === 0 ? (
        <div className="flex h-full min-h-32 flex-col items-center justify-center gap-1 px-6 text-center">
          <GitBranch className="size-5 text-muted-foreground" />
          <p className="text-sm font-medium">No branches found</p>
          <p className="text-xs text-muted-foreground">
            Try another name or fetch the latest remote branches.
          </p>
        </div>
      ) : (
        <ul
          aria-label={`${tab === "local" ? "Local" : "Remote"} branches`}
          className={cn(!shouldVirtualize && "py-1")}
          data-branch-list
          data-virtualized={shouldVirtualize ? "true" : "false"}
          style={
            shouldVirtualize
              ? {
                  height: `${branchVirtualizer.getTotalSize()}px`,
                  position: "relative",
                  width: "100%",
                }
              : undefined
          }
        >
          {shouldVirtualize
            ? branchVirtualizer.getVirtualItems().map((virtualRow) => {
                const item = branchListItems[virtualRow.index];
                if (!item) return null;

                return renderBranchListItem(
                  item,
                  {
                    height: `${virtualRow.size}px`,
                    left: 0,
                    position: "absolute",
                    top: 0,
                    transform: `translateY(${virtualRow.start}px)`,
                    width: "100%",
                  },
                  virtualRow.index,
                );
              })
            : branchListItems.map((item) => renderBranchListItem(item))}
        </ul>
      )}
    </ScrollArea>
  );

  return (
    <>
      <Popover
        open={open}
        onOpenChange={(nextOpen) => {
          if (!nextOpen) resetBranchScroll();
          setOpen(nextOpen);
          if (!nextOpen) setQuery("");
        }}
      >
        <PopoverTrigger
          render={
            <Button
              data-current-branch-trigger
              aria-label={`Current branch: ${displayName ?? "unknown"}. Open branches.`}
              className="flex min-h-full w-full max-w-72 items-center justify-between rounded-none border-x-0 data-[popup-open]:bg-accent"
              variant="ghost"
            />
          }
        >
          <div className="flex min-w-0 flex-1 items-center gap-4">
            {detached ? (
              <GitCommitVertical className="size-7.5" strokeWidth={1.5} />
            ) : (
              <GitBranch className="size-7.5" strokeWidth={1.5} />
            )}
            <div className="flex min-w-0 flex-1 flex-col items-start">
              <span className="text-xs font-[450] text-muted-foreground">
                {displayLabel}
              </span>
              <span className="block w-full truncate text-left">
                {displayName ?? "Loading…"}
              </span>
            </div>
          </div>
          <ChevronDown
            aria-hidden="true"
            className="transition-transform duration-100 in-data-[popup-open]:rotate-180"
            size={18}
          />
        </PopoverTrigger>

        <PopoverPopup
          align="start"
          collisionAvoidance={{
            side: "none",
            align: "shift",
            fallbackAxisSide: "none",
          }}
          collisionPadding={0}
          data-current-branch-panel
          side="bottom"
          sideOffset={0}
          style={{
            height:
              "calc(var(--available-height) - var(--main-actual-content-padding) - var(--main-status-bar-height))",
          }}
          viewport={false}
          className="w-[365px] max-w-[calc(100vw-var(--main-actual-content-padding))] rounded-none! border-y-0 border-l-0 bg-background shadow-none! transition-none before:hidden data-starting-style:scale-100 data-starting-style:opacity-100"
        >
          <PopoverTitle className="sr-only">Branches</PopoverTitle>
          <PopoverDescription className="sr-only">
            Search, switch, create, and manage repository branches.
          </PopoverDescription>
          <div className="flex h-full min-h-0 w-full flex-col bg-background">
            <Tabs
              value={tab}
              onValueChange={(value) => {
                resetBranchScroll();
                setTab(value as BranchTab);
              }}
              className={"gap-0 h-full flex flex-col"}
            >
              <TabsList
                className={
                  "select-none rounded-none bg-background w-full shrink-0 border-b *:data-[slot=tab-indicator]:bg-secondary *:data-[slot=tab-indicator]:transition-none"
                }
              >
                <TabsTab className={"rounded-none!"} value="local">
                  <LaptopMinimal className="mr-1!" />
                  Local
                  <Badge variant={"secondary"}>{localBranches.length}</Badge>
                </TabsTab>
                <TabsTab className={"rounded-none!"} value="remote">
                  <HardDrive className="mr-1!" />
                  Remote
                  <Badge variant={"secondary"}>{remoteBranches.length}</Badge>
                </TabsTab>
              </TabsList>

              <div className="p-1.5 max-h-10 min-h-10 border-b">
                <Group className="w-full">
                  <InputGroup>
                    <InputGroupInput
                      aria-label="Filter branches"
                      autoFocus
                      type="search"
                      size="sm"
                      placeholder="Filter branches…"
                      spellCheck={false}
                      value={query}
                      onChange={(event) => {
                        resetBranchScroll();
                        setQuery(event.target.value);
                      }}
                      onKeyDown={(event) => {
                        if (event.key === "ArrowDown") {
                          event.preventDefault();
                          focusBranchAtIndex(0);
                        }
                      }}
                    />
                    <InputGroupAddon>
                      <SearchIcon
                        className="opacity-50 -translate-x-0.5"
                        aria-hidden="true"
                      />
                    </InputGroupAddon>
                  </InputGroup>

                  <GroupSeparator />

                  <Button
                    type="button"
                    size="icon-sm"
                    variant="outline"
                    disabled={branchChangesLocked || isMutating}
                    onClick={() => openDialog({ kind: "create" })}
                  >
                    <GitBranchPlus />
                  </Button>
                </Group>
              </div>

              {operationLocked ? (
                <div
                  className="flex flex-none items-start gap-2 border-b bg-warning/8 px-3 py-2 text-xs text-warning-foreground"
                  role="status"
                >
                  <TriangleAlert className="mt-0.5 size-3.5 shrink-0" />
                  Finish the active Git operation before changing branches.
                </div>
              ) : worktreeStateLoading ? (
                <div
                  className="flex flex-none items-center gap-2 border-b px-3 py-2 text-xs text-muted-foreground"
                  role="status"
                >
                  <Loader2 className="size-3.5 animate-spin" />
                  Checking the working tree before branch changes…
                </div>
              ) : null}

              <TabsPanel
                className="min-h-0 flex-1 overflow-hidden"
                value="local"
              >
                {tab === "local" ? branchListContent : null}
              </TabsPanel>
              <TabsPanel
                className="min-h-0 flex-1 overflow-hidden"
                value="remote"
              >
                {tab === "remote" ? branchListContent : null}
              </TabsPanel>
            </Tabs>
          </div>
        </PopoverPopup>
      </Popover>

      {dialog?.kind === "create" ? (
        <CreateBranchDialog
          currentBranchName={currentBranchName}
          dirty={Boolean(hasUncommittedChanges)}
          pending={isMutating}
          onClose={closeDialog}
          onCreate={onCreateBranch}
        />
      ) : null}
      {dialog?.kind === "checkout" ? (
        <DirtyCheckoutDialog
          branch={dialog.branch}
          pending={isMutating}
          onClose={closeDialog}
          onCheckout={onSwitchBranch}
        />
      ) : null}
      {dialog?.kind === "rename" ? (
        <RenameBranchDialog
          branch={dialog.branch}
          pending={isMutating}
          onClose={closeDialog}
          onRename={onRenameBranch}
        />
      ) : null}
      {dialog?.kind === "upstream" ? (
        <UpstreamDialog
          branch={dialog.branch}
          remoteBranches={remoteBranches}
          pending={isMutating}
          onClose={closeDialog}
          onSetUpstream={onSetUpstream}
        />
      ) : null}
      {dialog?.kind === "delete" ? (
        <DeleteBranchDialog
          branch={dialog.branch}
          pending={isMutating}
          onClose={closeDialog}
          onDelete={onDeleteBranch}
        />
      ) : null}
    </>
  );
}

interface BranchRowProps {
  branch: BranchInfo;
  branchIndex: number;
  branchCount: number;
  currentBranchName?: string;
  currentUpstream?: string;
  disabled: boolean;
  ref: (node: HTMLButtonElement | null) => void;
  style?: CSSProperties;
  tabIndex: number;
  virtualIndex?: number;
  onCheckout: () => void;
  onFocus: () => void;
  onKeyDown: (event: KeyboardEvent<HTMLButtonElement>) => void;
  onOpenDialog: (dialog: Exclude<BranchDialog, null>) => void;
  onUnsetUpstream: () => Promise<void>;
}

function BranchRow({
  branch,
  branchIndex,
  branchCount,
  currentBranchName,
  currentUpstream,
  disabled,
  ref,
  style,
  tabIndex,
  virtualIndex,
  onCheckout,
  onFocus,
  onKeyDown,
  onOpenDialog,
  onUnsetUpstream,
}: BranchRowProps) {
  const isCurrent = branch.name === currentBranchName || branch.is_head;
  const deleteBlockReason = branchDeleteBlockReason(
    branch,
    currentBranchName,
    currentUpstream,
  );
  const accessibilityDetails = [
    branch.is_protected ? "default or protected" : null,
    branch.upstream ? `tracks ${branch.upstream}` : null,
    branch.ahead ? `${branch.ahead} ahead` : null,
    branch.behind ? `${branch.behind} behind` : null,
  ]
    .filter(Boolean)
    .join(", ");
  const rowAction = branch.is_remote ? "Checkout and track" : "Checkout";
  const rowLabel = isCurrent
    ? `Current branch ${branch.display_name}${accessibilityDetails ? `, ${accessibilityDetails}` : ""}`
    : `${rowAction} ${branch.display_name}${accessibilityDetails ? `, ${accessibilityDetails}` : ""}`;
  const compactCommitTime = compactTimeAgoFromUnixSeconds(
    branch.commit.timestamp,
  );
  const exactCommitTime = formatUnixSecondsToDateTime(branch.commit.timestamp);
  const commitDateTime = new Date(branch.commit.timestamp * 1000).toISOString();

  return (
    <li
      aria-posinset={branchIndex + 1}
      aria-setsize={branchCount}
      className={cn(
        "flex h-9 items-center px-1 hover:bg-accent/64 focus-within:bg-accent/64",
        isCurrent && "bg-accent",
      )}
      data-branch-row
      data-index={virtualIndex}
      style={style}
    >
      <ContextMenu>
        <ContextMenuTrigger asChild>
          <button
            ref={ref}
            type="button"
            aria-current={isCurrent ? "true" : undefined}
            aria-description={`Last commit ${exactCommitTime}. Right-click for branch actions.`}
            aria-disabled={disabled || undefined}
            aria-label={rowLabel}
            className="flex min-w-0 flex-1 items-center gap-2 self-stretch rounded-sm px-2 text-left outline-none aria-disabled:cursor-default aria-disabled:opacity-64 focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-ring"
            onClick={onCheckout}
            onFocus={onFocus}
            onKeyDown={onKeyDown}
            tabIndex={tabIndex}
          >
            <span className="flex size-4 shrink-0 items-center justify-center text-muted-foreground">
              {isCurrent ? (
                <Check className="size-4 text-primary" strokeWidth={2.25} />
              ) : (
                <GitBranch className="size-3.5" />
              )}
            </span>
            <span className="min-w-0 flex-1 truncate text-[13px] font-[450]">
              {branch.display_name}
            </span>
            {branch.is_protected ? (
              <ShieldCheck
                aria-label="Default or protected branch"
                className="size-3.5 shrink-0 text-muted-foreground"
              />
            ) : null}
            <Tooltip>
              <TooltipTrigger
                render={
                  <time
                    className="shrink-0 text-xs tabular-nums text-muted-foreground"
                    data-branch-commit-time
                    dateTime={commitDateTime}
                  />
                }
              >
                {compactCommitTime}
              </TooltipTrigger>
              <TooltipPopup align="end" side="bottom">
                {exactCommitTime}
              </TooltipPopup>
            </Tooltip>
          </button>
        </ContextMenuTrigger>
        <ContextMenuContent
          className="w-64 animate-none! opacity-100!"
          data-branch-context-menu
          onEscapeKeyDown={(event) => event.stopPropagation()}
        >
          {!isCurrent ? (
            <ContextMenuItem
              className="gap-2"
              disabled={disabled}
              onSelect={onCheckout}
            >
              {branch.is_remote ? (
                <Cloud className="size-4 shrink-0" />
              ) : (
                <GitBranch className="size-4 shrink-0" />
              )}
              {branch.is_remote ? "Checkout and track" : "Checkout branch"}
            </ContextMenuItem>
          ) : null}
          {!branch.is_remote ? (
            <>
              <ContextMenuItem
                className="gap-2"
                disabled={disabled}
                onSelect={() => onOpenDialog({ kind: "rename", branch })}
              >
                <Pencil className="size-4 shrink-0" />
                Rename branch
              </ContextMenuItem>
              <ContextMenuItem
                className="gap-2"
                disabled={disabled}
                onSelect={() => onOpenDialog({ kind: "upstream", branch })}
              >
                <Link2 className="size-4 shrink-0" />
                {branch.upstream ? "Change upstream" : "Set upstream"}
              </ContextMenuItem>
              {branch.upstream ? (
                <ContextMenuItem
                  className="gap-2"
                  disabled={disabled}
                  onSelect={() => void onUnsetUpstream()}
                >
                  <Unlink className="size-4 shrink-0" />
                  Unset upstream
                </ContextMenuItem>
              ) : null}
            </>
          ) : null}
          <ContextMenuSeparator />
          <ContextMenuItem
            disabled={disabled}
            aria-disabled={Boolean(deleteBlockReason)}
            className={cn(
              "gap-2 text-destructive-foreground focus:text-destructive-foreground",
              deleteBlockReason && "cursor-not-allowed items-start opacity-64",
            )}
            onSelect={(event) => {
              if (deleteBlockReason) {
                event.preventDefault();
              } else {
                onOpenDialog({ kind: "delete", branch });
              }
            }}
          >
            <Trash2 className="mt-0.5 size-4 shrink-0" />
            <span className="flex min-w-0 flex-col">
              <span>
                {branch.is_remote ? "Delete remote branch" : "Delete branch"}
              </span>
              {deleteBlockReason ? (
                <span className="whitespace-normal text-[11px] text-muted-foreground">
                  {deleteBlockReason}
                </span>
              ) : null}
            </span>
          </ContextMenuItem>
        </ContextMenuContent>
      </ContextMenu>
    </li>
  );
}

export function normalizeBranchName(name: string) {
  return name.trim().replace(/\s+/g, "-");
}

function CreateBranchDialog({
  currentBranchName,
  dirty,
  pending,
  onClose,
  onCreate,
}: {
  currentBranchName?: string;
  dirty: boolean;
  pending: boolean;
  onClose: () => void;
  onCreate: CurrentBranchPickerProps["onCreateBranch"];
}) {
  const [name, setName] = useState("");
  const [pendingChoice, setPendingChoice] = useState<
    "default" | UncommittedChangesStrategy | null
  >(null);
  const normalizedName = normalizeBranchName(name);

  const create = async (strategy?: UncommittedChangesStrategy) => {
    if (!normalizedName || pendingChoice) return;
    setPendingChoice(strategy ?? "default");
    if (await onCreate(normalizedName, strategy)) {
      onClose();
    } else {
      setPendingChoice(null);
    }
  };

  const submit = (event: FormEvent) => {
    event.preventDefault();
    if (!dirty) void create();
  };

  return (
    <Dialog
      open
      onOpenChange={(nextOpen) => {
        if (!nextOpen && !pending && !pendingChoice) onClose();
      }}
    >
      <DialogPopup>
        <form className="contents" onSubmit={submit}>
          <DialogHeader>
            <DialogTitle>Create a branch</DialogTitle>
            <DialogDescription>
              Create from {currentBranchName ?? "the current commit"} and check
              it out immediately.
            </DialogDescription>
          </DialogHeader>
          <Field name="new-branch-name">
            <FieldLabel>Branch name</FieldLabel>
            <Input
              autoFocus
              type="text"
              placeholder="feature/my-branch"
              spellCheck={false}
              value={name}
              onChange={(event) => setName(event.target.value)}
            />
            <FieldDescription>
              Spaces become hyphens. Slashes and Unicode are supported.
            </FieldDescription>
          </Field>
          {dirty ? (
            <div className="flex items-start gap-2 rounded-lg border border-warning/24 bg-warning/8 p-3 text-sm">
              <TriangleAlert className="mt-0.5 size-4 shrink-0 text-warning-foreground" />
              Choose whether to stash the current changes or bring them onto the
              new branch.
            </div>
          ) : null}
          <DialogFooter>
            <DialogClose
              render={
                <Button
                  type="button"
                  variant="outline"
                  disabled={pending || Boolean(pendingChoice)}
                />
              }
            >
              Cancel
            </DialogClose>
            {dirty ? (
              <>
                <Button
                  type="button"
                  variant="outline"
                  disabled={
                    !normalizedName || pending || Boolean(pendingChoice)
                  }
                  onClick={() => void create("StashOnCurrentBranch")}
                >
                  {pendingChoice === "StashOnCurrentBranch" ? (
                    <Loader2 className="animate-spin" />
                  ) : null}
                  Stash & Create
                </Button>
                <Button
                  type="button"
                  disabled={
                    !normalizedName || pending || Boolean(pendingChoice)
                  }
                  onClick={() => void create("BringChanges")}
                >
                  {pendingChoice === "BringChanges" ? (
                    <Loader2 className="animate-spin" />
                  ) : null}
                  Bring & Create
                </Button>
              </>
            ) : (
              <Button
                type="submit"
                disabled={!normalizedName || pending || Boolean(pendingChoice)}
              >
                {pendingChoice === "default" || pending ? (
                  <Loader2 className="animate-spin" />
                ) : (
                  <GitBranchPlus />
                )}
                Create & Checkout
              </Button>
            )}
          </DialogFooter>
        </form>
      </DialogPopup>
    </Dialog>
  );
}

function DirtyCheckoutDialog({
  branch,
  pending,
  onClose,
  onCheckout,
}: {
  branch: BranchInfo;
  pending: boolean;
  onClose: () => void;
  onCheckout: CurrentBranchPickerProps["onSwitchBranch"];
}) {
  const [pendingStrategy, setPendingStrategy] =
    useState<UncommittedChangesStrategy | null>(null);

  const checkout = async (strategy: UncommittedChangesStrategy) => {
    if (pendingStrategy) return;
    setPendingStrategy(strategy);
    if (await onCheckout(branch.name, strategy)) {
      onClose();
    } else {
      setPendingStrategy(null);
    }
  };

  return (
    <Dialog
      open
      onOpenChange={(nextOpen) => {
        if (!nextOpen && !pending && !pendingStrategy) onClose();
      }}
    >
      <DialogPopup>
        <DialogHeader>
          <DialogTitle>Uncommitted changes</DialogTitle>
          <DialogDescription>
            Choose what to do with your changes before checking out{" "}
            <span className="font-medium text-foreground">
              {branch.display_name}
            </span>
            .
          </DialogDescription>
        </DialogHeader>
        <div className="flex items-start gap-2 rounded-lg border border-warning/24 bg-warning/8 p-3 text-sm">
          <TriangleAlert className="mt-0.5 size-4 shrink-0 text-warning-foreground" />
          Stashing keeps them on the current branch. Bringing them attempts to
          carry them into the selected branch.
        </div>
        <DialogFooter>
          <DialogClose
            render={
              <Button
                type="button"
                variant="outline"
                disabled={pending || Boolean(pendingStrategy)}
              />
            }
          >
            Cancel
          </DialogClose>
          <Button
            type="button"
            variant="outline"
            disabled={pending || Boolean(pendingStrategy)}
            onClick={() => void checkout("StashOnCurrentBranch")}
          >
            {pendingStrategy === "StashOnCurrentBranch" ? (
              <Loader2 className="animate-spin" />
            ) : null}
            Stash & Checkout
          </Button>
          <Button
            type="button"
            disabled={pending || Boolean(pendingStrategy)}
            onClick={() => void checkout("BringChanges")}
          >
            {pendingStrategy === "BringChanges" ? (
              <Loader2 className="animate-spin" />
            ) : null}
            Bring & Checkout
          </Button>
        </DialogFooter>
      </DialogPopup>
    </Dialog>
  );
}

function RenameBranchDialog({
  branch,
  pending,
  onClose,
  onRename,
}: {
  branch: BranchInfo;
  pending: boolean;
  onClose: () => void;
  onRename: CurrentBranchPickerProps["onRenameBranch"];
}) {
  const [name, setName] = useState(branch.name);
  const normalizedName = normalizeBranchName(name);
  const canRename = normalizedName.length > 0 && normalizedName !== branch.name;

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    if (!canRename) return;
    if (await onRename(branch.name, normalizedName)) onClose();
  };

  return (
    <Dialog
      open
      onOpenChange={(nextOpen) => {
        if (!nextOpen && !pending) onClose();
      }}
    >
      <DialogPopup>
        <form className="contents" onSubmit={submit}>
          <DialogHeader>
            <DialogTitle>Rename branch</DialogTitle>
            <DialogDescription>
              Rename {branch.display_name}. Its upstream configuration is kept.
            </DialogDescription>
          </DialogHeader>
          <Field name="renamed-branch-name">
            <FieldLabel>New branch name</FieldLabel>
            <Input
              autoFocus
              type="text"
              spellCheck={false}
              value={name}
              onFocus={(event) => event.currentTarget.select()}
              onChange={(event) => setName(event.target.value)}
            />
            <FieldDescription>
              Slashes and Unicode are supported.
            </FieldDescription>
          </Field>
          <DialogFooter>
            <DialogClose
              render={
                <Button type="button" variant="outline" disabled={pending} />
              }
            >
              Cancel
            </DialogClose>
            <Button type="submit" disabled={!canRename || pending}>
              {pending ? <Loader2 className="animate-spin" /> : <Pencil />}
              Rename branch
            </Button>
          </DialogFooter>
        </form>
      </DialogPopup>
    </Dialog>
  );
}

function UpstreamDialog({
  branch,
  remoteBranches,
  pending,
  onClose,
  onSetUpstream,
}: {
  branch: BranchInfo;
  remoteBranches: BranchInfo[];
  pending: boolean;
  onClose: () => void;
  onSetUpstream: CurrentBranchPickerProps["onSetUpstream"];
}) {
  const [query, setQuery] = useState("");
  const filteredBranches = remoteBranches.filter((remote) =>
    `${remote.name} ${remote.display_name}`
      .toLocaleLowerCase()
      .includes(query.trim().toLocaleLowerCase()),
  );

  const select = async (upstream: string) => {
    if (await onSetUpstream(branch.name, upstream)) onClose();
  };

  return (
    <Dialog
      open
      onOpenChange={(nextOpen) => {
        if (!nextOpen && !pending) onClose();
      }}
    >
      <DialogPopup className="sm:max-w-xl">
        <DialogHeader>
          <DialogTitle>
            {branch.upstream ? "Change upstream" : "Set upstream"}
          </DialogTitle>
          <DialogDescription>
            Choose the remote branch that {branch.display_name} should track.
          </DialogDescription>
        </DialogHeader>
        <InputGroup>
          <InputGroupInput
            aria-label="Search remote branches"
            autoFocus
            type="search"
            placeholder="Search remote branches…"
            spellCheck={false}
            value={query}
            onChange={(event) => setQuery(event.target.value)}
          />
          <InputGroupAddon align="inline-start">
            <Search aria-hidden="true" />
          </InputGroupAddon>
        </InputGroup>
        <ScrollArea className="h-56 rounded-lg border" scrollbarGutter>
          {filteredBranches.length ? (
            <ul aria-label="Remote branches">
              {filteredBranches.map((remote) => (
                <li key={remote.name} className="border-b last:border-b-0">
                  <button
                    type="button"
                    className="flex min-h-10 w-full items-center gap-2 px-3 py-2 text-left outline-none hover:bg-accent focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-ring disabled:opacity-64"
                    disabled={pending || remote.name === branch.upstream}
                    onClick={() => void select(remote.name)}
                  >
                    <Cloud className="size-4 shrink-0 text-muted-foreground" />
                    <span className="min-w-0 flex-1 truncate text-sm">
                      {remote.display_name}
                    </span>
                    {remote.name === branch.upstream ? (
                      <Badge variant="secondary">Current</Badge>
                    ) : null}
                    {remote.is_protected ? (
                      <ShieldCheck
                        aria-label="Default or protected branch"
                        className="size-3.5 text-muted-foreground"
                      />
                    ) : null}
                  </button>
                </li>
              ))}
            </ul>
          ) : (
            <div className="flex h-full items-center justify-center text-sm text-muted-foreground">
              No remote branches found.
            </div>
          )}
        </ScrollArea>
        <DialogFooter>
          <DialogClose
            render={
              <Button type="button" variant="outline" disabled={pending} />
            }
          >
            Cancel
          </DialogClose>
        </DialogFooter>
      </DialogPopup>
    </Dialog>
  );
}

function DeleteBranchDialog({
  branch,
  pending,
  onClose,
  onDelete,
}: {
  branch: BranchInfo;
  pending: boolean;
  onClose: () => void;
  onDelete: CurrentBranchPickerProps["onDeleteBranch"];
}) {
  const force = !branch.is_merged;
  const remove = async () => {
    if (await onDelete(branch.name, branch.is_remote, force)) onClose();
  };

  return (
    <AlertDialog
      open
      onOpenChange={(nextOpen) => {
        if (!nextOpen && !pending) onClose();
      }}
    >
      <AlertDialogPopup>
        <AlertDialogHeader>
          <AlertDialogTitle>
            Delete {branch.is_remote ? "remote " : ""}branch?
          </AlertDialogTitle>
          <AlertDialogDescription>
            {branch.is_remote
              ? `This removes ${branch.display_name} from its remote.`
              : `This removes ${branch.display_name} from this repository.`}{" "}
            {force
              ? "It contains commits that are not reachable from HEAD, so this requires a force delete."
              : "Its tip is already reachable from HEAD."}
          </AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogClose
            render={
              <Button type="button" variant="outline" disabled={pending} />
            }
          >
            Cancel
          </AlertDialogClose>
          <Button
            type="button"
            variant="destructive"
            disabled={pending}
            onClick={() => void remove()}
          >
            {pending ? <Loader2 className="animate-spin" /> : <Trash2 />}
            {force ? "Force delete branch" : "Delete branch"}
          </Button>
        </AlertDialogFooter>
      </AlertDialogPopup>
    </AlertDialog>
  );
}

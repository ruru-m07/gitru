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
import { Input } from "@gitru/ui/components/input";
import {
  InputGroup,
  InputGroupAddon,
  InputGroupInput,
} from "@gitru/ui/components/input-group";
import {
  Menu,
  MenuItem,
  MenuPopup,
  MenuSeparator,
  MenuTrigger,
} from "@gitru/ui/components/menu";
import {
  Popover,
  PopoverDescription,
  PopoverPopup,
  PopoverTitle,
  PopoverTrigger,
} from "@gitru/ui/components/popover";
import { ScrollArea } from "@gitru/ui/components/scroll-area";
import { Tabs, TabsList, TabsTab } from "@gitru/ui/components/tabs";
import { cn } from "@gitru/ui/lib/utils";
import {
  ArrowDown,
  ArrowUp,
  Check,
  ChevronDown,
  Cloud,
  GitBranch,
  GitBranchPlus,
  GitCommitVertical,
  Link2,
  Loader2,
  MoreHorizontal,
  Pencil,
  RefreshCw,
  Search,
  ShieldCheck,
  Trash2,
  TriangleAlert,
  Unlink,
} from "lucide-react";
import {
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
  useGitFetch,
  useGitRenameBranch,
  useGitSetBranchUpstream,
  useGitSwitchBranch,
  useGitUnsetBranchUpstream,
  useHasUncommittedChanges,
} from "@/hooks";

type BranchTab = "local" | "remote";
type RepoOperationKind = RepoOperation["kind"];
const EMPTY_BRANCHES: BranchInfo[] = [];

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
  isFetching?: boolean;
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
  onFetch: () => Promise<boolean>;
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
  const fetch = useGitFetch();

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
      isFetching={fetch.isPending}
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
      onFetch={() =>
        runAction(
          () => fetch.mutateAsync(),
          "Remote branches refreshed",
          "Unable to fetch remote branches",
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
  isFetching = false,
  onSwitchBranch,
  onCreateBranch,
  onRenameBranch,
  onDeleteBranch,
  onSetUpstream,
  onUnsetUpstream,
  onFetch,
}: CurrentBranchPickerProps) {
  const [open, setOpen] = useState(false);
  const [tab, setTab] = useState<BranchTab>("local");
  const [query, setQuery] = useState("");
  const [dialog, setDialog] = useState<BranchDialog>(null);
  const rowRefs = useRef<Array<HTMLButtonElement | null>>([]);

  const operationLocked = Boolean(operationKind && operationKind !== "clean");
  const branchChangesLocked = operationLocked || worktreeStateLoading;
  const currentInfo = localBranches.find(
    (branch) => branch.name === currentBranchName || branch.is_head,
  );
  const visibleBranches = useMemo(() => {
    const source = tab === "local" ? localBranches : remoteBranches;
    const search = query.trim().toLocaleLowerCase();
    const filtered = search
      ? source.filter((branch) =>
          `${branch.name} ${branch.display_name}`
            .toLocaleLowerCase()
            .includes(search),
        )
      : source;

    return [...filtered].sort((a, b) => {
      if (tab === "local") {
        const aCurrent = a.name === currentBranchName || a.is_head;
        const bCurrent = b.name === currentBranchName || b.is_head;
        if (aCurrent !== bCurrent) return aCurrent ? -1 : 1;
        if (a.is_protected !== b.is_protected) return a.is_protected ? -1 : 1;
      }
      return a.display_name.localeCompare(b.display_name);
    });
  }, [currentBranchName, localBranches, query, remoteBranches, tab]);

  const closePopover = () => {
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
    if (
      branchChangesLocked ||
      isMutating ||
      branch.name === currentBranchName ||
      branch.is_head
    ) {
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
    if (event.key !== "ArrowDown" && event.key !== "ArrowUp") return;
    event.preventDefault();
    const offset = event.key === "ArrowDown" ? 1 : -1;
    const nextIndex =
      (index + offset + visibleBranches.length) % visibleBranches.length;
    rowRefs.current[nextIndex]?.focus();
  };

  const displayLabel = rebasing
    ? "Rebasing"
    : detached
      ? "Detached HEAD"
      : "Current Branch";
  const displayName = rebasing
    ? (rebaseBranch ?? currentBranchDisplayName)
    : currentBranchDisplayName;

  return (
    <>
      <Popover
        open={open}
        onOpenChange={(nextOpen) => {
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
          side="bottom"
          sideOffset={0}
          className="w-[365px] max-w-[calc(100vw-1rem)] [&>div]:overflow-hidden [&>div]:p-0"
        >
          <div className="flex max-h-[min(30rem,var(--available-height))] min-h-0 w-full flex-col">
            <div className="flex items-center justify-between gap-3 border-b px-3 py-2.5">
              <div className="min-w-0">
                <PopoverTitle className="text-sm">Branches</PopoverTitle>
                <PopoverDescription className="sr-only">
                  Search, switch, create, and manage repository branches.
                </PopoverDescription>
              </div>
              <Button
                type="button"
                size="sm"
                variant="outline"
                disabled={branchChangesLocked || isMutating}
                onClick={() => openDialog({ kind: "create" })}
              >
                <GitBranchPlus />
                New branch
              </Button>
            </div>

            {operationLocked ? (
              <div
                className="flex items-start gap-2 border-b bg-warning/8 px-3 py-2 text-xs text-warning-foreground"
                role="status"
              >
                <TriangleAlert className="mt-0.5 size-3.5 shrink-0" />
                Finish the active Git operation before changing branches.
              </div>
            ) : worktreeStateLoading ? (
              <div
                className="flex items-center gap-2 border-b px-3 py-2 text-xs text-muted-foreground"
                role="status"
              >
                <Loader2 className="size-3.5 animate-spin" />
                Checking the working tree before branch changes…
              </div>
            ) : null}

            <div className="px-3 pt-2">
              <Tabs
                value={tab}
                onValueChange={(value) => setTab(value as BranchTab)}
              >
                <TabsList variant="underline" className="w-full">
                  <TabsTab className="flex-1" value="local">
                    Local
                    <span className="text-xs text-muted-foreground">
                      {localBranches.length}
                    </span>
                  </TabsTab>
                  <TabsTab className="flex-1" value="remote">
                    Remote
                    <span className="text-xs text-muted-foreground">
                      {remoteBranches.length}
                    </span>
                  </TabsTab>
                </TabsList>
              </Tabs>
            </div>

            <div className="px-3 py-2">
              <InputGroup>
                <InputGroupInput
                  aria-label="Filter branches"
                  autoFocus
                  type="search"
                  placeholder="Filter branches…"
                  spellCheck={false}
                  value={query}
                  onChange={(event) => setQuery(event.target.value)}
                  onKeyDown={(event) => {
                    if (event.key === "ArrowDown") {
                      event.preventDefault();
                      rowRefs.current[0]?.focus();
                    }
                  }}
                />
                <InputGroupAddon align="inline-start">
                  <Search aria-hidden="true" />
                </InputGroupAddon>
              </InputGroup>
            </div>

            <ScrollArea
              className="min-h-32 flex-1 border-y"
              scrollFade
              scrollbarGutter
            >
              {branchesLoading ? (
                <div
                  className="flex h-32 items-center justify-center gap-2 text-sm text-muted-foreground"
                  role="status"
                >
                  <Loader2 className="size-4 animate-spin" />
                  Loading branches…
                </div>
              ) : visibleBranches.length === 0 ? (
                <div className="flex h-32 flex-col items-center justify-center gap-1 px-6 text-center">
                  <GitBranch className="size-5 text-muted-foreground" />
                  <p className="text-sm font-medium">No branches found</p>
                  <p className="text-xs text-muted-foreground">
                    Try another name or fetch the latest remote branches.
                  </p>
                </div>
              ) : (
                <ul
                  aria-label={`${tab === "local" ? "Local" : "Remote"} branches`}
                >
                  {visibleBranches.map((branch, index) => (
                    <BranchRow
                      key={`${branch.is_remote ? "remote" : "local"}:${branch.name}`}
                      branch={branch}
                      currentBranchName={currentBranchName}
                      currentUpstream={currentInfo?.upstream}
                      disabled={branchChangesLocked || isMutating}
                      ref={(node) => {
                        rowRefs.current[index] = node;
                      }}
                      onCheckout={() => void checkout(branch)}
                      onKeyDown={(event) => focusAdjacentRow(event, index)}
                      onOpenDialog={openDialog}
                      onUnsetUpstream={async () => {
                        const succeeded = await onUnsetUpstream(branch.name);
                        if (succeeded) closePopover();
                      }}
                    />
                  ))}
                </ul>
              )}
            </ScrollArea>

            <div className="flex items-center justify-between gap-3 px-3 py-2">
              <span
                className="truncate text-xs text-muted-foreground"
                aria-live="polite"
              >
                {isMutating
                  ? "Updating branches…"
                  : tab === "local"
                    ? `${localBranches.length} local branches`
                    : `${remoteBranches.length} remote branches`}
              </span>
              <Button
                type="button"
                size="sm"
                variant="ghost"
                disabled={isFetching || isMutating || operationLocked}
                onClick={() => void onFetch()}
              >
                <RefreshCw className={cn(isFetching && "animate-spin")} />
                {isFetching ? "Fetching…" : "Fetch & prune"}
              </Button>
            </div>
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
  currentBranchName?: string;
  currentUpstream?: string;
  disabled: boolean;
  ref: (node: HTMLButtonElement | null) => void;
  onCheckout: () => void;
  onKeyDown: (event: KeyboardEvent<HTMLButtonElement>) => void;
  onOpenDialog: (dialog: Exclude<BranchDialog, null>) => void;
  onUnsetUpstream: () => Promise<void>;
}

function BranchRow({
  branch,
  currentBranchName,
  currentUpstream,
  disabled,
  ref,
  onCheckout,
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

  return (
    <li className="group flex min-h-11 items-center border-b last:border-b-0 hover:bg-accent/64 focus-within:bg-accent/64">
      <button
        ref={ref}
        type="button"
        aria-current={isCurrent ? "true" : undefined}
        aria-disabled={isCurrent || disabled}
        aria-label={rowLabel}
        className="flex min-w-0 flex-1 items-center gap-2.5 self-stretch rounded-sm px-3 py-1.5 text-left outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-ring disabled:cursor-default disabled:opacity-64"
        disabled={disabled}
        onClick={onCheckout}
        onKeyDown={onKeyDown}
      >
        <span className="flex size-4 shrink-0 items-center justify-center text-muted-foreground">
          {isCurrent ? (
            <Check className="size-4 text-primary" strokeWidth={2.25} />
          ) : branch.is_remote ? (
            <Cloud className="size-3.5" />
          ) : (
            <GitBranch className="size-3.5" />
          )}
        </span>
        <span className="flex min-w-0 flex-1 flex-col">
          <span className="flex min-w-0 items-center gap-1.5">
            <span className="truncate text-sm font-medium">
              {branch.display_name}
            </span>
            {branch.is_protected ? (
              <ShieldCheck
                aria-label="Default or protected branch"
                className="size-3.5 shrink-0 text-muted-foreground"
              />
            ) : null}
          </span>
          <span className="flex min-w-0 items-center gap-1.5 truncate text-[11px] text-muted-foreground">
            {isCurrent ? <span>Current</span> : null}
            {branch.upstream ? (
              <span className="truncate">Tracks {branch.upstream}</span>
            ) : null}
            {!isCurrent && !branch.upstream ? (
              <span>{branch.is_remote ? "Remote branch" : "Local branch"}</span>
            ) : null}
          </span>
        </span>
        {branch.ahead ? (
          <Badge
            size="sm"
            variant="secondary"
            aria-label={`${branch.ahead} ahead`}
          >
            <ArrowUp />
            {branch.ahead}
          </Badge>
        ) : null}
        {branch.behind ? (
          <Badge
            size="sm"
            variant="secondary"
            aria-label={`${branch.behind} behind`}
          >
            <ArrowDown />
            {branch.behind}
          </Badge>
        ) : null}
      </button>

      <Menu>
        <MenuTrigger
          render={
            <Button
              type="button"
              size="icon-xs"
              variant="ghost"
              aria-label={`Actions for branch ${branch.display_name}`}
              className="mr-2 shrink-0 opacity-0 group-hover:opacity-100 focus:opacity-100 data-[popup-open]:opacity-100 group-focus-within:opacity-100"
            />
          }
        >
          <MoreHorizontal />
        </MenuTrigger>
        <MenuPopup align="end" className="w-64">
          {!isCurrent ? (
            <MenuItem closeOnClick disabled={disabled} onClick={onCheckout}>
              {branch.is_remote ? <Cloud /> : <GitBranch />}
              {branch.is_remote ? "Checkout and track" : "Checkout branch"}
            </MenuItem>
          ) : null}
          {!branch.is_remote ? (
            <>
              <MenuItem
                closeOnClick
                disabled={disabled}
                onClick={() => onOpenDialog({ kind: "rename", branch })}
              >
                <Pencil />
                Rename branch
              </MenuItem>
              <MenuItem
                closeOnClick
                disabled={disabled}
                onClick={() => onOpenDialog({ kind: "upstream", branch })}
              >
                <Link2 />
                {branch.upstream ? "Change upstream" : "Set upstream"}
              </MenuItem>
              {branch.upstream ? (
                <MenuItem
                  closeOnClick
                  disabled={disabled}
                  onClick={() => void onUnsetUpstream()}
                >
                  <Unlink />
                  Unset upstream
                </MenuItem>
              ) : null}
            </>
          ) : null}
          <MenuSeparator />
          <MenuItem
            closeOnClick
            variant="destructive"
            disabled={disabled}
            aria-disabled={Boolean(deleteBlockReason)}
            className={cn(
              deleteBlockReason && "cursor-not-allowed items-start opacity-64",
            )}
            onClick={() => {
              if (!deleteBlockReason) {
                onOpenDialog({ kind: "delete", branch });
              }
            }}
          >
            <Trash2 className="mt-0.5" />
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
          </MenuItem>
        </MenuPopup>
      </Menu>
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

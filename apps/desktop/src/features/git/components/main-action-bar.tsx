import { Button } from "@gitru/ui/components/button";
import { useCommandNavigation } from "@gitru/ui/components/command";
import {
  Menu,
  MenuItem,
  MenuPopup,
  MenuTrigger,
} from "@gitru/ui/components/menu";
import { Separator } from "@gitru/ui/components/separator";
import {
  ArrowDownToLine,
  ArrowUpFromLine,
  ChevronDown,
  GitBranch,
  GitCommitVertical,
  GitCompareArrows,
  Loader2,
} from "lucide-react";
import { toast } from "sonner";
import {
  useGetCurrentBranch,
  useGetRepoOperation,
  useGetStatusAheadBehind,
  useGitPublishBranch,
  useGitPull,
  useGitPush,
} from "@/hooks";
import { resolveSyncState, type SyncAction } from "./sync-state";

const successMessages: Record<SyncAction, string> = {
  publish: "Branch published",
  push: "Changes pushed",
  pull: "Changes pulled",
};

export const MainActionBar = () => {
  const { data: currentBranch } = useGetCurrentBranch();
  const { data: statusAheadBehind } = useGetStatusAheadBehind();
  const { data: operation } = useGetRepoOperation();
  const branchNavigation = useCommandNavigation();

  const publishMutation = useGitPublishBranch();
  const pushMutation = useGitPush();
  const pullMutation = useGitPull();

  const detached =
    currentBranch?.is_detached ?? statusAheadBehind?.is_detached ?? false;
  const rebasing = operation?.isRebasing ?? false;
  // While rebasing HEAD is detached, so the rebased branch name lives on the operation.
  const rebaseBranch = operation?.headName?.replace(/^refs\/heads\//, "");
  const syncState = resolveSyncState({
    isDetached: detached,
    operationKind: operation?.kind,
    status: statusAheadBehind
      ? {
          ahead: statusAheadBehind.ahead,
          behind: statusAheadBehind.behind,
          isPublished: statusAheadBehind.is_published,
          isDetached: statusAheadBehind.is_detached,
          localBranch: statusAheadBehind.local_branch,
          upstreamBranch: statusAheadBehind.upstream_branch,
        }
      : undefined,
  });
  const isPending =
    publishMutation.isPending ||
    pushMutation.isPending ||
    pullMutation.isPending;

  const runSyncAction = async (action: SyncAction) => {
    try {
      if (action === "publish") await publishMutation.mutateAsync();
      if (action === "push") await pushMutation.mutateAsync();
      if (action === "pull") await pullMutation.mutateAsync();
      toast.success(successMessages[action]);
    } catch {
      // Mutation hooks surface the command's actionable error message.
    }
  };

  const syncIcon = isPending ? (
    <Loader2 className="animate-spin size-7.5" strokeWidth={1.5} />
  ) : syncState.kind === "behind" ? (
    <ArrowDownToLine className="size-7.5" strokeWidth={1.5} />
  ) : syncState.kind === "diverged" ? (
    <GitCompareArrows className="size-7.5" strokeWidth={1.5} />
  ) : (
    <ArrowUpFromLine className="size-7.5" strokeWidth={1.5} />
  );

  const syncContent = (
    <div className="flex items-center gap-4 min-w-0 flex-1">
      {syncIcon}
      <div className="flex flex-col flex-1 items-start min-w-0">
        <span className="text-xs text-muted-foreground font-normal">
          {isPending ? "Syncing…" : syncState.label}
        </span>
        <span className="truncate block w-full text-left">
          {syncState.detail}
        </span>
      </div>
    </div>
  );

  return (
    <div className="w-full justify-between min-h-14 max-h-14 h-14 border-b flex">
      <div className="min-h-14 max-h-14 h-14 flex w-full">
        <Button
          aria-label="Switch branch"
          className="flex justify-between items-center min-h-full rounded-none border-x-0 max-w-72 w-full"
          variant="ghost"
          onClick={() => {
            branchNavigation.setOpen(true);
            branchNavigation.push("branch-list");
          }}
        >
          <div className="flex items-center gap-4 min-w-0 flex-1">
            {detached ? (
              <GitCommitVertical className="size-7.5" strokeWidth={1.5} />
            ) : (
              <GitBranch className="size-7.5" strokeWidth={1.5} />
            )}

            <div className="flex flex-col items-start min-w-0 flex-1">
              <span className="text-xs text-muted-foreground font-[450]">
                {rebasing
                  ? "Rebasing"
                  : detached
                    ? "Detached HEAD"
                    : "Current Branch"}
              </span>
              <span className="truncate block w-full text-left">
                {rebasing
                  ? (rebaseBranch ?? currentBranch?.display_name)
                  : currentBranch?.display_name}
              </span>
            </div>
          </div>

          <ChevronDown aria-hidden="true" size={18} />
        </Button>
        <Separator orientation="vertical" className="border-0" />

        {syncState.kind === "detached" ? null : syncState.kind ===
          "diverged" ? (
          <>
            <Menu>
              <MenuTrigger
                render={
                  <Button
                    aria-label={`${syncState.label}. Choose pull or push.`}
                    className="flex justify-between items-center min-h-full rounded-none border-x-0 w-72"
                    variant="ghost"
                    disabled={isPending}
                  />
                }
              >
                {syncContent}
                <ChevronDown aria-hidden="true" size={18} />
              </MenuTrigger>
              <MenuPopup align="end" className="w-64">
                <MenuItem
                  closeOnClick
                  onClick={() => void runSyncAction("pull")}
                >
                  <ArrowDownToLine />
                  Pull remote changes
                </MenuItem>
                <MenuItem
                  closeOnClick
                  onClick={() => void runSyncAction("push")}
                >
                  <ArrowUpFromLine />
                  Push local commits
                </MenuItem>
              </MenuPopup>
            </Menu>
            <Separator orientation="vertical" className="border-0" />
          </>
        ) : syncState.primaryAction ? (
          <>
            <Button
              aria-label={syncState.label}
              className="flex border-x-0 justify-between items-center min-h-full rounded-none max-w-72 w-72"
              variant="ghost"
              disabled={isPending}
              onClick={() => void runSyncAction(syncState.primaryAction!)}
            >
              {syncContent}
            </Button>
            <Separator orientation="vertical" className="border-0" />
          </>
        ) : (
          <>
            <div
              aria-live="polite"
              className="flex border-x-0 items-center min-h-full max-w-72 w-72 px-4"
              role="status"
            >
              {syncContent}
            </div>
            <Separator orientation="vertical" className="border-0" />
          </>
        )}
      </div>
    </div>
  );
};

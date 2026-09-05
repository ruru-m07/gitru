export type SyncAction = "publish" | "push" | "pull" | "fetch";

export type SyncStateKind =
  | "loading"
  | "detached"
  | "inProgress"
  | "unpublished"
  | "ahead"
  | "behind"
  | "diverged"
  | "synced";

export interface SyncState {
  kind: SyncStateKind;
  label: string;
  detail: string;
  primaryAction: SyncAction | null;
}

export const isSyncControlVisible = (state: SyncState) =>
  state.kind !== "loading" && state.kind !== "detached";

interface SyncStatus {
  ahead: number;
  behind: number;
  isPublished: boolean;
  isDetached: boolean;
  localBranch?: string;
  upstreamBranch?: string;
}

interface ResolveSyncStateInput {
  status?: SyncStatus;
  operationKind?: string;
  isDetached?: boolean;
}

const operationLabels: Record<string, string> = {
  merge: "Merge in progress",
  revert: "Revert in progress",
  cherryPick: "Cherry-pick in progress",
  bisect: "Bisect in progress",
  rebase: "Rebase in progress",
  rebaseInteractive: "Rebase in progress",
  rebaseMerge: "Rebase in progress",
  applyMailbox: "Patch apply in progress",
  other: "Git operation in progress",
};

export function resolveSyncState({
  status,
  operationKind,
  isDetached,
}: ResolveSyncStateInput): SyncState {
  if (operationKind && operationKind !== "clean") {
    return {
      kind: "inProgress",
      label: operationLabels[operationKind] ?? "Git operation in progress",
      detail: "Finish or abort it before syncing",
      primaryAction: null,
    };
  }

  if (isDetached || status?.isDetached) {
    return {
      kind: "detached",
      label: "Detached HEAD",
      detail: "Checkout a branch to sync",
      primaryAction: null,
    };
  }

  if (!status) {
    return {
      kind: "loading",
      label: "Checking remote status",
      detail: "Please wait…",
      primaryAction: null,
    };
  }

  if (!status.isPublished) {
    return {
      kind: "unpublished",
      label: "Publish Branch",
      detail: status.localBranch ?? "Set an upstream branch",
      primaryAction: "publish",
    };
  }

  if (status.ahead > 0 && status.behind > 0) {
    return {
      kind: "diverged",
      label: "Branches Diverged",
      detail: `↑${status.ahead} ↓${status.behind}`,
      primaryAction: null,
    };
  }

  if (status.ahead > 0) {
    return {
      kind: "ahead",
      label: "Push to Origin",
      detail: `${status.ahead} commit${status.ahead === 1 ? "" : "s"} ahead`,
      primaryAction: "push",
    };
  }

  if (status.behind > 0) {
    return {
      kind: "behind",
      label: "Pull from Origin",
      detail: `${status.behind} commit${status.behind === 1 ? "" : "s"} behind`,
      primaryAction: "pull",
    };
  }

  return {
    kind: "synced",
    label: "Fetch",
    detail: status.upstreamBranch ?? "Check for remote changes",
    primaryAction: "fetch",
  };
}

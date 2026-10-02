import type { QueryClient, QueryKey } from "@tanstack/react-query";
import { queryClient } from "./state-manager";

export const REPOSITORY_CHANGED_EVENT = "gitru://repository-changed";

export type RepositoryChangeKind =
  | "worktree"
  | "index"
  | "head"
  | "refs"
  | "stash"
  | "operation"
  | "config";

export type RepositoryChangedPayload = {
  contextId: string;
  changes: RepositoryChangeKind[];
};

const isRepositoryQueryForContext = (queryKey: QueryKey, contextId: string) =>
  queryKey[0] === "repository" && queryKey[1] === contextId;

const isWorktreeFileQueryForContext = (queryKey: QueryKey, contextId: string) =>
  queryKey[0] === "worktree-file" && queryKey[1] === contextId;

const hasBranchKey = (queryKey: QueryKey, keys: readonly string[]) =>
  queryKey[3] === "branches" &&
  typeof queryKey[4] === "string" &&
  keys.includes(queryKey[4]);

const isWorktreeDiff = (queryKey: QueryKey) =>
  queryKey[3] === "diff" && queryKey[4] === "worktree";

const isStashDiff = (queryKey: QueryKey) =>
  queryKey[3] === "diff" &&
  typeof queryKey[4] === "string" &&
  queryKey[4].startsWith("stash:");

const isCommitTimeline = (queryKey: QueryKey) =>
  queryKey[3] === "commit" &&
  ["last", "history", "historyGraph", "commitActivity"].includes(
    String(queryKey[4]),
  );

const includesStash = (value: unknown) =>
  typeof value === "object" &&
  value !== null &&
  "include_stash" in value &&
  value.include_stash === true;

const isStashTimeline = (queryKey: QueryKey) =>
  queryKey[3] === "commit" &&
  ["historyGraph", "commitActivity"].includes(String(queryKey[4])) &&
  queryKey.some(includesStash);

const isAffectedByChange = (
  queryKey: QueryKey,
  change: RepositoryChangeKind,
) => {
  const domain = queryKey[3];

  switch (change) {
    case "worktree":
      return (
        domain === "status" ||
        isWorktreeDiff(queryKey) ||
        hasBranchKey(queryKey, ["hasUncommittedChanges"])
      );
    case "index":
      return (
        domain === "status" ||
        domain === "operation" ||
        isWorktreeDiff(queryKey) ||
        hasBranchKey(queryKey, ["hasUncommittedChanges"])
      );
    case "head":
      return (
        domain === "status" ||
        isWorktreeDiff(queryKey) ||
        isCommitTimeline(queryKey) ||
        hasBranchKey(queryKey, [
          "list",
          "current",
          "statusAheadBehind",
          "hasUncommittedChanges",
          "currentBranchStash",
        ])
      );
    case "refs":
      return (
        domain === "status" ||
        isWorktreeDiff(queryKey) ||
        isCommitTimeline(queryKey) ||
        hasBranchKey(queryKey, [
          "list",
          "statusAheadBehind",
          "hasUncommittedChanges",
        ])
      );
    case "stash":
      return (
        domain === "stash" ||
        isStashDiff(queryKey) ||
        isStashTimeline(queryKey) ||
        hasBranchKey(queryKey, ["currentBranchStash"])
      );
    case "operation":
      return (
        domain === "operation" ||
        domain === "status" ||
        isWorktreeDiff(queryKey) ||
        hasBranchKey(queryKey, ["current", "hasUncommittedChanges"])
      );
    case "config":
      return (
        domain === "status" ||
        domain === "diff" ||
        domain === "origin" ||
        hasBranchKey(queryKey, [
          "list",
          "statusAheadBehind",
          "hasUncommittedChanges",
        ])
      );
  }
};

export const shouldInvalidateRepositoryQuery = (
  queryKey: QueryKey,
  payload: RepositoryChangedPayload,
) => {
  if (isWorktreeFileQueryForContext(queryKey, payload.contextId)) {
    return payload.changes.includes("worktree");
  }

  return (
    isRepositoryQueryForContext(queryKey, payload.contextId) &&
    payload.changes.some((change) => isAffectedByChange(queryKey, change))
  );
};

export const invalidateRepositoryQueries = async (
  client: QueryClient,
  payload: RepositoryChangedPayload,
) => {
  const predicate = (query: { queryKey: QueryKey }) =>
    shouldInvalidateRepositoryQuery(query.queryKey, payload);

  // An invalidation alone does not restart an active initial fetch with no
  // cached data. Cancel first so a pre-change result cannot clear the
  // invalidation and leave the query stale indefinitely.
  await client.cancelQueries({ predicate });
  await client.invalidateQueries({
    predicate,
    refetchType: "active",
  });
};

let repositoryChangeBridgeInitialized = false;

/**
 * Installs one native change listener for this webview runtime. The context ID
 * in each event keeps unrelated repository tabs from refetching.
 */
export function initializeRepositoryChangeBridge() {
  if (repositoryChangeBridgeInitialized || typeof window === "undefined") {
    return;
  }

  repositoryChangeBridgeInitialized = true;

  void import("@tauri-apps/api/event")
    .then(async ({ listen }) => {
      await listen<RepositoryChangedPayload>(
        REPOSITORY_CHANGED_EVENT,
        ({ payload }) => {
          if (!payload?.contextId || !Array.isArray(payload.changes)) return;
          void invalidateRepositoryQueries(queryClient, payload);
        },
      );
    })
    .catch(() => {
      // The bridge is optional in a plain browser runtime. Native focus
      // refresh remains the recovery path if listener setup fails in Tauri.
    });
}

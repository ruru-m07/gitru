import { type UseQueryOptions, useQuery } from "@tanstack/react-query";
import { useMemo } from "react";
import type { Branch, GetStatusResponse } from "@/tauri";
import { appState } from "../index";

type QueryOptions<T> = Omit<
  UseQueryOptions<T | null, Error>,
  "queryKey" | "queryFn" | "enabled"
>;

/**
 * Hook to get the current repository state
 * Returns null if no repository is selected
 */
export function useRepository() {
  return appState.repository;
}

/**
 * Hook to check if a repository is selected
 */
export function useHasRepository() {
  return appState.repository !== null;
}

// ============================================
// Status Hooks
// ============================================

/**
 * Hook to get git status for the current repository
 *
 * @example
 * const { data, isLoading, isFetching } = useStatus();
 * console.log(data?.files);
 */
export function useStatus(options?: QueryOptions<GetStatusResponse>) {
  const repo = appState.repository;

  return useQuery({
    queryKey: repo?.status.queryKey ?? ["repository", "none", "status"],
    queryFn: async () => {
      if (!repo) return null;
      return await repo.status.get();
    },
    enabled: !!repo,
    ...options,
  });
}

// ============================================
// Branch Hooks
// ============================================

/**
 * Hook to get current branch
 *
 * @example
 * const { data: branch } = useCurrentBranch();
 * console.log(branch?.display_name);
 */
export function useCurrentBranch(options?: QueryOptions<Branch>) {
  const repo = appState.repository;

  return useQuery({
    queryKey: repo?.branches.currentQueryKey ?? [
      "repository",
      "none",
      "branches",
      "current",
    ],
    queryFn: async () => {
      if (!repo) return null;
      return await repo.branches.current();
    },
    enabled: !!repo,
    ...options,
  });
}

/**
 * Hook to get all branches
 *
 * @example
 * const { data: branches } = useBranches();
 * branches?.map(b => b.name);
 */
export function useBranches(options?: QueryOptions<Branch[]>) {
  const repo = appState.repository;

  return useQuery({
    queryKey: repo?.branches.listQueryKey ?? [
      "repository",
      "none",
      "branches",
      "list",
    ],
    queryFn: async () => {
      if (!repo) return null;
      return await repo.branches.list();
    },
    enabled: !!repo,
    ...options,
  });
}

// ============================================
// Diff Hooks
// ============================================

/**
 * Hook to get diff for a specific file
 *
 * @example
 * const { data: diff } = useDiff("src/main.ts");
 * console.log(diff?.head?.content);
 */
export function useDiff(
  filePath: string | null,
  options?: QueryOptions<unknown>,
) {
  const repo = appState.repository;

  return useQuery({
    queryKey: filePath
      ? (repo?.diff.getQueryKey(filePath) ?? [
          "repository",
          "none",
          "diff",
          filePath,
        ])
      : ["repository", "none", "diff"],
    queryFn: async () => {
      if (!repo || !filePath) return null;
      return await repo.diff.get(filePath);
    },
    enabled: !!repo && !!filePath,
    ...options,
  });
}

// ============================================
// Combined / Convenience Hooks
// ============================================

/**
 * Hook that returns commonly used repository data together
 * Useful for components that need multiple pieces of repo info
 *
 * @example
 * const { status, currentBranch, branches, isLoading } = useRepositoryData();
 */
export function useRepositoryData() {
  const statusQuery = useStatus();
  const currentBranchQuery = useCurrentBranch();
  const branchesQuery = useBranches();

  return {
    status: statusQuery.data,
    currentBranch: currentBranchQuery.data,
    branches: branchesQuery.data,
    isLoading:
      statusQuery.isLoading ||
      currentBranchQuery.isLoading ||
      branchesQuery.isLoading,
    isFetching:
      statusQuery.isFetching ||
      currentBranchQuery.isFetching ||
      branchesQuery.isFetching,
    // Individual query results if needed
    queries: {
      status: statusQuery,
      currentBranch: currentBranchQuery,
      branches: branchesQuery,
    },
  };
}

// ============================================
// Action Hooks (for mutations)
// ============================================

/**
 * Hook that returns repository actions
 * These are imperative functions, not queries
 *
 * @example
 * const actions = useRepositoryActions();
 * await actions.pull();
 * await actions.checkout("feature-branch");
 */
export function useRepositoryActions() {
  const repo = appState.repository;
  if (!repo) throw new Error("No repository selected");

  return useMemo(
    () => ({
      add: async (filePath: string) => {
        const data = await repo?.file.add(filePath);
        if (data.success) {
          await repo.status.invalidate();
        }
        return data.success;
      },
      unstage: async (filePath: string) => {
        const data = await repo?.file.unstage(filePath);
        if (data.success) {
          await repo.status.invalidate();
        }
        return data.success;
      },
      discard: async (filePath: string) => {
        const data = await repo?.file.discard(filePath);
        if (data.success) {
          await repo.status.invalidate();
        }
        return data;
      },
      addAll: async () => {
        const data = await repo?.file.addAll();
        if (data.success) {
          await repo.status.invalidate();
        }
        return data;
      },
      removeAll: async () => {
        const data = await repo?.file.removeAll();
        if (data.success) {
          await repo.status.invalidate();
        }
        return data;
      },

      pull: async () => {
        return await repo.pull();
      },
      push: async () => {
        return await repo.push();
      },
      fetch: async () => {
        return await repo.fetch();
      },

      checkout: async (branchName: string) => {
        return await repo.branches.checkout(branchName);
      },

      invalidateStatus: async () => {
        await repo.status.invalidate();
      },
      invalidateBranches: async () => {
        await repo.branches.invalidateAll();
      },
      invalidateAll: async () => {
        await repo.invalidateAll();
      },

      getDiff: async (filePath: string) => {
        return await repo.diff.get(filePath);
      },
      invalidateDiff: async (filePath?: string) => {
        await repo.diff.invalidate(filePath);
      },
    }),
    [repo],
  );
}

import type {
  AheadBehindStatus,
  Branch,
  BranchInfo,
  BranchKind,
  BranchStash,
  CommitInfo,
  CreateCommitParams,
  FileDiff,
  FullCommitInfo,
  GetStatusResponse,
  HistoryGraphParams,
  HistoryGraphResponse,
  RepositoryOrigin,
  UncommittedChangesStrategy,
} from "@gitru/commands";
import {
  InfiniteData,
  type UseQueryOptions,
  useInfiniteQuery,
  useMutation,
  useQuery,
} from "@tanstack/react-query";
import { toast } from "sonner";
import { appState } from "@/state";

type QueryOptions<T> = Omit<
  UseQueryOptions<T | null, Error>,
  "queryKey" | "queryFn" | "enabled"
>;

/* #region // ? Query */
export function useGetStatus(options?: QueryOptions<GetStatusResponse>) {
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

export function useGetCurrentBranch(options?: QueryOptions<Branch>) {
  const repo = appState.repository;
  return useQuery({
    queryKey: repo?.branches.getQueryKey("current") ?? [
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

export function useGetBranches(
  kind: BranchKind,
  options?: QueryOptions<BranchInfo[]>,
) {
  const repo = appState.repository;

  return useQuery({
    queryKey: [
      ...(repo?.branches.getQueryKey("list") ?? [
        "repository",
        "branches",
        "list",
      ]),
      kind,
    ],
    queryFn: async () => {
      if (!repo) return null;
      return await repo.branches.list(kind);
    },
    enabled: !!repo,
    ...options,
  });
}

export function useGetDiff(
  filePath: string | null,
  params?: {
    stashReference?: string | null;
    commitHash?: string | null;
    parentIndex?: number;
  },
  options?: QueryOptions<FileDiff>,
) {
  const repo = appState.repository;
  const stashReference = params?.stashReference ?? null;
  const commitHash = params?.commitHash ?? null;
  const parentIndex = params?.parentIndex ?? 1;
  const sourceScope = stashReference
    ? `stash:${stashReference}`
    : commitHash
      ? `commit:${commitHash}:p${parentIndex}`
      : "worktree";

  return useQuery({
    queryKey: filePath
      ? (repo?.diff.getDiffQueryKey(filePath, {
          stashReference: stashReference ?? undefined,
          commitHash: commitHash ?? undefined,
          parentIndex,
        }) ?? [
          "repository",
          "none",
          "diff",
          sourceScope,
          filePath,
        ])
      : ["repository", "none", "diff"],
    // queryFn: async () => {
    //   if (!repo || !filePath) return null;
    //   return await repo.diff.get(filePath);
    // },
    queryFn: async () => {
      if (!repo || !filePath) return null;

      const start = performance.now();
      const result = await repo.diff.get(filePath, {
        stashReference: stashReference ?? undefined,
        commitHash: commitHash ?? undefined,
        parentIndex,
      });
      const end = performance.now();

      console.log(
        `[useGetDiff] repo.diff.get took ${(end - start).toFixed(2)} ms`,
      );

      return result;
    },

    staleTime: 3000,
    enabled: !!repo && !!filePath,
    placeholderData: (prev) => prev,
    ...options,
  });
}

export function useGetLastCommit(options?: QueryOptions<CommitInfo>) {
  const repo = appState.repository;

  return useQuery({
    queryKey: repo?.commit.getQueryKey("last") ?? [
      "repository",
      "none",
      "commit",
      "last",
    ],
    queryFn: async () => {
      if (!repo) return null;
      return await repo.commit.last();
    },
    enabled: !!repo,
    ...options,
  });
}

export function useGetCommitHistory(options?: QueryOptions<CommitInfo[]>) {
  const repo = appState.repository;

  return useQuery({
    queryKey: repo?.commit.getQueryKey("history") ?? [
      "repository",
      "none",
      "commit",
      "history",
    ],
    queryFn: async () => {
      if (!repo) return null;
      return await repo.commit.history();
    },
    enabled: !!repo,
    ...options,
  });
}

export function useGitHistoryGraph(
  params: Partial<HistoryGraphParams["query"]> = {},
  options?: {
    enabled?: boolean;
  },
) {
  const repo = appState.repository;
  const baseParams: HistoryGraphParams["query"] = {
    limit: params.limit ?? 50,
    cursor: params.cursor,
    search: params.search,
    branch: params.branch,
    graph_state: params.graph_state,
    include_local: params.include_local ?? true,
    include_remotes: params.include_remotes ?? false,
    include_tags: params.include_tags ?? false,
    include_stash: params.include_stash ?? false,
  };

  return useInfiniteQuery<
    HistoryGraphResponse,
    Error,
    InfiniteData<HistoryGraphResponse>,
    (string | HistoryGraphParams["query"])[],
    HistoryGraphParams["query"]
  >({
    enabled: (options?.enabled ?? true) && !!repo,
    placeholderData: {
      pages: [],
      pageParams: [],
    },
    getNextPageParam: (lastPage) => {
      if (!lastPage.has_more || !lastPage.cursor) {
        return undefined;
      }
      return {
        ...baseParams,
        cursor: lastPage.cursor,
        // graph_state removed - using server-side cache now
      };
    },
    initialPageParam: baseParams,
    queryFn: async ({ pageParam }) => {
      if (!repo) throw new Error("No repository selected");
      console.log("calling...");
      const fn = await repo.commit.historyGraph(pageParam);

      console.log({
        fn,
      });

      return fn;
    },
    queryKey: [
      ...(repo?.commit.getQueryKey("historyGraph") ?? [
        "repository",
        "none",
        "commit",
        "historyGraph",
      ]),
      baseParams,
    ],
  });
}

export function useGetCommitById(
  hash: string,
  options?: QueryOptions<FullCommitInfo>,
) {
  const repo = appState.repository;

  return useQuery({
    queryKey: [
      ...(repo?.commit.getQueryKey("getCommitById") ?? [
        "repository",
        "none",
        "commit",
        "getCommitById",
      ]),
      hash,
    ],
    queryFn: async () => {
      if (!repo) return null;
      if (!hash) return null;
      return await repo.commit.getCommitById(hash);
    },
    enabled: !!repo && !!hash,
    ...options,
  });
}

export function useGetRepositoryOrigin(
  options?: QueryOptions<RepositoryOrigin>,
) {
  const repo = appState.repository;

  return useQuery({
    queryKey: repo?.getQueryKey("origin") ?? ["repository", "none", "origin"],
    queryFn: async () => {
      if (!repo) return null;
      return await repo.getRepositoryOrigin();
    },
    enabled: !!repo,
    ...options,
  });
}

export function useGetStatusAheadBehind(
  options?: QueryOptions<AheadBehindStatus>,
) {
  const repo = appState.repository;

  return useQuery({
    queryKey: repo?.branches.getQueryKey("statusAheadBehind") ?? [
      "repository",
      "none",
      "branches",
      "statusAheadBehind",
    ],
    queryFn: async () => {
      if (!repo) return null;
      return await repo.branches.statusAheadBehind();
    },
    enabled: !!repo,
    ...options,
  });
}
/* #endregion // ? Query */

/* #region // ! Mutations */
export function useCreateCommit() {
  const repo = appState.repository;

  const mutation = useMutation({
    mutationFn: async (payload: CreateCommitParams) => {
      if (!repo) throw new Error("No repository selected");
      return await repo.commit.createCommit(payload);
    },
    onSuccess: async () => {
      await repo?.status.invalidate();
      await repo?.commit.invalidate();
      await repo?.branches.invalidate("statusAheadBehind");
    },
    onError: (error: string) => {
      toast.error(error);
    },
  });

  return mutation;
}

export function useGitDiscard() {
  const repo = appState.repository;

  const mutation = useMutation({
    mutationFn: async ({ filePath }: { filePath: string }) => {
      if (!repo) throw new Error("No repository selected");
      return await repo.file.discard(filePath);
    },
    onSuccess: async () => {
      await repo?.status.invalidate();
    },
    onError: (error: string) => {
      toast.error(error);
    },
  });

  return mutation;
}

export function useGitAdd() {
  const repo = appState.repository;

  const mutation = useMutation({
    mutationFn: async (filePath: string) => {
      if (!repo) throw new Error("No repository selected");
      return await repo.file.add(filePath);
    },
    onSuccess: async () => {
      await repo?.status.invalidate();
    },
    onError: (error: string) => {
      toast.error(error);
    },
  });

  return mutation;
}

export function useGitUnstage() {
  const repo = appState.repository;

  const mutation = useMutation({
    mutationFn: async (filePath: string) => {
      if (!repo) throw new Error("No repository selected");
      return await repo.file.unstage(filePath);
    },
    onSuccess: async () => {
      await repo?.status.invalidate();
    },
    onError: (error: string) => {
      toast.error(error);
    },
  });

  return mutation;
}

export function useGitFetch() {
  const repo = appState.repository;

  const mutation = useMutation({
    mutationFn: async () => {
      if (!repo) throw new Error("No repository selected");
      return await repo.file.fetch();
    },
    onSuccess: async () => {
      await repo?.branches.invalidateAll();
      await repo?.commit.invalidate();
      await repo?.status.invalidate();
    },
    onError: (error: string) => {
      toast.error(error || "Failed to fetch");
    },
  });

  return mutation;
}

export function useGitPublishBranch() {
  const repo = appState.repository;

  const mutation = useMutation({
    mutationFn: async () => {
      if (!repo) throw new Error("No repository selected");
      return await repo.file.publishBranch();
    },
    onSuccess: async () => {
      await repo?.branches.invalidateAll();
      await repo?.commit.invalidate();
      await repo?.status.invalidate();
    },
    onError: (error: string) => {
      toast.error(error || "Failed to publish branch");
    },
  });

  return mutation;
}

export function useGitPush() {
  const repo = appState.repository;

  const mutation = useMutation({
    mutationFn: async () => {
      if (!repo) throw new Error("No repository selected");
      return await repo.file.push();
    },
    onSuccess: async () => {
      await repo?.branches.invalidateAll();
      await repo?.commit.invalidate();
      await repo?.status.invalidate();
    },
    onError: (error: string) => {
      toast.error(error || "Failed to push");
    },
  });

  return mutation;
}

export function useGitPull() {
  const repo = appState.repository;

  const mutation = useMutation({
    mutationFn: async () => {
      if (!repo) throw new Error("No repository selected");
      return await repo.file.pull();
    },
    onSuccess: async () => {
      await repo?.branches.invalidateAll();
      await repo?.commit.invalidate();
      await repo?.status.invalidate();
    },
    onError: (error: string) => {
      toast.error(error || "Failed to pull");
    },
  });

  return mutation;
}

export function useInvalidateAll() {
  const repo = appState.repository;

  const mutation = useMutation({
    mutationFn: async () => {
      if (!repo) throw new Error("No repository selected");
      return await repo.invalidateAll();
    },
    onError: (error: string) => {
      toast.error(error);
    },
  });

  return mutation;
}

export function useHasUncommittedChanges(options?: QueryOptions<boolean>) {
  const repo = appState.repository;

  return useQuery({
    queryKey: repo?.branches.getQueryKey("hasUncommittedChanges") ?? [
      "repository",
      "none",
      "branches",
      "hasUncommittedChanges",
    ],
    queryFn: async () => {
      if (!repo) return null;
      return await repo.branches.hasUncommittedChanges();
    },
    enabled: !!repo,
    ...options,
  });
}

export function useGetCurrentBranchStash(
  options?: QueryOptions<BranchStash | null>,
) {
  const repo = appState.repository;

  return useQuery({
    queryKey: repo?.branches.getQueryKey("currentBranchStash") ?? [
      "repository",
      "none",
      "branches",
      "currentBranchStash",
    ],
    queryFn: async () => {
      if (!repo) return null;
      return await repo.branches.currentBranchStash();
    },
    enabled: !!repo,
    ...options,
  });
}

export function useGitSwitchBranch() {
  const repo = appState.repository;

  const mutation = useMutation({
    mutationFn: async ({
      branchName,
      strategy,
    }: {
      branchName: string;
      strategy?: UncommittedChangesStrategy;
    }): Promise<string> => {
      if (!repo) throw new Error("No repository selected");
      return await repo.branches.switchBranch(branchName, strategy);
    },
    onSuccess: async () => {
      await repo?.branches.invalidateAll();
      await repo?.commit.invalidate();
      await repo?.status.invalidate();
    },
  });

  return mutation;
}

export function useGitCreateBranch() {
  const repo = appState.repository;

  const mutation = useMutation({
    mutationFn: async ({
      branchName,
      strategy,
    }: {
      branchName: string;
      strategy?: UncommittedChangesStrategy;
    }): Promise<string> => {
      if (!repo) throw new Error("No repository selected");
      return await repo.branches.createBranch(branchName, strategy);
    },
    onSuccess: async () => {
      await repo?.branches.invalidateAll();
      await repo?.commit.invalidate();
      await repo?.status.invalidate();
    },
  });

  return mutation;
}

export function usePopCurrentBranchStash() {
  const repo = appState.repository;

  const mutation = useMutation({
    mutationFn: async (): Promise<string> => {
      if (!repo) throw new Error("No repository selected");
      return await repo.branches.popCurrentBranchStash();
    },
    onSuccess: async () => {
      await repo?.branches.invalidateAll();
      await repo?.commit.invalidate();
      await repo?.status.invalidate();
    },
    onError: (error: string) => {
      toast.error(error);
    },
  });

  return mutation;
}

/* #endregion  // ! Mutations */

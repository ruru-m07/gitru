import {
  type UseQueryOptions,
  useMutation,
  useQuery,
} from "@tanstack/react-query";
import { toast } from "sonner";
import { appState } from "@/state";
import type {
  Branch,
  CommitInfo,
  CreateCommitParams,
  FullCommitInfo,
  GetDiffResponse,
  GetStatusResponse,
  RepositoryOrigin,
} from "@/tauri";

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

export function useGetBranches(options?: QueryOptions<Branch[]>) {
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

export function useGetDiff(
  filePath: string | null,
  options?: QueryOptions<GetDiffResponse>,
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
      return await repo.commit.getCommitById(hash);
    },
    enabled: !!repo,
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
/* #endregion // ? Query */

/* #region // ! Mutations */
export function useCreateCommit() {
  const repo = appState.repository;

  const mutation = useMutation({
    mutationFn: async (payload: CreateCommitParams["commitMeta"]) => {
      if (!repo) throw new Error("No repository selected");
      return await repo.commit.createCommit(payload);
    },
    onSuccess: async () => {
      await repo?.status.invalidate();
      await repo?.commit.invalidate();
    },
    onError: (error: string) => {
      toast.error(error);
    },
  });

  return mutation;
}

export function useDiscardChanges() {
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

export function useAddFile() {
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

export function useUnstageFile() {
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

/* #endregion  // ! Mutations */

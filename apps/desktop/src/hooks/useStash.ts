import type {
  StashApplyParams,
  StashBranchParams,
  StashEntry,
  StashPopParams,
  StashPushParams,
  StashQuickStat,
  StashRestoreFileParams,
  StashShowResponse,
} from "@gitru/commands";
import {
  type UseQueryOptions,
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
export function useStashList(options?: QueryOptions<StashEntry[]>) {
  const repo = appState.repository;

  return useQuery({
    queryKey: repo?.stash.getQueryKey("list") ?? [
      "repository",
      "none",
      "stash",
      "list",
    ],
    queryFn: async () => {
      if (!repo) return null;
      return await repo.stash.list();
    },
    enabled: !!repo,
    ...options,
  });
}

export function useStashQuickStat(
  reference: string | null,
  options?: QueryOptions<StashQuickStat>,
) {
  const repo = appState.repository;

  return useQuery({
    queryKey: [
      ...(repo?.stash.getQueryKey("quickStat") ?? [
        "repository",
        "none",
        "stash",
        "quickStat",
      ]),
      reference,
    ],
    queryFn: async () => {
      if (!repo || !reference) return null;
      return await repo.stash.quickStat(reference);
    },
    enabled: !!repo && !!reference,
    ...options,
  });
}

export function useStashShow(
  reference: string | null,
  options?: QueryOptions<StashShowResponse>,
) {
  const repo = appState.repository;

  return useQuery({
    queryKey: [
      ...(repo?.stash.getQueryKey("show") ?? [
        "repository",
        "none",
        "stash",
        "show",
      ]),
      reference,
    ],
    queryFn: async () => {
      if (!repo || !reference) return null;
      return await repo.stash.show(reference);
    },
    enabled: !!repo && !!reference,
    ...options,
  });
}
/* #endregion // ? Query */

/* #region // ! Mutations */
export function useStashPush() {
  const repo = appState.repository;

  return useMutation({
    mutationFn: async (params?: StashPushParams) => {
      if (!repo) throw new Error("No repository selected");
      return await repo.stash.push(params);
    },
    onSuccess: async () => {
      await repo?.stash.invalidateAll();
      await repo?.status.invalidate();
    },
    onError: (error: string) => {
      toast.error(error);
    },
  });
}

export function useStashClear() {
  const repo = appState.repository;

  return useMutation({
    mutationFn: async () => {
      if (!repo) throw new Error("No repository selected");
      return await repo.stash.clear();
    },
    onSuccess: async () => {
      await repo?.stash.invalidateAll();
    },
    onError: (error: string) => {
      toast.error(error);
    },
  });
}

export function useStashPop() {
  const repo = appState.repository;

  return useMutation({
    mutationFn: async (params?: StashPopParams) => {
      if (!repo) throw new Error("No repository selected");
      return await repo.stash.pop(params);
    },
    onSuccess: async () => {
      await repo?.stash.invalidateAll();
      await repo?.status.invalidate();
    },
    onError: (error: string) => {
      toast.error(error);
    },
  });
}

export function useStashApply() {
  const repo = appState.repository;

  return useMutation({
    mutationFn: async (params?: StashApplyParams) => {
      if (!repo) throw new Error("No repository selected");
      return await repo.stash.apply(params);
    },
    onSuccess: async () => {
      await repo?.status.invalidate();
    },
    onError: (error: string) => {
      toast.error(error);
    },
  });
}

export function useStashDrop() {
  const repo = appState.repository;

  return useMutation({
    mutationFn: async (reference: string) => {
      if (!repo) throw new Error("No repository selected");
      return await repo.stash.drop(reference);
    },
    onSuccess: async () => {
      await repo?.stash.invalidateAll();
    },
    onError: (error: string) => {
      toast.error(error);
    },
  });
}

export function useStashBranch() {
  const repo = appState.repository;

  return useMutation({
    mutationFn: async (params: StashBranchParams) => {
      if (!repo) throw new Error("No repository selected");
      return await repo.stash.branch(params);
    },
    onSuccess: async () => {
      await repo?.stash.invalidateAll();
      await repo?.branches.invalidateAll();
      await repo?.status.invalidate();
    },
    onError: (error: string) => {
      toast.error(error);
    },
  });
}

export function useStashRestoreFile() {
  const repo = appState.repository;

  return useMutation({
    mutationFn: async (params: StashRestoreFileParams) => {
      if (!repo) throw new Error("No repository selected");
      return await repo.stash.restoreFile(params);
    },
    onSuccess: async () => {
      await repo?.status.invalidate();
      await repo?.stash.invalidateAll();
    },
    onError: (error: string) => {
      toast.error(error);
    },
  });
}
/* #endregion  // ! Mutations */

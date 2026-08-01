import type {
  ConflictResolveRequest,
  RebaseAbortPreview,
  RebasePlan,
  RebasePlanEntry,
  RebaseStartRequest,
  RepoOperation,
} from "@gitru/commands";
import { writeWorktreeFile } from "@gitru/commands";
import {
  type UseQueryOptions,
  useMutation,
  useQuery,
} from "@tanstack/react-query";
import { toast } from "sonner";
import { useActiveRepositoryState } from "@/state/use-active-repository-state";

type QueryOptions<T> = Omit<
  UseQueryOptions<T | null, Error>,
  "queryKey" | "queryFn" | "enabled"
>;

export function useGetRepoOperation(options?: QueryOptions<RepoOperation>) {
  const repo = useActiveRepositoryState();

  return useQuery({
    queryKey: repo?.operation.queryKey ?? ["repository", "none", "operation"],
    queryFn: async () => {
      if (!repo) return null;
      return await repo.operation.get();
    },
    enabled: !!repo,
    refetchInterval: (query) => {
      const data = query.state.data;
      if (data?.isRebasing) return 2000;
      return 5000;
    },
    ...options,
  });
}

export function useRebasePlan() {
  const repo = useActiveRepositoryState();
  return useMutation({
    mutationFn: async ({
      onto,
      upstream,
    }: {
      onto: string;
      upstream?: string;
    }): Promise<RebasePlan> => {
      if (!repo) throw new Error("No active repository");
      return repo.operation.plan(onto, upstream);
    },
  });
}

export function useRebaseStart() {
  const repo = useActiveRepositoryState();
  return useMutation({
    mutationFn: async (request: RebaseStartRequest) => {
      if (!repo) throw new Error("No active repository");
      return repo.operation.start(request);
    },
    onSuccess: async () => {
      await repo?.status.invalidate();
      await repo?.branches.invalidate();
      await repo?.operation.invalidate();
    },
    onError: (error: Error) => {
      toast.error(error.message || "Failed to start rebase");
    },
  });
}

export function useRebaseContinue() {
  const repo = useActiveRepositoryState();
  return useMutation({
    mutationFn: async (message?: string) => {
      if (!repo) throw new Error("No active repository");
      return repo.operation.continue(message);
    },
    onSuccess: async () => {
      await repo?.invalidateAll();
    },
    onError: (error: Error) => {
      toast.error(error.message || "Failed to continue rebase");
    },
  });
}

export function useRebaseSkip() {
  const repo = useActiveRepositoryState();
  return useMutation({
    mutationFn: async () => {
      if (!repo) throw new Error("No active repository");
      return repo.operation.skip();
    },
    onSuccess: async () => {
      await repo?.invalidateAll();
    },
    onError: (error: Error) => {
      toast.error(error.message || "Failed to skip rebase step");
    },
  });
}

export function useRebaseAbort() {
  const repo = useActiveRepositoryState();
  return useMutation({
    mutationFn: async () => {
      if (!repo) throw new Error("No active repository");
      return repo.operation.abort();
    },
    onSuccess: async () => {
      await repo?.invalidateAll();
      toast.success("Rebase aborted");
    },
    onError: (error: Error) => {
      toast.error(error.message || "Failed to abort rebase");
    },
  });
}

export function useRebaseAbortPreview() {
  const repo = useActiveRepositoryState();
  return useMutation({
    mutationFn: async (): Promise<RebaseAbortPreview> => {
      if (!repo) throw new Error("No active repository");
      return repo.operation.abortPreview();
    },
  });
}

export function useRebaseUpdateTodo() {
  const repo = useActiveRepositoryState();
  return useMutation({
    mutationFn: async (entries: RebasePlanEntry[]) => {
      if (!repo) throw new Error("No active repository");
      return repo.operation.updateTodo(entries);
    },
    onSuccess: async () => {
      await repo?.operation.invalidate();
    },
    onError: (error: Error) => {
      toast.error(error.message || "Failed to update rebase todo");
    },
  });
}

export function useRebaseResolveConflict() {
  const repo = useActiveRepositoryState();
  return useMutation({
    mutationFn: async (request: ConflictResolveRequest) => {
      if (!repo) throw new Error("No active repository");
      return repo.operation.resolveConflict(request);
    },
    onSuccess: async () => {
      await repo?.status.invalidate();
      await repo?.operation.invalidate();
    },
    onError: (error: Error) => {
      toast.error(error.message || "Failed to resolve conflict");
    },
  });
}

export function useWriteWorktreeFile() {
  const repo = useActiveRepositoryState();
  return useMutation({
    mutationFn: async ({
      path,
      contents,
    }: {
      path: string;
      contents: string;
    }) => {
      if (!repo) throw new Error("No active repository");
      await writeWorktreeFile({
        contextId: repo.contextId,
        path,
        contents,
      });
    },
  });
}

import { useCallback, useEffect, useState } from "react";
import type { FileStatus } from "@/tauri/types";
import type { GitRepository } from "./GitRepository";
import { gitRepositoryManager } from "./GitRepositoryManager";

/**
 * Hook to get a Git repository instance
 * Automatically manages lifecycle and watching
 */
export function useGitRepository(
  path: string | null,
  name: string = "Repository",
) {
  const [repo, setRepo] = useState<GitRepository | null>(null);

  useEffect(() => {
    if (!path) {
      setRepo(null);
      return;
    }

    const repository = gitRepositoryManager.getRepository(path, name);
    setRepo(repository);

    // Start watching the repository
    repository.startWatching().catch(console.error);

    return () => {
      // Don't dispose here - let the manager handle it
      // This allows the same repo to be used across components
    };
  }, [path, name]);

  return repo;
}

/**
 * Hook to get repository status with automatic updates
 */
export function useRepositoryStatus(repo: GitRepository | null) {
  const [status, setStatus] = useState<FileStatus[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<Error | null>(null);

  // Load status
  const loadStatus = useCallback(
    async (force = false) => {
      if (!repo) return;

      setLoading(true);
      setError(null);

      try {
        const files = await repo.getStatus({ force });
        setStatus(files);
      } catch (err) {
        setError(err as Error);
        console.error("Failed to load status:", err);
      } finally {
        setLoading(false);
      }
    },
    [repo],
  );

  // Subscribe to status changes
  useEffect(() => {
    if (!repo) {
      setStatus([]);
      return;
    }

    // Load initial status
    loadStatus();

    // Subscribe to changes
    const unsubscribe = repo.on("status-changed", () => {
      loadStatus();
    });

    return unsubscribe;
  }, [repo, loadStatus]);

  const refresh = useCallback(() => {
    loadStatus(true);
  }, [loadStatus]);

  return {
    status,
    loading,
    error,
    refresh,
  };
}

/**
 * Hook to get diff for a specific file with automatic updates
 */
export function useFileDiff(
  repo: GitRepository | null,
  filePath: string | null,
) {
  const [diff, setDiff] = useState<string>("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<Error | null>(null);

  // Load diff
  const loadDiff = useCallback(
    async (force = false) => {
      if (!repo || !filePath) return;

      setLoading(true);
      setError(null);

      try {
        const diffContent = await repo.getDiff(filePath, { force });
        setDiff(diffContent);
      } catch (err) {
        setError(err as Error);
        console.error("Failed to load diff:", err);
      } finally {
        setLoading(false);
      }
    },
    [repo, filePath],
  );

  // Subscribe to diff changes for this specific file
  useEffect(() => {
    if (!repo || !filePath) {
      setDiff("");
      return;
    }

    // Load initial diff
    loadDiff();

    // Subscribe to changes
    const unsubscribe = repo.on("diff-changed", (data: any) => {
      // Only reload if this file changed or all diffs were invalidated
      if (!data || data.filePath === filePath) {
        loadDiff();
      }
    });

    return unsubscribe;
  }, [repo, filePath, loadDiff]);

  const refresh = useCallback(() => {
    loadDiff(true);
  }, [loadDiff]);

  return {
    diff,
    loading,
    error,
    refresh,
  };
}

/**
 * Hook for Git operations with the repository
 */
export function useGitOperations(repo: GitRepository | null) {
  const add = useCallback(
    async (filePath: string) => {
      if (!repo) return false;
      return repo.add(filePath);
    },
    [repo],
  );

  const unstage = useCallback(
    async (filePath: string) => {
      if (!repo) return false;
      return repo.unstage(filePath);
    },
    [repo],
  );

  const discard = useCallback(
    async (filePath: string) => {
      if (!repo) return false;
      return repo.discard(filePath);
    },
    [repo],
  );

  const commit = useCallback(
    async (message: string, description?: string) => {
      if (!repo) return false;
      return repo.commit(message, description);
    },
    [repo],
  );

  const addAll = useCallback(async () => {
    if (!repo) return false;
    return repo.addAll();
  }, [repo]);

  const removeAll = useCallback(async () => {
    if (!repo) return false;
    return repo.removeAll();
  }, [repo]);

  return {
    add,
    unstage,
    discard,
    commit,
    addAll,
    removeAll,
  };
}

/**
 * All-in-one hook that combines repository, status, and operations
 * Perfect for page-level components
 */
export function useGit(path: string | null, name?: string) {
  const repo = useGitRepository(path, name);
  const {
    status,
    loading: statusLoading,
    error: statusError,
    refresh: refreshStatus,
  } = useRepositoryStatus(repo);
  const operations = useGitOperations(repo);

  // Separate staged and unstaged changes
  const stagedChanges = status.filter((file) =>
    file.status.some((s) => s.startsWith("Index")),
  );
  const unstagedChanges = status.filter((file) =>
    file.status.some((s) => s.startsWith("Worktree")),
  );

  // Subscribe to commit events
  const [lastCommit, setLastCommit] = useState<any>(null);
  useEffect(() => {
    if (!repo) return;

    const unsubscribe = repo.on("commit", (data) => {
      setLastCommit(data);
    });

    return unsubscribe;
  }, [repo]);

  // Subscribe to errors
  const [lastError, setLastError] = useState<any>(null);
  useEffect(() => {
    if (!repo) return;

    const unsubscribe = repo.on("error", (data) => {
      setLastError(data);
    });

    return unsubscribe;
  }, [repo]);

  return {
    repo,
    status: {
      stagedChanges,
      unstagedChanges,
      all: status,
    },
    loading: statusLoading,
    error: statusError,
    lastCommit,
    lastError,
    operations,
    refresh: refreshStatus,
  };
}

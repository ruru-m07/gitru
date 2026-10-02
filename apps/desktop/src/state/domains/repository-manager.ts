import type { QueryClient } from "@tanstack/react-query";
import { queryClient } from "../core/state-manager";
import { RepositoryState } from "./repository-state";

export class RepositoryManager {
  private instances = new Map<string, RepositoryState>();

  constructor(private readonly client: QueryClient = queryClient) {}

  for(repoPath: string, contextId: string): RepositoryState {
    const normalizedPath = this.normalizePath(repoPath);
    const key = this.getKey(normalizedPath, contextId);

    if (!this.instances.has(key)) {
      this.instances.set(
        key,
        new RepositoryState(this.client, normalizedPath, contextId),
      );
    }

    return this.instances.get(key)!;
  }

  /**
   * Clear a specific repository from state cache
   */
  async dispose(repoPath: string, contextId: string): Promise<void> {
    const normalizedPath = this.normalizePath(repoPath);
    const key = this.getKey(normalizedPath, contextId);
    const queryKey = ["repository", contextId, normalizedPath];
    const worktreeFileKey = ["worktree-file", contextId];

    this.instances.delete(key);
    await Promise.all([
      this.client.cancelQueries({ queryKey }),
      this.client.cancelQueries({ queryKey: worktreeFileKey }),
    ]);
    this.client.removeQueries({ queryKey });
    this.client.removeQueries({ queryKey: worktreeFileKey });
  }

  /**
   * Remove every cached query and state facade owned by a native context.
   * Cancelling before removal prevents a disposing tab from refetching.
   */
  async disposeContext(contextId: string): Promise<void> {
    for (const [key, instance] of this.instances) {
      if (instance.contextId === contextId) {
        this.instances.delete(key);
      }
    }

    const queryKey = ["repository", contextId];
    const worktreeFileKey = ["worktree-file", contextId];
    await Promise.all([
      this.client.cancelQueries({ queryKey }),
      this.client.cancelQueries({ queryKey: worktreeFileKey }),
    ]);
    this.client.removeQueries({ queryKey });
    this.client.removeQueries({ queryKey: worktreeFileKey });
  }

  /**
   * Clear all repository state instances
   */
  async disposeAll(): Promise<void> {
    this.instances.clear();
    const queryKey = ["repository"];
    const worktreeFileKey = ["worktree-file"];
    await Promise.all([
      this.client.cancelQueries({ queryKey }),
      this.client.cancelQueries({ queryKey: worktreeFileKey }),
    ]);
    this.client.removeQueries({ queryKey });
    this.client.removeQueries({ queryKey: worktreeFileKey });
  }

  private getKey(repoPath: string, contextId: string): string {
    return `${contextId}::${repoPath}`;
  }

  /**
   * Normalize repository path for consistent keys
   * @example
   * "C:\\Repos\\MyRepo\\" => "C:/Repos/MyRepo"
   * "/home/user/repo//" => "/home/user/repo"
   */
  private normalizePath(path: string): string {
    return path.replace(/\\/g, "/").replace(/\/+$/, "");
  }
}

export const repositories = new RepositoryManager();

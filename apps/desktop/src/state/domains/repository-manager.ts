import { queryClient } from "../core/state-manager";
import { RepositoryState } from "./repository-state";

class RepositoryManager {
  private instances = new Map<string, RepositoryState>();

  for(repoPath: string, contextId: string): RepositoryState {
    const normalizedPath = this.normalizePath(repoPath);
    const key = this.getKey(normalizedPath, contextId);

    if (!this.instances.has(key)) {
      this.instances.set(
        key,
        new RepositoryState(queryClient, normalizedPath, contextId),
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
    const instance = this.instances.get(key);

    if (instance) {
      await instance.invalidateAll();
      this.instances.delete(key);
    }
  }

  /**
   * Clear all repository state instances
   */
  async disposeAll(): Promise<void> {
    for (const instance of this.instances.values()) {
      await instance.invalidateAll();
    }
    this.instances.clear();
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

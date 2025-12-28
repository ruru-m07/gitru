import { useAppStore } from "@/store/useAppStore";
import type { RepoSitoryStore } from "@/tauri/types";
import { queryClient } from "../core/StateManager";
import { RepositoryState } from "./RepositoryState";

class RepositoryManager {
  private instances = new Map<string, RepositoryState>();
  private _current: RepositoryState | null = null;
  private _unsubscribe: (() => void) | null = null;

  constructor() {
    this.initializeSync();
  }

  private initializeSync() {
    this._unsubscribe = useAppStore.subscribe(
      (state) => state.selectedRepository,
      (selectedRepo, previousRepo) => {
        if (selectedRepo?.path !== previousRepo?.path) {
          this.syncFromStore(selectedRepo);
        }
      },
      { fireImmediately: false },
    );

    const initialRepo = useAppStore.getState().selectedRepository;
    if (initialRepo) {
      this.syncFromStore(initialRepo);
    }
  }

  private syncFromStore(repo: RepoSitoryStore | null) {
    if (repo?.path) {
      this._current = this.for(repo.path);
    } else {
      this._current = null;
    }
  }

  for(repoPath: string): RepositoryState {
    const normalizedPath = this.normalizePath(repoPath);

    if (!this.instances.has(normalizedPath)) {
      this.instances.set(
        normalizedPath,
        new RepositoryState(queryClient, normalizedPath),
      );
    }

    return this.instances.get(normalizedPath)!;
  }

  /**
   * Get the current active repository
   * Returns **null** if **no** repository is selected
   */
  get current(): RepositoryState | null {
    return this._current;
  }

  /**
   * Clear a specific repository from state cache
   */
  async dispose(repoPath: string): Promise<void> {
    const normalizedPath = this.normalizePath(repoPath);
    const instance = this.instances.get(normalizedPath);

    if (instance) {
      await instance.invalidateAll();
      this.instances.delete(normalizedPath);

      if (this._current?.path === normalizedPath) {
        this._current = null;
      }
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
    this._current = null;
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

  destroy() {
    if (this._unsubscribe) {
      this._unsubscribe();
      this._unsubscribe = null;
    }
  }
}

export const repositories = new RepositoryManager();

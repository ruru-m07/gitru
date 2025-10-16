import { invoke } from '@tauri-apps/api/core';
import type { FileStatus } from '@/tauri/types';
import { fileWatcherService } from '@/services/fileWatcher';
import { commit, getDiff, getStatus, gitAdd, gitDiscard, gitRemove } from '@/tauri';

/**
 * Event emitter for repository changes
 */
type RepositoryEventType = 'status-changed' | 'diff-changed' | 'commit' | 'error';
type EventCallback = (data?: unknown) => void;

/**
 * OOP-based Git Repository Manager
 * 
 * Handles all Git operations for a single repository with:
 * - Automatic file watching and cache invalidation
 * - Event-based notifications for UI updates
 * - Optimistic updates for better UX
 * - Single source of truth for repository state
 */
export class GitRepository {
  private path: string;
  private name: string;
  private statusCache: FileStatus[] | null = null;
  private diffCache: Map<string, string> = new Map();
  private isWatching = false;
  private eventListeners: Map<RepositoryEventType, Set<EventCallback>> = new Map();
  private statusPromise: Promise<FileStatus[]> | null = null;
  private diffPromises: Map<string, Promise<string>> = new Map();
  private unsubscribeWatcher: (() => void) | null = null;

  constructor(path: string, name: string) {
    this.path = path;
    this.name = name;
    this.initializeEventListeners();
  }

  private initializeEventListeners() {
    // Initialize event listener sets
    this.eventListeners.set('status-changed', new Set());
    this.eventListeners.set('diff-changed', new Set());
    this.eventListeners.set('commit', new Set());
    this.eventListeners.set('error', new Set());
  }

  /**
   * Get repository path
   */
  getPath(): string {
    return this.path;
  }

  /**
   * Get repository name
   */
  getName(): string {
    return this.name;
  }

  /**
   * Start watching the repository for file changes
   */
  async startWatching(): Promise<void> {
    if (this.isWatching) {
      console.log(`Repository ${this.name} is already watching`);
      return;
    }

    try {
      // Check if watcher service is already active for this path
      const { useFileWatcherStore } = await import('@/store/useFileWatcherStore');
      const currentRepoPath = useFileWatcherStore.getState().repoPath;
      
      // If already watching the same path, just subscribe to changes
      if (currentRepoPath === this.path && useFileWatcherStore.getState().isActive) {
        console.log(`Watcher already active for ${this.path}, subscribing to changes`);
        this.subscribeToFileChanges();
        this.isWatching = true;
        return;
      }

      // Start new watcher
      await fileWatcherService.startWatching(this.path, {
        debounce_ms: 100,
        batch_size: 50,
      });

      // Subscribe to file changes
      this.subscribeToFileChanges();
      this.isWatching = true;
    } catch (error) {
      console.error('Failed to start watching repository:', error);
      this.emit('error', { type: 'watch-failed', error });
    }
  }

  /**
   * Stop watching the repository
   */
  async stopWatching(): Promise<void> {
    if (!this.isWatching) return;

    try {
      // Unsubscribe from file watcher store
      if (this.unsubscribeWatcher) {
        this.unsubscribeWatcher();
        this.unsubscribeWatcher = null;
      }

      // Don't stop the watcher service here - let the manager or service handle it
      // This prevents stopping the watcher if other repositories are still using it
      this.isWatching = false;
      
      console.log(`Stopped watching repository: ${this.name}`);
    } catch (error) {
      console.error('Failed to stop watching repository:', error);
    }
  }

  /**
   * Subscribe to file change events from watcher
   */
  private subscribeToFileChanges() {
    // Unsubscribe from previous subscription if exists
    if (this.unsubscribeWatcher) {
      console.log('[GitRepository] Already subscribed, unsubscribing first');
      this.unsubscribeWatcher();
      this.unsubscribeWatcher = null;
    }

    // Import the store dynamically to avoid circular dependencies
    import('@/store/useFileWatcherStore').then(({ useFileWatcherStore }) => {
      // Double-check we haven't subscribed in the meantime
      if (this.unsubscribeWatcher) {
        console.log('[GitRepository] Race condition detected, already subscribed');
        return;
      }

      console.log(`[GitRepository] Subscribing to file changes for ${this.name}`);
      
      // Subscribe to Zustand store changes (not Immer state)
      this.unsubscribeWatcher = useFileWatcherStore.subscribe((state, prevState) => {
        // Only invalidate if files actually changed
        if (state.files !== prevState.files) {
          console.log(`[GitRepository:${this.name}] Files changed, invalidating caches`);
          
          // Always invalidate status when files change
          this.invalidateStatus();
          
          // Invalidate specific file diffs that changed
          const changedPaths = new Set<string>();
          
          // Check which files were added/modified
          state.files.forEach((file, path) => {
            const prevFile = prevState.files.get(path);
            if (!prevFile || prevFile.lastModified !== file.lastModified) {
              changedPaths.add(path);
            }
          });
          
          // Check which files were deleted
          prevState.files.forEach((_file, path) => {
            if (!state.files.has(path)) {
              changedPaths.add(path);
            }
          });
          
          // Invalidate diffs for changed files
          if (changedPaths.size > 0) {
            console.log(`[GitRepository:${this.name}] Invalidating diffs for ${changedPaths.size} files`);
            changedPaths.forEach(path => {
              this.invalidateDiffs(path);
            });
          }
        }
      });
    });
  }

  /**
   * Subscribe to repository events
   */
  on(event: RepositoryEventType, callback: EventCallback): () => void {
    const listeners = this.eventListeners.get(event);
    if (listeners) {
      listeners.add(callback);
    }

    // Return unsubscribe function
    return () => {
      const listeners = this.eventListeners.get(event);
      if (listeners) {
        listeners.delete(callback);
      }
    };
  }

  /**
   * Emit an event to all listeners
   */
  private emit(event: RepositoryEventType, data?: unknown) {
    const listeners = this.eventListeners.get(event);
    if (listeners) {
      listeners.forEach(callback => callback(data));
    }
  }

  /**
   * Get repository status (cached)
   * Automatically invalidates when files change
   */
  async getStatus(options?: { force?: boolean }): Promise<FileStatus[]> {
    // Return cached status if available and not forced
    if (!options?.force && this.statusCache) {
      return this.statusCache;
    }

    // Deduplicate concurrent requests
    if (this.statusPromise) {
      return this.statusPromise;
    }

    this.statusPromise = this.fetchStatus();
    
    try {
      const status = await this.statusPromise;
      this.statusCache = status;
      this.emit('status-changed', status);
      return status;
    } finally {
      this.statusPromise = null;
    }
  }

  /**
   * Fetch status from backend
   */
  private async fetchStatus(): Promise<FileStatus[]> {
    try {
      const result = await getStatus({
        repo_path: this.path,
      });
      return result.files;
    } catch (error) {
      console.error('Failed to get status:', error);
      this.emit('error', { type: 'status-failed', error });
      return [];
    }
  }

  /**
   * Invalidate status cache
   */
  invalidateStatus() {
    this.statusCache = null;
    this.emit('status-changed', null);
  }

  /**
   * Get diff for a specific file (cached)
   */
  async getDiff(filePath: string, options?: { force?: boolean }): Promise<string> {
    // Return cached diff if available and not forced
    if (!options?.force && this.diffCache.has(filePath)) {
      return this.diffCache.get(filePath)!;
    }

    // Deduplicate concurrent requests
    if (this.diffPromises.has(filePath)) {
      return this.diffPromises.get(filePath)!;
    }

    const promise = this.fetchDiff(filePath);
    this.diffPromises.set(filePath, promise);

    try {
      const diff = await promise;
      this.diffCache.set(filePath, diff);
      this.emit('diff-changed', { filePath, diff });
      return diff;
    } finally {
      this.diffPromises.delete(filePath);
    }
  }

  /**
   * Fetch diff from backend
   */
  private async fetchDiff(filePath: string): Promise<string> {
    try {
      const result = await getDiff({
        repo_path: this.path,
        file_path: filePath,
      });
      
      // Handle binary files
      if (result.head?.is_binary || result.workdir?.is_binary) {
        return 'Binary file (not shown)';
      }
      
      // Return the actual diff content
      // The backend returns { head?: {content}, workdir?: {content} }
      // For now, just return the workdir content or an empty string
      // You may want to implement a proper diff algorithm later
      if (result.workdir?.content) {
        return result.workdir.content;
      }
      
      if (result.head?.content) {
        return result.head.content;
      }
      
      return '';
    } catch (error) {
      console.error('Failed to get diff:', error);
      this.emit('error', { type: 'diff-failed', error, filePath });
      return '';
    }
  }

  /**
   * Invalidate diff cache for a specific file or all files
   */
  invalidateDiffs(filePath?: string) {
    if (filePath) {
      this.diffCache.delete(filePath);
      this.emit('diff-changed', { filePath });
    } else {
      this.diffCache.clear();
      this.emit('diff-changed', null);
    }
  }

  /**
   * Stage a file (git add)
   */
  async add(filePath: string): Promise<boolean> {
    try {
      const result = await gitAdd({
        repo_path: this.path,
        file: filePath,
      });

      if (result.success) {
        // Invalidate status to trigger refresh
        this.invalidateStatus();
        return true;
      } else {
        this.emit('error', { type: 'add-failed', message: result.message });
        return false;
      }
    } catch (error) {
      console.error('Failed to add file:', error);
      this.emit('error', { type: 'add-failed', error });
      return false;
    }
  }

  /**
   * Unstage a file (git reset)
   */
  async unstage(filePath: string): Promise<boolean> {
    try {
      
      const result = await gitRemove({
        repo_path: this.path,
        file: filePath,
      });

      if (result.success) {
        this.invalidateStatus();
        return true;
      } else {
        this.emit('error', { type: 'unstage-failed', message: result.message });
        return false;
      }
    } catch (error) {
      console.error('Failed to unstage file:', error);
      this.emit('error', { type: 'unstage-failed', error });
      return false;
    }
  }

  /**
   * Discard changes in a file (git checkout/restore)
   */
  async discard(filePath: string): Promise<boolean> {
    try {
      
      const result = await gitDiscard({
        repo_path: this.path,
        file: filePath,
      });

      if (result.success) {
        this.invalidateStatus();
        this.invalidateDiffs();
        return true;
      } else {
        this.emit('error', { type: 'discard-failed', message: result.message });
        return false;
      }
    } catch (error) {
      console.error('Failed to discard changes:', error);
      this.emit('error', { type: 'discard-failed', error });
      return false;
    }
  }

  /**
   * Commit staged changes
   */
  async commit(message: string, description?: string): Promise<boolean> {
    try {
      const fullMessage = description ? `${message}\n\n${description}` : message;
      
      const result = await commit(
        {
          repo_path: this.path,
          message: fullMessage,
        }
      );

      if (result.success) {
        this.invalidateStatus();
        this.invalidateDiffs();
        this.emit('commit', { message });
        return true;
      } else {
        this.emit('error', { type: 'commit-failed', message: result.message });
        return false;
      }
    } catch (error) {
      console.error('Failed to commit:', error);
      this.emit('error', { type: 'commit-failed', error });
      return false;
    }
  }

  /**
   * Stage all changes
   */
  async addAll(): Promise<boolean> {
    try {
      const result = await gitAdd({
        repo_path: this.path,
        file: '.'
      });

      if (result.success) {
        this.invalidateStatus();
        return true;
      } else {
        this.emit('error', { type: 'add-all-failed', message: result.message });
        return false;
      }
    } catch (error) {
      console.error('Failed to add all:', error);
      this.emit('error', { type: 'add-all-failed', error });
      return false;
    }
  }

  async removeAll(): Promise<boolean> {
    try {
      const result = await gitRemove({
        repo_path: this.path,
        file: '.'
      });

      if (result.success) {
        this.invalidateStatus();
        return true;  
      } else {
        this.emit('error', { type: 'remove-all-failed', message: result.message });
        return false;
      }
    } catch (error) {
      console.error('Failed to remove all:', error);
      this.emit('error', { type: 'remove-all-failed', error });
      return false;
    }
  }

  /**
   * Get current branch name
   */
  async getCurrentBranch(): Promise<string | null> {
    try {
      const result = await invoke<{ branch: string }>('git_current_branch', {
        repoPath: this.path,
      });
      return result.branch;
    } catch (error) {
      console.error('Failed to get current branch:', error);
      this.emit('error', { type: 'branch-failed', error });
      return null;
    }
  }

  /**
   * Get commit history
   */
  async getHistory(options?: { limit?: number; skip?: number }): Promise<any[]> {
    try {
      const result = await invoke<{ commits: any[] }>('git_history', {
        repoPath: this.path,
        limit: options?.limit || 50,
        skip: options?.skip || 0,
      });
      return result.commits;
    } catch (error) {
      console.error('Failed to get history:', error);
      this.emit('error', { type: 'history-failed', error });
      return [];
    }
  }

  /**
   * Clean up resources
   */
  async dispose() {
    await this.stopWatching();
    this.eventListeners.clear();
    this.statusCache = null;
    this.diffCache.clear();
  }
}

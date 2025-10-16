import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';
import { useFileWatcherStore } from '@/store/useFileWatcherStore';
import type {
  BatchUpdate,
  WatcherConfig,
  WatcherError,
  WatcherStateChanged,
  WatcherStats,
  StartWatchingResponse,
  StopWatchingResponse,
} from '@/types/watcher';

/**
 * Service for managing file watcher integration with Tauri backend
 */
export class FileWatcherService {
  private unlisteners: UnlistenFn[] = [];
  private isListening = false;
  private startingWatcher: Promise<StartWatchingResponse> | null = null;

  /**
   * Start watching a repository
   */
  async startWatching(
    repoPath: string,
    config?: WatcherConfig
  ): Promise<StartWatchingResponse> {
    // If already starting a watcher, wait for it
    if (this.startingWatcher) {
      console.log('[FileWatcher] Already starting watcher, waiting...');
      return this.startingWatcher;
    }

    const store = useFileWatcherStore.getState();

    // If already watching the same path, return success
    if (store.watcherId && store.repoPath === repoPath && store.isActive) {
      console.log(`[FileWatcher] Already watching ${repoPath}`);
      return {
        success: true,
        watcher_id: store.watcherId,
      };
    }

    // Stop any existing watcher for different path
    if (store.watcherId && store.repoPath !== repoPath) {
      await this.stopWatching();
    }

    try {
      this.startingWatcher = this.doStartWatching(repoPath, config);
      const result = await this.startingWatcher;
      return result;
    } finally {
      this.startingWatcher = null;
    }
  }

  /**
   * Internal method to actually start watching
   */
  private async doStartWatching(
    repoPath: string,
    config?: WatcherConfig
  ): Promise<StartWatchingResponse> {
    const store = useFileWatcherStore.getState();

    try {
      // Call Tauri command
      const response = await invoke<StartWatchingResponse>('start_watching', {
        repoPath,
        config,
      });

      if (response.success && response.watcher_id) {
        // Update store
        store.setWatcherId(response.watcher_id);
        store.setRepoPath(repoPath);

        // Setup event listeners
        await this.setupEventListeners();

        console.log(`Started watching ${repoPath} with ID ${response.watcher_id}`);
        return response;
      } else {
        throw new Error(response.error || 'Failed to start watching');
      }
    } catch (error) {
      console.error('Failed to start watching:', error);
      throw error;
    }
  }

  /**
   * Stop watching the current repository
   */
  async stopWatching(): Promise<void> {
    const store = useFileWatcherStore.getState();

    if (!store.watcherId) {
      return;
    }

    try {
      // Call Tauri command
      await invoke<StopWatchingResponse>('stop_watching', {
        watcherId: store.watcherId,
      });

      // Cleanup event listeners
      this.cleanup();

      // Clear store
      store.clear();

      console.log('Stopped watching');
    } catch (error) {
      console.error('Failed to stop watching:', error);
      throw error;
    }
  }

  /**
   * Setup event listeners for file watcher events
   */
  private async setupEventListeners(): Promise<void> {
    if (this.isListening) {
      return;
    }

    const store = useFileWatcherStore.getState();

    try {
      // Listen for batch updates (primary event)
      const unlistenBatch = await listen<BatchUpdate>(
        'watcher:batch-update',
        (event) => {
          console.log(
            `Received batch update: ${event.payload.events.length} events`
          );
          store.applyBatchUpdate(event.payload.events);
        }
      );
      this.unlisteners.push(unlistenBatch);

      // Listen for errors
      const unlistenError = await listen<WatcherError>(
        'watcher:error',
        (event) => {
          console.error('Watcher error:', event.payload);
          store.handleError(event.payload);
        }
      );
      this.unlisteners.push(unlistenError);

      // Listen for state changes
      const unlistenState = await listen<WatcherStateChanged>(
        'watcher:state-changed',
        (event) => {
          console.log(
            `Watcher state changed: ${event.payload.old_state} -> ${event.payload.new_state}`
          );
          store.setState(event.payload.new_state);
        }
      );
      this.unlisteners.push(unlistenState);

      // Listen for statistics (if enabled)
      const unlistenStats = await listen<WatcherStats>(
        'watcher:stats',
        (event) => {
          console.log('Watcher stats:', event.payload);
          store.updateStats(event.payload);
        }
      );
      this.unlisteners.push(unlistenStats);

      this.isListening = true;
      console.log('Event listeners setup complete');
    } catch (error) {
      console.error('Failed to setup event listeners:', error);
      this.cleanup();
      throw error;
    }
  }

  /**
   * Cleanup event listeners
   */
  private cleanup(): void {
    for (const unlisten of this.unlisteners) {
      unlisten();
    }
    this.unlisteners = [];
    this.isListening = false;
  }

  /**
   * Force a rescan of the repository
   */
  async rescan(fullRescan = false): Promise<void> {
    const store = useFileWatcherStore.getState();

    if (!store.watcherId) {
      throw new Error('No active watcher');
    }

    try {
      await invoke('rescan_repository', {
        watcherId: store.watcherId,
        fullRescan,
      });
      console.log('Repository rescan triggered');
    } catch (error) {
      console.error('Failed to rescan repository:', error);
      throw error;
    }
  }

  /**
   * Get current watcher state from backend
   */
  async getWatcherState(): Promise<any> {
    const store = useFileWatcherStore.getState();

    if (!store.watcherId) {
      throw new Error('No active watcher');
    }

    try {
      return await invoke('get_watcher_state', {
        watcherId: store.watcherId,
      });
    } catch (error) {
      console.error('Failed to get watcher state:', error);
      throw error;
    }
  }

  /**
   * Check if service is currently listening
   */
  isActive(): boolean {
    return this.isListening;
  }
}

// Singleton instance
export const fileWatcherService = new FileWatcherService();

// React hook for easy access
export function useFileWatcher() {
  return {
    startWatching: (repoPath: string, config?: WatcherConfig) =>
      fileWatcherService.startWatching(repoPath, config),
    stopWatching: () => fileWatcherService.stopWatching(),
    rescan: (fullRescan?: boolean) => fileWatcherService.rescan(fullRescan),
    getState: () => fileWatcherService.getWatcherState(),
    isActive: fileWatcherService.isActive(),
  };
}

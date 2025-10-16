import { create } from 'zustand';
import { immer } from 'zustand/middleware/immer';
import { enableMapSet } from 'immer';
import type {
  FileEvent,
  FileEventType,
  WatcherError,
  WatcherStats,
} from '@/types/watcher';
import { WatcherState } from '@/types/watcher';

// File state representation
export interface FileState {
  path: string;
  absolutePath: string;
  lastModified: string;
  metadata?: {
    size?: number;
    isDirectory: boolean;
  };
  eventType: FileEventType;
}

// Store state
export interface FileWatcherState {
  // Core state
  watcherId: string | null;
  repoPath: string | null;
  state: WatcherState;
  isActive: boolean;

  // File tracking (single-ref data structure)
  // Map of relative path -> FileState
  files: Map<string, FileState>;

  // Statistics
  stats: WatcherStats | null;
  lastError: WatcherError | null;

  // Event history (limited size for debugging)
  recentEvents: FileEvent[];
  maxRecentEvents: number;

  // Subscription management
  subscribers: Set<() => void>;
  pathSubscribers: Map<string, Set<() => void>>;

  // Actions
  setWatcherId: (id: string | null) => void;
  setRepoPath: (path: string | null) => void;
  setState: (state: WatcherState) => void;
  applyBatchUpdate: (events: FileEvent[]) => void;
  handleError: (error: WatcherError) => void;
  updateStats: (stats: WatcherStats) => void;
  clear: () => void;

  // Subscription API
  subscribe: (callback: () => void) => () => void;
  subscribePath: (path: string, callback: () => void) => () => void;
  notifySubscribers: () => void;
  notifyPathSubscribers: (path: string) => void;

  // Query API
  getFile: (path: string) => FileState | undefined;
  getFiles: () => FileState[];
  getFilesByType: (eventType: FileEventType) => FileState[];
  getFilesInDirectory: (dirPath: string) => FileState[];
  hasFile: (path: string) => boolean;
  getFileCount: () => number;
}

// Create the store with Immer for immutability
export const useFileWatcherStore = create<FileWatcherState>()(
  immer((set, get) => ({
    // Initial state
    watcherId: null,
    repoPath: null,
    state: WatcherState.Stopped,
    isActive: false,
    files: new Map(),
    stats: null,
    lastError: null,
    recentEvents: [],
    maxRecentEvents: 100,
    subscribers: new Set(),
    pathSubscribers: new Map(),

    // Actions
    setWatcherId: (id) =>
      set((state) => {
        state.watcherId = id;
        state.isActive = id !== null;
      }),

    setRepoPath: (path) =>
      set((state) => {
        state.repoPath = path;
      }),

    setState: (watcherState) =>
      set((state) => {
        state.state = watcherState;
        state.isActive = watcherState === WatcherState.Active;
      }),

    applyBatchUpdate: (events) => {
      set((state) => {
        for (const event of events) {
          // Update file state based on event type
          switch (event.event_type) {
            case 'created':
            case 'modified':
              state.files.set(event.path, {
                path: event.path,
                absolutePath: event.absolute_path,
                lastModified: event.timestamp,
                metadata: event.metadata
                  ? {
                      size: event.metadata.size,
                      isDirectory: event.metadata.is_directory,
                    }
                  : undefined,
                eventType: event.event_type,
              });
              break;

            case 'deleted':
              state.files.delete(event.path);
              break;

            case 'renamed':
              // Remove old path and add new path
              if (event.metadata?.old_path) {
                state.files.delete(event.metadata.old_path);
              }
              state.files.set(event.path, {
                path: event.path,
                absolutePath: event.absolute_path,
                lastModified: event.timestamp,
                metadata: event.metadata
                  ? {
                      size: event.metadata.size,
                      isDirectory: event.metadata.is_directory,
                    }
                  : undefined,
                eventType: event.event_type,
              });
              break;
          }

          // Add to recent events
          state.recentEvents.unshift(event);
          if (state.recentEvents.length > state.maxRecentEvents) {
            state.recentEvents = state.recentEvents.slice(0, state.maxRecentEvents);
          }

          // Notify path-specific subscribers
          get().notifyPathSubscribers(event.path);
          if (event.metadata?.old_path) {
            get().notifyPathSubscribers(event.metadata.old_path);
          }
        }
      });

      // Notify global subscribers
      get().notifySubscribers();
    },

    handleError: (error) =>
      set((state) => {
        state.lastError = error;
        if (!error.recoverable) {
          state.state = WatcherState.Error;
          state.isActive = false;
        }
      }),

    updateStats: (stats) =>
      set((state) => {
        state.stats = stats;
      }),

    clear: () =>
      set((state) => {
        state.watcherId = null;
        state.repoPath = null;
        state.state = WatcherState.Stopped;
        state.isActive = false;
        state.files.clear();
        state.stats = null;
        state.lastError = null;
        state.recentEvents = [];
        state.subscribers.clear();
        state.pathSubscribers.clear();
      }),

    // Subscription API
    subscribe: (callback) => {
      const { subscribers } = get();
      subscribers.add(callback);
      return () => {
        subscribers.delete(callback);
      };
    },

    subscribePath: (path, callback) => {
      const { pathSubscribers } = get();
      if (!pathSubscribers.has(path)) {
        pathSubscribers.set(path, new Set());
      }
      pathSubscribers.get(path)!.add(callback);
      return () => {
        const subs = pathSubscribers.get(path);
        if (subs) {
          subs.delete(callback);
          if (subs.size === 0) {
            pathSubscribers.delete(path);
          }
        }
      };
    },

    notifySubscribers: () => {
      const { subscribers } = get();
      subscribers.forEach((callback) => callback());
    },

    notifyPathSubscribers: (path) => {
      const { pathSubscribers } = get();
      const subs = pathSubscribers.get(path);
      if (subs) {
        subs.forEach((callback) => callback());
      }
    },

    // Query API
    getFile: (path) => {
      const { files } = get();
      return files.get(path);
    },

    getFiles: () => {
      const { files } = get();
      return Array.from(files.values());
    },

    getFilesByType: (eventType) => {
      const { files } = get();
      return Array.from(files.values()).filter((file) => file.eventType === eventType);
    },

    getFilesInDirectory: (dirPath) => {
      const { files } = get();
      const normalizedDir = dirPath.endsWith('/') ? dirPath : `${dirPath}/`;
      return Array.from(files.values()).filter((file) =>
        file.path.startsWith(normalizedDir)
      );
    },

    hasFile: (path) => {
      const { files } = get();
      return files.has(path);
    },

    getFileCount: () => {
      const { files } = get();
      return files.size;
    },
  }))
);

// Selector hooks for optimized rendering
export const useWatcherState = () =>
  useFileWatcherStore((state) => ({
    state: state.state,
    isActive: state.isActive,
    watcherId: state.watcherId,
  }));

export const useWatcherStats = () => useFileWatcherStore((state) => state.stats);

export const useWatcherError = () => useFileWatcherStore((state) => state.lastError);

export const useFileCount = () => useFileWatcherStore((state) => state.getFileCount());

export const useFile = (path: string) =>
  useFileWatcherStore((state) => state.getFile(path));

export const useFilesInDirectory = (dirPath: string) =>
  useFileWatcherStore((state) => state.getFilesInDirectory(dirPath));

export const useRecentEvents = () =>
  useFileWatcherStore((state) => state.recentEvents);

// Hook for subscribing to file changes
export function useFileSubscription(path: string, callback: () => void) {
  const subscribePath = useFileWatcherStore((state) => state.subscribePath);

  React.useEffect(() => {
    const unsubscribe = subscribePath(path, callback);
    return unsubscribe;
  }, [path, callback, subscribePath]);
}

// Hook for subscribing to all file changes
export function useFileWatcherSubscription(callback: () => void) {
  const subscribe = useFileWatcherStore((state) => state.subscribe);

  React.useEffect(() => {
    const unsubscribe = subscribe(callback);
    return unsubscribe;
  }, [callback, subscribe]);
}

// Type-safe version of React import for the hooks
import React from 'react';

enableMapSet();
import React, { useState } from "react";
import { useFileWatcher } from "@/services/fileWatcher";
import {
  useFileCount,
  useFileWatcherStore,
  useRecentEvents,
} from "@/store/useFileWatcherStore";
import type { WatcherConfig } from "@/types/watcher";

/**
 * Component for controlling the file watcher
 */
export function FileWatcherControl() {
  const [repoPath, setRepoPath] = useState(
    "/Users/ruru/Projects/voiceweave-monorepo",
  );
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const { startWatching, stopWatching } = useFileWatcher();
  const { isActive, state } = useFileWatcherStore();
  const fileCount = useFileCount();

  const handleStart = async () => {
    setIsLoading(true);
    setError(null);

    try {
      const config: WatcherConfig = {
        debounce_ms: 100,
        batch_size: 50,
        emit_stats: true,
      };

      await startWatching(repoPath, config);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to start watching");
      console.error(err);
    } finally {
      setIsLoading(false);
    }
  };

  const handleStop = async () => {
    setIsLoading(true);
    setError(null);

    try {
      await stopWatching();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to stop watching");
      console.error(err);
    } finally {
      setIsLoading(false);
    }
  };

  return (
    <div className="p-4 border rounded-lg">
      <h2 className="text-xl font-bold mb-4">File Watcher Control</h2>

      <div className="space-y-4">
        {/* Repository Path Input */}
        <div>
          <label className="block text-sm font-medium mb-2">
            Repository Path
          </label>
          <input
            type="text"
            value={repoPath}
            onChange={(e) => setRepoPath(e.target.value)}
            disabled={isActive}
            className="w-full px-3 py-2 border rounded"
            placeholder="/Users/ruru/Projects/voiceweave-monorepo"
          />
        </div>

        {/* Status */}
        <div className="flex items-center gap-4">
          <div className="flex items-center gap-2">
            <div
              className={`w-3 h-3 rounded-full ${
                isActive ? "bg-green-500" : "bg-gray-400"
              }`}
            />
            <span className="text-sm font-medium">Status: {state}</span>
          </div>
          <div className="text-sm text-gray-600">
            Files tracked: {fileCount}
          </div>
        </div>

        {/* Error Display */}
        {error && (
          <div className="p-3 bg-red-50 border border-red-200 rounded text-red-700 text-sm">
            {error}
          </div>
        )}

        {/* Control Buttons */}
        <div className="flex gap-2">
          <button
            onClick={handleStart}
            disabled={isActive || isLoading}
            className="px-4 py-2 bg-blue-600 text-white rounded hover:bg-blue-700 disabled:bg-gray-400 disabled:cursor-not-allowed"
          >
            {isLoading ? "Starting..." : "Start Watching"}
          </button>
          <button
            onClick={handleStop}
            disabled={!isActive || isLoading}
            className="px-4 py-2 bg-red-600 text-white rounded hover:bg-red-700 disabled:bg-gray-400 disabled:cursor-not-allowed"
          >
            {isLoading ? "Stopping..." : "Stop Watching"}
          </button>
        </div>
      </div>
    </div>
  );
}

/**
 * Component showing recent file events
 */
export function RecentEventsPanel() {
  const recentEvents = useRecentEvents();

  return (
    <div className="p-4 border rounded-lg">
      <h2 className="text-xl font-bold mb-4">Recent Events</h2>

      <div className="space-y-2 max-h-96 overflow-y-auto">
        {recentEvents.length === 0 ? (
          <p className="text-gray-500 text-sm">No events yet</p>
        ) : (
          recentEvents.map((event, index) => (
            <div
              key={`${event.path}-${event.timestamp}-${index}`}
              className="p-2 bg-gray-50 rounded text-sm"
            >
              <div className="flex items-center justify-between">
                <span className="font-mono text-xs">{event.path}</span>
                <span
                  className={`px-2 py-1 rounded text-xs font-medium ${getEventTypeColor(
                    event.event_type,
                  )}`}
                >
                  {event.event_type}
                </span>
              </div>
              <div className="text-xs text-gray-500 mt-1">
                {new Date(event.timestamp).toLocaleTimeString()}
              </div>
            </div>
          ))
        )}
      </div>
    </div>
  );
}

function getEventTypeColor(eventType: string): string {
  switch (eventType) {
    case "created":
      return "bg-green-100 text-green-800";
    case "modified":
      return "bg-blue-100 text-blue-800";
    case "deleted":
      return "bg-red-100 text-red-800";
    case "renamed":
      return "bg-yellow-100 text-yellow-800";
    default:
      return "bg-gray-100 text-gray-800";
  }
}

/**
 * Component showing all tracked files
 */
export function FileListPanel() {
  const files = useFileWatcherStore((state) => state.getFiles());
  const [sortBy, setSortBy] = useState<"name" | "modified">("modified");

  const sortedFiles = React.useMemo(() => {
    const sorted = [...files];
    sorted.sort((a, b) => {
      if (sortBy === "name") {
        return a.path.localeCompare(b.path);
      } else {
        return (
          new Date(b.lastModified).getTime() -
          new Date(a.lastModified).getTime()
        );
      }
    });
    return sorted;
  }, [files, sortBy]);

  return (
    <div className="p-4 border rounded-lg">
      <div className="flex items-center justify-between mb-4">
        <h2 className="text-xl font-bold">Tracked Files</h2>
        <select
          value={sortBy}
          onChange={(e) => setSortBy(e.target.value as "name" | "modified")}
          className="px-3 py-1 border rounded text-sm"
        >
          <option value="modified">Sort by Modified</option>
          <option value="name">Sort by Name</option>
        </select>
      </div>

      <div className="space-y-1 max-h-96 overflow-y-auto">
        {sortedFiles.length === 0 ? (
          <p className="text-gray-500 text-sm">No files tracked yet</p>
        ) : (
          sortedFiles.map((file) => (
            <div
              key={file.path}
              className="p-2 hover:bg-gray-50 rounded text-sm flex items-center justify-between"
            >
              <div className="flex-1 min-w-0">
                <div className="font-mono text-xs truncate">{file.path}</div>
                {file.metadata && (
                  <div className="text-xs text-gray-500">
                    {file.metadata.isDirectory
                      ? "Directory"
                      : formatBytes(file.metadata.size)}
                  </div>
                )}
              </div>
              <div className="text-xs text-gray-500 ml-2">
                {new Date(file.lastModified).toLocaleTimeString()}
              </div>
            </div>
          ))
        )}
      </div>
    </div>
  );
}

function formatBytes(bytes?: number): string {
  if (bytes === undefined) return "Unknown";
  if (bytes === 0) return "0 B";
  const k = 1024;
  const sizes = ["B", "KB", "MB", "GB"];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return `${Math.round((bytes / k ** i) * 100) / 100} ${sizes[i]}`;
}

/**
 * Main file watcher dashboard component
 */
export function FileWatcherDashboard() {
  return (
    <div className="p-6 space-y-6">
      <h1 className="text-3xl font-bold">File Watcher Dashboard</h1>

      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        <FileWatcherControl />
        <RecentEventsPanel />
      </div>

      {/* <FileListPanel /> */}
    </div>
  );
}

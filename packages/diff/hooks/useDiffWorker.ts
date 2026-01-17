import { useEffect, useRef, useState } from "react";

import type { FileDiff, SupportedTheme } from "../types";
import { cancelRender, queueRender } from "../utils/render-queue";
import { getDiffWorkerPool } from "../worker/pool";

// ============================================================================
// Types
// ============================================================================

export interface DiffWorkerState {
  /** Map of line content to highlighted HTML (may include inline diff markers) */
  cache: Map<string, string>;
  /** Whether initial processing is complete */
  ready: boolean;
  /** Progress 0-1 during processing */
  progress: number;
  /** Error if processing failed */
  error?: Error;
  /** Processing time in ms (available after completion) */
  duration?: number;
}

export interface UseDiffWorkerOptions {
  /** Theme for syntax highlighting */
  theme: SupportedTheme;
  /** Whether to compute and apply inline diff highlighting */
  highlightInline: boolean;
  /** Called on each streaming chunk */
  onProgress?: (progress: number) => void;
}

// ============================================================================
// Hook
// ============================================================================

/**
 * Hook that processes a FileDiff using the worker pool with streaming results.
 *
 * - All heavy computation (highlighting, inline diff) runs in workers
 * - Results stream back incrementally for faster perceived performance
 * - Automatically cancels previous requests when diff changes
 * - Request cancellation prevents stale updates
 */
export function useDiffWorker(
  diff: FileDiff | null,
  options: UseDiffWorkerOptions,
): DiffWorkerState {
  const { theme, highlightInline, onProgress } = options;

  const [state, setState] = useState<DiffWorkerState>({
    cache: new Map(),
    ready: false,
    progress: 0,
  });

  // Track the current request for cancellation
  const cancelRef = useRef<(() => void) | null>(null);

  // Track mounted state to prevent updates after unmount
  const mountedRef = useRef(true);

  // Memoize the cache to avoid recreating on each chunk
  const cacheRef = useRef(new Map<string, string>());

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
    };
  }, []);

  useEffect(() => {
    // Cancel previous request
    if (cancelRef.current) {
      cancelRef.current();
      cancelRef.current = null;
    }

    // Handle null diff
    if (!diff) {
      setState({
        cache: new Map(),
        ready: true,
        progress: 1,
      });
      return;
    }

    // Handle empty diff
    if (diff.hunks.length === 0) {
      setState({
        cache: new Map(),
        ready: true,
        progress: 1,
      });
      return;
    }

    // Reset state for new diff
    cacheRef.current = new Map();
    setState({
      cache: new Map(),
      ready: false,
      progress: 0,
    });

    // Get worker pool
    const pool = getDiffWorkerPool();

    // Track pending updates for rAF batching
    let pendingProgress = 0;
    let hasPendingUpdate = false;

    // Batched state update function - runs in requestAnimationFrame
    const flushUpdate = () => {
      if (!mountedRef.current || !hasPendingUpdate) return;
      hasPendingUpdate = false;

      // Update state with new cache (create new Map for React to detect change)
      setState((prev) => ({
        ...prev,
        cache: new Map(cacheRef.current),
        progress: pendingProgress,
      }));

      onProgress?.(pendingProgress);
    };

    // Start processing
    const unsubscribe = pool.processDiff(diff, theme, highlightInline, {
      onChunk: (entries, progress) => {
        if (!mountedRef.current) return;

        // Merge new entries into cache (immediate - no DOM)
        for (const [content, html] of entries) {
          cacheRef.current.set(content, html);
        }

        // Queue batched update via requestAnimationFrame
        // This prevents multiple React re-renders per frame
        pendingProgress = progress;
        if (!hasPendingUpdate) {
          hasPendingUpdate = true;
          queueRender(flushUpdate);
        }
      },

      onComplete: (totalLines, duration) => {
        if (!mountedRef.current) return;

        setState((prev) => ({
          ...prev,
          cache: new Map(cacheRef.current),
          ready: true,
          progress: 1,
          duration,
        }));

        console.log(
          `[useDiffWorker] Completed: ${totalLines} lines in ${duration.toFixed(1)}ms`,
        );
      },

      onError: (error) => {
        if (!mountedRef.current) return;

        setState((prev) => ({
          ...prev,
          ready: true,
          progress: 1,
          error: new Error(error),
        }));

        console.error("[useDiffWorker] Error:", error);
      },
    });

    cancelRef.current = unsubscribe;

    // Cleanup on unmount or dependency change
    return () => {
      // Cancel any pending rAF updates
      cancelRender(flushUpdate);
      hasPendingUpdate = false;

      if (cancelRef.current) {
        cancelRef.current();
        cancelRef.current = null;
      }
    };
  }, [diff, theme, highlightInline, onProgress]);

  return state;
}

// ============================================================================
// Additional Utilities
// ============================================================================

/**
 * Hook for getting worker pool statistics (for debugging/monitoring)
 */
export function useDiffWorkerStats() {
  const [stats, setStats] = useState(() => getDiffWorkerPool().getStats());

  useEffect(() => {
    const interval = setInterval(() => {
      setStats(getDiffWorkerPool().getStats());
    }, 1000);

    return () => clearInterval(interval);
  }, []);

  return stats;
}

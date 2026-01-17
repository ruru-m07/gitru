// ============================================================================
// Worker Pool Manager - Multi-worker architecture with language affinity
// ============================================================================

import type { FileDiff, SupportedTheme } from "../types";
import type {
  HighlightBatchRequest,
  HighlightBatchResponse,
  HighlightRequest,
  HighlightResponse,
  InitializeResponse,
  PoolConfig,
  PoolStats,
  ProcessDiffChunk,
  ProcessDiffComplete,
  ProcessDiffError,
  WorkerRequest,
  WorkerResponse,
  WorkerState,
} from "./types";

// ============================================================================
// Constants
// ============================================================================

// Use 8 workers by default (like the diffs package) for better parallelism
const DEFAULT_WORKER_COUNT =
  typeof navigator !== "undefined"
    ? Math.min(navigator.hardwareConcurrency || 4, 8)
    : 8;
const DEFAULT_TIMEOUT = 60_000;
const DEFAULT_CHUNK_SIZE = 25;

// ============================================================================
// Types
// ============================================================================

interface PendingRequest {
  resolve: (value: unknown) => void;
  reject: (error: Error) => void;
  timeout: ReturnType<typeof setTimeout>;
}

interface StreamingCallback {
  onChunk: (entries: Array<[string, string]>, progress: number) => void;
  onComplete: (totalLines: number, duration: number) => void;
  onError: (error: string) => void;
}

interface ProcessDiffTask {
  instanceId: string;
  diff: FileDiff;
  theme: SupportedTheme;
  highlightInline: boolean;
  callbacks: StreamingCallback;
}

// ============================================================================
// Worker Pool Class
// ============================================================================

export class DiffWorkerPool {
  private workers: WorkerState[] = [];
  private pendingRequests = new Map<number, PendingRequest>();
  private streamingCallbacks = new Map<string, StreamingCallback>();
  private activeInstances = new Set<string>();
  private taskQueue: ProcessDiffTask[] = [];
  private requestId = 0;
  private config: Required<PoolConfig>;
  private stats = {
    processedRequests: 0,
    cacheHits: 0,
    cacheMisses: 0,
  };
  private isTerminated = false;
  private drainScheduled = false;

  constructor(config: PoolConfig = {}) {
    this.config = {
      workerCount: config.workerCount ?? DEFAULT_WORKER_COUNT,
      chunkSize: config.chunkSize ?? DEFAULT_CHUNK_SIZE,
      timeout: config.timeout ?? DEFAULT_TIMEOUT,
    };

    this.initializeWorkers();
  }

  // ==========================================================================
  // Initialization
  // ==========================================================================

  private initializeWorkers(): void {
    for (let i = 0; i < this.config.workerCount; i++) {
      const worker = new Worker(new URL("./diff-worker.ts", import.meta.url), {
        type: "module",
      });

      const state: WorkerState = {
        id: i,
        worker,
        busy: false,
        loadedLanguages: new Set(["text", "plaintext"]),
        loadedThemes: new Set(),
        pendingRequests: 0,
      };

      worker.onmessage = (event: MessageEvent<WorkerResponse>) => {
        this.handleWorkerMessage(state, event.data);
      };

      worker.onerror = (error) => {
        console.error(`[DiffWorkerPool] Worker ${i} error:`, error);
      };

      this.workers.push(state);

      // Initialize each worker
      this.sendToWorker(state, { type: "initialize", id: this.nextId() });
    }

    console.log(
      `[DiffWorkerPool] Initialized ${this.config.workerCount} workers`,
    );
  }

  private nextId(): number {
    return ++this.requestId;
  }

  // ==========================================================================
  // Worker Selection (Language Affinity)
  // ==========================================================================

  private selectWorker(language?: string, _theme?: string): WorkerState {
    // First, try to find a non-busy worker with the language already loaded
    if (language) {
      const preferredWorker = this.workers.find(
        (w) => !w.busy && w.loadedLanguages.has(language),
      );
      if (preferredWorker) return preferredWorker;
    }

    // Fall back to least busy worker
    const availableWorkers = this.workers.filter((w) => !w.busy);
    if (availableWorkers.length > 0) {
      // Pick one with fewest pending requests
      return availableWorkers.reduce((a, b) =>
        a.pendingRequests <= b.pendingRequests ? a : b,
      );
    }

    // All workers busy, pick least loaded
    return this.workers.reduce((a, b) =>
      a.pendingRequests <= b.pendingRequests ? a : b,
    );
  }

  // ==========================================================================
  // Send Message to Worker
  // ==========================================================================

  private sendToWorker(worker: WorkerState, request: WorkerRequest): void {
    worker.worker.postMessage(request);
    worker.pendingRequests++;
  }

  // ==========================================================================
  // Handle Worker Messages
  // ==========================================================================

  private handleWorkerMessage(
    worker: WorkerState,
    response: WorkerResponse,
  ): void {
    worker.pendingRequests = Math.max(0, worker.pendingRequests - 1);

    switch (response.type) {
      case "initialize": {
        const initResponse = response as InitializeResponse;
        if (initResponse.success) {
          for (const lang of initResponse.loadedLanguages) {
            worker.loadedLanguages.add(lang);
          }
        }
        this.resolvePending(response.id, initResponse);
        break;
      }

      case "highlight": {
        const highlightResponse = response as HighlightResponse;
        this.resolvePending(response.id, highlightResponse);
        this.stats.processedRequests++;
        break;
      }

      case "highlight-batch": {
        const batchResponse = response as HighlightBatchResponse;
        this.resolvePending(response.id, batchResponse);
        this.stats.processedRequests++;
        break;
      }

      case "process-diff-chunk": {
        const chunkResponse = response as ProcessDiffChunk;
        const callbacks = this.streamingCallbacks.get(chunkResponse.instanceId);

        // Only process if instance is still active (not cancelled)
        if (callbacks && this.activeInstances.has(chunkResponse.instanceId)) {
          callbacks.onChunk(chunkResponse.entries, chunkResponse.progress);
        }
        break;
      }

      case "process-diff-complete": {
        const completeResponse = response as ProcessDiffComplete;
        const callbacks = this.streamingCallbacks.get(
          completeResponse.instanceId,
        );

        if (
          callbacks &&
          this.activeInstances.has(completeResponse.instanceId)
        ) {
          callbacks.onComplete(
            completeResponse.totalLines,
            completeResponse.duration,
          );
        }

        // Clean up
        this.streamingCallbacks.delete(completeResponse.instanceId);
        this.activeInstances.delete(completeResponse.instanceId);
        worker.busy = false;
        this.stats.processedRequests++;

        // Process next in queue
        this.scheduleDrain();
        break;
      }

      case "process-diff-error": {
        const errorResponse = response as ProcessDiffError;
        const callbacks = this.streamingCallbacks.get(errorResponse.instanceId);

        if (callbacks) {
          callbacks.onError(errorResponse.error);
        }

        // Clean up
        this.streamingCallbacks.delete(errorResponse.instanceId);
        this.activeInstances.delete(errorResponse.instanceId);
        worker.busy = false;

        // Process next in queue
        this.scheduleDrain();
        break;
      }

      case "parse-patch": {
        this.resolvePending(response.id, response);
        break;
      }
    }
  }

  private resolvePending(id: number, response: unknown): void {
    const pending = this.pendingRequests.get(id);
    if (pending) {
      clearTimeout(pending.timeout);
      pending.resolve(response);
      this.pendingRequests.delete(id);
    }
  }

  // ==========================================================================
  // Task Queue Management
  // ==========================================================================

  private scheduleDrain(): void {
    if (this.drainScheduled || this.taskQueue.length === 0) return;

    this.drainScheduled = true;
    queueMicrotask(() => {
      this.drainScheduled = false;
      this.drainQueue();
    });
  }

  private drainQueue(): void {
    while (this.taskQueue.length > 0) {
      const availableWorker = this.workers.find((w) => !w.busy);
      if (!availableWorker) break;

      const task = this.taskQueue.shift();
      if (!task) break;

      // Skip if instance was cancelled
      if (!this.activeInstances.has(task.instanceId)) {
        continue;
      }

      this.executeProcessDiff(availableWorker, task);
    }
  }

  private executeProcessDiff(worker: WorkerState, task: ProcessDiffTask): void {
    worker.busy = true;
    worker.loadedLanguages.add(task.diff.language);

    this.streamingCallbacks.set(task.instanceId, task.callbacks);

    const request: WorkerRequest = {
      type: "process-diff",
      id: this.nextId(),
      instanceId: task.instanceId,
      diff: task.diff,
      theme: task.theme,
      highlightInline: task.highlightInline,
    };

    this.sendToWorker(worker, request);
  }

  // ==========================================================================
  // Public API
  // ==========================================================================

  /**
   * Process a diff with streaming results.
   * Returns an unsubscribe function for cancellation.
   */
  processDiff(
    diff: FileDiff,
    theme: SupportedTheme,
    highlightInline: boolean,
    callbacks: StreamingCallback,
  ): () => void {
    if (this.isTerminated) {
      callbacks.onError("Worker pool has been terminated");
      return () => {};
    }

    const instanceId = `${Date.now()}-${Math.random().toString(36).slice(2)}`;
    this.activeInstances.add(instanceId);

    const task: ProcessDiffTask = {
      instanceId,
      diff,
      theme,
      highlightInline,
      callbacks,
    };

    // Try to find an available worker
    const availableWorker = this.workers.find((w) => !w.busy);

    if (availableWorker) {
      this.executeProcessDiff(availableWorker, task);
    } else {
      // Queue the task
      this.taskQueue.push(task);
    }

    // Return cancellation function
    return () => {
      this.activeInstances.delete(instanceId);
      this.streamingCallbacks.delete(instanceId);
      // Remove from queue if still there
      const idx = this.taskQueue.findIndex((t) => t.instanceId === instanceId);
      if (idx !== -1) {
        this.taskQueue.splice(idx, 1);
      }
    };
  }

  /**
   * Highlight a single line (for backward compatibility)
   */
  async highlight(
    content: string,
    language: string,
    theme: SupportedTheme,
  ): Promise<HighlightResponse> {
    if (this.isTerminated) {
      throw new Error("Worker pool has been terminated");
    }

    const worker = this.selectWorker(language, theme);
    const id = this.nextId();

    return new Promise((resolve, reject) => {
      const timeout = setTimeout(() => {
        this.pendingRequests.delete(id);
        reject(new Error("Highlight request timed out"));
      }, this.config.timeout);

      this.pendingRequests.set(id, {
        resolve: resolve as (v: unknown) => void,
        reject,
        timeout,
      });

      const request: HighlightRequest = {
        type: "highlight",
        id,
        content,
        language,
        theme,
      };

      this.sendToWorker(worker, request);

      // Track loaded language
      worker.loadedLanguages.add(language);
    });
  }

  /**
   * Highlight multiple lines at once
   */
  async highlightBatch(
    lines: string[],
    language: string,
    theme: SupportedTheme,
  ): Promise<HighlightBatchResponse> {
    if (this.isTerminated) {
      throw new Error("Worker pool has been terminated");
    }

    const worker = this.selectWorker(language, theme);
    const id = this.nextId();

    return new Promise((resolve, reject) => {
      const timeout = setTimeout(() => {
        this.pendingRequests.delete(id);
        reject(new Error("Batch highlight request timed out"));
      }, this.config.timeout);

      this.pendingRequests.set(id, {
        resolve: resolve as (v: unknown) => void,
        reject,
        timeout,
      });

      const request: HighlightBatchRequest = {
        type: "highlight-batch",
        id,
        lines,
        language,
        theme,
      };

      this.sendToWorker(worker, request);
      worker.loadedLanguages.add(language);
    });
  }

  /**
   * Get pool statistics
   */
  getStats(): PoolStats {
    return {
      totalWorkers: this.workers.length,
      busyWorkers: this.workers.filter((w) => w.busy).length,
      queuedTasks: this.taskQueue.length,
      ...this.stats,
    };
  }

  /**
   * Terminate all workers and clean up
   */
  terminate(): void {
    this.isTerminated = true;

    // Reject all pending requests
    for (const [_id, pending] of this.pendingRequests) {
      clearTimeout(pending.timeout);
      pending.reject(new Error("Worker pool terminated"));
    }
    this.pendingRequests.clear();

    // Clear streaming callbacks
    for (const [_instanceId, callbacks] of this.streamingCallbacks) {
      callbacks.onError("Worker pool terminated");
    }
    this.streamingCallbacks.clear();
    this.activeInstances.clear();

    // Clear task queue
    this.taskQueue = [];

    // Terminate workers
    for (const worker of this.workers) {
      worker.worker.terminate();
    }
    this.workers = [];

    console.log("[DiffWorkerPool] Terminated");
  }
}

// ============================================================================
// Singleton Instance
// ============================================================================

let defaultPool: DiffWorkerPool | null = null;

export function getDiffWorkerPool(config?: PoolConfig): DiffWorkerPool {
  if (!defaultPool) {
    defaultPool = new DiffWorkerPool(config);
  }
  return defaultPool;
}

export function terminateDiffWorkerPool(): void {
  if (defaultPool) {
    defaultPool.terminate();
    defaultPool = null;
  }
}

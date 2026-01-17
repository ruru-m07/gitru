// ============================================================================
// requestAnimationFrame Batching for DOM Updates
// Batches multiple render callbacks into a single animation frame
// ============================================================================

type RenderCallback = (time: number) => unknown;

// Queued callbacks waiting for next frame
const queuedCallbacks = new Set<RenderCallback>();
// Callbacks currently being processed
let activeCallbacks = new Set<RenderCallback>();
// Current animation frame ID
let frameId: number | null = null;
// Whether we're currently in a render cycle
let isRendering = false;

/**
 * Queue a render callback to run in the next animation frame.
 * Multiple callbacks queued before the next frame will all run together.
 * This prevents layout thrashing from multiple React state updates.
 */
export function queueRender(callback: RenderCallback): void {
  // If we're already rendering and get a new callback, queue it for next frame
  if (isRendering) {
    queuedCallbacks.add(callback);
    return;
  }

  activeCallbacks.add(callback);

  // Schedule frame if not already scheduled
  if (frameId === null) {
    frameId = requestAnimationFrame(render);
  }
}

/**
 * Cancel a queued render callback.
 */
export function cancelRender(callback: RenderCallback): void {
  queuedCallbacks.delete(callback);
  activeCallbacks.delete(callback);
}

/**
 * Internal render function that processes all queued callbacks.
 */
function render(time: number): void {
  isRendering = true;

  // Process all active callbacks
  for (const callback of activeCallbacks) {
    try {
      callback(time);
    } catch (error) {
      console.error("[queueRender] Callback error:", error);
    }
  }

  activeCallbacks.clear();

  // If new callbacks were queued during render, schedule another frame
  if (queuedCallbacks.size > 0) {
    activeCallbacks = new Set(queuedCallbacks);
    queuedCallbacks.clear();
    frameId = requestAnimationFrame(render);
  } else {
    frameId = null;
  }

  isRendering = false;
}

// ============================================================================
// Batched State Updates
// ============================================================================

type BatchUpdateCallback<T> = (updates: Map<string, T>) => void;

/**
 * Creates a batched update queue that collects updates and flushes them
 * in a single animation frame.
 */
export function createBatchedUpdater<T>(onFlush: BatchUpdateCallback<T>): {
  add: (key: string, value: T) => void;
  flush: () => void;
} {
  const pending = new Map<string, T>();
  let scheduled = false;

  const flush = () => {
    if (pending.size === 0) return;

    const updates = new Map(pending);
    pending.clear();
    scheduled = false;

    onFlush(updates);
  };

  const add = (key: string, value: T) => {
    pending.set(key, value);

    if (!scheduled) {
      scheduled = true;
      queueRender(flush);
    }
  };

  return { add, flush };
}

// ============================================================================
// Microtask batching (for non-DOM operations)
// ============================================================================

type MicrotaskCallback = () => void;

let microtaskCallbacks: MicrotaskCallback[] = [];
let microtaskScheduled = false;

/**
 * Queue a callback to run in the next microtask.
 * Useful for batching operations that don't need to wait for rAF.
 */
export function queueMicrotask(callback: MicrotaskCallback): void {
  microtaskCallbacks.push(callback);

  if (!microtaskScheduled) {
    microtaskScheduled = true;
    Promise.resolve().then(runMicrotasks);
  }
}

function runMicrotasks(): void {
  const callbacks = microtaskCallbacks;
  microtaskCallbacks = [];
  microtaskScheduled = false;

  for (const callback of callbacks) {
    try {
      callback();
    } catch (error) {
      console.error("[queueMicrotask] Callback error:", error);
    }
  }
}

// ============================================================================
// Throttled Updates (for high-frequency events)
// ============================================================================

/**
 * Creates a throttled function that batches calls into animation frames.
 * Useful for scroll, resize, and other high-frequency events.
 */
export function createThrottledUpdater<T>(
  callback: (value: T) => void,
): (value: T) => void {
  let lastValue: T | undefined;
  let scheduled = false;

  return (value: T) => {
    lastValue = value;

    if (!scheduled) {
      scheduled = true;
      queueRender(() => {
        scheduled = false;
        if (lastValue !== undefined) {
          callback(lastValue);
        }
      });
    }
  };
}

// ============================================================================
// Idle Callback (for low-priority work)
// ============================================================================

type IdleCallback = (deadline: IdleDeadline) => void;

/**
 * Queue a callback to run when the browser is idle.
 * Falls back to setTimeout if requestIdleCallback is not available.
 */
export function queueIdleCallback(
  callback: IdleCallback,
  options?: { timeout?: number },
): number {
  if (typeof requestIdleCallback !== "undefined") {
    return requestIdleCallback(callback, options);
  }

  // Fallback for environments without requestIdleCallback
  const start = Date.now();
  return setTimeout(() => {
    callback({
      didTimeout: false,
      timeRemaining: () => Math.max(0, 50 - (Date.now() - start)),
    });
  }, 1) as unknown as number;
}

/**
 * Cancel an idle callback.
 */
export function cancelIdleCallback(id: number): void {
  if (typeof window !== "undefined" && "cancelIdleCallback" in window) {
    window.cancelIdleCallback(id);
  } else {
    clearTimeout(id);
  }
}

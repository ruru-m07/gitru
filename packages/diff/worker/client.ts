import type {
  HighlightRequest,
  HighlightResponse,
  SupportedLanguage,
  SupportedTheme,
  WorkerMessage,
} from "../types";

interface PendingRequest {
  resolve: (response: HighlightResponse) => void;
  reject: (error: Error) => void;
  startTime: number;
}

export interface HighlightClientOptions {
  /** Worker URL override */
  workerUrl?: URL;
  /** Timeout for requests in ms (default: 30000) */
  timeout?: number;
  /** Enable debug logging */
  debug?: boolean;
}

/**
 * Client for communicating with the Shiki highlight worker.
 * Handles request/response lifecycle, timeouts, and error handling.
 */
export class HighlightClient {
  private worker: Worker;
  private nextId = 0;
  private pending = new Map<number, PendingRequest>();
  private timeout: number;
  private debug: boolean;
  private terminated = false;

  constructor(options: HighlightClientOptions = {}) {
    const {
      workerUrl = new URL("./highlight-worker.ts", import.meta.url),
      timeout = 30000,
      debug = false,
    } = options;

    this.timeout = timeout;
    this.debug = debug;

    this.worker = new Worker(workerUrl, { type: "module" });
    this.worker.onmessage = this.handleMessage.bind(this);
    this.worker.onerror = this.handleError.bind(this);

    if (this.debug) {
      console.log("[HighlightClient] Worker created");
    }
  }

  private handleMessage(event: MessageEvent<WorkerMessage>): void {
    const message = event.data;
    const pending = this.pending.get(message.id);

    if (!pending) {
      if (this.debug) {
        console.warn(
          "[HighlightClient] Received response for unknown request:",
          message.id,
        );
      }
      return;
    }

    this.pending.delete(message.id);

    if ("error" in message) {
      pending.reject(new Error(message.error));
    } else {
      if (this.debug) {
        const elapsed = performance.now() - pending.startTime;
        console.log(
          `[HighlightClient] Request ${message.id} completed in ${elapsed.toFixed(2)}ms`,
        );
      }
      pending.resolve(message);
    }
  }

  private handleError(event: ErrorEvent): void {
    console.error("[HighlightClient] Worker error:", event.message);

    // Reject all pending requests
    for (const [, pending] of this.pending) {
      pending.reject(new Error(`Worker error: ${event.message}`));
    }
    this.pending.clear();
  }

  /**
   * Highlight code with the given language and theme.
   */
  async highlight(
    code: string,
    language: SupportedLanguage,
    theme: SupportedTheme,
  ): Promise<HighlightResponse> {
    if (this.terminated) {
      throw new Error("HighlightClient has been terminated");
    }

    const id = this.nextId++;
    const startTime = performance.now();

    return new Promise<HighlightResponse>((resolve, reject) => {
      // Set up timeout
      const timeoutId = setTimeout(() => {
        const pending = this.pending.get(id);
        if (pending) {
          this.pending.delete(id);
          reject(
            new Error(`Highlight request timed out after ${this.timeout}ms`),
          );
        }
      }, this.timeout);

      // Store pending request with timeout cleanup
      this.pending.set(id, {
        resolve: (response) => {
          clearTimeout(timeoutId);
          resolve(response);
        },
        reject: (error) => {
          clearTimeout(timeoutId);
          reject(error);
        },
        startTime,
      });

      // Send request to worker
      const request: HighlightRequest = { id, code, language, theme };
      this.worker.postMessage(request);

      if (this.debug) {
        console.log(`[HighlightClient] Sent request ${id} (${language})`);
      }
    });
  }

  /**
   * Highlight multiple code snippets in parallel.
   * Useful for highlighting all lines of a diff at once.
   */
  async highlightBatch(
    items: Array<{
      code: string;
      language: SupportedLanguage;
      theme: SupportedTheme;
    }>,
  ): Promise<HighlightResponse[]> {
    return Promise.all(
      items.map(({ code, language, theme }) =>
        this.highlight(code, language, theme),
      ),
    );
  }

  /**
   * Get the number of pending requests.
   */
  getPendingCount(): number {
    return this.pending.size;
  }

  /**
   * Terminate the worker and reject all pending requests.
   */
  terminate(): void {
    if (this.terminated) return;

    this.terminated = true;
    this.worker.terminate();

    // Reject all pending requests
    for (const [, pending] of this.pending) {
      pending.reject(new Error("HighlightClient terminated"));
    }
    this.pending.clear();

    if (this.debug) {
      console.log("[HighlightClient] Terminated");
    }
  }

  /**
   * Check if the client has been terminated.
   */
  isTerminated(): boolean {
    return this.terminated;
  }
}

let defaultClient: HighlightClient | null = null;

/**
 * Get or create the default HighlightClient instance.
 */
export function getHighlightClient(
  options?: HighlightClientOptions,
): HighlightClient {
  if (!defaultClient || defaultClient.isTerminated()) {
    defaultClient = new HighlightClient(options);
  }
  return defaultClient;
}

/**
 * Convenience function to highlight code using the default client.
 */
export async function highlight(
  code: string,
  language: SupportedLanguage,
  theme: SupportedTheme,
): Promise<string> {
  const client = getHighlightClient();
  const response = await client.highlight(code, language, theme);
  return response.html;
}

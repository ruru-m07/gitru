// ============================================================================
// Worker Message Types - Communication Protocol
// ============================================================================

import type { FileDiff, SupportedTheme } from "../types";

// ============================================================================
// Request Types (Main Thread → Worker)
// ============================================================================

export interface InitializeRequest {
  type: "initialize";
  id: number;
}

export interface HighlightRequest {
  type: "highlight";
  id: number;
  content: string;
  language: string;
  theme: SupportedTheme;
}

export interface HighlightBatchRequest {
  type: "highlight-batch";
  id: number;
  lines: string[];
  language: string;
  theme: SupportedTheme;
}

export interface ParsePatchRequest {
  type: "parse-patch";
  id: number;
  patch: string;
}

export interface ProcessDiffRequest {
  type: "process-diff";
  id: number;
  instanceId: string; // For cancellation
  diff: FileDiff;
  theme: SupportedTheme;
  highlightInline: boolean;
}

export type WorkerRequest =
  | InitializeRequest
  | HighlightRequest
  | HighlightBatchRequest
  | ParsePatchRequest
  | ProcessDiffRequest;

// ============================================================================
// Response Types (Worker → Main Thread)
// ============================================================================

export interface InitializeResponse {
  type: "initialize";
  id: number;
  success: boolean;
  loadedLanguages: string[];
}

export interface HighlightResponse {
  type: "highlight";
  id: number;
  html: string;
  error?: string;
}

export interface HighlightBatchResponse {
  type: "highlight-batch";
  id: number;
  results: Array<{ content: string; html: string }>;
  error?: string;
}

export interface ParsePatchResponse {
  type: "parse-patch";
  id: number;
  files: FileDiff[];
  error?: string;
}

// Streaming response - sent multiple times during processing
export interface ProcessDiffChunk {
  type: "process-diff-chunk";
  id: number;
  instanceId: string;
  /** Map entries as array for structured clone: [lineContent, highlightedHtml] */
  entries: Array<[string, string]>;
  /** Progress 0-1 */
  progress: number;
}

export interface ProcessDiffComplete {
  type: "process-diff-complete";
  id: number;
  instanceId: string;
  /** Total entries processed */
  totalLines: number;
  /** Processing time in ms */
  duration: number;
}

export interface ProcessDiffError {
  type: "process-diff-error";
  id: number;
  instanceId: string;
  error: string;
}

export type WorkerResponse =
  | InitializeResponse
  | HighlightResponse
  | HighlightBatchResponse
  | ParsePatchResponse
  | ProcessDiffChunk
  | ProcessDiffComplete
  | ProcessDiffError;

// ============================================================================
// Worker State Types
// ============================================================================

export interface WorkerState {
  id: number;
  worker: Worker;
  busy: boolean;
  loadedLanguages: Set<string>;
  loadedThemes: Set<string>;
  pendingRequests: number;
}

export interface PoolStats {
  totalWorkers: number;
  busyWorkers: number;
  queuedTasks: number;
  processedRequests: number;
  cacheHits: number;
  cacheMisses: number;
}

// ============================================================================
// Inline Diff Types (moved to worker)
// ============================================================================

export interface InlineDiffResult {
  /** Original line content */
  content: string;
  /** Highlighted HTML with inline diff markers applied */
  html: string;
  /** Line type for styling */
  lineType: "addition" | "deletion" | "context";
}

// ============================================================================
// Pool Configuration
// ============================================================================

export interface PoolConfig {
  /** Number of workers (default: navigator.hardwareConcurrency || 4) */
  workerCount?: number;
  /** Chunk size for streaming (default: 25) */
  chunkSize?: number;
  /** Request timeout in ms (default: 60000) */
  timeout?: number;
}

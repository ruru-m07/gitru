// Legacy client (still works but uses single worker)
// export {
//   getHighlightClient,
//   HighlightClient,
//   type HighlightClientOptions,
//   highlight,
// } from "./client";

// New worker pool (multi-worker with streaming)
export {
  DiffWorkerPool,
  getDiffWorkerPool,
  terminateDiffWorkerPool,
} from "./pool";

// Types
export type {
  PoolConfig,
  PoolStats,
  ProcessDiffChunk,
  ProcessDiffComplete,
  ProcessDiffError,
  WorkerRequest,
  WorkerResponse,
} from "./types";

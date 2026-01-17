// Components
export { DiffViewer, type DiffViewerProps } from "./components";
// Constants
export { CSS_CLASSES, DEFAULT_THEME_CONFIG, PATTERNS } from "./constants";
// Hooks
export { useDiffWorker, useDiffWorkerStats } from "./hooks/useDiffWorker";
// Types
export type * from "./types";
// Utils
export {
  computeInlineDiff,
  detectLanguage,
  escapeHtml,
  parseDiffFromContents,
  parsePatch,
  renderInlineDiffHtml,
  splitLines,
} from "./utils";
// Worker (legacy single-worker client)
// Worker Pool (new multi-worker architecture)
export {
  DiffWorkerPool,
  getDiffWorkerPool,
  // getHighlightClient,
  // HighlightClient,
  // type HighlightClientOptions,
  // highlight,
  type PoolConfig,
  type PoolStats,
  terminateDiffWorkerPool,
} from "./worker";

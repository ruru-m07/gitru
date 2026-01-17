// Components
export { DiffViewer, type DiffViewerProps } from "./components";
// Constants
export { CSS_CLASSES, DEFAULT_THEME_CONFIG, PATTERNS } from "./constants";
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

// Worker
export {
  getHighlightClient,
  HighlightClient,
  type HighlightClientOptions,
  highlight,
} from "./worker";

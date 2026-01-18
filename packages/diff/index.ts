// Re-export @pierre/diffs core types and utilities
export {
  type AnnotationSide,
  type DiffLineAnnotation,
  diffAcceptRejectHunk,
  // Types
  type FileDiffMetadata,
  type GetHoveredLineResult,
  getSingularPatch,
  type ParsedPatch,
  // Parsing
  parsePatchFiles,
  type SelectedLineRange,
} from "@pierre/diffs";

// Re-export @pierre/diffs React components for advanced usage
export {
  type DiffBasePropsReact,
  FileDiff,
  type FileDiffProps,
  MultiFileDiff,
  type MultiFileDiffProps,
  PatchDiff,
  type PatchDiffProps,
  WorkerPoolContextProvider,
} from "@pierre/diffs/react";

// Gitru components (customized wrappers)
export {
  DiffViewer,
  type DiffViewerOptions,
  type DiffViewerProps,
  type DiffViewStyle,
  usePatchFiles,
} from "./components";

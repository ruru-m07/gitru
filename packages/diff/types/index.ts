import type { BundledLanguage, BundledTheme } from "shiki";

// ============================================================================
// Language & Theme Types
// ============================================================================

export type SupportedLanguage = BundledLanguage | "text" | "plaintext";
export type SupportedTheme = BundledTheme | (string & {});

export interface ThemeConfig {
  light: SupportedTheme;
  dark: SupportedTheme;
}

// ============================================================================
// Diff Line Types
// ============================================================================

export type DiffLineType =
  | "context"
  | "addition"
  | "deletion"
  | "metadata"
  | "expanded";

export interface DiffLine {
  /** Original line content (without +/- prefix) */
  content: string;
  /** Type of the line */
  type: DiffLineType;
  /** Line number in the old file (null for additions) */
  oldLineNumber: number | null;
  /** Line number in the new file (null for deletions) */
  newLineNumber: number | null;
  /** Whether this line has no newline at end of file */
  noNewlineAtEOF?: boolean;
}

// ============================================================================
// Hunk Types
// ============================================================================

export interface HunkHeader {
  /** Starting line in old file */
  oldStart: number;
  /** Number of lines from old file */
  oldCount: number;
  /** Starting line in new file */
  newStart: number;
  /** Number of lines in new file */
  newCount: number;
  /** Optional context/function name after @@ */
  context?: string;
  /** Original header string */
  raw: string;
}

export interface DiffHunk {
  /** Parsed header information */
  header: HunkHeader;
  /** Lines in this hunk */
  lines: DiffLine[];
  /** Number of collapsed/hidden lines before this hunk */
  collapsedBefore: number;
}

// ============================================================================
// File Diff Types
// ============================================================================

export type FileChangeType =
  | "modified"
  | "added"
  | "deleted"
  | "renamed"
  | "copied";

export interface FileDiff {
  /** Current/new file path */
  newPath: string;
  /** Previous file path (for renames) */
  oldPath?: string;
  /** Type of change */
  changeType: FileChangeType;
  /** Detected or specified language */
  language: SupportedLanguage;
  /** Hunks in this diff */
  hunks: DiffHunk[];
  /** File mode (e.g., 100644) */
  mode?: string;
  /** Old file mode */
  oldMode?: string;
  /** Whether the file is binary */
  isBinary?: boolean;
  /** Optional cache key for worker caching */
  cacheKey?: string;
}

export interface ParsedPatch {
  /** Metadata at the start of the patch (commit info, etc.) */
  metadata?: string;
  /** Array of file diffs */
  files: FileDiff[];
}

// ============================================================================
// Diff View Options
// ============================================================================

export type DiffViewStyle = "split" | "unified";

export interface DiffViewerOptions {
  /** Display style: split (side-by-side) or unified */
  style?: DiffViewStyle;
  /** Whether to show line numbers */
  showLineNumbers?: boolean;
  /** Whether to highlight inline word-level changes */
  highlightInlineDiff?: boolean;
  /** Theme configuration */
  theme?: SupportedTheme | ThemeConfig;
  /** Current theme type for multi-theme setups */
  themeType?: "light" | "dark" | "system";
  /** Enable line wrapping instead of horizontal scroll */
  wrapLines?: boolean;
  /** Maximum line length to tokenize (performance optimization) */
  maxLineLength?: number;
  /** Whether to show diff indicators (+/-) */
  showDiffIndicators?: boolean;
}

// ============================================================================
// Worker Types
// ============================================================================

export interface HighlightRequest {
  id: number;
  code: string;
  language: SupportedLanguage;
  theme: SupportedTheme;
  /** Optional: line numbers to include in response */
  lineNumbers?: boolean;
}

export interface HighlightResponse {
  id: number;
  html: string;
  /** Time taken in ms */
  timing?: number;
}

export interface HighlightErrorResponse {
  id: number;
  error: string;
  stack?: string;
}

export type WorkerMessage = HighlightResponse | HighlightErrorResponse;

export function isErrorResponse(
  msg: WorkerMessage,
): msg is HighlightErrorResponse {
  return "error" in msg;
}

// ============================================================================
// Highlighted Line Types
// ============================================================================

export interface HighlightedLine {
  /** Original line content */
  content: string;
  /** Highlighted HTML */
  html: string;
  /** Line type */
  type: DiffLineType;
  /** Old line number */
  oldLineNumber: number | null;
  /** New line number */
  newLineNumber: number | null;
}

export interface HighlightedHunk {
  header: HunkHeader;
  lines: HighlightedLine[];
}

export interface HighlightedFileDiff {
  file: FileDiff;
  hunks: HighlightedHunk[];
}

// ============================================================================
// Inline Diff Types (word-level highlighting)
// ============================================================================

export interface InlineDiffSegment {
  /** Text content */
  text: string;
  /** Whether this was added */
  added?: boolean;
  /** Whether this was removed */
  removed?: boolean;
}

// ============================================================================
// Selection Types
// ============================================================================

export type DiffSide = "old" | "new" | "unified";

export interface LineSelection {
  side: DiffSide;
  startLine: number;
  endLine: number;
}

// ============================================================================
// Annotation Types (for comments, etc.)
// ============================================================================

export interface LineAnnotation<T = unknown> {
  lineNumber: number;
  side: DiffSide;
  data: T;
}

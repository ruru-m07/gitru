import { cn } from "@gitru/ui/lib/utils";
import {
  type DiffLineAnnotation,
  type GetHoveredLineResult,
  parsePatchFiles,
  type SelectedLineRange,
} from "@pierre/diffs";
import type { DiffBasePropsReact } from "@pierre/diffs/react";
import { FileDiff } from "@pierre/diffs/react";
import React, {
  type CSSProperties,
  type ReactNode,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";

export type DiffViewStyle = "split" | "unified";

export interface DiffViewerOptions {
  /** View style: 'split' or 'unified' (default: 'split') */
  style?: DiffViewStyle;
  /** Show line numbers (default: true) */
  showLineNumbers?: boolean;
  /** Highlight inline character differences (default: true) */
  highlightInlineDiff?: boolean;
  /** Theme for syntax highlighting */
  theme?: "pierre-dark" | "pierre-light" | string;
  /** Show +/- indicators (default: true) */
  showDiffIndicators?: boolean;
  /** Wrap long lines (default: false) */
  wrapLines?: boolean;
  /** Enable line selection for comments */
  enableLineSelection?: boolean;
  /** Enable hover utility button */
  enableHoverUtility?: boolean;
  /** Callback when line selection ends */
  onLineSelectionEnd?: (range: SelectedLineRange | null) => void;
  /** Show dual line numbers in unified view (old + new) like GitHub (default: true) */
  dualLineNumbers?: boolean;
}

export interface DiffViewerProps<TAnnotation = undefined> {
  /** Raw patch string to parse and render */
  patch: string;
  /** Index of file to show when patch has multiple files (default: 0) */
  fileIndex?: number;
  /** Viewer options */
  options?: DiffViewerOptions;
  /** Custom class name */
  className?: string;
  /** Custom styles */
  style?: CSSProperties;
  /** Line annotations (comments, etc.) */
  lineAnnotations?: DiffLineAnnotation<TAnnotation>[];
  /** Selected line range */
  selectedLines?: SelectedLineRange | null;
  /** Render function for annotations */
  renderAnnotation?: (annotation: DiffLineAnnotation<TAnnotation>) => ReactNode;
  /** Render function for header metadata */
  renderHeaderMetadata?: DiffBasePropsReact<TAnnotation>["renderHeaderMetadata"];
  /** Render function for hover utility (e.g., add comment button) */
  renderHoverUtility?: (
    getHoveredLine: () => GetHoveredLineResult<"diff"> | undefined,
  ) => ReactNode;
}

// ============================================================================
// Hooks
// ============================================================================

/**
 * Parse a patch string into file diffs
 */
export function usePatchFiles(patch: string) {
  return useMemo(() => {
    try {
      const patches = parsePatchFiles(patch);
      // Flatten all files from all patches
      const files = patches.flatMap((p) => p.files);
      return { files, patches };
    } catch {
      return { files: [], patches: [] };
    }
  }, [patch]);
}

/**
 * Hook to inject dual line numbers (old + new) in unified view.
 * Uses DOM manipulation since @pierre/diffs only renders one line number column.
 *
 * In unified view from @pierre/diffs:
 * - context: data-line = old, data-alt-line = new
 * - deletion: data-line = old, data-alt-line = undefined
 * - addition: data-line = new, data-alt-line = undefined
 */
function useDualLineNumbers(
  containerRef: React.RefObject<HTMLDivElement | null>,
  enabled: boolean,
) {
  const processLine = useCallback((line: Element) => {
    // Skip if already processed
    if (line.querySelector("[data-line-number-old]")) return;

    const lineType = line.getAttribute("data-line-type");
    const lineNum = line.getAttribute("data-line");
    const altLineNum = line.getAttribute("data-alt-line");
    const numberCol = line.querySelector("[data-column-number]");

    if (!numberCol) return;

    // Create old line number element (will be first column)
    const oldNumEl = document.createElement("span");
    oldNumEl.setAttribute("data-line-number-old", "");

    // Create inner span for the actual number
    const oldNumContent = document.createElement("span");
    oldNumContent.setAttribute("data-line-number-content", "");

    // Get the existing content element (currently shows lineNum)
    const existingContent = numberCol.querySelector(
      "[data-line-number-content]",
    );

    if (lineType === "change-addition") {
      // Addition: old column is empty, new column shows the line number
      oldNumContent.textContent = "";
      // existingContent already shows lineNum which is the new line number - correct
    } else if (lineType === "change-deletion") {
      // Deletion: old column shows line number, new column is empty
      oldNumContent.textContent = lineNum ?? "";
      if (existingContent) existingContent.textContent = "";
    } else {
      // Context (or context-expanded): old shows lineNum, new shows altLineNum
      oldNumContent.textContent = lineNum ?? "";
      if (existingContent)
        existingContent.textContent = altLineNum ?? lineNum ?? "";
    }

    oldNumEl.appendChild(oldNumContent);

    // Insert old number column before the existing number column
    line.insertBefore(oldNumEl, numberCol);
  }, []);

  const processAllLines = useCallback(
    (container: Element) => {
      const lines = container.querySelectorAll("[data-line]");
      lines.forEach(processLine);
    },
    [processLine],
  );

  useEffect(() => {
    if (!enabled || typeof window === "undefined") return;

    const container = containerRef.current;
    if (!container) return;

    // Process after render - try multiple times to catch async rendering
    const process = () => processAllLines(container);

    // Immediate attempt
    process();

    // rAF for after paint
    const rafId = requestAnimationFrame(process);

    // Fallback timeout for slower renders
    const timeoutId = setTimeout(process, 100);

    // Watch for dynamic changes (e.g., expanding collapsed sections)
    const observer = new MutationObserver((mutations) => {
      for (const mutation of mutations) {
        if (mutation.type === "childList") {
          mutation.addedNodes.forEach((node) => {
            if (node instanceof Element) {
              if (node.hasAttribute("data-line")) {
                processLine(node);
              }
              // Also check children
              node.querySelectorAll?.("[data-line]").forEach(processLine);
            }
          });
        }
      }
    });

    observer.observe(container, {
      childList: true,
      subtree: true,
    });

    return () => {
      cancelAnimationFrame(rafId);
      clearTimeout(timeoutId);
      observer.disconnect();
    };
  }, [enabled, containerRef, processAllLines, processLine]);
}

// ============================================================================
// Main Component
// ============================================================================

export function DiffViewer<TAnnotation = undefined>({
  patch,
  fileIndex: controlledFileIndex,
  options = {},
  className = "",
  style,
  lineAnnotations,
  selectedLines,
  renderAnnotation,
  renderHeaderMetadata,
  renderHoverUtility,
}: DiffViewerProps<TAnnotation>): React.JSX.Element {
  const {
    style: viewStyle = "split",
    showLineNumbers = true,
    highlightInlineDiff = true,
    theme = "pierre-light",
    showDiffIndicators = true,
    wrapLines = false,
    enableLineSelection = false,
    enableHoverUtility = false,
    onLineSelectionEnd,
    dualLineNumbers = true,
  } = options;

  const containerRef = useRef<HTMLDivElement>(null);
  const parsed = usePatchFiles(patch);
  const files = parsed.files;

  const [internalFileIndex, setInternalFileIndex] = useState(0);
  const fileIndex = controlledFileIndex ?? internalFileIndex;

  const currentFile = files[fileIndex];

  // Enable dual line numbers only in unified view
  const enableDualNumbers =
    dualLineNumbers && viewStyle === "unified" && showLineNumbers;
  useDualLineNumbers(containerRef, enableDualNumbers);

  const diffOptions = useMemo(
    () => ({
      diffStyle: viewStyle,
      disableLineNumbers: !showLineNumbers,
      lineDiffType: highlightInlineDiff
        ? ("word-alt" as const)
        : ("none" as const),
      theme: typeof theme === "string" ? { dark: theme, light: theme } : theme,
      diffIndicators: showDiffIndicators
        ? ("bars" as const)
        : ("none" as const),
      overflow: wrapLines ? ("wrap" as const) : ("scroll" as const),
      enableLineSelection,
      enableHoverUtility,
      onLineSelectionEnd,
    }),
    [
      viewStyle,
      showLineNumbers,
      highlightInlineDiff,
      theme,
      showDiffIndicators,
      wrapLines,
      enableLineSelection,
      enableHoverUtility,
      onLineSelectionEnd,
    ],
  );

  if (!currentFile) {
    return (
      <div className={cn("gitru-diff-viewer", className)} style={style}>
        <div className="gitru-diff-notice">No diff to display</div>
      </div>
    );
  }

  return (
    <div
      ref={containerRef}
      className={cn(
        "gitru-diff-viewer",
        enableDualNumbers && "gitru-dual-line-numbers",
        className,
      )}
      style={style}
    >
      <FileDiff
        fileDiff={currentFile}
        options={{ ...diffOptions, disableFileHeader: true }}
        className="gitru-diff-content"
        lineAnnotations={lineAnnotations}
        selectedLines={selectedLines}
        renderAnnotation={renderAnnotation}
        renderHeaderMetadata={renderHeaderMetadata}
        renderHoverUtility={renderHoverUtility}
      />
    </div>
  );
}

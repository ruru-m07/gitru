"use client";

import React, { useCallback, useEffect, useMemo, useRef } from "react";
import { useDiffWorker } from "../hooks/useDiffWorker";
import type {
  DiffHunk,
  DiffLine,
  DiffViewerOptions,
  DiffViewStyle,
  FileDiff,
  SupportedTheme,
} from "../types";
import { escapeHtml } from "../utils/common";
import { parsePatch } from "../utils/parse-patch";

// ============================================================================
// Types
// ============================================================================

export interface DiffViewerProps {
  /** Raw patch string to parse and render (preferred) */
  patch?: string;
  /** Pre-parsed file diff to render (alternative to patch) */
  diff?: FileDiff;
  /** When using patch prop with multiple files, which file index to show (default: 0) */
  fileIndex?: number;
  /** Viewer options */
  options?: DiffViewerOptions;
  /** Custom class name */
  className?: string;
  /** Custom styles */
  style?: React.CSSProperties;
  /** Callback when a line is clicked */
  onLineClick?: (line: DiffLine, side: "old" | "new") => void;
}

// ============================================================================
// Line Type Helpers
// ============================================================================

function getLineTypeAttr(type: DiffLine["type"]): string {
  switch (type) {
    case "addition":
      return "addition";
    case "deletion":
      return "deletion";
    case "context":
      return "context";
    default:
      return "context";
  }
}

// ============================================================================
// Scroll Sync Hook
// ============================================================================

function useScrollSync(enabled: boolean) {
  const leftRef = useRef<HTMLDivElement>(null);
  const rightRef = useRef<HTMLDivElement>(null);
  const isLeftScrolling = useRef(false);
  const isRightScrolling = useRef(false);
  const timeoutRef = useRef<NodeJS.Timeout | null>(null);

  useEffect(() => {
    if (!enabled) return;

    const leftEl = leftRef.current;
    const rightEl = rightRef.current;

    if (!leftEl || !rightEl) return;

    const handleLeftScroll = () => {
      if (isRightScrolling.current) return;
      isLeftScrolling.current = true;

      if (timeoutRef.current) clearTimeout(timeoutRef.current);
      timeoutRef.current = setTimeout(() => {
        isLeftScrolling.current = false;
      }, 150);

      rightEl.scrollLeft = leftEl.scrollLeft;
    };

    const handleRightScroll = () => {
      if (isLeftScrolling.current) return;
      isRightScrolling.current = true;

      if (timeoutRef.current) clearTimeout(timeoutRef.current);
      timeoutRef.current = setTimeout(() => {
        isRightScrolling.current = false;
      }, 150);

      leftEl.scrollLeft = rightEl.scrollLeft;
    };

    leftEl.addEventListener("scroll", handleLeftScroll, { passive: true });
    rightEl.addEventListener("scroll", handleRightScroll, { passive: true });

    return () => {
      leftEl.removeEventListener("scroll", handleLeftScroll);
      rightEl.removeEventListener("scroll", handleRightScroll);
      if (timeoutRef.current) clearTimeout(timeoutRef.current);
    };
  }, [enabled]);

  return { leftRef, rightRef };
}

// ============================================================================
// Sub-components
// ============================================================================

interface HunkHeaderProps {
  header: DiffHunk["header"];
  style: DiffViewStyle;
}

const HunkHeader = React.memo(function HunkHeader({
  header,
  style,
}: HunkHeaderProps) {
  return (
    <div className="diff-separator" data-line-type="separator">
      <div className="diff-separator-number" data-column-number="" />
      {style === "unified" && (
        <div className="diff-separator-number" data-column-number="" />
      )}
      <div className="diff-separator-content" data-column-content="">
        <span className="diff-separator-specs">
          @@ -{header.oldStart},{header.oldCount} +{header.newStart},
          {header.newCount} @@
        </span>
        {header.context && (
          <span className="diff-separator-context">{header.context}</span>
        )}
      </div>
    </div>
  );
});

// ============================================================================
// Unified View
// ============================================================================

interface UnifiedViewProps {
  hunks: DiffHunk[];
  highlightCache: Map<string, string>;
  showLineNumbers: boolean;
  onLineClick?: DiffViewerProps["onLineClick"];
}

const UnifiedView = React.memo(function UnifiedView({
  hunks,
  highlightCache,
  showLineNumbers,
  onLineClick,
}: UnifiedViewProps) {
  // Get highlighted HTML - inline diff is already applied by worker
  const getLineHtml = useCallback(
    (line: DiffLine): string => {
      const cached = highlightCache.get(line.content);
      return cached ?? escapeHtml(line.content);
    },
    [highlightCache],
  );

  return (
    <div className="diff-code" data-code="">
      {hunks.map((hunk, hunkIdx) => (
        <React.Fragment key={`hunk-${hunkIdx}`}>
          <HunkHeader header={hunk.header} style="unified" />
          {hunk.lines.map((line, lineIdx) => {
            const lineType = getLineTypeAttr(line.type);
            const html = getLineHtml(line);

            return (
              <div
                key={`line-${hunkIdx}-${lineIdx}`}
                className="diff-line"
                data-line-type={lineType}
                onClick={() =>
                  onLineClick?.(line, line.type === "deletion" ? "old" : "new")
                }
              >
                {showLineNumbers && (
                  <>
                    <div className="diff-line-number" data-column-number="">
                      <span>{line.oldLineNumber ?? ""}</span>
                    </div>
                    <div className="diff-line-number" data-column-number="">
                      <span>{line.newLineNumber ?? ""}</span>
                    </div>
                  </>
                )}
                <div
                  className="diff-line-content"
                  data-column-content=""
                  dangerouslySetInnerHTML={{ __html: html || "&nbsp;" }}
                />
              </div>
            );
          })}
        </React.Fragment>
      ))}
    </div>
  );
});

// ============================================================================
// Split View
// ============================================================================

interface SplitPair {
  left: DiffLine | null;
  right: DiffLine | null;
}

interface SplitViewProps {
  hunks: DiffHunk[];
  highlightCache: Map<string, string>;
  showLineNumbers: boolean;
  onLineClick?: DiffViewerProps["onLineClick"];
}

const SplitView = React.memo(function SplitView({
  hunks,
  highlightCache,
  showLineNumbers,
  onLineClick,
}: SplitViewProps) {
  // Group lines into side-by-side pairs
  const groupedHunks = useMemo(() => {
    return hunks.map((hunk) => {
      const pairs: SplitPair[] = [];
      const lines = hunk.lines;

      for (let i = 0; i < lines.length; i++) {
        const line = lines[i];
        if (!line) continue;

        if (line.type === "context") {
          pairs.push({ left: line, right: line });
          continue;
        }

        if (line.type === "deletion") {
          // Check if next line is addition (paired change)
          const next = lines[i + 1];
          if (next && next.type === "addition") {
            pairs.push({ left: line, right: next });
            i++; // Skip the next line
          } else {
            pairs.push({ left: line, right: null });
          }
          continue;
        }

        if (line.type === "addition") {
          pairs.push({ left: null, right: line });
        }
      }

      return { header: hunk.header, pairs };
    });
  }, [hunks]);

  // Get highlighted HTML - inline diff is already applied by worker
  const getLineHtml = useCallback(
    (line: DiffLine | null): string => {
      if (!line) return "";
      const cached = highlightCache.get(line.content);
      return cached ?? escapeHtml(line.content);
    },
    [highlightCache],
  );

  // Scroll sync between left and right panels
  const { leftRef, rightRef } = useScrollSync(true);

  // Flatten all pairs with hunk headers for rendering
  type RenderItem =
    | { type: "header"; header: DiffHunk["header"]; hunkIdx: number }
    | { type: "pair"; pair: SplitPair; hunkIdx: number; pairIdx: number };

  const renderItems = useMemo(() => {
    const items: RenderItem[] = [];
    groupedHunks.forEach((hunk, hunkIdx) => {
      items.push({ type: "header", header: hunk.header, hunkIdx });
      hunk.pairs.forEach((pair, pairIdx) => {
        items.push({ type: "pair", pair, hunkIdx, pairIdx });
      });
    });
    return items;
  }, [groupedHunks]);

  return (
    <div className="diff-split-container">
      {/* Left side (deletions) */}
      <div
        ref={leftRef}
        className="diff-code diff-code--left"
        data-code=""
        data-deletions=""
      >
        {renderItems.map((item) => {
          if (item.type === "header") {
            return (
              <div
                key={`header-left-${item.hunkIdx}`}
                className="diff-split-separator-side"
                data-line-type="separator"
              >
                <span className="diff-separator-specs">
                  @@ -{item.header.oldStart},{item.header.oldCount} +
                  {item.header.newStart},{item.header.newCount} @@
                </span>
                {item.header.context && (
                  <span className="diff-separator-context">
                    {item.header.context}
                  </span>
                )}
              </div>
            );
          }

          const leftHtml = getLineHtml(item.pair.left);
          const leftType = item.pair.left
            ? getLineTypeAttr(item.pair.left.type)
            : "empty";

          return (
            <div
              key={`line-left-${item.hunkIdx}-${item.pairIdx}`}
              className="diff-line"
              data-line-type={leftType}
              onClick={() =>
                item.pair.left && onLineClick?.(item.pair.left, "old")
              }
            >
              {showLineNumbers && (
                <div className="diff-line-number" data-column-number="">
                  <span>{item.pair.left?.oldLineNumber ?? ""}</span>
                </div>
              )}
              <div
                className="diff-line-content"
                data-column-content=""
                dangerouslySetInnerHTML={{ __html: leftHtml || "&nbsp;" }}
              />
            </div>
          );
        })}
      </div>

      {/* Right side (additions) */}
      <div
        ref={rightRef}
        className="diff-code diff-code--right"
        data-code=""
        data-additions=""
      >
        {renderItems.map((item) => {
          if (item.type === "header") {
            // Empty placeholder to maintain alignment with left side header
            return (
              <div
                key={`header-right-${item.hunkIdx}`}
                className="diff-split-separator-side diff-split-separator-side--empty"
                data-line-type="separator"
              />
            );
          }

          const rightHtml = getLineHtml(item.pair.right);
          const rightType = item.pair.right
            ? getLineTypeAttr(item.pair.right.type)
            : "empty";

          return (
            <div
              key={`line-right-${item.hunkIdx}-${item.pairIdx}`}
              className="diff-line"
              data-line-type={rightType}
              onClick={() =>
                item.pair.right && onLineClick?.(item.pair.right, "new")
              }
            >
              {showLineNumbers && (
                <div className="diff-line-number" data-column-number="">
                  <span>{item.pair.right?.newLineNumber ?? ""}</span>
                </div>
              )}
              <div
                className="diff-line-content"
                data-column-content=""
                dangerouslySetInnerHTML={{ __html: rightHtml || "&nbsp;" }}
              />
            </div>
          );
        })}
      </div>
    </div>
  );
});

// ============================================================================
// Main Component
// ============================================================================

export function DiffViewer({
  patch,
  diff: diffProp,
  fileIndex = 0,
  options = {},
  className = "",
  style,
  onLineClick,
}: DiffViewerProps): React.JSX.Element {
  const {
    style: viewStyle = "split",
    showLineNumbers = true,
    highlightInlineDiff = true,
    theme = "github-dark",
    wrapLines = false,
  } = options;

  // Parse patch string if provided, otherwise use diff prop
  const diff = useMemo(() => {
    if (patch) {
      const parsed = parsePatch(patch);
      return parsed.files[fileIndex] ?? null;
    }
    return diffProp ?? null;
  }, [patch, diffProp, fileIndex]);

  // Resolve theme to a single theme string
  const resolvedTheme: SupportedTheme =
    typeof theme === "string" ? theme : theme.dark;

  // Process diff in worker pool with streaming results
  // Inline diff is computed in worker, so we pass highlightInlineDiff option
  // Results stream back progressively - we show content immediately
  const {
    cache: highlightCache,
    ready,
    progress: _progress,
    error: _error,
  } = useDiffWorker(diff, {
    theme: resolvedTheme,
    highlightInline: highlightInlineDiff,
  });

  // Build container attributes
  const containerClass = [
    "diff-viewer",
    wrapLines && "diff-viewer--wrap",
    !ready && "diff-viewer--loading",
    className,
  ]
    .filter(Boolean)
    .join(" ");

  // Handle null diff (invalid patch or missing prop)
  if (!diff) {
    return (
      <div
        className={containerClass}
        style={style}
        data-diffs=""
        data-type={viewStyle}
      >
        <div className="diff-notice">No diff to display</div>
      </div>
    );
  }

  // Handle empty diff
  if (diff.hunks.length === 0) {
    if (diff.isBinary) {
      return (
        <div
          className={containerClass}
          style={style}
          data-diffs=""
          data-type={viewStyle}
        >
          <div className="diff-notice">Binary file not shown</div>
        </div>
      );
    }

    return (
      <div
        className={containerClass}
        style={style}
        data-diffs=""
        data-type={viewStyle}
      >
        <div className="diff-notice">No changes</div>
      </div>
    );
  }

  // Show diff immediately - highlighting streams in progressively
  // Lines without highlighting yet will fall back to escaped HTML
  return (
    <pre
      className={containerClass}
      style={style}
      data-diffs=""
      data-type={viewStyle}
      data-overflow={wrapLines ? "wrap" : "scroll"}
    >
      <code>
        {viewStyle === "split" ? (
          <SplitView
            hunks={diff.hunks}
            highlightCache={highlightCache}
            showLineNumbers={showLineNumbers}
            onLineClick={onLineClick}
          />
        ) : (
          <UnifiedView
            hunks={diff.hunks}
            highlightCache={highlightCache}
            showLineNumbers={showLineNumbers}
            onLineClick={onLineClick}
          />
        )}
      </code>
    </pre>
  );
}

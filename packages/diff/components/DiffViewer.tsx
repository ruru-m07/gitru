"use client";

import React, {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";

import type {
  DiffHunk,
  DiffLine,
  DiffViewerOptions,
  DiffViewStyle,
  FileDiff,
  InlineDiffSegment,
  SupportedTheme,
} from "../types";
import { escapeHtml } from "../utils/common";
import { computeInlineDiff } from "../utils/inline-diff";
import { getHighlightClient } from "../worker";

// ============================================================================
// Types
// ============================================================================

export interface DiffViewerProps {
  /** Parsed file diff to render */
  diff: FileDiff;
  /** Viewer options */
  options?: DiffViewerOptions;
  /** Custom class name */
  className?: string;
  /** Custom styles */
  style?: React.CSSProperties;
  /** Callback when a line is clicked */
  onLineClick?: (line: DiffLine, side: "old" | "new") => void;
}

interface HighlightState {
  /** Map of line content to highlighted HTML */
  cache: Map<string, string>;
  /** Whether highlighting is complete */
  ready: boolean;
  /** Error if highlighting failed */
  error?: Error;
}

// ============================================================================
// Hooks
// ============================================================================

function useHighlighting(
  diff: FileDiff,
  theme: SupportedTheme,
): HighlightState {
  const [state, setState] = useState<HighlightState>({
    cache: new Map(),
    ready: false,
  });

  useEffect(() => {
    let cancelled = false;

    (async () => {
      const startTime = performance.now();

      async function highlightAll() {
        const client = getHighlightClient();
        const cache = new Map<string, string>();

        // Collect all unique line contents
        const uniqueLines = new Set<string>();
        for (const hunk of diff.hunks) {
          for (const line of hunk.lines) {
            if (line.content && line.type !== "metadata") {
              uniqueLines.add(line.content);
            }
          }
        }

        // Highlight all lines
        try {
          const lineArray = Array.from(uniqueLines);

          // Process in batches to avoid overwhelming the worker
          const batchSize = 50;
          for (let i = 0; i < lineArray.length; i += batchSize) {
            if (cancelled) return;

            const batch = lineArray.slice(i, i + batchSize);
            const responses = await Promise.all(
              batch.map((content) =>
                client.highlight(content, diff.language, theme),
              ),
            );

            for (let j = 0; j < batch.length; j++) {
              const batchItem = batch[j];
              const response = responses[j];
              // Extract inner HTML from shiki output
              if (batchItem && response) {
                const html = response.html;
                const inner = extractInnerHtml(html);
                cache.set(batchItem, inner);
              }
            }
          }

          if (!cancelled) {
            setState({ cache, ready: true });
          }
        } catch (error) {
          if (!cancelled) {
            setState({ cache, ready: true, error: error as Error });
          }
        }
      }

      await highlightAll();

      const endTime = performance.now();
      console.log(
        `[DiffViewer] Highlighting initiated in:::::::::::::::: ${endTime - startTime} ms`,
      );
    })();

    return () => {
      cancelled = true;
    };
  }, [diff, theme]);

  return state;
}

/**
 * Extract the inner content from shiki's HTML output
 */
function extractInnerHtml(html: string): string {
  // Shiki wraps output in <pre><code>...</code></pre>
  const codeStart = html.indexOf("<code>");
  const codeEnd = html.lastIndexOf("</code>");

  if (codeStart !== -1 && codeEnd !== -1) {
    return html.slice(codeStart + 6, codeEnd);
  }

  // Try <span> wrapper (single line)
  const spanMatch = html.match(/<span[^>]*>([\s\S]*)<\/span>/);
  if (spanMatch) {
    return spanMatch[0];
  }

  return html;
}

// ============================================================================
// Inline Diff Utility
// ============================================================================

/**
 * Apply inline diff markers to already syntax-highlighted HTML.
 * This wraps changed segments with <span data-diff-inline> without breaking existing spans.
 */
function applyInlineDiffToHighlighted(
  plainText: string,
  highlightedHtml: string,
  segments: InlineDiffSegment[],
): string {
  // If no changed segments, return the highlighted HTML as-is
  const hasChanges = segments.some((s) => s.added || s.removed);
  if (!hasChanges) {
    return highlightedHtml;
  }

  // Build a map of character positions that should be marked
  const markedPositions = new Set<number>();
  let pos = 0;
  for (const segment of segments) {
    if (segment.added || segment.removed) {
      for (let i = 0; i < segment.text.length; i++) {
        markedPositions.add(pos + i);
      }
    }
    pos += segment.text.length;
  }

  // Walk through the highlighted HTML and wrap marked characters
  let result = "";
  let textPos = 0;
  let inMarkedSpan = false;
  let i = 0;

  while (i < highlightedHtml.length) {
    // Check if we're at a tag
    if (highlightedHtml[i] === "<") {
      // Close any open marked span before the tag
      if (inMarkedSpan) {
        result += "</span>";
        inMarkedSpan = false;
      }

      // Find the end of the tag
      const tagEnd = highlightedHtml.indexOf(">", i);
      if (tagEnd === -1) break;

      // Copy the tag as-is
      result += highlightedHtml.slice(i, tagEnd + 1);
      i = tagEnd + 1;
      continue;
    }

    // Handle HTML entities (e.g., &lt;, &gt;, &amp;)
    if (highlightedHtml[i] === "&") {
      const entityEnd = highlightedHtml.indexOf(";", i);
      if (entityEnd !== -1 && entityEnd - i < 10) {
        const shouldMark = markedPositions.has(textPos);

        if (shouldMark && !inMarkedSpan) {
          result += `<span data-diff-inline="">`;
          inMarkedSpan = true;
        } else if (!shouldMark && inMarkedSpan) {
          result += "</span>";
          inMarkedSpan = false;
        }

        result += highlightedHtml.slice(i, entityEnd + 1);
        i = entityEnd + 1;
        textPos++;
        continue;
      }
    }

    // Regular character
    const shouldMark = markedPositions.has(textPos);

    if (shouldMark && !inMarkedSpan) {
      result += `<span data-diff-inline="">`;
      inMarkedSpan = true;
    } else if (!shouldMark && inMarkedSpan) {
      result += "</span>";
      inMarkedSpan = false;
    }

    result += highlightedHtml[i];
    textPos++;
    i++;
  }

  // Close any remaining marked span
  if (inMarkedSpan) {
    result += "</span>";
  }

  return result;
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

/** Split view hunk header that spans across both columns */
interface SplitHunkHeaderProps {
  header: DiffHunk["header"];
}

const SplitHunkHeader = React.memo(function SplitHunkHeader({
  header,
}: SplitHunkHeaderProps) {
  return (
    <div className="diff-split-separator" data-line-type="separator">
      <span className="diff-separator-specs">
        @@ -{header.oldStart},{header.oldCount} +{header.newStart},
        {header.newCount} @@
      </span>
      {header.context && (
        <span className="diff-separator-context">{header.context}</span>
      )}
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
  highlightInline: boolean;
  onLineClick?: DiffViewerProps["onLineClick"];
}

const UnifiedView = React.memo(function UnifiedView({
  hunks,
  highlightCache,
  showLineNumbers,
  highlightInline,
  onLineClick,
}: UnifiedViewProps) {
  const getLineHtml = useCallback(
    (line: DiffLine): string => {
      const cached = highlightCache.get(line.content);
      return cached ?? escapeHtml(line.content);
    },
    [highlightCache],
  );

  // Pre-compute inline diffs for paired deletion/addition lines
  const inlineDiffMap = useMemo(() => {
    if (!highlightInline) return new Map<DiffLine, string>();

    const map = new Map<DiffLine, string>();

    for (const hunk of hunks) {
      const lines = hunk.lines;
      for (let i = 0; i < lines.length; i++) {
        const line = lines[i];
        if (!line || line.type !== "deletion") continue;

        // Look for immediately following addition
        const next = lines[i + 1];
        if (next && next.type === "addition") {
          // Compute inline diff
          const segments = computeInlineDiff(line.content, next.content);

          // Apply to deletion line
          const deletionSegments = segments.filter(
            (s: InlineDiffSegment) => !s.added,
          );
          const deletionHtml = applyInlineDiffToHighlighted(
            line.content,
            getLineHtml(line),
            deletionSegments,
          );
          map.set(line, deletionHtml);

          // Apply to addition line
          const additionSegments = segments.filter(
            (s: InlineDiffSegment) => !s.removed,
          );
          const additionHtml = applyInlineDiffToHighlighted(
            next.content,
            getLineHtml(next),
            additionSegments,
          );
          map.set(next, additionHtml);
        }
      }
    }

    return map;
  }, [hunks, highlightInline, getLineHtml]);

  const getFinalHtml = useCallback(
    (line: DiffLine): string => {
      // Check if we have an inline diff version
      const inlineDiffHtml = inlineDiffMap.get(line);
      if (inlineDiffHtml) return inlineDiffHtml;
      return getLineHtml(line);
    },
    [inlineDiffMap, getLineHtml],
  );

  return (
    <div className="diff-code" data-code="">
      {hunks.map((hunk, hunkIdx) => (
        <React.Fragment key={`hunk-${hunkIdx}`}>
          <HunkHeader header={hunk.header} style="unified" />
          {hunk.lines.map((line, lineIdx) => {
            const lineType = getLineTypeAttr(line.type);
            const html = getFinalHtml(line);

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
  highlightInline: boolean;
  onLineClick?: DiffViewerProps["onLineClick"];
}

const SplitView = React.memo(function SplitView({
  hunks,
  highlightCache,
  showLineNumbers,
  highlightInline,
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

  const getLineHtml = useCallback(
    (line: DiffLine | null): string => {
      if (!line) return "";
      const cached = highlightCache.get(line.content);
      return cached ?? escapeHtml(line.content);
    },
    [highlightCache],
  );

  const computeInlineDiffHtml = useCallback(
    (
      left: DiffLine | null,
      right: DiffLine | null,
    ): { leftHtml: string; rightHtml: string } => {
      // If not highlighting inline or not a paired change, return regular highlighted HTML
      if (
        !highlightInline ||
        !left ||
        !right ||
        left.type !== "deletion" ||
        right.type !== "addition"
      ) {
        return {
          leftHtml: getLineHtml(left),
          rightHtml: getLineHtml(right),
        };
      }

      // Get the syntax highlighted versions
      const leftHighlighted = getLineHtml(left);
      const rightHighlighted = getLineHtml(right);

      // Compute word-level diff on plain text
      const segments = computeInlineDiff(left.content, right.content);

      // Build left (deletion) HTML - filter out "added" segments
      const leftSegments = segments.filter((s: InlineDiffSegment) => !s.added);
      const leftHtml = applyInlineDiffToHighlighted(
        left.content,
        leftHighlighted,
        leftSegments,
      );

      // Build right (addition) HTML - filter out "removed" segments
      const rightSegments = segments.filter(
        (s: InlineDiffSegment) => !s.removed,
      );
      const rightHtml = applyInlineDiffToHighlighted(
        right.content,
        rightHighlighted,
        rightSegments,
      );

      return { leftHtml, rightHtml };
    },
    [highlightInline, getLineHtml],
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

          const { leftHtml } = computeInlineDiffHtml(
            item.pair.left,
            item.pair.right,
          );
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

          const { rightHtml } = computeInlineDiffHtml(
            item.pair.left,
            item.pair.right,
          );
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
  diff,
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

  // Resolve theme to a single theme string
  const resolvedTheme: SupportedTheme =
    typeof theme === "string" ? theme : theme.dark;

  // Highlight all lines
  const {
    cache: highlightCache,
    ready,
    error: _error,
  } = useHighlighting(diff, resolvedTheme);

  // Build container attributes
  const containerClass = [
    "diff-viewer",
    wrapLines && "diff-viewer--wrap",
    className,
  ]
    .filter(Boolean)
    .join(" ");

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

  // Show loading state while highlighting
  if (!ready) {
    return (
      <div
        className={containerClass}
        style={style}
        data-diffs=""
        data-type={viewStyle}
      >
        <div className="diff-notice">Loading...</div>
      </div>
    );
  }

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
            highlightInline={highlightInlineDiff}
            onLineClick={onLineClick}
          />
        ) : (
          <UnifiedView
            hunks={diff.hunks}
            highlightCache={highlightCache}
            showLineNumbers={showLineNumbers}
            highlightInline={highlightInlineDiff}
            onLineClick={onLineClick}
          />
        )}
      </code>
    </pre>
  );
}

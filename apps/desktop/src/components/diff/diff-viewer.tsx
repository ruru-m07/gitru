import type { FileStatusKind, GetDiffResponse } from "@gitru/commands";
import { Card, CardContent } from "@gitru/ui/components/card";
import { FoldVertical } from "lucide-react";
import React, {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { highlighter } from "../highlighter";
import { DiffHunk, DiffRow, DiffSegment } from "./diff-types";
import {
  buildDiffSegments,
  buildHunksFromDiff,
  computeInlineDiff,
  escapeHtml,
  formatBytes,
  formatRange,
  resolveLanguage,
  splitLines,
} from "./diff-utils";
import { useDiffViewerSettings } from "./useDiffViewSettingStore";

const highlightCache = new Map<string, string>();

const HIGHLIGHT_CACHE = new Map<string, string>();
const MAX_CACHE = 1000;

// TODO(ruru-m07): enable inline diff later
const INLINE_DIFF = false;

// ? LRU trim
function cacheSet(key: string, value: string) {
  if (HIGHLIGHT_CACHE.size > MAX_CACHE) {
    const value = HIGHLIGHT_CACHE.keys().next().value;
    if (value) {
      HIGHLIGHT_CACHE.delete(value);
    }
  }
  HIGHLIGHT_CACHE.set(key, value);
}

function highlightCode(code: string, language: string): string {
  if (!highlighter || !code) return escapeHtml(code);
  if (language === "plaintext") return escapeHtml(code);

  const key = `${language}:${code}`;
  const cached = HIGHLIGHT_CACHE.get(key);
  if (cached) return cached;

  const html = highlighter.codeToHtml(code, {
    lang: language,
    themes: {
      light: "vesper-light",
      "dark-classic": "vesper",
    },
    defaultColor: "light",
    cssVariablePrefix: "--shiki-",
  });

  const inner = html.slice(
    html.indexOf("<code>") + 6,
    html.lastIndexOf("</code>"),
  );

  cacheSet(key, inner);
  return inner;
}

export function DiffViewer({
  diff,
  filePath,
  status,
}: {
  diff: GetDiffResponse | null;
  filePath: string;
  status?: FileStatusKind[];
}) {
  const { diffStyle: viewMode } = useDiffViewerSettings();
  const language = useMemo(() => resolveLanguage(filePath), [filePath]);
  const [expandedSkips, setExpandedSkips] = useState<Set<string>>(new Set());

  const masterScrollRef = useRef<HTMLDivElement>(null);
  const [sharedScrollX, setSharedScrollX] = useState(0);
  const [sharedMasterWidth, setSharedMasterWidth] = useState(0);
  const segmentWidthsRef = useRef<Map<string, number>>(new Map());

  const updateMasterWidth = useCallback((segmentId: string, width: number) => {
    segmentWidthsRef.current.set(segmentId, width);
    const maxWidth = Math.max(...segmentWidthsRef.current.values(), 0);
    setSharedMasterWidth(maxWidth);
  }, []);

  const onSharedWheel = useCallback((e: React.WheelEvent) => {
    if (!masterScrollRef.current) return;

    if (Math.abs(e.deltaX) > Math.abs(e.deltaY)) {
      e.preventDefault();
      masterScrollRef.current.scrollLeft += e.deltaX;
    }
  }, []);

  useEffect(() => {
    const el = masterScrollRef.current;
    if (!el) return;

    const onScroll = () => {
      setSharedScrollX(el.scrollLeft);
    };

    el.addEventListener("scroll", onScroll, { passive: true });
    return () => el.removeEventListener("scroll", onScroll);
  }, []);

  const statusSet = useMemo(() => new Set(status ?? []), [status]);
  const treatAsNewFile =
    statusSet.has("WorktreeNew") ||
    statusSet.has("IndexNew") ||
    (!!diff?.workdir && !diff?.head);
  const treatAsDeletedFile =
    statusSet.has("WorktreeDeleted") ||
    statusSet.has("IndexDeleted") ||
    (!!diff?.head && !diff?.workdir);

  const binaryVersion = useMemo(() => {
    if (!diff) return null;
    if (diff.workdir?.is_binary) return diff.workdir;
    if (diff.head?.is_binary) return diff.head;
    return null;
  }, [diff]);

  const hunks = useMemo(() => {
    if (!diff || binaryVersion) {
      return [] as DiffHunk[];
    }

    const before = diff.head?.content ?? "";
    const after = diff.workdir?.content ?? "";

    return buildHunksFromDiff(before, after, {
      treatAsNewFile,
      treatAsDeletedFile,
    });
  }, [diff, binaryVersion, treatAsNewFile, treatAsDeletedFile]);

  const beforeLines = useMemo(
    () => splitLines(diff?.head?.content ?? ""),
    [diff?.head?.content],
  );
  const afterLines = useMemo(
    () => splitLines(diff?.workdir?.content ?? ""),
    [diff?.workdir?.content],
  );

  const segments = useMemo(() => {
    if (!diff || binaryVersion) {
      return [] as DiffSegment[];
    }

    return buildDiffSegments(hunks, beforeLines, afterLines);
  }, [diff, binaryVersion, hunks, beforeLines, afterLines]);

  const toggleSkip = useCallback((id: string) => {
    setExpandedSkips((prev) => {
      const next = new Set(prev);
      if (next.has(id)) {
        next.delete(id);
      } else {
        next.add(id);
      }
      return next;
    });
  }, []);

  // TODO(ruru-m07): will implement empty svg and something batter later
  // const hasAnyRows = hunks.some((hunk) => hunk.lines.length > 0);

  const highlight = useCallback(
    (code: string) => {
      if (!highlighter || !code) return escapeHtml(code);

      const cacheKey = `${language}:${code}`;
      const cached = highlightCache.get(cacheKey);
      if (cached) return cached;

      try {
        const html = highlighter.codeToHtml(code, {
          lang: language,
          themes: {
            light: "vesper-light",
            "dark-classic": "vesper",
          },
          defaultColor: "light",
          cssVariablePrefix: "--shiki-",
        });

        const inner = html.slice(
          html.indexOf("<code>") + 6,
          html.lastIndexOf("</code>"),
        );

        highlightCache.set(cacheKey, inner);
        return inner;
      } catch {
        return escapeHtml(code);
      }
    },
    [highlighter, language],
  );

  const highlightedLines = useMemo(() => {
    if (!segments.length) return new Map<string, string>();

    const map = new Map<string, string>();

    for (const segment of segments) {
      if (segment.type !== "hunk") continue;

      for (const line of segment.hunk.lines) {
        if (line.metadata) continue;
        map.set(line.content, highlightCode(line.content, language));
      }
    }

    return map;
  }, [segments, language]);

  const renderUnifiedView = (lines: DiffRow[], keyPrefix: string) => (
    <div className="overflow-x-auto shiki bg-background">
      <table className="w-full border-collapse diff-content">
        <tbody>
          {lines.map((line, index) => {
            let bgClass = "";
            if (line.type === "added") bgClass = "diff-added";
            if (line.type === "removed") bgClass = "diff-removed";

            const key = `${keyPrefix}-${line.lineNumberOld ?? "x"}-${line.lineNumberNew ?? "y"}-${index}`;

            let contentHtml: string;

            if (
              INLINE_DIFF &&
              (line.type === "added" || line.type === "removed") &&
              !line.metadata
            ) {
              const oppositeType = line.type === "added" ? "removed" : "added";
              const oppositeLine = lines.find(
                (l, i) => l.type === oppositeType && Math.abs(i - index) <= 1, // Look at adjacent lines
              );

              if (oppositeLine) {
                const oldText =
                  line.type === "removed" ? line.content : oppositeLine.content;
                const newText =
                  line.type === "added" ? line.content : oppositeLine.content;
                const inlineDiff = computeInlineDiff(oldText, newText);

                // Build HTML with inline highlighting
                const parts = inlineDiff
                  .filter((part) => {
                    if (line.type === "removed") return !part.added;
                    if (line.type === "added") return !part.removed;
                    return true;
                  })
                  .map((part) => {
                    const highlighted =
                      highlightedLines.get(part.value) ??
                      escapeHtml(part.value);

                    if (
                      (line.type === "removed" && part.removed) ||
                      (line.type === "added" && part.added)
                    ) {
                      // Stronger highlight for actual changes
                      return `<span class="diff-word-${line.type}">${highlighted}</span>`;
                    }
                    return highlighted;
                  })
                  .join("");

                contentHtml = parts;
              } else {
                contentHtml = highlight(line.content);
              }
            } else {
              contentHtml = line.metadata
                ? escapeHtml(line.content)
                : highlight(line.content);
            }

            const textClass = line.metadata
              ? "italic text-muted-foreground"
              : undefined;

            return (
              <DiffRowView
                key={key}
                line={line}
                html={contentHtml}
                bgClass={bgClass}
                textClass={textClass}
                oldNo={line.lineNumberOld}
                newNo={line.lineNumberNew}
              />
            );
          })}
        </tbody>
      </table>
    </div>
  );

  if (binaryVersion) {
    return (
      <Card className="overflow-hidden rounded-none border border-border/50">
        <CardContent className="p-6">
          <p className="text-sm text-muted-foreground">
            Binary file preview is not supported. Size:{" "}
            {formatBytes(binaryVersion.byte_length)}
          </p>
        </CardContent>
      </Card>
    );
  }

  // TODO(ruru-m07): will implement empty svg and something batter later
  // if (!hasAnyRows) {
  //   return (
  //     <Card className="overflow-hidden rounded-none border border-border/50">
  //       <CardContent className="p-6">
  //         <p className="text-sm text-muted-foreground">
  //           No content to display for this file.
  //         </p>
  //       </CardContent>
  //     </Card>
  //   );
  // }

  return (
    <Card className="overflow-hidden rounded-none! before:rounded-none border-0 py-0">
      {/* // TODO(ruru-m07): will do something better here */}
      {/* {treatAsNewFile || treatAsDeletedFile ? (
				<div
					className={`px-4 py-2 text-xs font-medium border-b border-border/50 ${treatAsNewFile ? "bg-emerald-500/10 text-emerald-300" : "bg-red-500/10 text-red-300"}`}
				>
					{treatAsNewFile
						? "Untracked file – displaying working tree contents"
						: "Deleted file – displaying last committed contents"}
				</div>
			) : null} */}
      <CardContent className="p-0 rounded-none!">
        {viewMode === "split" && (
          <div
            ref={masterScrollRef}
            className="overflow-x-auto overflow-y-hidden h-0"
          >
            <div style={{ width: sharedMasterWidth, height: 1 }} />
          </div>
        )}
        <div className="divide-y divide-border/50">
          {segments.map((segment) => {
            if (segment.type === "skip") {
              const isExpanded = expandedSkips.has(segment.id);
              // const lineCount = segment.lines.length;
              const oldCount =
                segment.oldStart > 0 && segment.oldEnd >= segment.oldStart
                  ? segment.oldEnd - segment.oldStart + 1
                  : 0;
              const newCount =
                segment.newStart > 0 && segment.newEnd >= segment.newStart
                  ? segment.newEnd - segment.newStart + 1
                  : 0;
              const header = `@@ -${formatRange(segment.oldStart, oldCount)} +${formatRange(segment.newStart, newCount)} @@`;

              return (
                <div key={segment.id} className="">
                  {isExpanded ? (
                    <div className="bg-(--diff-segment-diff-bg) [--diff-segment-bg-unified:var(--diff-segment-diff-bg)] diff-line-number-segment">
                      {viewMode === "split" ? (
                        <RenderSplitView
                          lines={segment.lines}
                          keyPrefix={segment.id}
                          highlight={highlight}
                          highlightedLines={highlightedLines}
                          scrollX={sharedScrollX}
                          onWheel={onSharedWheel}
                          onWidthChange={(w) =>
                            updateMasterWidth(segment.id, w)
                          }
                        />
                      ) : (
                        renderUnifiedView(segment.lines, segment.id)
                      )}
                    </div>
                  ) : (
                    <div className="flex items-center justify-between gap-4 bg-(--diff-segment-bg) _border-y-[0.5px] _border-primary/10">
                      <div className="flex items-center">
                        <div
                          onClick={() => toggleSkip(segment.id)}
                          className="w-[calc(calc(var(--spacing)*12)-1px)] py-1.5 text-muted-foreground hover:text-foreground cursor-pointer hover:bg-(--diff-segment-expend-button-bg-hover) bg-(--diff-segment-expend-button-bg) flex items-center justify-center"
                        >
                          <FoldVertical size={16} />
                        </div>
                        <span className="font-mono text-xs sm:text-sm text-muted-foreground! pl-3 py-1 border-l tabular-nums border-(--diff-segment-expend-button-border) ">
                          {header}
                        </span>
                      </div>
                      <span className="flex items-center gap-2 text-xs pr-4">
                        {/* // TODO(ruru-m07):  */}
                        {/* {lineCount} */}
                      </span>
                    </div>
                  )}
                </div>
              );
            }

            const { hunk } = segment;

            return (
              <div key={hunk.id} className="bg-background">
                {/* // TODO(ruru): find out a way to show the header only when there are changes */}
                {/* {false ? null : (
                  <div className="flex items-center justify-between gap-4 bg-primary/10 border-y border-primary/10">
                    <div className="flex items-center">
                      <div className="w-[calc(calc(var(--spacing)_*_12)+1px)] py-1.5 border-r text-muted-foreground hover:text-foreground cursor-pointer border-primary/10 hover:bg-primary/30 flex items-center justify-center">
                        <FoldVertical size={16} />
                      </div>
                      <span className="font-mono text-xs sm:text-sm text-primary/80 pl-3 py-1">
                        {hunk.header}
                      </span>
                    </div>
                    <span className="flex items-center gap-2 text-xs pr-4">
                      <span className="text-green-700 tabular-nums">
                        +{hunk.additions}
                      </span>
                      <span className="text-red-700 tabular-nums">
                        -{hunk.deletions}
                      </span>
                    </span>
                  </div>
                )} */}

                <div className="_border-t _border-border/40">
                  {viewMode === "split" ? (
                    <RenderSplitView
                      lines={hunk.lines}
                      keyPrefix={hunk.id}
                      highlight={highlight}
                      highlightedLines={highlightedLines}
                      scrollX={sharedScrollX}
                      onWheel={onSharedWheel}
                      onWidthChange={(w) => updateMasterWidth(hunk.id, w)}
                    />
                  ) : (
                    renderUnifiedView(hunk.lines, hunk.id)
                  )}
                </div>
              </div>
            );
          })}
        </div>
      </CardContent>
    </Card>
  );
}

export function HighlightedLine({
  html,
  className,
}: {
  html: string;
  className?: string;
}) {
  return (
    <span className={className} dangerouslySetInnerHTML={{ __html: html }} />
  );
}

const DiffRowView = React.memo(function DiffRowView({
  line,
  html,
  bgClass,
  textClass,
  oldNo,
  newNo,
}: {
  line: DiffRow;
  html: string;
  bgClass: string;
  textClass?: string;
  oldNo?: number | null;
  newNo?: number | null;
}) {
  return (
    <tr className={`${bgClass} diff-hover`}>
      <td
        className={`border-r py-1 w-12 text-muted-foreground text-xs select-none diff-line-number-segment-td ${line.type === "removed" && "diff-line-number-bg-removed"} ${line.type === "added" && "diff-line-number-bg-added"} diff-line-number-bg border-l-0`}
      >
        {/* // ! we can do text-right if we want a better number alignment */}
        <span className="font-mono px-3 block w-full text-center _text-right tabular-nums">
          {oldNo ?? ""}
        </span>
      </td>
      <td
        className={`border-r py-1 w-12 text-muted-foreground text-xs select-none diff-line-number-segment-td ${line.type === "removed" && "diff-line-number-bg-removed"} ${line.type === "added" && "diff-line-number-bg-added"} diff-line-number-bg`}
      >
        {/* // ! we can do text-right if we want a better number alignment */}
        <span className="font-mono px-3 block w-full text-center _text-right tabular-nums">
          {newNo ?? ""}
        </span>
      </td>
      <td className="pl-3 pr-2 text-sm font-mono leading-6 whitespace-pre bg-(--diff-segment-bg-unified)">
        <HighlightedLine className={textClass} html={html} />
      </td>
    </tr>
  );
});

const RenderSplitView = ({
  highlight,
  highlightedLines,
  keyPrefix,
  lines,
  scrollX,
  onWheel,
  onWidthChange,
}: {
  lines: DiffRow[];
  keyPrefix: string;
  highlight: (code: string) => string;
  highlightedLines: Map<string, string>;
  scrollX: number;
  onWheel: (e: React.WheelEvent) => void;
  onWidthChange: (width: number) => void;
}) => {
  const measureRefs = useRef<HTMLDivElement[]>([]);

  useEffect(() => {
    const measure = () => {
      const widths = measureRefs.current
        .filter(Boolean)
        .map((el) => el.scrollWidth);

      const maxWidth = Math.max(...widths, 0) + 1000;
      onWidthChange(maxWidth);
    };

    measure();

    const ro = new ResizeObserver(measure);
    measureRefs.current.forEach((el) => el && ro.observe(el));

    window.addEventListener("resize", measure);

    return () => {
      ro.disconnect();
      window.removeEventListener("resize", measure);
    };
  }, [onWidthChange]);

  const grouped: Array<{ left: DiffRow | null; right: DiffRow | null }> = [];

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];

    if (line.type === "context") {
      grouped.push({ left: line, right: line });
      continue;
    }

    if (line.type === "removed") {
      const nextLine = lines[i + 1];
      if (nextLine && nextLine.type === "added") {
        grouped.push({ left: line, right: nextLine });
        i++;
      } else {
        grouped.push({ left: line, right: null });
      }
      continue;
    }

    if (line.type === "added") {
      grouped.push({ left: null, right: line });
    }
  }

  return (
    <div>
      {grouped.map((pair, index) => {
        const leftBg = pair.left?.type === "removed" ? "diff-removed" : "";
        const rightBg = pair.right?.type === "added" ? "diff-added" : "";
        const key = `${keyPrefix}-${pair.left?.lineNumberOld ?? "x"}-${pair.right?.lineNumberNew ?? "y"}-${index}`;

        let leftHtml = "";
        let rightHtml = "";

        if (
          INLINE_DIFF &&
          pair.left &&
          pair.right &&
          pair.left.type === "removed" &&
          pair.right.type === "added" &&
          !pair.left.metadata &&
          !pair.right.metadata
        ) {
          // Compute inline diff for paired removed/added lines
          const inlineDiff = computeInlineDiff(
            pair.left.content,
            pair.right.content,
          );

          // Build left side (removed) HTML
          const leftParts = inlineDiff
            .filter((part) => !part.added)
            .map((part) => {
              const highlighted =
                highlightedLines.get(part.value) ?? escapeHtml(part.value);

              if (part.removed) {
                return `<span class="diff-word-removed">${highlighted}</span>`;
              }
              return highlighted;
            })
            .join("");
          leftHtml = leftParts;

          // Build right side (added) HTML
          const rightParts = inlineDiff
            .filter((part) => !part.removed)
            .map((part) => {
              const highlighted =
                highlightedLines.get(part.value) ?? escapeHtml(part.value);

              if (part.added) {
                return `<span class="diff-word-added">${highlighted}</span>`;
              }
              return highlighted;
            })
            .join("");
          rightHtml = rightParts;
        } else {
          // Regular highlighting without inline diff
          leftHtml = pair.left
            ? pair.left.metadata
              ? escapeHtml(pair.left.content)
              : highlight(pair.left.content)
            : "";
          rightHtml = pair.right
            ? pair.right.metadata
              ? escapeHtml(pair.right.content)
              : highlight(pair.right.content)
            : "";
        }

        return (
          <div key={key} className="h-6 w-full flex relative">
            <div
              className={`py-1 h-6 w-12 fit text-muted-foreground diff-line-number-segment-td text-xs select-none ${leftBg ? "diff-line-number-bg-removed" : "diff-line-number-bg border-r"} ${leftBg} border-l-0`}
            >
              <span className="px-3 block w-full text-center _text-right tabular-nums">
                {pair.left?.lineNumberOld ?? ""}
              </span>
            </div>

            <div
              className={`flex-1 h-6 overflow-hidden relative text-sm font-mono leading-6 whitespace-pre ${leftBg}`}
              onWheel={onWheel}
            >
              {pair.right ? (
                <div
                  ref={(el) => {
                    if (el) measureRefs.current[index * 2] = el;
                  }}
                  className="whitespace-pre text-sm invisible absolute pointer-events-none"
                >
                  <HighlightedLine
                    className={`_text-wrap _flex-wrap pl-3 pr-2 ${pair.right?.metadata ? "italic text-muted-foreground" : ""}`}
                    html={rightHtml}
                  />
                </div>
              ) : (
                <div
                  ref={(el) => {
                    if (el) measureRefs.current[index * 2] = el;
                  }}
                  className="whitespace-pre text-sm absolute pointer-events-none"
                >
                  <span className="[--pattern-fg:var(--input)]/50 w-full h-full">
                    <div className="_border-x h-6 w-full border-x-(--pattern-fg) bg-[repeating-linear-gradient(315deg,var(--pattern-fg)_0,var(--pattern-fg)_1.5px,transparent_0,transparent_50%)] bg-size-[9px_9px] bg-fixed"></div>
                  </span>
                </div>
              )}

              <div
                style={{
                  transform: pair.left
                    ? `translateX(-${scrollX}px)`
                    : `translateX(0px)`,
                }}
                className="whitespace-pre text-sm will-change-transform"
              >
                {pair.left ? (
                  <HighlightedLine
                    className={`_text-wrap _flex-wrap pl-3 pr-2 ${pair.left?.metadata ? "italic text-muted-foreground" : ""}`}
                    html={leftHtml}
                  />
                ) : (
                  <span className="[--pattern-fg:var(--input)]/50 h-full w-full">
                    <div className="_border-x h-6 w-full border-x-(--pattern-fg) bg-[repeating-linear-gradient(315deg,var(--pattern-fg)_0,var(--pattern-fg)_1.5px,transparent_0,transparent_50%)] bg-size-[9px_9px] bg-fixed"></div>
                  </span>
                )}
              </div>
            </div>

            <div
              className={`py-1 w-12 text-muted-foreground diff-line-number-segment-td text-xs select-none ${rightBg ? "diff-line-number-bg-added" : "diff-line-number-bg border-x"} ${rightBg}`}
            >
              <span className="px-3 block w-full text-center _text-right tabular-nums">
                {pair.right?.lineNumberNew ?? ""}
              </span>
            </div>

            <div
              className={`flex-1 h-6 overflow-hidden relative text-sm font-mono leading-6 whitespace-pre ${rightBg}`}
              onWheel={onWheel}
            >
              {pair.left ? (
                <div
                  ref={(el) => {
                    if (el) measureRefs.current[index * 2 + 1] = el;
                  }}
                  className="whitespace-pre text-sm invisible absolute pointer-events-none"
                >
                  <HighlightedLine
                    className={`_text-wrap _flex-wrap pl-3 pr-2 ${pair.left?.metadata ? "italic text-muted-foreground" : ""}`}
                    html={leftHtml}
                  />
                </div>
              ) : (
                <div
                  ref={(el) => {
                    if (el) measureRefs.current[index * 2 + 1] = el;
                  }}
                  className="whitespace-pre text-sm h-10 absolute pointer-events-none"
                >
                  <span className="[--pattern-fg:var(--input)]/50 w-full h-full">
                    <div className="_border-x h-6 w-full border-x-(--pattern-fg) bg-[repeating-linear-gradient(315deg,var(--pattern-fg)_0,var(--pattern-fg)_1.5px,transparent_0,transparent_50%)] bg-size-[9px_9px] bg-fixed"></div>
                  </span>
                </div>
              )}

              <div
                style={{
                  transform: pair.right
                    ? `translateX(-${scrollX}px)`
                    : `translateX(0px)`,
                }}
                className="whitespace-pre text-sm will-change-transform relative"
              >
                {pair.right ? (
                  <HighlightedLine
                    className={`_text-wrap _flex-wrap pl-3 pr-2 ${pair.right?.metadata ? "italic text-muted-foreground" : ""}`}
                    html={rightHtml}
                  />
                ) : (
                  <span className="[--pattern-fg:var(--input)]/50 w-full h-full">
                    <div className="_border-x h-6 w-full border-x-(--pattern-fg) bg-[repeating-linear-gradient(315deg,var(--pattern-fg)_0,var(--pattern-fg)_1.5px,transparent_0,transparent_50%)] bg-size-[9px_9px] bg-fixed"></div>
                  </span>
                )}
              </div>
            </div>
          </div>
        );
      })}
    </div>
  );
};

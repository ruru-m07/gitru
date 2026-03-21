"use client";

import { BlameInfo } from "@gitru/commands";
import { cn } from "@gitru/ui/lib/utils";
import { ChevronDownIcon, ChevronsUpDown, ChevronUpIcon } from "lucide-react";
import React from "react";
import { refractor } from "refractor/all";
import type {
  File,
  Hunk as HunkType,
  LineSegment,
  Line as LineType,
  ParseOptions,
  SkipBlock,
} from "./utils";
import { guessLang, parseDiff } from "./utils";
import "./style.css";
import "./theme.css";

const EXPENDED_SKIP_BLOCK_LINES = 10;
const SKIP_BLOCK_EXPAND_STEP = 20;
const VIRTUAL_HUNK_THRESHOLD = 1200;
const VIRTUAL_OVERSCAN_LINES = 200;
const ESTIMATED_ROW_HEIGHT = 20;

/* -------------------------------------------------------------------------- */
/*                                — Context —                                 */
/* -------------------------------------------------------------------------- */

interface DiffContextValue {
  language: string;
}

const DiffContext = React.createContext<DiffContextValue | null>(null);

function useDiffContext() {
  const context = React.useContext(DiffContext);
  if (!context) {
    throw new Error("useDiffContext must be used within a Diff component");
  }
  return context;
}

/* -------------------------------------------------------------------------- */
/*                                — Helpers —                                 */
/* -------------------------------------------------------------------------- */

interface TokenRun {
  text: string;
  className?: string;
}

interface HighlightedSegment {
  type: LineSegment["type"];
  runs: TokenRun[];
}

function mergeRuns(runs: TokenRun[]): TokenRun[] {
  const merged: TokenRun[] = [];

  for (const run of runs) {
    if (!run.text) continue;

    const last = merged[merged.length - 1];
    if (last && last.className === run.className) {
      last.text += run.text;
    } else {
      merged.push({ ...run });
    }
  }

  return merged;
}

function flattenHighlightedNodes(
  nodes: ReturnType<typeof refractor.highlight>["children"],
  inheritedClassNames: string[] = [],
): TokenRun[] {
  const runs: TokenRun[] = [];

  for (const node of nodes) {
    if (node.type === "text") {
      runs.push({
        text: node.value,
        className:
          inheritedClassNames.length > 0
            ? inheritedClassNames.join(" ")
            : undefined,
      });
      continue;
    }

    if (node.type === "element") {
      const nextClassNames = [
        ...inheritedClassNames,
        ...((node.properties.className as string[] | undefined) ?? []),
      ];

      runs.push(...flattenHighlightedNodes(node.children, nextClassNames));
    }
  }

  return mergeRuns(runs);
}

function highlightToRuns(code: string, lang: string): TokenRun[] {
  if (!code) return [];

  try {
    const tree = refractor.highlight(code, lang);
    return flattenHighlightedNodes(tree.children);
  } catch {
    return [{ text: code }];
  }
}

function splitRunsByLine(runs: TokenRun[]): TokenRun[][] {
  const lines: TokenRun[][] = [[]];

  for (const run of runs) {
    const parts = run.text.split("\n");

    for (let i = 0; i < parts.length; i++) {
      if (parts[i]) {
        lines[lines.length - 1].push({
          text: parts[i],
          className: run.className,
        });
      }

      if (i < parts.length - 1) {
        lines[lines.length - 1] = mergeRuns(lines[lines.length - 1]);
        lines.push([]);
      }
    }
  }

  return lines.map(mergeRuns);
}

function lineText(line: LineType): string {
  return line.content.map((segment) => segment.value).join("");
}

function distributeRunsToSegments(
  runs: TokenRun[],
  segments: LineSegment[],
): HighlightedSegment[] {
  const highlightedSegments: HighlightedSegment[] = [];
  let runIndex = 0;
  let runOffset = 0;

  for (const segment of segments) {
    let remaining = segment.value.length;
    const segmentRuns: TokenRun[] = [];

    while (remaining > 0 && runIndex < runs.length) {
      const run = runs[runIndex];
      const available = run.text.length - runOffset;
      const take = Math.min(remaining, available);

      segmentRuns.push({
        text: run.text.slice(runOffset, runOffset + take),
        className: run.className,
      });

      remaining -= take;
      runOffset += take;

      if (runOffset >= run.text.length) {
        runIndex += 1;
        runOffset = 0;
      }
    }

    if (remaining > 0) {
      segmentRuns.push({
        text: segment.value.slice(segment.value.length - remaining),
      });
    }

    highlightedSegments.push({
      type: segment.type,
      runs: mergeRuns(segmentRuns),
    });
  }

  return highlightedSegments;
}

function highlightLines(
  lines: LineType[],
  lang: string,
): HighlightedSegment[][] {
  if (lines.length === 0) return [];

  const highlightedLineRuns = splitRunsByLine(
    highlightToRuns(lines.map(lineText).join("\n"), lang),
  );

  return lines.map((line, index) =>
    distributeRunsToSegments(highlightedLineRuns[index] ?? [], line.content),
  );
}

function renderRuns(runs: TokenRun[], keyPrefix: string): React.ReactNode[] {
  return runs.map((run, index) => {
    if (!run.className) {
      return (
        <React.Fragment key={`${keyPrefix}-${index}`}>
          {run.text}
        </React.Fragment>
      );
    }

    return (
      <span key={`${keyPrefix}-${index}`} className={run.className}>
        {run.text}
      </span>
    );
  });
}

function useVirtualHunkRange(totalLines: number, enabled: boolean) {
  const anchorRef = React.useRef<HTMLTableCellElement | null>(null);
  const [range, setRange] = React.useState(() => ({
    start: 0,
    end: Math.max(0, totalLines - 1),
  }));

  React.useEffect(() => {
    if (!enabled) {
      setRange({ start: 0, end: Math.max(0, totalLines - 1) });
      return;
    }
    if (totalLines <= 0) {
      setRange({ start: 0, end: 0 });
      return;
    }

    let rafId = 0;

    const updateRange = () => {
      if (!anchorRef.current) return;

      const anchorTop =
        anchorRef.current.getBoundingClientRect().top + window.scrollY;
      const viewportHeight = window.innerHeight || 0;
      const overscanPx = VIRTUAL_OVERSCAN_LINES * ESTIMATED_ROW_HEIGHT;

      const startPx = window.scrollY - anchorTop - overscanPx;
      const endPx = window.scrollY + viewportHeight - anchorTop + overscanPx;

      const rawStart = Math.floor(startPx / ESTIMATED_ROW_HEIGHT);
      const rawEnd = Math.ceil(endPx / ESTIMATED_ROW_HEIGHT);

      const start = Math.max(0, Math.min(totalLines - 1, rawStart));
      const end = Math.max(start, Math.min(totalLines - 1, rawEnd));

      setRange((prev) =>
        prev.start === start && prev.end === end ? prev : { start, end },
      );
    };

    const onScroll = () => {
      if (rafId) return;
      rafId = window.requestAnimationFrame(() => {
        rafId = 0;
        updateRange();
      });
    };

    updateRange();
    window.addEventListener("scroll", onScroll, { passive: true });
    window.addEventListener("resize", onScroll);

    return () => {
      if (rafId) window.cancelAnimationFrame(rafId);
      window.removeEventListener("scroll", onScroll);
      window.removeEventListener("resize", onScroll);
    };
  }, [enabled, totalLines]);

  return { anchorRef, range };
}

/* -------------------------------------------------------------------------- */
/*                               — Root —                                     */
/* -------------------------------------------------------------------------- */
export interface DiffSelectionRange {
  startLine: number;
  endLine: number;
}

export interface DiffProps
  extends React.TableHTMLAttributes<HTMLTableElement>,
    Pick<File, "hunks" | "type"> {
  fileName?: string;
  language?: string;
  newBlame?: BlameInfo[];
  oldBlame?: BlameInfo[];
  isBlameLoading?: boolean;
}

export const Hunk = ({
  hunk,
  hasHunkBefore = true,
  hasHunkAfter = true,
  newBlame,
  oldBlame,
  isBlameLoading = false,
}: {
  hunk: HunkType | SkipBlock;
  hasHunkBefore?: boolean;
  hasHunkAfter?: boolean;
  newBlame?: BlameInfo[];
  oldBlame?: BlameInfo[];
  isBlameLoading?: boolean;
}) => {
  return hunk.type === "hunk" ? (
    <RenderedHunk
      hunk={hunk}
      newBlame={newBlame}
      oldBlame={oldBlame}
      isBlameLoading={isBlameLoading}
    />
  ) : (
    <SkipBlockRow
      lines={hunk.count}
      content={hunk.content}
      hiddenLines={hunk.hiddenLines}
      hasHunkBefore={hasHunkBefore}
      hasHunkAfter={hasHunkAfter}
      newBlame={newBlame}
      oldBlame={oldBlame}
    />
  );
};

const RenderedHunk: React.FC<{
  hunk: HunkType;
  newBlame?: BlameInfo[];
  oldBlame?: BlameInfo[];
  isBlameLoading?: boolean;
}> = ({ hunk, newBlame, oldBlame }) => {
  const { language } = useDiffContext();
  const highlightedLines = React.useMemo(
    () => highlightLines(hunk.lines, language),
    [hunk.lines, language],
  );
  const shouldVirtualize = hunk.lines.length > VIRTUAL_HUNK_THRESHOLD;
  const { anchorRef, range } = useVirtualHunkRange(
    hunk.lines.length,
    shouldVirtualize,
  );
  const start = shouldVirtualize ? range.start : 0;
  const end = shouldVirtualize ? range.end : Math.max(0, hunk.lines.length - 1);
  const topSpacerHeight = shouldVirtualize
    ? Math.max(0, start * ESTIMATED_ROW_HEIGHT)
    : 0;
  const bottomSpacerHeight = shouldVirtualize
    ? Math.max(0, (hunk.lines.length - end - 1) * ESTIMATED_ROW_HEIGHT)
    : 0;
  const visibleLines = shouldVirtualize
    ? hunk.lines.slice(start, end + 1)
    : hunk.lines;

  return (
    <>
      {shouldVirtualize && (
        <tr aria-hidden>
          <td ref={anchorRef} colSpan={3} className="p-0 border-0 h-0" />
        </tr>
      )}

      {topSpacerHeight > 0 && (
        <tr aria-hidden>
          <td colSpan={3} className="p-0 border-0">
            <div style={{ height: topSpacerHeight }} />
          </td>
        </tr>
      )}

      {visibleLines.map((line, index) => {
        const lineIndex = shouldVirtualize ? start + index : index;
        return (
          <Line
            key={`line-${index}-${getLineKeyNumber(line)}`}
            line={line}
            newBlame={newBlame}
            oldBlame={oldBlame}
            highlightedSegments={highlightedLines[lineIndex] ?? []}
          />
        );
      })}

      {bottomSpacerHeight > 0 && (
        <tr aria-hidden>
          <td colSpan={3} className="p-0 border-0">
            <div style={{ height: bottomSpacerHeight }} />
          </td>
        </tr>
      )}
    </>
  );
};

export const Diff: React.FC<DiffProps> = ({
  fileName,
  language = guessLang(fileName),
  hunks,
  className,
  children,
  newBlame,
  oldBlame,
  isBlameLoading = false,
  ...props
}) => {
  const items = React.useMemo(
    () =>
      hunks.map((hunk, index) => {
        const hasHunkBefore = hunks
          .slice(0, index)
          .some((candidate) => candidate.type === "hunk");
        const hasHunkAfter = hunks
          .slice(index + 1)
          .some((candidate) => candidate.type === "hunk");

        return {
          hunk,
          index,
          hasHunkBefore,
          hasHunkAfter,
        };
      }),
    [hunks],
  );

  return (
    <DiffContext.Provider value={{ language }}>
      <table
        {...props}
        className={cn(
          "relative [--background:#ffffff] dark:[--background:#000000] [--code-added:var(--color-green-600)] [--code-removed:var(--color-red-600)] font-mono text-sm _text-[0.8rem] w-full m-0 border-separate border-0 outline-none overflow-x-auto border-spacing-0",
          className,
        )}
      >
        <colgroup>
          <col className="w-0.75" />
          <col className="w-0.75" />
          <col />
        </colgroup>
        <tbody className="w-full box-border [&:has(td[data-line-type]:nth-child(2):hover)_td[data-line-type]:nth-child(2)_span.group-hover\:hidden]:hidden [&:has(td[data-line-type]:nth-child(2):hover)_td[data-line-type]:nth-child(2)_span.group-hover\:flex]:flex">
          {children ??
            items.map(({ hunk, index, hasHunkBefore, hasHunkAfter }) => (
              <Hunk
                key={index}
                hunk={hunk}
                hasHunkBefore={hasHunkBefore}
                hasHunkAfter={hasHunkAfter}
                newBlame={newBlame}
                oldBlame={oldBlame}
                isBlameLoading={isBlameLoading}
              />
            ))}
        </tbody>
      </table>
    </DiffContext.Provider>
  );
};

const getLineKeyNumber = (line: LineType): number | undefined => {
  if (line.type === "normal") {
    return line.newLineNumber ?? line.oldLineNumber;
  }

  return line.lineNumber;
};

const SkipBlockRow: React.FC<{
  lines: number;
  content?: string;
  hiddenLines?: LineType[];
  hasHunkBefore?: boolean;
  hasHunkAfter?: boolean;
  newBlame?: BlameInfo[];
  oldBlame?: BlameInfo[];
}> = ({
  lines,
  hiddenLines = [],
  hasHunkBefore = true,
  hasHunkAfter = true,
  newBlame,
  oldBlame,
}) => {
  const { language } = useDiffContext();
  const highlightedHiddenLines = React.useMemo(
    () => highlightLines(hiddenLines, language),
    [hiddenLines, language],
  );

  const [expandedTop, setExpandedTop] = React.useState(0);
  const [expandedBottom, setExpandedBottom] = React.useState(0);

  const totalHiddenLines = hiddenLines.length;
  const remainingLines = Math.max(
    0,
    totalHiddenLines - expandedTop - expandedBottom,
  );

  const expandTop = () => {
    if (totalHiddenLines === 0) return;

    setExpandedTop((current) => {
      const available = totalHiddenLines - current - expandedBottom;
      return current + Math.min(SKIP_BLOCK_EXPAND_STEP, Math.max(available, 0));
    });
  };

  const expandBottom = () => {
    if (totalHiddenLines === 0) return;

    setExpandedBottom((current) => {
      const available = totalHiddenLines - expandedTop - current;
      return current + Math.min(SKIP_BLOCK_EXPAND_STEP, Math.max(available, 0));
    });
  };

  const expandAll = () => {
    setExpandedTop(totalHiddenLines);
    setExpandedBottom(0);
  };

  const expandAllToTop = () => {
    setExpandedTop(totalHiddenLines);
    setExpandedBottom(0);
  };

  const expandAllToBottom = () => {
    setExpandedTop(0);
    setExpandedBottom(totalHiddenLines);
  };

  const isLeadingBoundarySkip = !hasHunkBefore && hasHunkAfter;
  const isTrailingBoundarySkip = hasHunkBefore && !hasHunkAfter;
  const isOnlySkipInView = !hasHunkBefore && !hasHunkAfter;

  const topLines = hiddenLines.slice(0, expandedTop);
  const bottomStart = Math.max(expandedTop, totalHiddenLines - expandedBottom);
  const bottomLines = hiddenLines.slice(bottomStart);

  return (
    <>
      {topLines.map((line, index) => (
        <Line
          key={`skip-top-${getLineKeyNumber(line) ?? index}`}
          line={line}
          highlightedSegments={highlightedHiddenLines[index] ?? []}
          newBlame={newBlame}
          oldBlame={oldBlame}
        />
      ))}

      {remainingLines > 0 && (
        <>
          {hasHunkBefore && (
            <tr className="h-1">
              <td data-line-type className="w-0.75" />
              <td data-line-type />
              <td />
            </tr>
          )}
          <tr className={cn("h-8 font-mono text-muted-foreground")}>
            <td className="w-0.75" />
            <td data-line-type className="select-none">
              <div className="w-full">
                {isLeadingBoundarySkip ? (
                  <div className="flex flex-col w-full h-8 justify-between">
                    <button
                      type="button"
                      onClick={expandAllToTop}
                      className="group h-8 flex items-center justify-center bg-muted rounded-l-[4px] transition-colors duration-75 hover:bg-secondary-foreground/15 cursor-pointer"
                    >
                      <ChevronUpIcon className="size-3.5 text-muted-foreground/50 group-hover:text-muted-foreground duration-75 transition-colors" />
                    </button>
                  </div>
                ) : isTrailingBoundarySkip ? (
                  <div className="flex flex-col w-full h-8 justify-between">
                    <button
                      type="button"
                      onClick={expandAllToBottom}
                      className="group h-8 flex items-center justify-center bg-muted rounded-l-[4px] transition-colors duration-75 hover:bg-secondary-foreground/15 cursor-pointer"
                    >
                      <ChevronDownIcon className="size-3.5 text-muted-foreground/50 group-hover:text-muted-foreground duration-75 transition-colors" />
                    </button>
                  </div>
                ) : remainingLines > EXPENDED_SKIP_BLOCK_LINES ? (
                  <div className="flex flex-col w-full h-8 justify-between">
                    <button
                      type="button"
                      onClick={expandTop}
                      className="group h-3.75 flex items-center justify-center bg-muted rounded-tl-[4px] transition-colors duration-75 hover:bg-secondary-foreground/15 cursor-pointer"
                    >
                      <ChevronDownIcon className="size-3.5 text-muted-foreground/50 group-hover:text-muted-foreground duration-75 transition-colors" />
                    </button>
                    <button
                      type="button"
                      onClick={expandBottom}
                      className="group h-3.75 flex items-center justify-center bg-muted rounded-bl-[4px] transition-colors duration-75 hover:bg-secondary-foreground/15 cursor-pointer"
                    >
                      <ChevronUpIcon className="size-3.5 text-muted-foreground/50 group-hover:text-muted-foreground duration-75 transition-colors" />
                    </button>
                  </div>
                ) : isOnlySkipInView ? (
                  <div className="flex flex-col w-full h-8 justify-between">
                    <button
                      type="button"
                      onClick={expandAll}
                      className="group h-8 flex items-center justify-center bg-muted rounded-l-[4px] transition-colors duration-75 hover:bg-secondary-foreground/15 cursor-pointer"
                    >
                      <ChevronsUpDown className="size-3.5 text-muted-foreground/50 group-hover:text-muted-foreground duration-75 transition-colors" />
                    </button>
                  </div>
                ) : (
                  <div className="flex flex-col w-full h-8 justify-between">
                    <button
                      type="button"
                      onClick={expandAll}
                      className="group h-8 flex items-center justify-center bg-muted rounded-l-[4px] transition-colors duration-75 hover:bg-secondary-foreground/15 cursor-pointer"
                    >
                      <ChevronsUpDown className="size-3.5 text-muted-foreground/50 group-hover:text-muted-foreground duration-75 transition-colors" />
                    </button>
                  </div>
                )}
              </div>
            </td>
            <td className="bg-muted relative  select-none h-8 flex items-center w-full">
              <div className="absolute h-full left-px top-1/2 w-0.5 bg-background -translate-x-1/2 -translate-y-1/2" />
              <span className="px-0 sticky left-0 pl-2">
                <span>{`${remainingLines} `}</span>
                {"unchanged lines"}
              </span>
            </td>
          </tr>
          {hasHunkAfter && (
            <tr className="h-1">
              <td data-line-type className="w-0.75" />
              <td data-line-type />
              <td />
            </tr>
          )}
        </>
      )}

      {bottomLines.map((line, index) => {
        const highlightedIndex = bottomStart + index;
        return (
          <Line
            key={`skip-bottom-${getLineKeyNumber(line) ?? highlightedIndex}`}
            line={line}
            highlightedSegments={highlightedHiddenLines[highlightedIndex] ?? []}
            newBlame={newBlame}
            oldBlame={oldBlame}
          />
        );
      })}

      {totalHiddenLines === 0 && lines > 0 && (
        <>
          <tr className="h-2" />
          <tr className={cn("h-8 font-mono text-muted-foreground")}>
            <td colSpan={2} className="select-none">
              <div className="h-8 flex items-center justify-center bg-muted rounded-l-sm">
                <ChevronsUpDown className="size-4" />
              </div>
            </td>
            <td className="bg-muted relative select-none">
              <div className="absolute h-full left-px top-1/2 w-0.5 bg-background -translate-x-1/2 -translate-y-1/2" />
              <span className="px-0 sticky left-0 pl-2">
                <span>{`${lines} `}</span>
                {"lines hidden"}
              </span>
            </td>
          </tr>
          <tr className="h-2" />
        </>
      )}
    </>
  );
};

const Line: React.FC<{
  line: LineType;
  highlightedSegments: HighlightedSegment[];
  oldBlame?: BlameInfo[];
  newBlame?: BlameInfo[];
}> = ({ line, highlightedSegments, newBlame = [], oldBlame = [] }) => {
  const segmentsToRender =
    highlightedSegments.length > 0
      ? highlightedSegments
      : line.content.map((segment) => ({
          type: segment.type,
          runs: [{ text: segment.value }],
        }));
  const Tag =
    line.type === "insert" ? "ins" : line.type === "delete" ? "del" : "span";
  const lineNumberNew =
    line.type === "normal" ? line.newLineNumber : line.lineNumber;
  const lineNumberOld = line.type === "normal" ? line.oldLineNumber : undefined;

  const isLineMerged =
    line.type === "normal" &&
    highlightedSegments.some(
      (segment) => segment.type === "insert" || segment.type === "delete",
    );

  const normalisezedEmail = normalizeToAuthor(
    lineNumberNew ?? -1,
    line.type === "delete" ? oldBlame : newBlame,
  );

  return (
    <tr
      data-line-new={lineNumberNew ?? undefined}
      data-line-old={lineNumberOld ?? undefined}
      data-line-kind={line.type}
      className={cn(
        "relative whitespace-pre-wrap box-border border-none h-5 min-h-5 group",
        {
          "bg-(--code-added)/10": line.type === "insert",
          "bg-(--code-removed)/10": line.type === "delete",
        },
        isLineMerged &&
          "[--code-line-merge:var(--color-blue-500)] bg-(--code-line-merge)/10",
      )}
    >
      <td
        className={cn(
          "border-transparent _border-l-3",
          "sticky left-0 z-20",
          {
            "border-(--code-added)/60 [--code-line-bg:var(--code-added)] diff-added-lines":
              line.type === "insert",
            "border-(--code-removed)/80 [--code-line-bg:var(--code-removed)] diff-deletion":
              line.type === "delete",
          },
          isLineMerged &&
            "[--code-line-bg:var(--code-line-merge)] border-(--code-line-merge)/20 diff-merged-lines",
          !isLineMerged && line.type === "normal" && "diff-normal-lines",
        )}
      />
      <td
        className={cn(
          "tabular-nums relative px-2 text-xs select-none text-end z-10",
          "bg-[color-mix(in_oklch,var(--muted)_60%,var(--background))] _bg-muted/60 text-foreground/70 relative",
          "sticky left-0 min-w-0",
          {
            "bg-[color-mix(in_oklch,var(--code-added)_20%,var(--background))] _bg-(--code-added)/10 [--code-line-bg:var(--code-added)] text-(--code-added) opacity-100":
              line.type === "insert",
            "bg-[color-mix(in_oklch,var(--code-removed)_20%,var(--background))] _bg-(--code-removed)/10 [--code-line-bg:var(--code-removed)] opacity-100 text-(--code-removed)":
              line.type === "delete",
          },
          isLineMerged &&
            "bg-[color-mix(in_oklch,var(--code-line-merge)_20%,var(--background))] _bg-(--code-line-merge)/10 text-(--code-line-merge) opacity-100",
          "border-r border-current/5",
        )}
        // style={{
        //   width: "minmax(min-content, max-content)",
        //   maxWidth: "100%",
        //   minWidth: line.type === "normal" ? "3ch" : "2ch",
        // }}
        data-line-type={line.type}
      >
        <div className="flex items-center justify-between px-1">
          {/* <div className="w-fit flex items-center justify-between mr-2">
            {normalisezedEmail ? (
              <img
                src={`https://avatars.githubusercontent.com/u/e?email=${normalisezedEmail}&s=64`}
                alt="avatar"
                className={cn(
                  "rounded-[4px] size-3.5 ml-1 shrink-0",
                  line.type === "delete" && "opacity-50",
                  line.type === "normal" && !isLineMerged ? "opacity-80" : "",
                )}
              />
            ) : (
              <div
                className={cn(
                  "rounded-[4px] flex items-center justify-center text-[10px] size-3.5 ml-1 shrink-0 ring-[1px] ring-inset",
                  line.type === "insert"
                    ? "text-[color-mix(in_oklch,var(--code-added)_90%,var(--background))] ring-[color-mix(in_oklch,var(--code-added)_65%,var(--background))]"
                    : "ring-[color-mix(in_oklch,var(--code-line-merge)_70%,var(--background))]",
                )}
              >
                {line.type === "insert" ? (
                  <span className="font-mono -translate-y-[0.5px]">{"+"}</span>
                ) : (
                  <span className="size-[4.5px] rounded-full bg-[color-mix(in_oklch,var(--code-line-merge)_90%,var(--background))]" />
                )}
              </div>
            )}
          </div> */}
          <div className="w-0.5" />
          {line.type === "delete" ? (
            <span>
              <span className="group-hover:hidden delay-[10s] flex justify-end">
                {"–"}
              </span>
              <span className="group-hover:flex delay-[10s] hidden justify-end">
                {lineNumberNew}
              </span>
            </span>
          ) : (
            lineNumberNew
          )}
        </div>
      </td>
      <td className="text-nowrap pr-6 pl-2">
        <Tag>
          {segmentsToRender.map((segment, index) => (
            <span
              key={index}
              className={cn({
                "bg-[color-mix(in_oklch,var(--code-added)_25%,var(--background))] _bg-[var(--code-added)]/20 rounded-xs":
                  segment.type === "insert",
                "bg-[color-mix(in_oklch,var(--code-removed)_25%,var(--background))] _bg-[var(--code-removed)]/20 rounded-xs":
                  segment.type === "delete",
              })}
            >
              {renderRuns(
                segment.runs,
                `${lineNumberNew ?? lineNumberOld ?? index}-${index}`,
              )}
            </span>
          ))}
        </Tag>
      </td>
    </tr>
  );
};

/** we will get line number and email,
 * the email as a string we get will look like
 * from: <4586894 mischnic@users.noreply.github.com>
 * to: mischnic@users.noreply.github.com
 *
 * now here the state matters.
 *
 * if the state is "delete" then we will use the oldBlame to get the.
 * else we need to use the newBlame for rest of the thing
 *
 * also if the email is "<not.committed.yet>" then we will return `false`
 */
export function normalizeToAuthor(
  lineNumber: number,
  blame: BlameInfo[],
): string | false {
  const email = blame[lineNumber - 1]?.["author_mail"] ?? "<not.committed.yet>";

  if (email === "<not.committed.yet>" || email === "not.committed.yet") {
    return false;
  }

  return email.replace(/<|>/g, "");
}

export function DiffViewer({
  patch,
  options = {},
  newBlame,
  oldBlame,
  isBlameLoading = false,
}: {
  patch: string;
  options?: Partial<ParseOptions>;
  newBlame?: BlameInfo[];
  oldBlame?: BlameInfo[];
  isBlameLoading?: boolean;
}) {
  const [file] = parseDiff(patch, options);

  return (
    <Diff
      fileName={file.newPath}
      hunks={file.hunks}
      type={file.type}
      newBlame={newBlame}
      oldBlame={oldBlame}
      isBlameLoading={isBlameLoading}
    />
  );
}

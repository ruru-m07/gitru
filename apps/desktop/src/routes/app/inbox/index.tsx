import { requestDiff, useTauriEvent } from "@gitru/commands";
import { Button } from "@gitru/ui/components/button";
import { createFileRoute } from "@tanstack/react-router";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import PageLayout from "@/components/pageLayout";

const DEMO_FILE_PATH = "crates/ipc/src/commands.rs";
const EXPAND_STEP = 5;

type DiffStatus = "processing" | "ready" | { error: string };
type LineKind = "added" | "removed" | "context";

type TokenSpan = {
  content: string;
  color: string;
  bold: boolean;
  italic: boolean;
  underline: boolean;
};

type HighlightedLine = {
  lineNo: number;
  content: string;
  tokens: TokenSpan[];
};

type DiffLine = {
  kind: LineKind;
  oldLineno: number | null;
  newLineno: number | null;
  content: string;
  oldContent: string | null;
  newContent: string | null;
  oldTokens: TokenSpan[];
  newTokens: TokenSpan[];
};

type Hunk = {
  header: string;
  oldStart: number;
  newStart: number;
  oldLines: number;
  newLines: number;
  lines: DiffLine[];
};

type SemanticChange = {
  kind: string;
  name: string;
  oldStartLine?: number;
  oldEndLine?: number;
  newStartLine?: number;
  newEndLine?: number;
};

type MovedBlock = {
  key: string;
  name: string;
  oldStart: number;
  oldEnd: number;
  newStart: number;
  newEnd: number;
};

type InlinePart = {
  text: string;
  kind: "same" | "removed" | "added";
};

type MovePath = {
  key: string;
  d: string;
  active: boolean;
};

export type DiffPayload = {
  jobId: string;
  filePath: string;
  status: DiffStatus;
  hunks: Hunk[];
  semanticChanges: SemanticChange[];
  oldLines: HighlightedLine[];
  newLines: HighlightedLine[];
};

type SkipSegment = {
  type: "skip";
  id: string;
  oldStart: number;
  oldEnd: number;
  newStart: number;
  newEnd: number;
  oldLines: HighlightedLine[];
  newLines: HighlightedLine[];
};

type HunkSegment = {
  type: "hunk";
  id: string;
  hunk: Hunk;
};

type DiffSegment = SkipSegment | HunkSegment;

type SkipExpansion = {
  top: number;
  bottom: number;
};

function formatStatus(status: DiffStatus) {
  if (typeof status === "string") {
    return status;
  }

  return `error: ${status.error}`;
}

function buildMovedBlocks(payload: DiffPayload | null): MovedBlock[] {
  if (!payload) {
    return [];
  }

  return payload.semanticChanges
    .filter((change) => {
      return (
        change.kind === "moved" &&
        typeof change.oldStartLine === "number" &&
        typeof change.oldEndLine === "number" &&
        typeof change.newStartLine === "number" &&
        typeof change.newEndLine === "number"
      );
    })
    .map((change) => ({
      key: `${change.name}-${change.oldStartLine}-${change.newStartLine}`,
      name: change.name,
      oldStart: change.oldStartLine as number,
      oldEnd: change.oldEndLine as number,
      newStart: change.newStartLine as number,
      newEnd: change.newEndLine as number,
    }));
}

function intersects(
  aStart: number,
  aEnd: number,
  bStart: number,
  bEnd: number,
) {
  return aStart <= bEnd && bStart <= aEnd;
}

function tokenizeWords(text: string): string[] {
  return text.match(/\s+|[^\s]+/g) ?? [];
}

function buildInlineWordDiff(
  oldText: string,
  newText: string,
): {
  oldParts: InlinePart[];
  newParts: InlinePart[];
} {
  const a = tokenizeWords(oldText);
  const b = tokenizeWords(newText);
  const m = a.length;
  const n = b.length;
  const dp: number[][] = Array.from({ length: m + 1 }, () =>
    Array.from({ length: n + 1 }, () => 0),
  );

  for (let i = m - 1; i >= 0; i--) {
    for (let j = n - 1; j >= 0; j--) {
      if (a[i] === b[j]) {
        dp[i][j] = dp[i + 1][j + 1] + 1;
      } else {
        dp[i][j] = Math.max(dp[i + 1][j], dp[i][j + 1]);
      }
    }
  }

  const oldParts: InlinePart[] = [];
  const newParts: InlinePart[] = [];
  let i = 0;
  let j = 0;

  while (i < m && j < n) {
    if (a[i] === b[j]) {
      oldParts.push({ text: a[i], kind: "same" });
      newParts.push({ text: b[j], kind: "same" });
      i++;
      j++;
    } else if (dp[i + 1][j] >= dp[i][j + 1]) {
      oldParts.push({ text: a[i], kind: "removed" });
      i++;
    } else {
      newParts.push({ text: b[j], kind: "added" });
      j++;
    }
  }

  while (i < m) {
    oldParts.push({ text: a[i], kind: "removed" });
    i++;
  }
  while (j < n) {
    newParts.push({ text: b[j], kind: "added" });
    j++;
  }

  return { oldParts, newParts };
}

// ─── Split-row helpers ────────────────────────────────────────────────────

type SplitCell = {
  lineNo: number | null;
  content: string | null;
  tokens: TokenSpan[];
  kind: LineKind;
};

type SplitRow = {
  old: SplitCell | null;
  new: SplitCell | null;
};

/**
 * Converts a flat DiffLine array (from one hunk) into paired rows:
 * consecutive removed+added blocks are aligned side-by-side.
 */
function buildSplitRows(lines: DiffLine[]): SplitRow[] {
  const rows: SplitRow[] = [];
  let i = 0;
  while (i < lines.length) {
    const line = lines[i];
    if (line.kind === "context") {
      rows.push({
        old: {
          lineNo: line.oldLineno,
          content: line.oldContent,
          tokens: line.oldTokens,
          kind: "context",
        },
        new: {
          lineNo: line.newLineno,
          content: line.newContent,
          tokens: line.newTokens,
          kind: "context",
        },
      });
      i++;
    } else if (line.kind === "removed") {
      // Collect the remove block then the immediately following add block.
      const removals: DiffLine[] = [];
      while (i < lines.length && lines[i].kind === "removed")
        removals.push(lines[i++]);
      const additions: DiffLine[] = [];
      while (i < lines.length && lines[i].kind === "added")
        additions.push(lines[i++]);
      const count = Math.max(removals.length, additions.length);
      for (let j = 0; j < count; j++) {
        const rem = removals[j];
        const add = additions[j];
        rows.push({
          old: rem
            ? {
                lineNo: rem.oldLineno,
                content: rem.oldContent,
                tokens: rem.oldTokens,
                kind: "removed",
              }
            : null,
          new: add
            ? {
                lineNo: add.newLineno,
                content: add.newContent,
                tokens: add.newTokens,
                kind: "added",
              }
            : null,
        });
      }
    } else {
      // "added" with no preceding "removed"
      rows.push({
        old: null,
        new: {
          lineNo: line.newLineno,
          content: line.newContent,
          tokens: line.newTokens,
          kind: "added",
        },
      });
      i++;
    }
  }
  return rows;
}

function cellBg(kind: LineKind | null, side: "old" | "new") {
  if (!kind || kind === "context") return "bg-background";
  if (kind === "removed" && side === "old") return "bg-rose-500/10";
  if (kind === "added" && side === "new") return "bg-emerald-500/10";
  return "bg-muted/20";
}

function sliceLines(lines: HighlightedLine[], start: number, end: number) {
  if (end < start || start <= 0) {
    return [];
  }

  return lines.slice(start - 1, end);
}

function buildDiffSegments(payload: DiffPayload | null): DiffSegment[] {
  if (!payload) {
    return [];
  }

  const segments: DiffSegment[] = [];
  let previousOldEnd = 0;
  let previousNewEnd = 0;

  payload.hunks.forEach((hunk, index) => {
    const gapOldStart = previousOldEnd + 1;
    const gapOldEnd = hunk.oldStart > 0 ? hunk.oldStart - 1 : 0;
    const gapNewStart = previousNewEnd + 1;
    const gapNewEnd = hunk.newStart > 0 ? hunk.newStart - 1 : 0;

    if (gapOldEnd >= gapOldStart || gapNewEnd >= gapNewStart) {
      segments.push({
        type: "skip",
        id: `skip-${index}-${gapOldStart}-${gapNewStart}`,
        oldStart: gapOldStart,
        oldEnd: gapOldEnd,
        newStart: gapNewStart,
        newEnd: gapNewEnd,
        oldLines: sliceLines(payload.oldLines, gapOldStart, gapOldEnd),
        newLines: sliceLines(payload.newLines, gapNewStart, gapNewEnd),
      });
    }

    segments.push({
      type: "hunk",
      id: `hunk-${index}-${hunk.header}`,
      hunk,
    });

    previousOldEnd =
      hunk.oldLines > 0 ? hunk.oldStart + hunk.oldLines - 1 : hunk.oldStart - 1;
    previousNewEnd =
      hunk.newLines > 0 ? hunk.newStart + hunk.newLines - 1 : hunk.newStart - 1;
  });

  const trailingOldStart = previousOldEnd + 1;
  const trailingNewStart = previousNewEnd + 1;
  const trailingOldEnd = payload.oldLines.length;
  const trailingNewEnd = payload.newLines.length;

  if (
    trailingOldEnd >= trailingOldStart ||
    trailingNewEnd >= trailingNewStart
  ) {
    segments.push({
      type: "skip",
      id: `skip-tail-${trailingOldStart}-${trailingNewStart}`,
      oldStart: trailingOldStart,
      oldEnd: trailingOldEnd,
      newStart: trailingNewStart,
      newEnd: trailingNewEnd,
      oldLines: sliceLines(payload.oldLines, trailingOldStart, trailingOldEnd),
      newLines: sliceLines(payload.newLines, trailingNewStart, trailingNewEnd),
    });
  }

  return segments;
}

function renderTokens(tokens: TokenSpan[], fallback: string) {
  if (!tokens.length) {
    return <span>{fallback || " "}</span>;
  }

  return tokens.map((token, index) => (
    <span
      key={`${token.content}-${index}`}
      style={{
        color: token.color,
        fontWeight: token.bold ? 700 : 400,
        fontStyle: token.italic ? "italic" : "normal",
        textDecoration: token.underline ? "underline" : "none",
      }}
    >
      {token.content}
    </span>
  ));
}

function renderInlineParts(parts: InlinePart[], side: "old" | "new") {
  return parts.map((part, index) => {
    const changed =
      (side === "old" && part.kind === "removed") ||
      (side === "new" && part.kind === "added");

    return (
      <span
        className={changed ? "rounded bg-amber-500/30" : undefined}
        key={`${part.kind}-${part.text}-${index}`}
      >
        {part.text}
      </span>
    );
  });
}

function renderSplitCell(
  cell: SplitCell | null,
  side: "old" | "new",
  inlineParts?: InlinePart[],
) {
  if (!cell) {
    return (
      <div className="grid grid-cols-[56px_minmax(0,1fr)] gap-3 px-3 py-1 bg-muted/20 min-h-[28px]" />
    );
  }

  const anchor = `${side}-${cell.lineNo ?? ""}`;

  return (
    <div
      data-line-anchor={anchor}
      className={`grid grid-cols-[56px_minmax(0,1fr)] gap-3 px-3 py-1 ${cellBg(cell.kind, side)}`}
    >
      <div className="select-text text-right text-[11px] text-muted-foreground">
        {cell.lineNo ?? ""}
      </div>
      <pre className="select-text overflow-x-auto whitespace-pre-wrap font-mono text-xs leading-5">
        {inlineParts?.length
          ? renderInlineParts(inlineParts, side)
          : renderTokens(cell.tokens, cell.content ?? "")}
      </pre>
    </div>
  );
}

export const Route = createFileRoute("/app/inbox/")({
  component: RouteComponent,
});

function RouteComponent() {
  const [activeJobId, setActiveJobId] = useState<string | null>(null);
  const [latestPayload, setLatestPayload] = useState<DiffPayload | null>(null);
  const [isRequesting, setIsRequesting] = useState(false);
  const [requestError, setRequestError] = useState<string | null>(null);
  const [expandedSkips, setExpandedSkips] = useState<
    Record<string, SkipExpansion>
  >({});
  const [activeMoveKey, setActiveMoveKey] = useState<string | null>(null);
  const [movePaths, setMovePaths] = useState<MovePath[]>([]);
  const activeJobIdRef = useRef<string | null>(null);
  const diffViewportRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    activeJobIdRef.current = activeJobId;
  }, [activeJobId]);

  const handleDiffEvent = useCallback((payload: DiffPayload) => {
    console.log({ payload });

    if (payload.filePath !== DEMO_FILE_PATH) {
      return;
    }

    const currentJobId = activeJobIdRef.current;
    if (currentJobId && payload.jobId !== currentJobId) {
      return;
    }

    setLatestPayload(payload);
  }, []);

  useTauriEvent<DiffPayload>("diff_event", handleDiffEvent);

  const requestDemoDiff = useCallback(async () => {
    setIsRequesting(true);
    setRequestError(null);

    try {
      const jobId = await requestDiff({
        filePath: DEMO_FILE_PATH,
      });
      setActiveJobId(jobId);
      setLatestPayload(null);
      setExpandedSkips({});
    } catch (error) {
      setRequestError(error instanceof Error ? error.message : String(error));
    } finally {
      setIsRequesting(false);
    }
  }, []);

  const segments = useMemo(
    () => buildDiffSegments(latestPayload),
    [latestPayload],
  );
  const movedBlocks = useMemo(
    () => buildMovedBlocks(latestPayload),
    [latestPayload],
  );

  const expandSkipTop = useCallback((id: string, total: number) => {
    setExpandedSkips((current) => {
      const state = current[id] ?? { top: 0, bottom: 0 };
      return {
        ...current,
        [id]: {
          ...state,
          top: Math.min(total, state.top + EXPAND_STEP),
        },
      };
    });
  }, []);

  const expandSkipBottom = useCallback((id: string, total: number) => {
    setExpandedSkips((current) => {
      const state = current[id] ?? { top: 0, bottom: 0 };
      return {
        ...current,
        [id]: {
          ...state,
          bottom: Math.min(total, state.bottom + EXPAND_STEP),
        },
      };
    });
  }, []);

  const expandSkipAll = useCallback((id: string, total: number) => {
    setExpandedSkips((current) => ({
      ...current,
      [id]: { top: total, bottom: 0 },
    }));
  }, []);

  const focusMovePair = useCallback((block: MovedBlock) => {
    const viewport = diffViewportRef.current;
    if (!viewport) {
      return;
    }

    const oldSelector = `[data-line-anchor='old-${block.oldStart}']`;
    const newSelector = `[data-line-anchor='new-${block.newStart}']`;
    const oldTarget = viewport.querySelector(oldSelector) as HTMLElement | null;
    const newTarget = viewport.querySelector(newSelector) as HTMLElement | null;

    if (!oldTarget || !newTarget) {
      return;
    }

    setActiveMoveKey(block.key);

    const oldY = oldTarget.offsetTop;
    const newY = newTarget.offsetTop;
    const midpoint = (oldY + newY) / 2;
    const targetTop = Math.max(midpoint - viewport.clientHeight / 2, 0);

    viewport.scrollTo({
      top: targetTop,
      behavior: "smooth",
    });
  }, []);

  useEffect(() => {
    const viewport = diffViewportRef.current;
    if (!viewport) {
      return;
    }

    const recompute = () => {
      const viewportRect = viewport.getBoundingClientRect();
      const nextPaths: MovePath[] = movedBlocks
        .map((block) => {
          const oldEl = viewport.querySelector(
            `[data-line-anchor='old-${block.oldStart}']`,
          ) as HTMLElement | null;
          const newEl = viewport.querySelector(
            `[data-line-anchor='new-${block.newStart}']`,
          ) as HTMLElement | null;

          if (!oldEl || !newEl) {
            return null;
          }

          const oldRect = oldEl.getBoundingClientRect();
          const newRect = newEl.getBoundingClientRect();

          const y1 = oldRect.top - viewportRect.top + oldRect.height / 2;
          const y2 = newRect.top - viewportRect.top + newRect.height / 2;
          const x1 = oldRect.right - viewportRect.left;
          const x2 = newRect.left - viewportRect.left;

          // Only render connectors if at least one endpoint is in viewport.
          const inViewport =
            (y1 >= -24 && y1 <= viewport.clientHeight + 24) ||
            (y2 >= -24 && y2 <= viewport.clientHeight + 24);
          if (!inViewport) {
            return null;
          }

          const c1 = x1 + 28;
          const c2 = x2 - 28;
          const d = `M ${x1} ${y1} C ${c1} ${y1}, ${c2} ${y2}, ${x2} ${y2}`;

          return {
            key: block.key,
            d,
            active: activeMoveKey === block.key,
          };
        })
        .filter((value): value is MovePath => value !== null);

      setMovePaths(nextPaths);
    };

    recompute();
    viewport.addEventListener("scroll", recompute, { passive: true });
    window.addEventListener("resize", recompute);

    return () => {
      viewport.removeEventListener("scroll", recompute);
      window.removeEventListener("resize", recompute);
    };
  }, [movedBlocks, activeMoveKey, segments]);

  return (
    <PageLayout className="gap-4 p-4 h-screen overflow-y-auto">
      <div className="border-b border-border/60 pb-4">
        <div className="text-xl font-semibold">Diff Engine Split View POC</div>
        <div className="mt-1 text-sm text-muted-foreground">
          Backend builds the diff, semantic changes, and syntax tokens for
          <span className="font-mono"> {DEMO_FILE_PATH}</span>
        </div>
        <div className="mt-4 flex flex-wrap items-center gap-3">
          <Button disabled={isRequesting} onClick={requestDemoDiff}>
            {isRequesting ? "Requesting..." : "Request Diff"}
          </Button>
          <div className="rounded-md border border-border/60 px-3 py-2 text-sm">
            <div className="text-muted-foreground">Active job</div>
            <div className="font-mono text-xs">{activeJobId ?? "none"}</div>
          </div>
          <div className="rounded-md border border-border/60 px-3 py-2 text-sm">
            <div className="text-muted-foreground">Status</div>
            <div className="font-medium">
              {latestPayload ? formatStatus(latestPayload.status) : "idle"}
            </div>
          </div>
          <div className="rounded-md border border-border/60 px-3 py-2 text-sm">
            <div className="text-muted-foreground">Semantic changes</div>
            <div className="font-medium">
              {latestPayload?.semanticChanges.length ?? 0}
            </div>
          </div>
          <div className="rounded-md border border-border/60 px-3 py-2 text-sm">
            <div className="text-muted-foreground">Hunks</div>
            <div className="font-medium">
              {latestPayload?.hunks.length ?? 0}
            </div>
          </div>
        </div>
        {requestError ? (
          <div className="mt-3 rounded-md border border-rose-500/30 bg-rose-500/10 px-3 py-2 text-sm text-rose-700 dark:text-rose-300">
            {requestError}
          </div>
        ) : null}
      </div>

      {/* <section className="rounded-lg border border-border/60 bg-muted/20 p-3">
        <div className="mb-3 text-sm font-semibold">Semantic Changes</div>
        {latestPayload?.semanticChanges.length ? (
          <div className="flex flex-wrap gap-2">
            {latestPayload.semanticChanges.map((change, index) => (
              <div
                className={`rounded-full border px-3 py-1 text-xs font-medium ${semanticChangeClassName(change.kind)}`}
                key={`${change.kind}-${change.name}-${index}`}
              >
                <span className="uppercase tracking-wide">{change.kind}</span>
                <span className="mx-2 text-muted-foreground">•</span>
                <span className="font-mono">{change.name}</span>
              </div>
            ))}
          </div>
        ) : (
          <div className="text-sm text-muted-foreground">
            No semantic changes reported for the current payload yet.
          </div>
        )}
      </section> */}

      <section className="min-h-0 rounded-lg border border-border/60 bg-muted/20 p-3">
        <div className="mb-3 flex items-center justify-between">
          <div className="text-sm font-semibold">Split Diff</div>
          <div className="text-xs text-muted-foreground">
            VS Code style split renderer with expandable unchanged ranges
          </div>
        </div>

        <div className="overflow-hidden rounded-md border border-border/60 bg-background">
          <div className="grid grid-cols-[minmax(0,1fr)_90px_minmax(0,1fr)] border-b border-border/60 bg-muted text-xs font-medium uppercase tracking-wide text-muted-foreground">
            <div className="border-r border-border/60 px-3 py-2">Old</div>
            <div className="border-r border-border/60 px-2 py-2 text-center">
              Move
            </div>
            <div className="px-3 py-2">New</div>
          </div>

          <div
            className="relative max-h-[68vh] overflow-auto"
            ref={diffViewportRef}
          >
            <svg
              className="pointer-events-none absolute inset-0 z-20"
              height="100%"
              width="100%"
            >
              <defs>
                <marker
                  id="move-arrow-head"
                  markerHeight="7"
                  markerUnits="strokeWidth"
                  markerWidth="7"
                  orient="auto"
                  refX="6"
                  refY="3.5"
                >
                  <path d="M0,0 L7,3.5 L0,7 z" fill="rgba(245, 158, 11, 0.9)" />
                </marker>
              </defs>
              {movePaths.map((path) => (
                <path
                  d={path.d}
                  fill="none"
                  key={path.key}
                  markerEnd="url(#move-arrow-head)"
                  stroke={
                    path.active
                      ? "rgba(245, 158, 11, 0.95)"
                      : "rgba(245, 158, 11, 0.7)"
                  }
                  strokeWidth={path.active ? 2.5 : 1.6}
                />
              ))}
            </svg>
            {segments.length ? (
              segments.map((segment) => {
                if (segment.type === "hunk") {
                  const splitRows = buildSplitRows(segment.hunk.lines);
                  const hunkOldEnd =
                    segment.hunk.oldLines > 0
                      ? segment.hunk.oldStart + segment.hunk.oldLines - 1
                      : segment.hunk.oldStart;
                  const hunkNewEnd =
                    segment.hunk.newLines > 0
                      ? segment.hunk.newStart + segment.hunk.newLines - 1
                      : segment.hunk.newStart;

                  const movedOldBlocksInHunk = movedBlocks.filter((block) =>
                    intersects(
                      block.oldStart,
                      block.oldEnd,
                      segment.hunk.oldStart,
                      hunkOldEnd,
                    ),
                  );
                  const movedNewBlocksInHunk = movedBlocks.filter((block) =>
                    intersects(
                      block.newStart,
                      block.newEnd,
                      segment.hunk.newStart,
                      hunkNewEnd,
                    ),
                  );

                  return (
                    <div key={segment.id}>
                      {splitRows.map((row, index) => (
                        <div
                          className="grid grid-cols-[minmax(0,1fr)_90px_minmax(0,1fr)]"
                          key={`${segment.id}-row-${index}`}
                        >
                          <div className="border-r border-border/60">
                            {(() => {
                              const lineNo = row.old?.lineNo;
                              const inMovedBlock =
                                typeof lineNo === "number" &&
                                movedOldBlocksInHunk.some(
                                  (block) =>
                                    lineNo >= block.oldStart &&
                                    lineNo <= block.oldEnd,
                                );
                              const startMarker =
                                typeof lineNo === "number"
                                  ? movedOldBlocksInHunk.find(
                                      (block) => block.oldStart === lineNo,
                                    )
                                  : undefined;

                              const inlineDiff =
                                row.old?.kind === "removed" &&
                                row.new?.kind === "added"
                                  ? buildInlineWordDiff(
                                      row.old.content ?? "",
                                      row.new.content ?? "",
                                    )
                                  : null;

                              return (
                                <div
                                  className={
                                    startMarker?.key === activeMoveKey
                                      ? "border-l-2 border-amber-500 bg-amber-500/10"
                                      : inMovedBlock
                                        ? "border-l-2 border-amber-500/80"
                                        : undefined
                                  }
                                >
                                  {startMarker ? (
                                    <div className="flex items-center justify-between gap-2 border-y border-amber-500/40 bg-amber-500/10 px-2 py-0.5 text-[11px] text-amber-700 dark:text-amber-300">
                                      <span>
                                        Code moved to lines{" "}
                                        {startMarker.newStart}-
                                        {startMarker.newEnd}
                                      </span>
                                      <button
                                        className="rounded border border-amber-500/40 bg-amber-500/10 px-1.5 py-0.5 text-[10px] transition-colors hover:bg-amber-500/20"
                                        onClick={() =>
                                          focusMovePair(startMarker)
                                        }
                                        title={`Compare moved block ${startMarker.name}`}
                                      >
                                        Compare
                                      </button>
                                    </div>
                                  ) : null}
                                  {renderSplitCell(
                                    row.old,
                                    "old",
                                    inlineDiff?.oldParts,
                                  )}
                                </div>
                              );
                            })()}
                          </div>
                          <div className="border-r border-border/60 bg-muted/10" />
                          <div>
                            {(() => {
                              const lineNo = row.new?.lineNo;
                              const inMovedBlock =
                                typeof lineNo === "number" &&
                                movedNewBlocksInHunk.some(
                                  (block) =>
                                    lineNo >= block.newStart &&
                                    lineNo <= block.newEnd,
                                );
                              const startMarker =
                                typeof lineNo === "number"
                                  ? movedNewBlocksInHunk.find(
                                      (block) => block.newStart === lineNo,
                                    )
                                  : undefined;

                              const inlineDiff =
                                row.old?.kind === "removed" &&
                                row.new?.kind === "added"
                                  ? buildInlineWordDiff(
                                      row.old.content ?? "",
                                      row.new.content ?? "",
                                    )
                                  : null;

                              return (
                                <div
                                  className={
                                    startMarker?.key === activeMoveKey
                                      ? "border-l-2 border-amber-500 bg-amber-500/10"
                                      : inMovedBlock
                                        ? "border-l-2 border-amber-500/80"
                                        : undefined
                                  }
                                >
                                  {startMarker ? (
                                    <div className="border-y border-amber-500/40 bg-amber-500/10 px-2 py-0.5 text-[11px] text-amber-700 dark:text-amber-300">
                                      Code moved from lines{" "}
                                      {startMarker.oldStart}-
                                      {startMarker.oldEnd}
                                    </div>
                                  ) : null}
                                  {renderSplitCell(
                                    row.new,
                                    "new",
                                    inlineDiff?.newParts,
                                  )}
                                </div>
                              );
                            })()}
                          </div>
                        </div>
                      ))}
                    </div>
                  );
                }

                // ── Skip segment ───────────────────────────────────────────
                const total = Math.max(
                  segment.oldLines.length,
                  segment.newLines.length,
                );
                const expansion = expandedSkips[segment.id] ?? {
                  top: 0,
                  bottom: 0,
                };
                const visibleTop = Math.min(expansion.top, total);
                const visibleBottom = Math.min(
                  expansion.bottom,
                  Math.max(total - visibleTop, 0),
                );
                const hiddenCount = Math.max(
                  total - visibleTop - visibleBottom,
                  0,
                );

                const toCell = (
                  line: HighlightedLine | null,
                ): SplitCell | null =>
                  line
                    ? {
                        lineNo: line.lineNo,
                        content: line.content,
                        tokens: line.tokens,
                        kind: "context",
                      }
                    : null;

                const topLines = Array.from(
                  { length: visibleTop },
                  (_, idx) => ({
                    old: toCell(segment.oldLines[idx] ?? null),
                    new: toCell(segment.newLines[idx] ?? null),
                  }),
                );
                const bottomStart = total - visibleBottom;
                const bottomLines = Array.from(
                  { length: visibleBottom },
                  (_, idx) => ({
                    old: toCell(segment.oldLines[bottomStart + idx] ?? null),
                    new: toCell(segment.newLines[bottomStart + idx] ?? null),
                  }),
                );

                return (
                  <div key={segment.id}>
                    {topLines.map((row, idx) => (
                      <div
                        className="grid grid-cols-[minmax(0,1fr)_90px_minmax(0,1fr)]"
                        key={`${segment.id}-top-${idx}`}
                      >
                        <div className="border-r border-border/60">
                          {renderSplitCell(row.old, "old")}
                        </div>
                        <div className="border-r border-border/60 bg-muted/10" />
                        <div>{renderSplitCell(row.new, "new")}</div>
                      </div>
                    ))}

                    {/* Compact expand bar — hidden when nothing is collapsed */}
                    {hiddenCount > 0 && (
                      <div className="flex items-center gap-1.5 border-y border-dashed border-border/60 bg-muted/40 px-3 py-1.5 text-xs text-muted-foreground">
                        {hiddenCount <= EXPAND_STEP ? (
                          <button
                            className="rounded px-2 py-0.5 transition-colors hover:bg-muted"
                            onClick={() => expandSkipAll(segment.id, total)}
                          >
                            ↕ {hiddenCount} line{hiddenCount === 1 ? "" : "s"}
                          </button>
                        ) : (
                          <>
                            <button
                              className="rounded px-2 py-0.5 transition-colors hover:bg-muted"
                              onClick={() => expandSkipTop(segment.id, total)}
                            >
                              ↑ {EXPAND_STEP}
                            </button>
                            <span className="flex-1 text-center">
                              ··· {hiddenCount} unchanged ···
                            </span>
                            <button
                              className="rounded px-2 py-0.5 transition-colors hover:bg-muted"
                              onClick={() =>
                                expandSkipBottom(segment.id, total)
                              }
                            >
                              ↓ {EXPAND_STEP}
                            </button>
                            <button
                              className="rounded px-2 py-0.5 transition-colors hover:bg-muted"
                              onClick={() => expandSkipAll(segment.id, total)}
                            >
                              ↕ all
                            </button>
                          </>
                        )}
                      </div>
                    )}

                    {bottomLines.map((row, idx) => (
                      <div
                        className="grid grid-cols-[minmax(0,1fr)_90px_minmax(0,1fr)]"
                        key={`${segment.id}-bottom-${idx}`}
                      >
                        <div className="border-r border-border/60">
                          {renderSplitCell(row.old, "old")}
                        </div>
                        <div className="border-r border-border/60 bg-muted/10" />
                        <div>{renderSplitCell(row.new, "new")}</div>
                      </div>
                    ))}
                  </div>
                );
              })
            ) : (
              <div className="px-4 py-8 text-sm text-muted-foreground">
                Request a diff to populate the split view.
              </div>
            )}
          </div>
        </div>
      </section>

      {/* <section className="rounded-lg border border-border/60 bg-muted/20 p-3">
        <div className="mb-3 text-sm font-semibold">Recent Events</div>
        <div className="space-y-2">
          {history.length ? (
            history.map((event) => (
              <div
                className="rounded-md border border-border/60 bg-background px-3 py-2 text-xs"
                key={`${event.jobId}-${formatStatus(event.status)}`}
              >
                <div className="font-mono text-[11px] text-muted-foreground">
                  {event.jobId}
                </div>
                <div className="mt-1 font-medium">
                  {formatStatus(event.status)}
                </div>
              </div>
            ))
          ) : (
            <div className="text-sm text-muted-foreground">
              No diff events received yet.
            </div>
          )}
        </div>
      </section> */}

      {/* <Link
        className={buttonVariants({
          className: "w-fit",
        })}
        to="/auth/onboarding"
      >
        Go to Onboarding
      </Link> */}
    </PageLayout>
  );
}

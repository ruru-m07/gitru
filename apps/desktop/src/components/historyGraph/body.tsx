import { GraphRow } from "@gitru/commands";
import { cn } from "@gitru/ui/lib/utils";
import { useEffect, useMemo, useRef } from "react";
import { useOnInView } from "react-intersection-observer";
import Branch from "./columns/branch";
import CommitHash from "./columns/commit-hash";
import Commiters from "./columns/commiters";
import Lanes from "./columns/lanes";
import Stats from "./columns/stats";
import Summary from "./columns/summary";
import Timestamp from "./columns/timestamp";
import { computeGraphLayout, getColumnStyle, processeRows } from "./helper";

const columns = [
  { id: "branchs-col", Component: Branch, colWidth: "300px" },
  { id: "lanes-col", Component: Lanes, colWidth: "400px" },
  { id: "summary-col", Component: Summary, colWidth: "1fr" },
  { id: "commiters-col", Component: Commiters, colWidth: "fit-content(100px)" },
  { id: "timestamp-col", Component: Timestamp, colWidth: "fit-content(100px)" },
  {
    id: "commithash-col",
    Component: CommitHash,
    colWidth: "fit-content(100px)",
  },
  { id: "stats-col", Component: Stats, colWidth: "fit-content(100px)" },
];

type GraphBodyProps = {
  rows: GraphRow[];
  fetchNextPage: () => void;
  hasNextPage: boolean;
  isFetchingNextPage: boolean;
  onVisibleRangeChange?: (range: { start: number; end: number }) => void;
  scrollerRef?: React.RefObject<HTMLDivElement | null>;
};

const GraphBody = ({
  rows,
  fetchNextPage,
  hasNextPage,
  isFetchingNextPage,
  onVisibleRangeChange,
  scrollerRef,
}: GraphBodyProps) => {
  const internalScrollRef = useRef<HTMLDivElement>(null);
  const scrollRef = scrollerRef ?? internalScrollRef;

  const hoverClasses = [
    cn("bg-secondary/70"),
    cn("[&_[data-hidden-branch-refs]]:flex"),
  ];

  const processedRows = useMemo(() => processeRows(rows), [rows]);
  const layout = useMemo(() => computeGraphLayout(rows), [rows]);
  const TOTAL_ROWS = useMemo(() => rows.length, [rows]);

  const bottomRef = useOnInView(
    (inView) => {
      if (inView && hasNextPage && !isFetchingNextPage) {
        fetchNextPage();
      }
    },
    {
      root: scrollRef.current,
      threshold: 0,
      rootMargin: "500px",
    },
  );

  // Compute visible range from scroll position (index in the rows array = global index for current filter view).
  const ROW_H = 32;

  // Throttled visible range reporter to reduce parent re-renders / lag.
  const rafRef = useRef<number | null>(null);
  const lastReportedRef = useRef<{ start: number; end: number } | null>(null);

  const reportVisibleRange = (root: HTMLDivElement) => {
    if (!onVisibleRangeChange) return;

    if (rafRef.current) cancelAnimationFrame(rafRef.current);

    rafRef.current = requestAnimationFrame(() => {
      const start = Math.max(0, Math.floor(root.scrollTop / ROW_H));
      const viewportEnd = Math.ceil(
        (root.scrollTop + root.clientHeight) / ROW_H,
      );
      const end = Math.min(Math.max(start, viewportEnd - 1), rows.length - 1);

      const next = { start, end };
      const last = lastReportedRef.current;

      // Only notify if the range actually changed (avoids unnecessary updates)
      if (!last || last.start !== next.start || last.end !== next.end) {
        lastReportedRef.current = next;
        onVisibleRangeChange(next);
      }
      rafRef.current = null;
    });
  };

  // const handleScroll = (e: React.UIEvent<HTMLDivElement>) => {
  //   reportVisibleRange(e.currentTarget);
  // };

  // Report initial visible range when rows or scroller is ready
  useEffect(() => {
    const root = scrollRef.current;
    if (root && onVisibleRangeChange) {
      // small delay to let layout settle
      const id = window.setTimeout(() => {
        if (root) reportVisibleRange(root);
      }, 0);
      return () => window.clearTimeout(id);
    }
  }, [rows.length, onVisibleRangeChange]);

  // Cleanup any pending raf
  useEffect(() => {
    return () => {
      if (rafRef.current) cancelAnimationFrame(rafRef.current);
    };
  }, []);

  useEffect(() => {
    const root = scrollRef.current;
    if (!root) return;

    let activeRowId: string | undefined | null = null;

    const map = new Map<string, HTMLElement[]>();

    const getRowEls = (rowId: string) => {
      let els = map.get(rowId);
      if (!els) {
        els = Array.from(
          root.querySelectorAll(`[data-cell-id="${rowId}"]`),
        ) as HTMLElement[];
        map.set(rowId, els);
      }
      return els;
    };

    const apply = (rowId: string | undefined | null) => {
      if (activeRowId === rowId) return;

      // remove old
      if (activeRowId) {
        getRowEls(activeRowId).forEach((el) =>
          hoverClasses.forEach((c) => el.classList.remove(c)),
        );
      }

      // apply new
      if (rowId) {
        getRowEls(rowId).forEach((el) =>
          hoverClasses.forEach((c) => el.classList.add(c)),
        );
      }

      activeRowId = rowId;
    };

    const onMove = (e: PointerEvent) => {
      const el = (e.target as Element)?.closest?.("[data-cell]");
      if (!el) {
        apply(null);
        return;
      }

      const rowId = (el as HTMLElement).dataset.cellId;
      apply(rowId);
    };

    const onLeave = () => apply(null);

    root.addEventListener("pointerover", onMove);
    root.addEventListener("pointerleave", onLeave);

    return () => {
      root.removeEventListener("pointerover", onMove);
      root.removeEventListener("pointerleave", onLeave);
    };
  }, []);

  return (
    <div ref={scrollRef} className="flex-1 flex-col min-h-0 overflow-y-auto">
      <div
        className="w-full overflow-x-hidden grid flex-1"
        style={{
          gridTemplateColumns: `${columns.map((c) => c.colWidth).join(" ")}`,
        }}
      >
        {columns.map(({ Component }, idx) => (
          <Component
            key={idx}
            rows={processedRows}
            style={getColumnStyle(idx + 1, TOTAL_ROWS)}
            layout={layout}
            scrollRef={scrollRef}
          />
        ))}
      </div>

      <div className="w-full flex justify-center">
        {isFetchingNextPage && "Loading more..."}
      </div>

      <div ref={bottomRef} className="h-0" />
    </div>
  );
};

export default GraphBody;

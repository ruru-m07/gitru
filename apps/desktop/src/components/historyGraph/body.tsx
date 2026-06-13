import { GraphRow } from "@gitru/commands";
import { cn } from "@gitru/ui/lib/utils";
import { memo, useEffect, useMemo, useRef } from "react";
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
  { id: "branchs-col", Component: Branch, colWidth: "400px" },
  { id: "lanes-col", Component: Lanes, colWidth: "300px" },
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
  scrollerRef?: React.RefObject<HTMLDivElement | null>;
  // Ref to the highlight band div in the overview chart. We update its position/width via direct style mutation on scroll (cheap, no React re-render).
  highlightBandRef?: React.RefObject<HTMLDivElement | null>;
  // The total number of commits in the (filtered) history, needed to compute percentages for the highlight band.
  total?: number;
};

const GraphBodyInner = ({
  rows,
  fetchNextPage,
  hasNextPage,
  isFetchingNextPage,
  scrollerRef,
  highlightBandRef,
  total = 0,
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

  // On scroll, we update the highlight band in the overview chart via direct DOM style mutation (left/width %).
  // This is cheap and does *not* cause React re-renders of the chart or parent.
  // The band position is kept in sync with the list viewport without going through state.
  const reportVisibleRange = (root: HTMLDivElement) => {
    const start = Math.max(0, Math.floor(root.scrollTop / ROW_H));
    const visibleCount = Math.max(1, Math.ceil(root.clientHeight / ROW_H));
    const end = start + visibleCount - 1;

    // Direct DOM update on the highlight band (the "brush" visual in the chart).
    if (highlightBandRef?.current && total > 0) {
      const leftPct = (start / total) * 100;
      const widthPct = (visibleCount / total) * 100;
      highlightBandRef.current.style.left = `${leftPct}%`;
      highlightBandRef.current.style.width = `${widthPct}%`;
    }
  };

  const handleScroll = (e: React.UIEvent<HTMLDivElement>) => {
    reportVisibleRange(e.currentTarget);
  };

  // Initial report (so the band is positioned correctly on mount / after data load).
  useEffect(() => {
    const root = scrollRef.current;
    if (root) {
      // small delay for layout
      const id = window.setTimeout(() => reportVisibleRange(root), 0);
      return () => window.clearTimeout(id);
    }
  }, [rows.length, total]);

  // Also update on container resize (e.g. user resizes the window/panel).
  useEffect(() => {
    const root = scrollRef.current;
    if (!root) return;

    const ro = new ResizeObserver(() => {
      reportVisibleRange(root);
    });
    ro.observe(root);
    return () => ro.disconnect();
  }, [total]);

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
    <div className="flex min-h-0 flex-1 flex-col">
      <div
        ref={scrollRef}
        className="h-full flex-1 overflow-y-auto"
        style={{
          transform: "translateZ(0)",
          willChange: "transform",
          contain: "layout paint size",
        }}
        onScroll={handleScroll}
      >
        <div
          className="overscroll-y-contain w-full overflow-x-hidden grid"
          style={{
            gridTemplateColumns: `${columns.map((c) => c.colWidth).join(" ")}`,
            transform: "translateZ(0)",
            willChange: "transform",
            contain: "paint",
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

        <div ref={bottomRef} className="h-4" />
      </div>
    </div>
  );
};

const GraphBody = memo(GraphBodyInner);
export default GraphBody;

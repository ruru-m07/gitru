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
};

const GraphBody = ({
  rows,
  fetchNextPage,
  hasNextPage,
  isFetchingNextPage,
}: GraphBodyProps) => {
  const scrollRef = useRef<HTMLDivElement>(null);

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
      console.log(rowId);
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

export default GraphBody;

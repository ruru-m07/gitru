import { GraphRow } from "@gitru/commands";
import { useMemo, useRef } from "react";
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

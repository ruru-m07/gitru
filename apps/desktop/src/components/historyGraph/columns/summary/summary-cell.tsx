import React from "react";
import { ProcessedRow } from "../../helper";

type SummaryCellProps = {
  row: ProcessedRow;
};

const SummaryCell = React.memo(({ row }: SummaryCellProps) => {
  const {
    row: { commit },
  } = row;
  return (
    <div className="flex items-center gap-2 px-2 min-w-0 relative">
      <span className="flex items-center min-w-0">
        <span className="min-w-0 truncate text-sm">{commit.summary}</span>
      </span>
      <div className="absolute py-0.5 top-0 left-0 h-full">
        <div
          className="h-full w-0.5"
          style={{
            background: `repeating-linear-gradient(
              0deg,
              color-mix(in oklab, ${row.color} 90%, var(--color-background)) 0px,
              color-mix(in oklab, ${row.color} 90%, var(--color-background)) 1.5px,
              transparent 1.5px,
              transparent 3px
            )`,
            // backgroundColor: row.color,
          }}
        />
      </div>
    </div>
  );
});

export default SummaryCell;

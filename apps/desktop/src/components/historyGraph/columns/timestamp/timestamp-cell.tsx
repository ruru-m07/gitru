import React from "react";
import { timeAgoFromUnixSeconds } from "@/lib/time";
import { ProcessedRow } from "../../helper";

type TimestampCellProps = {
  row: ProcessedRow;
};

const TimestampCell = React.memo(({ row }: TimestampCellProps) => {
  const { row: commitRow } = row;

  return (
    <div
      data-cell
      data-cell-id={row.row.oid}
      className="flex items-center gap-2 border-l px-2 min-w-0"
    >
      <div className="min-w-0 flex items-center justify-center gap-1 text-muted-foreground">
        <span className="text-nowrap truncate min-w-0 text-muted-foreground text-sm shrink-0">
          {timeAgoFromUnixSeconds(commitRow.commit?.timestamp || 0)}
        </span>
      </div>
    </div>
  );
});

export default TimestampCell;

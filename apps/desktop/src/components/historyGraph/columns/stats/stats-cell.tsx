import { FileDiff } from "lucide-react";
import React from "react";
import { DiffStat } from "@/components/diffBoxes";
import { ProcessedRow } from "../../helper";

type StatsCellProps = {
  row: ProcessedRow;
};

const StatsCell = React.memo(({ row }: StatsCellProps) => {
  const { row: innerRow } = row;

  return (
    <div
      data-cell
      data-cell-id={innerRow.oid}
      className="flex items-center justify-between gap-2 border-l px-2"
    >
      <div className="flex items-center justify-center gap-1 text-muted-foreground">
        <FileDiff className="size-4 opacity-75" />
        <span className="flex text-sm gap-2 font-mono tabular-nums">
          {innerRow.commit.stats.files_changed}
        </span>
      </div>
      <div className="flex items-center justify-end gap-2">
        <span className="flex text-sm gap-2 font-mono">
          <span className="tabular-nums text-green-600">
            +{innerRow.commit.stats.insertions}
          </span>
          <span className="tabular-nums text-red-600">
            -{innerRow.commit.stats.deletions}
          </span>
        </span>
        <DiffStat stats={innerRow.commit.stats} />
      </div>
    </div>
  );
});

export default StatsCell;

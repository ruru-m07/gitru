import React, { useMemo } from "react";
import { ProcessedRow } from "../../helper";
import LaneBackground from "./lane-background";
import LaneGraph, { laneToX, ROW_H } from "./lane-graph";

type LanesCellProps = {
  row: ProcessedRow;
  maxLane: number;
};

function getLaneX(row: ProcessedRow) {
  const input = row.row.input_swimlanes;

  const idx = input.findIndex((n) => n.id === row.row.oid);
  const laneIndex = idx !== -1 ? idx : input.length;

  return laneToX(laneIndex);
}

const LanesCell = React.memo(({ row, maxLane }: LanesCellProps) => {
  const { color, row: innerRow } = row;
  const cx = useMemo(() => getLaneX(row), [row]);

  const isAnyRefs = row.row.refs.length > 0;

  return (
    <div
      style={{
        transform: "translateZ(0)",
        willChange: "transform",
        contain: "layout paint",
        height: ROW_H,
      }}
      className="relative"
    >
      <LaneBackground cx={cx} color={color} disableLeftLine={!isAnyRefs} />
      <LaneGraph row={innerRow} maxLane={maxLane} />
    </div>
  );
});

export default LanesCell;

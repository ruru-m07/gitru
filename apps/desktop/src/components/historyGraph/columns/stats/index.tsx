import React from "react";
import { GraphColumnsType } from "../../helper";
import StatsCell from "./stats-cell";

const Stats = React.memo(({ rows, ...restProps }: GraphColumnsType) => {
  return (
    <div {...restProps}>
      {rows.map((row) => {
        return <StatsCell key={row.row.oid} row={row} />;
      })}
    </div>
  );
});

export default Stats;

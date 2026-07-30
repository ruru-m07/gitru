import React from "react";
import { GraphColumnsType } from "../../helper";
import SummaryCell from "./summary-cell";

const Summary = React.memo(({ rows, ...restProps }: GraphColumnsType) => {
  return (
    <div className="min-w-0 overflow-x-auto hidden" {...restProps}>
      {rows.map((row) => {
        return <SummaryCell row={row} />;
      })}
    </div>
  );
});

export default Summary;

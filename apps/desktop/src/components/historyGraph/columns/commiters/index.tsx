import React from "react";
import { GraphColumnsType } from "../../helper";
import CommitersCell from "./commiters-cell";

const Commiters = React.memo(({ rows, ...restProps }: GraphColumnsType) => {
  return (
    <div {...restProps}>
      {rows.map((row) => (
        <CommitersCell key={row.row.oid} row={row} />
      ))}
    </div>
  );
});

export default Commiters;

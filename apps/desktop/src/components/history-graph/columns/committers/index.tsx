import React from "react";
import { GraphColumnsType } from "../../helper";
import CommittersCell from "./committers-cell";

const Committers = React.memo(({ rows, ...restProps }: GraphColumnsType) => {
  return (
    <div {...restProps}>
      {rows.map((row) => (
        <CommittersCell key={row.row.oid} row={row} />
      ))}
    </div>
  );
});

export default Committers;

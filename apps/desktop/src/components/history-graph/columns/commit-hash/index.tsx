import React from "react";
import { GraphColumnsType } from "../../helper";
import CommitHashCell from "./commit-hash-cell";

const CommitHash = React.memo(({ rows, ...restProps }: GraphColumnsType) => {
  return (
    <div {...restProps}>
      {rows.map((row) => {
        return <CommitHashCell key={row.row.oid} row={row} />;
      })}
    </div>
  );
});

export default CommitHash;

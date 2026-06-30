import React from "react";
import { GraphColumnsType } from "../../helper";
import TimestampCell from "./timestamp-cell";

const Timestamp = React.memo(({ rows, ...restProps }: GraphColumnsType) => {
  return (
    <div {...restProps}>
      {rows.map((row) => {
        return <TimestampCell key={row.row.oid} row={row} />;
      })}
    </div>
  );
});

export default Timestamp;

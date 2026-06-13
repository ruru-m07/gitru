import React from "react";
import { GraphColumnsType } from "../../helper";
import LanesCell from "./lane-cell";

const Lanes = React.memo(
  ({ rows, layout, style, scrollRef, ...restProps }: GraphColumnsType) => {
    return (
      <div
        className="min-w-0 overflow-x-auto"
        style={{
          ...style,
          // contain: "layout paint size",
        }}
        {...restProps}
      >
        {rows.map((row) => {
          return (
            <LanesCell key={row.row.oid} row={row} maxLane={layout.maxLane} />
          );
        })}
      </div>
    );
  },
);

export default Lanes;

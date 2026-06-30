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
        /**
         * ? there is bug in webkit/WKWebView where if the child contaniner
         * ? has overflow-x: auto, the parent container's vertical scroll will
         * ? not work when the mouse is over the child container. to deal with
         * ? this, we listen on wheel event and manually scroll the parent
         * ? container when the scroll intent is vertical.
         */
        onWheel={(e: React.WheelEvent<HTMLDivElement>) => {
          const parent = scrollRef.current;
          if (!parent) return;

          const isVerticalIntent = Math.abs(e.deltaY) > Math.abs(e.deltaX);

          if (isVerticalIntent) {
            parent.scrollTop += e.deltaY;
            e.preventDefault();
            e.stopPropagation(); // ! <-- for WKWebView
          }
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

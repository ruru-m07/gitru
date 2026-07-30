import { ROW_H } from "./lane-graph";

type LaneBackgroundProps = {
  cx: number;
  color: string;
  disableLeftLine?: boolean;
};

const LaneBackground = ({
  cx,
  color,
  disableLeftLine = false,
}: LaneBackgroundProps) => {
  return (
    <>
      <div
        className="absolute inset-0 mt-0.5 z-0"
        style={{
          top: 0,
          left: cx,
          height: ROW_H - 4,
          backgroundColor: `color-mix(in oklab, ${color} 15%, var(--color-background))`,
          transform: "translateZ(0)",
          willChange: "contents",
          contain: "layout paint size",
        }}
      />
      {!disableLeftLine && (
        <div
          className="absolute left-0 -translate-y-1/2 inset-0 mt-0.5 z-0"
          style={{
            height: 2,
            top: ROW_H / 2 - 2,
            width: cx,
            backgroundColor: `color-mix(in oklab, ${color} 50%, var(--color-background))`,
            transform: "translateZ(0)",
            willChange: "contents",
            contain: "layout paint size",
          }}
        />
      )}
    </>
  );
};

export default LaneBackground;

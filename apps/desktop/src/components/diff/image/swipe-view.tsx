import { ImgComparisonSlider } from "@img-comparison-slider/react";
import { formatBytes } from "./image-diff-viewer";
import type { ImageDiffViewProps } from "./types";

export const SwipeView = ({
  beforeUrl,
  afterUrl,
  before,
  after,
  width,
  height,
  onBeforeImageLoad,
  onAfterImageLoad,
  afterDimensions,
  beforeDimensions,
}: ImageDiffViewProps) => {
  if (!before || !after || !beforeUrl || !afterUrl) {
    return null;
  }

  const beforeBytes = before.bytes ? formatBytes(before.bytes) : null;
  const afterBytes = after.bytes ? formatBytes(after.bytes) : null;

  const bytesDelta = (after?.bytes ?? 0) - (before?.bytes ?? 0);
  return (
    <div className="space-y-1">
      <div className="flex items-center justify-between text-xs">
        <div className="tabular-nums text-red-600">
          <span className="font-medium">Deleted</span>
          {beforeDimensions && (
            <span className="ml-2 text-muted-foreground">
              {beforeDimensions.width}px × {beforeDimensions.height}px
            </span>
          )}
          {beforeBytes && <span className="ml-2">{beforeBytes}</span>}
        </div>

        <div className="tabular-nums text-green-600">
          <span className="font-medium">Added</span>
          {afterDimensions && (
            <span className="ml-2 text-muted-foreground">
              {afterDimensions.width}px × {afterDimensions.height}px
            </span>
          )}
          {afterBytes && <span className="ml-2">{afterBytes}</span>}
        </div>
      </div>

      <div className="checkerboard ring-2 ring-border p-px bg-background/70">
        <ImgComparisonSlider
          value={50}
          className="[--divider-width:4px] [--divider-color:var(--color-primary)] [--default-handle-opacity:0] focus:outline-0 mx-auto block"
          style={{ width, height, overflow: "hidden" }}
          aria-label="Before and after slider"
        >
          <img
            slot="first"
            src={beforeUrl}
            alt="Previous image"
            onLoad={(event) => onBeforeImageLoad(event.currentTarget)}
            className="h-full w-full object-contain"
            draggable={false}
          />
          <img
            slot="second"
            src={afterUrl}
            alt="Current image"
            onLoad={(event) => onAfterImageLoad(event.currentTarget)}
            className="h-full w-full object-contain"
            draggable={false}
          />
        </ImgComparisonSlider>
      </div>

      {before && after ? (
        <div className="w-full flex justify-center mt-4 text-sm gap-1">
          <span>Delta: </span>
          <span className={bytesDelta >= 0 ? "text-red-600" : "text-green-600"}>
            {bytesDelta >= 0 ? "+" : "-"}
            {formatBytes(Math.abs(bytesDelta))} (
            {((Math.abs(bytesDelta) / (before.bytes || 1)) * 100).toFixed(2)}%)
          </span>
        </div>
      ) : null}
    </div>
  );
};

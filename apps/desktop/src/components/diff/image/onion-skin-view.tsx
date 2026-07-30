import { Slider } from "@gitru/ui/components/slider";
import { useState } from "react";
import { formatBytes } from "./image-diff-viewer";
import type { ImageDiffViewProps } from "./types";

export const OnionSkinView = ({
  before,
  after,
  beforeUrl,
  afterUrl,
  width,
  height,
  onBeforeImageLoad,
  onAfterImageLoad,
  afterDimensions,
  beforeDimensions,
}: ImageDiffViewProps) => {
  const [opacity, setOpacity] = useState(0.5);

  if (!before || !after || !beforeUrl || !afterUrl) {
    return null;
  }

  const beforeBytes = before.bytes ? formatBytes(before.bytes) : null;
  const afterBytes = after.bytes ? formatBytes(after.bytes) : null;

  const bytesDelta = (after?.bytes ?? 0) - (before?.bytes ?? 0);

  return (
    <div>
      <div className="flex justify-center mb-3">
        <Slider
          min={0}
          max={100}
          step={1}
          value={Math.round(opacity * 100)}
          onValueChange={(v) => {
            setOpacity((v as number) / 100);
          }}
        />
      </div>

      <div className="flex items-center justify-between text-xs mb-2 select-none">
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

      <div className="relative mx-auto" style={{ width, height }}>
        <div className="absolute inset-0 checkerboard ring-2 ring-red-600 p-px flex justify-center items-center h-fit">
          <img
            src={beforeUrl}
            alt="Previous image"
            onLoad={(event) => onBeforeImageLoad(event.currentTarget)}
            style={{ width, height, objectFit: "contain" }}
          />
        </div>
        <div
          style={{
            opacity,
          }}
          className="absolute inset-0 checkerboard ring-2 ring-green-600 p-px flex justify-center items-center h-fit"
        >
          <img
            src={afterUrl}
            alt="Current image"
            onLoad={(event) => onAfterImageLoad(event.currentTarget)}
            style={{
              width,
              height,
              objectFit: "contain",
            }}
          />
        </div>
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

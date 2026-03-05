import { useEffect, useState } from "react";
import { useDiffViewerSettings } from "../useDiffViewSettingStore";
import { formatBytes } from "./ImageDiffViewer";
import {
  OdiffNodePixelDiffProvider,
  WorkerPixelDiffProvider,
} from "./pixelDiffProvider";
import type { ImageDiffViewProps } from "./types";

const workerProvider = new WorkerPixelDiffProvider();
const odiffProvider = new OdiffNodePixelDiffProvider();

export const DifferenceView = ({
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
  const { differenceDiffProvider } = useDiffViewerSettings();
  const [maskDataUrl, setMaskDataUrl] = useState<string | null>(null);
  const [ratio, setRatio] = useState<number | null>(null);

  const beforeBytes = before?.bytes ? formatBytes(before.bytes) : null;
  const afterBytes = after?.bytes ? formatBytes(after.bytes) : null;

  const bytesDelta = (after?.bytes ?? 0) - (before?.bytes ?? 0);

  useEffect(() => {
    let active = true;

    const run = async () => {
      if (!beforeUrl || !afterUrl || width <= 0 || height <= 0) {
        setMaskDataUrl(null);
        setRatio(null);
        return;
      }

      setMaskDataUrl(null);
      setRatio(null);
      if (differenceDiffProvider === "cssOnly") {
        return;
      }

      const runWorker = () =>
        workerProvider.compute({
          beforeUrl,
          afterUrl,
          width,
          height,
          beforePath: before?.absolute_path,
          afterPath: after?.absolute_path,
        });

      const result =
        differenceDiffProvider === "odiffNode"
          ? ((await odiffProvider.compute({
              beforeUrl,
              afterUrl,
              width,
              height,
              beforePath: before?.absolute_path,
              afterPath: after?.absolute_path,
            })) ?? (await runWorker()))
          : await runWorker();

      if (!active || !result) {
        return;
      }

      setMaskDataUrl(result.maskDataUrl);
      setRatio(result.mismatchRatio);
    };

    run();
    return () => {
      active = false;
    };
  }, [
    before,
    after,
    beforeUrl,
    afterUrl,
    width,
    height,
    differenceDiffProvider,
  ]);

  if (!before || !after || !beforeUrl || !afterUrl) {
    return null;
  }

  return (
    <div>
      <div className="text-sm text-muted-foreground mb-2">
        {(differenceDiffProvider === "odiffNode" ||
          differenceDiffProvider === "worker") && (
          <span className="tabular-nums">
            <span className="font-semibold text-foreground">
              {((ratio || 0) * 100).toFixed(2)}%
            </span>
            <span className="ml-1">pixels changed</span>
          </span>
        )}
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
      <div
        className="checkerboard ring-2 ring-red-600 p-px flex justify-center items-center h-fit relative"
        style={{ width, height }}
      >
        <img
          src={beforeUrl}
          alt="Previous image"
          onLoad={(event) => onBeforeImageLoad(event.currentTarget)}
          className="absolute inset-0"
          style={{ width, height, objectFit: "contain" }}
        />
        <img
          src={afterUrl}
          alt="Current image"
          onLoad={(event) => onAfterImageLoad(event.currentTarget)}
          className="absolute inset-0"
          style={{
            width,
            height,
            objectFit: "contain",
            mixBlendMode: "difference",
            opacity: 0.86,
          }}
        />
        {maskDataUrl ? (
          <img
            src={maskDataUrl}
            alt="Pixel diff mask"
            className="absolute inset-0"
            style={{
              width,
              height,
              objectFit: "contain",
              pointerEvents: "none",
            }}
          />
        ) : null}
      </div>
      {before && after ? (
        <div className="w-full flex justify-center mt-2 text-sm gap-1">
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

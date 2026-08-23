import { formatBytes } from "./image-diff-viewer";
import type { ImageDiffViewProps } from "./types";

export const TwoUpView = ({
  before,
  after,
  beforeUrl,
  afterUrl,
  onBeforeImageLoad,
  onAfterImageLoad,
  afterDimensions,
  beforeDimensions,
}: ImageDiffViewProps) => {
  const bytesDelta = (after?.bytes ?? 0) - (before?.bytes ?? 0);

  return (
    <div>
      <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
        {before && beforeUrl ? (
          <div>
            <div className="text-xs mb-1 flex items-center justify-between">
              <span className="text-red-600">Deleted</span>
              {beforeDimensions ? (
                <div>
                  <span className="ml-0.5 tabular-nums text-muted-foreground">
                    <span>
                      <span className="text-foreground font-semibold">W: </span>
                      <span>{beforeDimensions.width}px</span>
                    </span>
                    <span>
                      <span className="text-foreground ml-2 font-semibold">
                        H:{" "}
                      </span>
                      <span>{beforeDimensions.height}px</span>
                    </span>
                  </span>
                </div>
              ) : null}
              <span>
                <span className="ml-1 tabular-nums text-red-600">
                  {before.bytes ? `${formatBytes(before.bytes)}` : null}
                </span>
              </span>
            </div>
            <div className="checkerboard ring-2 ring-red-600 p-px flex justify-center items-center h-fit">
              <img
                src={beforeUrl}
                alt="Previous image"
                onLoad={(event) => onBeforeImageLoad(event.currentTarget)}
              />
            </div>
          </div>
        ) : null}

        {after && afterUrl ? (
          <div>
            <div className="text-xs mb-1 flex items-center justify-between">
              <span className="text-green-600">Added</span>
              {afterDimensions ? (
                <div>
                  <span className="ml-0.5 tabular-nums text-muted-foreground">
                    <span>
                      <span className="text-foreground font-semibold">W: </span>
                      <span>{afterDimensions.width}px</span>
                    </span>
                    <span>
                      <span className="text-foreground ml-2 font-semibold">
                        H:{" "}
                      </span>
                      <span>{afterDimensions.height}px</span>
                    </span>
                  </span>
                </div>
              ) : null}
              <span className="ml-1 tabular-nums text-green-600">
                {after.bytes ? `${formatBytes(after.bytes)}` : null}
              </span>
            </div>
            <div className="checkerboard ring-2 ring-green-600 p-0.5 flex justify-center items-center h-fit">
              <img
                src={afterUrl}
                alt="Current image"
                onLoad={(event) => onAfterImageLoad(event.currentTarget)}
              />
            </div>
          </div>
        ) : null}
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

import type { ScrollMetrics, VisibleRange } from "./types.js";

export const DEFAULT_MAX_PHYSICAL_HEIGHT = 33_500_000;

export function computeScrollMetrics(
  count: number,
  rowHeight: number,
  viewportHeight: number,
  maxPhysicalHeight = DEFAULT_MAX_PHYSICAL_HEIGHT,
): ScrollMetrics {
  const safeCount = Math.max(0, Math.floor(count));
  const safeRowHeight = Math.max(1, rowHeight);
  const safeViewportHeight = Math.max(0, viewportHeight);
  const logicalHeight = safeCount * safeRowHeight;
  const logicalScrollRange = Math.max(0, logicalHeight - safeViewportHeight);
  const physicalScrollRange = Math.min(
    logicalScrollRange,
    Math.max(0, maxPhysicalHeight - safeViewportHeight),
  );
  const scale =
    logicalScrollRange > 0 ? physicalScrollRange / logicalScrollRange : 1;

  return {
    count: safeCount,
    rowHeight: safeRowHeight,
    viewportHeight: safeViewportHeight,
    logicalHeight,
    logicalScrollRange,
    physicalScrollRange,
    scale,
  };
}

export function physicalToLogical(
  physicalScrollTop: number,
  metrics: ScrollMetrics,
): number {
  if (metrics.scale <= 0) return 0;
  return Math.max(
    0,
    Math.min(metrics.logicalScrollRange, physicalScrollTop / metrics.scale),
  );
}

export function logicalToPhysical(
  logicalScrollTop: number,
  metrics: ScrollMetrics,
): number {
  return (
    Math.max(0, Math.min(metrics.logicalScrollRange, logicalScrollTop)) *
    metrics.scale
  );
}

export function visibleRange(
  logicalScrollTop: number,
  metrics: ScrollMetrics,
): VisibleRange {
  const start = Math.max(
    0,
    Math.min(metrics.count, Math.floor(logicalScrollTop / metrics.rowHeight)),
  );
  const end = Math.max(
    start,
    Math.min(
      metrics.count,
      Math.ceil(
        (logicalScrollTop + metrics.viewportHeight) / metrics.rowHeight,
      ),
    ),
  );
  return { start, end };
}

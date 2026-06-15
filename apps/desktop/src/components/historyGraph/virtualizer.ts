export const ROW_H = 32;

// Rows to render above and below the visible viewport.
// Larger overscan = fewer React re-renders needed while scrolling.
export const OVERSCAN_ROWS = 40;

// Re-render when the visible window gets within this many rows of the
// rendered boundary. Must be < OVERSCAN_ROWS.
export const TRIGGER_ROWS = 15;

export type VirtualRange = { start: number; end: number };

export function computeRange(
  scrollTop: number,
  clientHeight: number,
  totalRows: number,
): VirtualRange {
  const start = Math.max(0, Math.floor(scrollTop / ROW_H) - OVERSCAN_ROWS);
  const end = Math.min(totalRows, Math.ceil((scrollTop + clientHeight) / ROW_H) + OVERSCAN_ROWS);
  return { start, end };
}

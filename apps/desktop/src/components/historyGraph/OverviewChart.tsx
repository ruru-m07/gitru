import { CommitOverview } from "@gitru/commands";
import { memo, useCallback, useMemo, useRef } from "react";
import {
  ComposedChart,
  Bar,
  Line,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  ResponsiveContainer,
  ReferenceLine,
} from "recharts";

export interface OverviewChartProps {
  data?: CommitOverview | null;
  isLoading?: boolean;
  onRangeRequest?: (range: { start: number; end: number }) => void;
  className?: string;
  /** Fixed logical height for the area. */
  height?: number;
  /** Ref to the list scroller (used during band drag to read live viewport size and to scroll the list). */
  scrollerRef?: React.RefObject<HTMLDivElement | null>;
  /** Ref to the highlight band div. The list scroll path updates this via direct style.left/width (DOM mutation, zero React re-renders). */
  highlightBandRef?: React.RefObject<HTMLDivElement | null>;
  /** Total number of items in the filtered history (for % calculations on the band). */
  total?: number;
}

/**
 * OverviewChart
 *
 * - Uses Recharts (ComposedChart + Bar + Line) to render the activity visualization.
 *   This is the "chart".
 * - The "brush" (current viewport range highlight) is a simple absolute overlay div
 *   drawn directly on top of the Recharts plot. This gives the integrated "chart with brush on it"
 *   look from the original request (shaded band + grips on the activity bars/line), instead of
 *   a separate brush strip below the chart.
 * - Dragging the band overlay calls onRangeRequest (which the parent uses to scroll the list via the scroller ref).
 * - List scroll updates the band position via cheap direct DOM mutation on the highlightBandRef (no setState, no re-render of the chart).
 *
 * This is scalable (Recharts handles the data rendering), uses almost no useEffect, reuses the library for the viz,
 * and avoids the rerender storm on scroll by using DOM mutations for the highlight sync.
 */
const OverviewChart = ({
  data,
  isLoading,
  onRangeRequest,
  className,
  height = 78,
  scrollerRef,
  highlightBandRef,
  total = 0,
}: OverviewChartProps) => {
  const wrapperRef = useRef<HTMLDivElement | null>(null);

  // Data for Recharts. Indices are kept 1:1 with the global history indices so range requests map directly.
  const chartData = useMemo(() => {
    if (!data) return [];
    const n = Math.min(data.insertions.length, data.total);
    const arr: Array<{ idx: number; ins: number; del: number; activity: number }> = [];
    for (let i = 0; i < n; i++) {
      const ins = data.insertions[i] || 0;
      const del = data.deletions[i] || 0;
      arr.push({
        idx: i,
        ins,
        del: -del, // negative so the bar renders below the baseline
        activity: ins + del,
      });
    }
    return arr;
  }, [data]);

  const seriesLen = chartData.length;

  // Basic drag logic for the brush band overlay (minimal, the heavy viz is Recharts).
  // We use the live scroller to know the current "window size" in data terms (how many indices fit in the list viewport).
  const dragStateRef = useRef<{
    pointerId: number;
    offset: number; // mouseIdx - start when drag started
    bandEl: HTMLDivElement | null;
  } | null>(null);

  const getMouseIdx = (clientX: number): number => {
    const rect = wrapperRef.current?.getBoundingClientRect();
    if (!rect || total <= 0) return 0;
    const frac = Math.max(0, Math.min(1, (clientX - rect.left) / rect.width));
    return Math.floor(frac * (total - 1));
  };

  const handleBandPointerDown = (e: React.PointerEvent<HTMLDivElement>) => {
    if (!onRangeRequest || total <= 0) return;

    const bandEl = e.currentTarget;
    const mouseIdx = getMouseIdx(e.clientX);

    // Current "window" size from the actual list viewport (how many rows fit).
    const scroller = scrollerRef?.current;
    const visibleCount = scroller
      ? Math.max(1, Math.ceil(scroller.clientHeight / 32))
      : 25;

    const currentStart = Math.max(0, Math.min(total - visibleCount, mouseIdx)); // rough
    const offset = mouseIdx - currentStart;

    bandEl.setPointerCapture(e.pointerId);
    dragStateRef.current = {
      pointerId: e.pointerId,
      offset,
      bandEl,
    };

    // Optional: give immediate feedback
    bandEl.style.cursor = "grabbing";
  };

  const handleBandPointerMove = (e: React.PointerEvent<HTMLDivElement>) => {
    const drag = dragStateRef.current;
    if (!drag || !onRangeRequest || total <= 0) return;

    const mouseIdx = getMouseIdx(e.clientX);
    const scroller = scrollerRef?.current;
    const visibleCount = scroller
      ? Math.max(1, Math.ceil(scroller.clientHeight / 32))
      : 25;

    let newStart = mouseIdx - drag.offset;
    if (newStart < 0) newStart = 0;
    if (newStart + visibleCount > total) newStart = total - visibleCount;

    // Live update the band's visual (DOM) and the list scroll (DOM) — no React state, no re-render.
    const leftPct = (newStart / total) * 100;
    const widthPct = (visibleCount / total) * 100;

    if (drag.bandEl) {
      drag.bandEl.style.left = `${leftPct}%`;
      drag.bandEl.style.width = `${widthPct}%`;
    }

    if (scroller) {
      scroller.scrollTop = newStart * 32;
    }
  };

  const handleBandPointerUp = (e: React.PointerEvent<HTMLDivElement>) => {
    const drag = dragStateRef.current;
    if (!drag) return;

    const bandEl = drag.bandEl || e.currentTarget;
    bandEl.releasePointerCapture(drag.pointerId);
    bandEl.style.cursor = "";

    // At the end of the gesture we can call onRangeRequest if the parent wants to record the final range
    // or trigger loading. The live scroll + style already happened above.
    const scroller = scrollerRef?.current;
    if (scroller && onRangeRequest) {
      const currentStart = Math.floor(scroller.scrollTop / 32);
      const visibleCount = Math.max(1, Math.ceil(scroller.clientHeight / 32));
      onRangeRequest({ start: currentStart, end: currentStart + visibleCount - 1 });
    }

    dragStateRef.current = null;
  };

  // Click anywhere on the chart (outside the band) to jump the list to that location.
  const handleChartClick = (e: React.MouseEvent<HTMLDivElement>) => {
    if (!onRangeRequest || total <= 0) return;

    // Ignore clicks that started on the band (they are handled by the pointer handlers).
    if ((e.target as HTMLElement).closest("[data-brush-band]")) return;

    const mouseIdx = getMouseIdx(e.clientX);
    const scroller = scrollerRef?.current;
    const visibleCount = scroller
      ? Math.max(1, Math.ceil(scroller.clientHeight / 32))
      : 25;

    let start = mouseIdx - Math.floor(visibleCount / 2);
    if (start < 0) start = 0;
    if (start + visibleCount > total) start = total - visibleCount;

    if (scroller) {
      scroller.scrollTop = start * 32;
    }
    onRangeRequest({ start, end: start + visibleCount - 1 });
  };

  const headIndex = data?.head_index ?? -1;

  return (
    <div
      ref={wrapperRef}
      className={`relative w-full select-none overflow-hidden rounded-sm border-b bg-[#0b0f14] ${className ?? ""}`}
      style={{ height }}
      onClick={handleChartClick}
    >
      {/* Recharts renders the actual chart (bars + line). This is the "chart". */}
      <ResponsiveContainer width="100%" height="100%">
        <ComposedChart
          data={chartData}
          margin={{ top: 4, right: 4, bottom: 4, left: 4 }}
        >
          <CartesianGrid strokeDasharray="2 2" stroke="rgba(148,163,184,0.15)" />
          <XAxis dataKey="idx" type="number" domain={[0, "dataMax"]} hide />
          <YAxis hide />
          <Tooltip
            contentStyle={{
              background: "var(--color-background)",
              border: "1px solid var(--color-border)",
              fontSize: 11,
            }}
          />

          {/* Thin bars for +ins (up) and -del (down) — the background activity bars */}
          <Bar dataKey="ins" fill="#22c55e" barSize={2} />
          <Bar dataKey="del" fill="#ef4444" barSize={2} />

          {/* The single line for the meaningful metric (activity / churn over the commit sequence) */}
          <Line
            type="monotone"
            dataKey="activity"
            stroke="#60a5fa"
            strokeWidth={1.5}
            dot={false}
          />

          {/* Green vertical for HEAD */}
          {headIndex >= 0 && (
            <ReferenceLine x={headIndex} stroke="#22c55e" strokeWidth={2} />
          )}
        </ComposedChart>
      </ResponsiveContainer>

      {/* The "brush" — range highlight band overlaid directly on the chart.
          This is the shaded draggable region the user sees on top of the activity bars/line.
          Positioned via % so it scales with the container. */}
      <div
        ref={highlightBandRef}
        data-brush-band
        className="absolute top-0 bottom-0 border border-blue-500/70 bg-blue-500/15 pointer-events-auto"
        style={{
          left: total > 0 && seriesLen > 0 ? "0%" : "0%",
          width: total > 0 && seriesLen > 0 ? "100%" : "0%",
          // The actual left/width are set by the parent via direct DOM mutation on scroll,
          // and by the drag handlers during band drag.
        }}
        onPointerDown={handleBandPointerDown}
        onPointerMove={handleBandPointerMove}
        onPointerUp={handleBandPointerUp}
        onPointerCancel={handleBandPointerUp}
      >
        {/* Little grips on the edges so it feels like a brush handle */}
        <div className="absolute left-0 top-0 bottom-0 w-[3px] bg-blue-500/80" />
        <div className="absolute right-0 top-0 bottom-0 w-[3px] bg-blue-500/80" />
      </div>

      {/* Loading / total label (same as before) */}
      {isLoading && (
        <div className="pointer-events-none absolute inset-x-0 top-0 flex h-full items-center justify-center">
          <div className="rounded bg-black/40 px-2 py-px text-[10px] text-white/70">
            loading overview…
          </div>
        </div>
      )}
      {total > 0 && (
        <div className="pointer-events-none absolute bottom-0.5 right-1.5 text-[9px] tabular-nums text-white/40">
          {total.toLocaleString()}
        </div>
      )}
    </div>
  );
};

export default memo(OverviewChart);

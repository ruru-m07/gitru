import { CommitOverview } from "@gitru/commands";
import { memo, useEffect, useMemo, useRef, useState } from "react";

export type VisibleRange = { start: number; end: number };

export interface OverviewChartProps {
  data?: CommitOverview | null;
  isLoading?: boolean;
  /** Current visible window in the list (global indices in 0..total). */
  visibleRange?: VisibleRange;
  /** Called when user interaction wants to change the list's visible window (pan via brush). */
  onRangeRequest?: (range: VisibleRange) => void;
  className?: string;
  /** Fixed logical height for the canvas area. */
  height?: number;
}

/** Internal viewport of the chart itself (what slice of history is currently displayed at what zoom). */
type ChartView = { start: number; end: number };

/**
 * OverviewChart
 * Canvas-based single-line + bidirectional thin-bar activity overview.
 * X = commit sequence index (0 = tip/HEAD side for the current filter).
 * - Single line: activity (ins + del), lightly smoothed for readability.
 * - Bars: green upward (insertions), red downward (deletions) from center baseline.
 * - Green vertical rule for HEAD position (data.head_index).
 * - Highlight band for the current list viewport (when provided).
 *
 * Designed to stay fast for 10k–100k+ commits via simple mapping.
 * (Future: per-pixel max aggregation + full chartView pan/zoom state.)
 */
const OverviewChart = ({
  data,
  isLoading,
  visibleRange,
  onRangeRequest,
  className,
  height = 78,
}: OverviewChartProps) => {
  const wrapperRef = useRef<HTMLDivElement | null>(null);
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const [width, setWidth] = useState(800);

  // Track container width responsively.
  useEffect(() => {
    const el = wrapperRef.current;
    if (!el) return;

    const update = () => {
      const w = Math.max(100, Math.floor(el.getBoundingClientRect().width));
      setWidth(w);
    };

    update();
    const ro = new ResizeObserver(update);
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  const total = data?.total ?? 0;
  const hasData = total > 0 && data != null;

  // Pre-compute max for scaling (robust).
  const maxChurn = useMemo(() => {
    if (!data) return 1;
    let m = 1;
    const n = Math.min(data.insertions.length, data.deletions.length);
    for (let i = 0; i < n; i++) {
      m = Math.max(m, data.insertions[i] || 0, data.deletions[i] || 0);
    }
    return m;
  }, [data]);

  // === Chart internal viewport (for pan/zoom/scrollable limited view) ===
  const [chartView, setChartView] = useState<ChartView | null>(null);

  // Initialize / reset chart view when data changes.
  // We show a limited window by default (scrollable/pannable) rather than the entire history at once.
  const seriesLen = hasData && data ? Math.min(data.insertions.length, data.total) : 0;

  useEffect(() => {
    if (!hasData || !data || seriesLen === 0) {
      setChartView(null);
      return;
    }
    // Default: show up to ~4000 most recent commits at good resolution, or full if smaller.
    // User can pan/zoom/scroll the chart view independently.
    const defaultWindow = Math.min(4000, seriesLen);
    const start = Math.max(0, seriesLen - defaultWindow);
    setChartView({ start, end: seriesLen - 1 });
  }, [data, hasData, seriesLen]);

  // Effective view for drawing (falls back to full series if not set)
  const effectiveView = chartView ?? (seriesLen > 0 ? { start: 0, end: seriesLen - 1 } : null);

  // Draw whenever relevant inputs change. We try to keep this cheap.
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;

    const dpr = Math.max(1, Math.min(2, window.devicePixelRatio || 1));
    const cssW = width;
    const cssH = height;

    canvas.width = Math.floor(cssW * dpr);
    canvas.height = Math.floor(cssH * dpr);
    canvas.style.width = `${cssW}px`;
    canvas.style.height = `${cssH}px`;

    const ctx = canvas.getContext("2d", { alpha: true });
    if (!ctx) return;

    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, cssW, cssH);

    // Background
    ctx.fillStyle = "rgba(0,0,0,0.0)";
    ctx.fillRect(0, 0, cssW, cssH);

    if (!hasData || !data || !effectiveView) {
      // Subtle empty state
      ctx.strokeStyle = "rgba(148,163,184,0.35)";
      ctx.lineWidth = 1;
      ctx.beginPath();
      ctx.moveTo(0, cssH * 0.5);
      ctx.lineTo(cssW, cssH * 0.5);
      ctx.stroke();
      return;
    }

    const viewStart = effectiveView.start;
    const viewEnd = effectiveView.end;
    const viewLen = Math.max(1, viewEnd - viewStart + 1);

    const yMid = cssH * 0.5;
    const pad = 3;
    const scale = (cssH * 0.5 - pad) / Math.max(1, maxChurn);

    // Colors
    const green = "#22c55e";
    const red = "#ef4444";
    const lineColor = "#60a5fa";
    const headColor = "#22c55e";
    const rangeFill = "rgba(59, 130, 246, 0.18)";
    const rangeStroke = "rgba(59, 130, 246, 0.65)";

    // Baseline
    ctx.strokeStyle = "rgba(148,163,184,0.25)";
    ctx.lineWidth = 1;
    ctx.beginPath();
    ctx.moveTo(0, yMid);
    ctx.lineTo(cssW, yMid);
    ctx.stroke();

    const linePts: Array<{ x: number; y: number }> = [];
    const step = Math.max(1, Math.floor(viewLen / (cssW * 1.8)));

    // Only draw the data inside the current chart view (this is the "limited" + scrollable part)
    for (let i = viewStart; i <= viewEnd; i += step) {
      const localI = i - viewStart;
      const x = (localI / Math.max(1, viewLen - 1)) * cssW;

      const ins = data.insertions[i] || 0;
      const del = data.deletions[i] || 0;

      const barW = Math.max(1, (cssW / Math.max(1, viewLen)) * 1.8);

      if (ins > 0) {
        const h = Math.min(ins * scale, yMid - pad);
        ctx.fillStyle = green;
        ctx.fillRect(x - barW / 2, yMid - h, barW, h);
      }
      if (del > 0) {
        const h = Math.min(del * scale, yMid - pad);
        ctx.fillStyle = red;
        ctx.fillRect(x - barW / 2, yMid, barW, h);
      }

      const act = ins + del;
      const y = yMid - act * scale * 0.55;
      linePts.push({ x, y });
    }

    // Activity line
    if (linePts.length > 1) {
      ctx.strokeStyle = lineColor;
      ctx.lineWidth = 1.5;
      ctx.lineJoin = "round";
      ctx.lineCap = "round";
      ctx.shadowColor = "rgba(96,165,250,0.2)";
      ctx.shadowBlur = 1.5;
      ctx.beginPath();
      ctx.moveTo(linePts[0].x, linePts[0].y);
      for (let k = 1; k < linePts.length; k++) {
        ctx.lineTo(linePts[k].x, linePts[k].y);
      }
      ctx.stroke();
      ctx.shadowBlur = 0;
    }

    // HEAD indicator (only if inside current view)
    if (data.head_index >= viewStart && data.head_index <= viewEnd) {
      const local = data.head_index - viewStart;
      const headX = (local / Math.max(1, viewLen - 1)) * cssW;
      ctx.strokeStyle = headColor;
      ctx.lineWidth = 2;
      ctx.beginPath();
      ctx.moveTo(headX, 2);
      ctx.lineTo(headX, cssH - 2);
      ctx.stroke();

      ctx.fillStyle = headColor;
      ctx.beginPath();
      ctx.moveTo(headX, 2);
      ctx.lineTo(headX - 4, 9);
      ctx.lineTo(headX + 4, 9);
      ctx.closePath();
      ctx.fill();
    }

    // List viewport highlight band (mapped into current chart view)
    if (visibleRange && total > 0 && effectiveView) {
      const vs = effectiveView.start;
      const ve = effectiveView.end;
      const vLen = Math.max(1, ve - vs + 1);

      // Clip the list range to what is currently visible in the chart
      const s = Math.max(vs, Math.min(visibleRange.start, ve));
      const e = Math.max(s, Math.min(visibleRange.end, ve));

      if (e >= vs && s <= ve) {
        const localS = s - vs;
        const localE = e - vs;
        const x1 = (localS / Math.max(1, vLen - 1)) * cssW;
        const x2 = (localE / Math.max(1, vLen - 1)) * cssW;
        const bw = Math.max(2, x2 - x1);

        ctx.fillStyle = rangeFill;
        ctx.fillRect(x1, 2, bw, cssH - 4);

        ctx.strokeStyle = rangeStroke;
        ctx.lineWidth = 1;
        ctx.strokeRect(x1 + 0.5, 2.5, bw - 1, cssH - 5);

        if (bw > 10) {
          ctx.fillStyle = rangeStroke;
          ctx.fillRect(x1, 2, 2, cssH - 4);
          ctx.fillRect(x1 + bw - 2, 2, 2, cssH - 4);
        }
      }
    }
  }, [data, hasData, width, height, maxChurn, visibleRange, total, effectiveView]);

  // --- Brush / Range interaction + Tooltip + Chart pan/zoom ---
  const [isHover, setIsHover] = useState(false);
  const [bandDrag, setBandDrag] = useState<null | { offset: number }>(null);
  const [chartDrag, setChartDrag] = useState<null | { lastMouseIdx: number }>(null);
  const [tooltip, setTooltip] = useState<{ index: number; clientX: number; clientY: number } | null>(null);

  const getIndexFromClientX = (clientX: number, rect: DOMRect): number => {
    if (!data || data.total === 0) return 0;
    const px = clientX - rect.left;
    const frac = Math.max(0, Math.min(1, px / Math.max(1, rect.width)));
    return Math.floor(frac * Math.max(1, data.total - 1));
  };

  const handlePointerDown = (e: React.PointerEvent<HTMLDivElement>) => {
    if (!data || data.total === 0) return;

    const rect = e.currentTarget.getBoundingClientRect();
    const mouseIdx = getIndexFromClientX(e.clientX, rect);

    // Decide: are we over the list visible band in the current chart view?
    const isOverBand = visibleRange && effectiveView
      ? (mouseIdx >= visibleRange.start && mouseIdx <= visibleRange.end)
      : false;

    if (isOverBand && onRangeRequest) {
      // Band drag → controls the *list* scroll position (existing behavior)
      const offset = mouseIdx - visibleRange!.start;
      setBandDrag({ offset });
      (e.currentTarget as HTMLDivElement).setPointerCapture(e.pointerId);
    } else {
      // Background drag → pan the *chart view* itself (scrollable history)
      if (effectiveView) {
        setChartDrag({ lastMouseIdx: mouseIdx });
        (e.currentTarget as HTMLDivElement).setPointerCapture(e.pointerId);
      } else if (onRangeRequest) {
        // Fallback: jump list
        const half = 12;
        const start = Math.max(0, mouseIdx - half);
        const end = Math.min(data.total - 1, mouseIdx + half);
        onRangeRequest({ start, end });
      }
    }

    setTooltip(null);
  };

  const handlePointerMove = (e: React.PointerEvent<HTMLDivElement>) => {
    const rect = e.currentTarget.getBoundingClientRect();
    const mouseIdx = getIndexFromClientX(e.clientX, rect);

    if (bandDrag && onRangeRequest && visibleRange && data) {
      // Pan list visible band
      let newStart = Math.max(0, mouseIdx - bandDrag.offset);
      const bandSize = Math.max(1, visibleRange.end - visibleRange.start);
      let newEnd = newStart + bandSize;
      if (newEnd >= data.total) {
        newEnd = data.total - 1;
        newStart = Math.max(0, newEnd - bandSize);
      }
      onRangeRequest({ start: Math.floor(newStart), end: Math.floor(newEnd) });
      setTooltip(null);
    } else if (chartDrag && effectiveView) {
      // Pan chart internal view
      const delta = chartDrag.lastMouseIdx - mouseIdx;
      const vs = effectiveView.start;
      const ve = effectiveView.end;
      let newStart = Math.max(0, vs + delta);
      let newEnd = newStart + (ve - vs);

      if (newEnd >= seriesLen) {
        newEnd = seriesLen - 1;
        newStart = Math.max(0, newEnd - (ve - vs));
      }
      setChartView({ start: Math.floor(newStart), end: Math.floor(newEnd) });
      setChartDrag({ lastMouseIdx: mouseIdx });
      setTooltip(null);
    } else if (isHover && hasData && data) {
      const clamped = Math.max(0, Math.min(seriesLen - 1, mouseIdx));
      setTooltip({ index: clamped, clientX: e.clientX, clientY: e.clientY });
    }
  };

  const handlePointerUp = (e: React.PointerEvent<HTMLDivElement>) => {
    const hadCapture = bandDrag || chartDrag;
    if (hadCapture) {
      (e.currentTarget as HTMLDivElement).releasePointerCapture(e.pointerId);
    }
    setBandDrag(null);
    setChartDrag(null);
  };

  const handlePointerLeave = () => {
    setIsHover(false);
    setTooltip(null);
    setBandDrag(null);
    setChartDrag(null);
  };

  // Wheel zoom on the chart view (limited + scrollable)
  const handleWheel = (e: React.WheelEvent<HTMLDivElement>) => {
    if (!effectiveView || !data || seriesLen === 0) return;
    e.preventDefault();

    const rect = e.currentTarget.getBoundingClientRect();
    const mouseIdx = getIndexFromClientX(e.clientX, rect);

    const vs = effectiveView.start;
    const ve = effectiveView.end;
    const currentLen = ve - vs + 1;

    const zoomFactor = e.deltaY > 0 ? 1.25 : 0.8;
    const newLen = Math.max(50, Math.min(seriesLen, Math.round(currentLen * zoomFactor)));

    let newStart = Math.round(mouseIdx - (mouseIdx - vs) * (newLen / currentLen));
    let newEnd = newStart + newLen - 1;

    if (newStart < 0) {
      newEnd += -newStart;
      newStart = 0;
    }
    if (newEnd >= seriesLen) {
      newStart -= (newEnd - seriesLen + 1);
      newEnd = seriesLen - 1;
      if (newStart < 0) newStart = 0;
    }

    setChartView({ start: Math.max(0, Math.floor(newStart)), end: Math.floor(newEnd) });
  };

  return (
    <div
      ref={wrapperRef}
      className={`relative w-full select-none rounded-sm border-b bg-[#0b0f14] ${className ?? ""}`}
      style={{ height }}
      onPointerEnter={() => setIsHover(true)}
      onPointerLeave={handlePointerLeave}
      onPointerDown={handlePointerDown}
      onPointerMove={handlePointerMove}
      onPointerUp={handlePointerUp}
      onWheel={handleWheel}
      onDoubleClick={(e) => {
        // Double-click background → zoom in around that point (quick "select to zoom")
        if (!effectiveView || !data) return;
        const rect = (e.currentTarget as HTMLDivElement).getBoundingClientRect();
        const mouseIdx = getIndexFromClientX(e.clientX, rect);
        const vs = effectiveView.start;
        const ve = effectiveView.end;
        const curLen = ve - vs + 1;
        const newLen = Math.max(30, Math.round(curLen * 0.4));
        let ns = Math.round(mouseIdx - newLen / 2);
        let ne = ns + newLen - 1;
        if (ns < 0) { ne += -ns; ns = 0; }
        if (ne >= seriesLen) { ns -= (ne - seriesLen + 1); ne = seriesLen - 1; if (ns < 0) ns = 0; }
        setChartView({ start: Math.max(0, ns), end: Math.min(seriesLen - 1, ne) });
      }}
      tabIndex={0}
      onKeyDown={(e) => {
        if (!effectiveView) return;
        const vs = effectiveView.start;
        const ve = effectiveView.end;
        const len = ve - vs + 1;
        if (e.key === "ArrowLeft" || e.key === "ArrowRight") {
          e.preventDefault();
          const dir = e.key === "ArrowLeft" ? -1 : 1;
          const step = Math.max(1, Math.round(len * 0.1));
          let ns = Math.max(0, vs + dir * step);
          let ne = ns + len - 1;
          if (ne >= seriesLen) { ne = seriesLen - 1; ns = Math.max(0, ne - len + 1); }
          setChartView({ start: ns, end: ne });
        }
        if (e.key === "+" || e.key === "=") {
          e.preventDefault();
          const center = Math.floor((vs + ve) / 2);
          const newLen = Math.max(30, Math.round(len * 0.6));
          setChartView({ start: Math.max(0, center - Math.floor(newLen/2)), end: Math.min(seriesLen-1, center + Math.floor(newLen/2)) });
        }
        if (e.key === "-") {
          e.preventDefault();
          const center = Math.floor((vs + ve) / 2);
          const newLen = Math.min(seriesLen, Math.round(len * 1.6));
          setChartView({ start: Math.max(0, center - Math.floor(newLen/2)), end: Math.min(seriesLen-1, center + Math.floor(newLen/2)) });
        }
        if (e.key.toLowerCase() === "r") {
          e.preventDefault();
          // Reset to a sensible recent window
          const def = Math.min(4000, seriesLen);
          setChartView({ start: Math.max(0, seriesLen - def), end: seriesLen - 1 });
        }
      }}
      title={hasData ? `${data!.total.toLocaleString()} commits` : "No history"}
    >
      <canvas ref={canvasRef} className="block" />

      {/* Subtle overlay label / loading */}
      {isLoading && (
        <div className="pointer-events-none absolute inset-x-0 top-0 flex h-full items-center justify-center">
          <div className="rounded bg-black/40 px-2 py-px text-[10px] text-white/70">loading overview…</div>
        </div>
      )}

      {hasData && (
        <div className="pointer-events-none absolute bottom-0.5 right-1.5 text-[9px] tabular-nums text-white/40">
          {data!.total.toLocaleString()}
        </div>
      )}

      {/* Drag affordance hint (stays inside the chart) */}
      {onRangeRequest && isHover && hasData && !bandDrag && !chartDrag && (
        <div className="pointer-events-none absolute left-1 top-1 rounded bg-black/30 px-1 text-[9px] text-white/50">
          drag band to scroll list • drag elsewhere to pan chart
        </div>
      )}

      {/* Tooltip - using fixed so it aligns correctly with viewport mouse coords and is not clipped by the small chart or overflow-hidden */}
      {tooltip && hasData && data && (
        <div
          className="pointer-events-none fixed z-[9999] rounded border border-border/50 bg-background px-2.5 py-1.5 text-xs shadow-2xl"
          style={{
            left: tooltip.clientX + 14,
            top: tooltip.clientY - 6,
            transform: "translateY(-100%)",
            zIndex: 9999,
          }}
        >
          <div className="font-mono text-[10px] text-muted-foreground tabular-nums">
            #{tooltip.index} / {data.total.toLocaleString()}
            {tooltip.index === data.head_index && "  • HEAD"}
          </div>
          <div className="flex gap-2 tabular-nums mt-0.5">
            <span className="text-emerald-500 font-medium">+{(data.insertions[tooltip.index] ?? 0).toLocaleString()}</span>
            <span className="text-red-500 font-medium">−{(data.deletions[tooltip.index] ?? 0).toLocaleString()}</span>
          </div>
        </div>
      )}
    </div>
  );
};

export default memo(OverviewChart);

import type { CommitActivityItem } from "@gitru/commands";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  Area,
  Bar,
  ComposedChart,
  Line,
  ReferenceLine,
  ResponsiveContainer,
  XAxis,
  YAxis,
} from "recharts";

const ROW_H = 32;
const ROLLING_WINDOW = 10;
const LEAD_PX = 100;
const MAX_ZOOM = 40;

type PanoramaChartProps = {
  items: CommitActivityItem[];
  headIndex: number | undefined;
  totalCommits: number;
  scrollRef: React.RefObject<HTMLDivElement | null>;
};

type ChartPoint = {
  index: number;
  activity: number;
  insertions: number;
  deletions: number;
};

function computeChartData(items: CommitActivityItem[]): ChartPoint[] {
  const half = Math.floor(ROLLING_WINDOW / 2);
  return items.map((item, i) => {
    const start = Math.max(0, i - half);
    const end = Math.min(items.length - 1, i + half);
    let sum = 0;
    for (let j = start; j <= end; j++) {
      sum += items[j].insertions + items[j].deletions;
    }
    return {
      index: i,
      activity: sum / (end - start + 1),
      insertions: item.insertions,
      deletions: item.deletions,
    };
  });
}

const PanoramaChart = ({
  items,
  headIndex,
  totalCommits,
  scrollRef,
}: PanoramaChartProps) => {
  const containerRef = useRef<HTMLDivElement>(null);
  const viewportRef = useRef<HTMLDivElement>(null);
  const chartWidthRef = useRef(0);

  // zoom/pan refs: always current, read by all DOM-mutation paths
  const zoomRef = useRef(1);
  const panRef = useRef(0); // panStart commit index

  // React state: only drives chart re-render (sliced data + domain)
  const [zoom, setZoom] = useState(1);
  const [pan, setPan] = useState(0);

  // RAF batching — collapses rapid wheel events into one re-render per frame
  const rafRef = useRef(0);
  const pendingRef = useRef({ z: 1, p: 0 });

  const dragRef = useRef({
    active: false,
    kind: "body" as "body" | "left" | "right",
    startX: 0,
    rangeAtStart: { start: 0, end: 0 },
    range: { start: 0, end: 50 },
  });

  const chartData = useMemo(() => computeChartData(items), [items]);

  // ─── helpers (inline so they always close over current totalCommits) ─────────

  const winOf = (z: number) =>
    z <= 1 ? totalCommits : Math.max(1, Math.round(totalCommits / z));

  const clampPan = (p: number, ws: number) =>
    Math.max(0, Math.min(totalCommits - ws, p));

  // ─── viewport overlay: reads refs so it's always in sync ────────────────────

  const applyViewport = useCallback(
    (start: number, end: number) => {
      const cz = zoomRef.current;
      const ws = winOf(cz);
      const ps = clampPan(panRef.current, ws);
      const cw = Math.max(chartWidthRef.current, 1);
      const left = ((start - ps) / ws) * cw;
      const right = ((end - ps) / ws) * cw;
      const vp = viewportRef.current;
      if (!vp) return;
      vp.style.left = `${left}px`;
      vp.style.width = `${Math.max(right - left, 6)}px`;
    },
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [totalCommits], // winOf/clampPan are inline, only depend on totalCommits
  );

  // ─── zoom/pan commit: update refs + overlay immediately, state async ─────────

  const commitZoomPan = useCallback(() => {
    rafRef.current = 0;
    const { z, p } = pendingRef.current;
    const cz = Math.max(1, Math.min(MAX_ZOOM, z));
    const ws = winOf(cz);
    const ps = clampPan(p, ws);

    zoomRef.current = cz;
    panRef.current = ps;

    // update overlay immediately — before React re-renders the chart
    const cw = Math.max(chartWidthRef.current, 1);
    const { start, end } = dragRef.current.range;
    const vp = viewportRef.current;
    if (vp) {
      const left = ((start - ps) / ws) * cw;
      const right = ((end - ps) / ws) * cw;
      vp.style.left = `${left}px`;
      vp.style.width = `${Math.max(right - left, 6)}px`;
    }

    setZoom(cz);
    setPan(ps);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [totalCommits]);

  const scheduleZoomPan = useCallback(
    (z: number, p: number) => {
      const cz = Math.max(1, Math.min(MAX_ZOOM, z));
      const ws = winOf(cz);
      const ps = clampPan(p, ws);
      // update refs immediately so any in-flight applyViewport calls are correct
      zoomRef.current = cz;
      panRef.current = ps;
      pendingRef.current = { z: cz, p: ps };
      if (!rafRef.current) {
        rafRef.current = requestAnimationFrame(commitZoomPan);
      }
    },
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [totalCommits, commitZoomPan],
  );

  // ─── derived chart data (only changes on zoom/pan state commit) ──────────────

  const winSize = winOf(zoom);
  const panStart = clampPan(pan, winSize);

  const visibleData = useMemo(() => {
    if (zoom <= 1) return chartData;
    return chartData.slice(panStart, panStart + winSize);
  }, [chartData, zoom, panStart, winSize]);

  // ─── scroll → overlay + auto-pan (disabled during drag to avoid jumps) ───────

  useEffect(() => {
    const scrollEl = scrollRef.current;
    if (!scrollEl) return;

    const onScroll = () => {
      const start = Math.floor(scrollEl.scrollTop / ROW_H);
      const count = Math.floor(scrollEl.clientHeight / ROW_H);
      const end = start + count;
      dragRef.current.range = { start, end };
      applyViewport(start, end);

      if (dragRef.current.active) return; // no auto-pan while dragging
      const cz = zoomRef.current;
      if (cz <= 1) return;
      const cw = chartWidthRef.current;
      if (!cw) return;

      const ws = winOf(cz);
      const ps = clampPan(panRef.current, ws);
      const startPx = ((start - ps) / ws) * cw;
      const endPx = ((end - ps) / ws) * cw;

      if (startPx < LEAD_PX || endPx > cw - LEAD_PX) {
        const center = (start + end) / 2;
        const newPan = clampPan(Math.round(center - ws / 2), ws);
        if (newPan !== ps) scheduleZoomPan(cz, newPan);
      }
    };

    scrollEl.addEventListener("scroll", onScroll, { passive: true });
    onScroll();
    return () => scrollEl.removeEventListener("scroll", onScroll);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [scrollRef, applyViewport, scheduleZoomPan, totalCommits]);

  // ─── ResizeObserver ──────────────────────────────────────────────────────────

  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;
    chartWidthRef.current = el.getBoundingClientRect().width;
    const ro = new ResizeObserver((entries) => {
      chartWidthRef.current = entries[0]?.contentRect.width ?? 0;
      const { start, end } = dragRef.current.range;
      applyViewport(start, end);
    });
    ro.observe(el);
    return () => ro.disconnect();
  }, [applyViewport]);

  // ─── wheel zoom ──────────────────────────────────────────────────────────────

  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;
    const handle = (e: WheelEvent) => {
      e.preventDefault();
      const cz = zoomRef.current;
      const cp = panRef.current;
      const ws = winOf(cz);
      const factor = e.deltaY < 0 ? 1.3 : 1 / 1.3;
      const nz = Math.max(1, Math.min(MAX_ZOOM, cz * factor));
      if (Math.abs(nz - cz) < 0.05) return;
      const rect = el.getBoundingClientRect();
      const cursorPx = e.clientX - rect.left;
      const cw = Math.max(chartWidthRef.current, 1);
      const cursorCommit = cp + (cursorPx / cw) * ws;
      const nws = winOf(nz);
      const np = clampPan(
        Math.round(cursorCommit - (cursorPx / cw) * nws),
        nws,
      );
      scheduleZoomPan(nz, np);
    };
    el.addEventListener("wheel", handle, { passive: false });
    return () => el.removeEventListener("wheel", handle);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [totalCommits, scheduleZoomPan]);

  // ─── drag ────────────────────────────────────────────────────────────────────

  useEffect(() => {
    const onMouseMove = (e: MouseEvent) => {
      const drag = dragRef.current;
      if (!drag.active) return;
      const cz = zoomRef.current;
      const ws = winOf(cz);
      const cw = Math.max(chartWidthRef.current, 1);
      const dIdx = Math.round(((e.clientX - drag.startX) / cw) * ws);
      const scrollEl = scrollRef.current;

      let { start, end } = drag.rangeAtStart;
      const width = end - start;
      if (drag.kind === "body") {
        start = Math.max(0, drag.rangeAtStart.start + dIdx);
        end = Math.min(totalCommits, start + width);
        start = end - width;
      } else if (drag.kind === "left") {
        start = Math.max(0, Math.min(drag.rangeAtStart.start + dIdx, end - 1));
      } else {
        end = Math.max(
          start + 1,
          Math.min(totalCommits, drag.rangeAtStart.end + dIdx),
        );
      }

      drag.range = { start, end };
      applyViewport(start, end);
      if (scrollEl) scrollEl.scrollTop = start * ROW_H;
    };
    const onMouseUp = () => {
      dragRef.current.active = false;
    };
    window.addEventListener("mousemove", onMouseMove);
    window.addEventListener("mouseup", onMouseUp);
    return () => {
      window.removeEventListener("mousemove", onMouseMove);
      window.removeEventListener("mouseup", onMouseUp);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [totalCommits, scrollRef, applyViewport]);

  const startDrag = useCallback(
    (e: React.MouseEvent, kind: "body" | "left" | "right") => {
      e.preventDefault();
      e.stopPropagation();
      const drag = dragRef.current;
      drag.active = true;
      drag.kind = kind;
      drag.startX = e.clientX;
      drag.rangeAtStart = { ...drag.range };
    },
    [],
  );

  const onChartClick = useCallback(
    (e: React.MouseEvent<HTMLDivElement>) => {
      const container = containerRef.current;
      if (!container) return;
      const rect = container.getBoundingClientRect();
      const cz = zoomRef.current;
      const ws = winOf(cz);
      const ps = clampPan(panRef.current, ws);
      const cw = Math.max(chartWidthRef.current, 1);
      const clicked = Math.max(
        0,
        Math.min(
          totalCommits - 1,
          Math.round(ps + ((e.clientX - rect.left) / cw) * ws),
        ),
      );
      const { start, end } = dragRef.current.range;
      const half = Math.floor((end - start) / 2);
      const newStart = Math.max(
        0,
        Math.min(totalCommits - (end - start), clicked - half),
      );
      const newEnd = newStart + (end - start);
      dragRef.current.range = { start: newStart, end: newEnd };
      applyViewport(newStart, newEnd);
      const scrollEl = scrollRef.current;
      if (scrollEl) scrollEl.scrollTop = newStart * ROW_H;
    },
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [totalCommits, applyViewport, scrollRef],
  );

  return (
    <div
      ref={containerRef}
      className="relative h-full w-full cursor-pointer overflow-hidden"
      onClick={onChartClick}
    >
      <div className="absolute inset-0">
        <ResponsiveContainer width="100%" height="100%">
          <ComposedChart
            data={visibleData}
            margin={{ top: 4, right: 0, bottom: 0, left: 0 }}
          >
            <defs>
              <linearGradient id="panoramaGradient" x1="0" y1="0" x2="0" y2="1">
                <stop
                  offset="5%"
                  stopColor="var(--color-blue-500)"
                  stopOpacity={0.8}
                />
                <stop
                  offset="95%"
                  stopColor="var(--color-blue-500)"
                  stopOpacity={0}
                />
              </linearGradient>
            </defs>

            <XAxis
              type="number"
              dataKey="index"
              domain={[panStart, panStart + winSize]}
              hide
              height={0}
            />
            <YAxis yAxisId="bars" hide width={0} domain={[0, "auto"]} />
            {/* separate axis so activity line always fills full height */}
            <YAxis yAxisId="activity" hide width={0} domain={[0, "auto"]} />

            <Bar
              yAxisId="bars"
              dataKey="insertions"
              stackId="c"
              fill="var(--color-green-500)"
              fillOpacity={0.6}
              barSize={10}
              isAnimationActive={false}
            />
            <Bar
              yAxisId="bars"
              dataKey="deletions"
              stackId="c"
              fill="var(--color-red-500)"
              fillOpacity={0.6}
              barSize={10}
              isAnimationActive={false}
            />
            <Area
              yAxisId="activity"
              type="monotone"
              dataKey="activity"
              stroke="none"
              fill="url(#panoramaGradient)"
              isAnimationActive={false}
              activeDot={false}
            />
            <Line
              yAxisId="activity"
              type="monotone"
              dataKey="activity"
              stroke="var(--color-blue-500)"
              strokeWidth={1.5}
              dot={false}
              activeDot={false}
              isAnimationActive={false}
            />

            {headIndex !== undefined && (
              <ReferenceLine
                yAxisId="activity"
                x={headIndex}
                stroke="var(--color-green-500)"
                strokeWidth={1.5}
                strokeDasharray="3 2"
              />
            )}
          </ComposedChart>
        </ResponsiveContainer>
      </div>

      {/* zoom controls */}
      {/* <div
        className="absolute bottom-1 right-1 flex items-center gap-0.5"
        onClick={(e) => e.stopPropagation()}
      >
        {zoom > 1.05 && (
          <button
            className="cursor-pointer select-none rounded px-1 text-[10px] text-muted-foreground/50 hover:text-muted-foreground"
            onClick={() => scheduleZoomPan(1, 0)}
            title="Reset zoom"
          >
            {zoom.toFixed(1)}×
          </button>
        )}
        <button
          className="flex h-4 w-4 cursor-pointer items-center justify-center rounded text-[10px] text-muted-foreground/50 hover:text-muted-foreground"
          onClick={() =>
            scheduleZoomPan(zoom / 1.5, pan + Math.round(winSize * 0.25))
          }
          title="Zoom out"
        >
          −
        </button>
        <button
          className="flex h-4 w-4 cursor-pointer items-center justify-center rounded text-[10px] text-muted-foreground/50 hover:text-muted-foreground"
          onClick={() =>
            scheduleZoomPan(zoom * 1.5, pan + Math.round(winSize * 0.25))
          }
          title="Zoom in"
        >
          +
        </button>
      </div> */}

      {/* viewport overlay — DOM-mutated, never re-rendered */}
      <div
        ref={viewportRef}
        className="pointer-events-none opacity-90 absolute inset-y-0 overflow-hidden border-dashed border border-blue-500/70 rounded-xs bg-[repeating-linear-gradient(-45deg,rgba(59,130,246,0.12)_0px,rgba(59,130,246,0.12)_8px,transparent_8px,transparent_16px),linear-gradient(to_bottom,color-mix(in_oklab,var(--color-blue-500)_20%,transparent),color-mix(in_oklab,var(--color-blue-500)_2%,transparent))]"
        style={{
          left: 0,
          width: 0,
          zIndex: 20,
        }}
        onClick={(e) => e.stopPropagation()}
      >
        {/* <div
          className="pointer-events-auto absolute inset-y-0 left-0 w-2 cursor-ew-resize"
          onMouseDown={(e) => startDrag(e, "left")}
        /> */}
        <div
          className="pointer-events-auto absolute inset-0 cursor-grab active:cursor-grabbing"
          style={{ marginLeft: 8, marginRight: 8 }}
          onMouseDown={(e) => startDrag(e, "body")}
        />
        {/* <div
          className="pointer-events-auto absolute inset-y-0 right-0 w-2 cursor-ew-resize"
          onMouseDown={(e) => startDrag(e, "right")}
        /> */}
      </div>
    </div>
  );
};

export default PanoramaChart;

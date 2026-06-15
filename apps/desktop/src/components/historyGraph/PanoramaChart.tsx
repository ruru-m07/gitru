import type { CommitActivityItem } from "@gitru/commands";
import { useCallback, useEffect, useMemo, useRef } from "react";
import {
  Area,
  Bar,
  ComposedChart,
  Line,
  ReferenceLine,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";

const ROW_H = 32;
const ROLLING_WINDOW = 10;

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

type TooltipPayload = {
  active?: boolean;
  payload?: Array<{ payload: ChartPoint }>;
};

const PanoramaTooltip = ({ active, payload }: TooltipPayload) => {
  if (!active || !payload?.length) return null;
  const d = payload[0].payload;
  return (
    <div className="rounded border bg-background/90 px-2 py-1.5 text-xs shadow-md">
      <p className="text-muted-foreground mb-1">Commit #{d.index + 1}</p>
      <div className="flex gap-3">
        <span className="text-green-500">+{d.insertions.toLocaleString()}</span>
        <span className="text-red-400">−{d.deletions.toLocaleString()}</span>
      </div>
    </div>
  );
};

const PanoramaChart = ({
  items,
  headIndex,
  totalCommits,
  scrollRef,
}: PanoramaChartProps) => {
  // chartAreaRef wraps ONLY the recharts SVG area so pixel ↔ index math is accurate
  const chartAreaRef = useRef<HTMLDivElement>(null);
  const viewportRef = useRef<HTMLDivElement>(null);
  const chartWidthRef = useRef(0);

  const dragRef = useRef({
    active: false,
    kind: "body" as "body" | "left" | "right",
    startX: 0,
    rangeAtStart: { start: 0, end: 0 },
    range: { start: 0, end: 50 },
  });

  const chartData = useMemo(() => computeChartData(items), [items]);

  const toPixel = useCallback(
    (idx: number) => (idx / Math.max(totalCommits, 1)) * chartWidthRef.current,
    [totalCommits],
  );

  const toIndex = useCallback(
    (px: number) =>
      Math.max(
        0,
        Math.min(
          totalCommits - 1,
          Math.round((px / Math.max(chartWidthRef.current, 1)) * totalCommits),
        ),
      ),
    [totalCommits],
  );

  const applyViewport = useCallback(
    (start: number, end: number) => {
      const vp = viewportRef.current;
      if (!vp) return;
      vp.style.left = `${toPixel(start)}px`;
      vp.style.width = `${Math.max(toPixel(end - start), 6)}px`;
    },
    [toPixel],
  );

  // Main scroll → viewport (pure DOM, no React state)
  useEffect(() => {
    const scrollEl = scrollRef.current;
    if (!scrollEl) return;

    const onScroll = () => {
      const start = Math.floor(scrollEl.scrollTop / ROW_H);
      const count = Math.floor(scrollEl.clientHeight / ROW_H);
      dragRef.current.range = { start, end: start + count };
      applyViewport(start, start + count);
    };

    scrollEl.addEventListener("scroll", onScroll, { passive: true });
    onScroll();
    return () => scrollEl.removeEventListener("scroll", onScroll);
  }, [scrollRef, applyViewport]);

  // Track chart area width (not full container — just the SVG area)
  useEffect(() => {
    const el = chartAreaRef.current;
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

  // Global drag tracking
  useEffect(() => {
    const onMouseMove = (e: MouseEvent) => {
      const drag = dragRef.current;
      if (!drag.active) return;

      const dx = e.clientX - drag.startX;
      const dIdx = Math.round(
        (dx / Math.max(chartWidthRef.current, 1)) * totalCommits,
      );
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
      const el = chartAreaRef.current;
      if (!el) return;
      const rect = el.getBoundingClientRect();
      const clickedIndex = toIndex(e.clientX - rect.left);
      const { start, end } = dragRef.current.range;
      const half = Math.floor((end - start) / 2);
      const newStart = Math.max(
        0,
        Math.min(totalCommits - (end - start), clickedIndex - half),
      );
      const newEnd = newStart + (end - start);
      dragRef.current.range = { start: newStart, end: newEnd };
      applyViewport(newStart, newEnd);
      const scrollEl = scrollRef.current;
      if (scrollEl) scrollEl.scrollTop = newStart * ROW_H;
    },
    [toIndex, totalCommits, applyViewport, scrollRef],
  );

  return (
    // Outer wrapper: full container height, clips viewport edges
    <div className="relative h-full w-full overflow-hidden">
      {/* Recharts SVG — zero axis margins so pixel coords align with data coords */}
      <div
        ref={chartAreaRef}
        className="absolute inset-0 cursor-pointer"
        onClick={onChartClick}
      >
        <ResponsiveContainer width="100%" height="100%">
          <ComposedChart
            data={chartData}
            margin={{ top: 4, right: 0, bottom: 0, left: 0 }}
          >
            <defs>
              <linearGradient id="panoramaGradient" x1="0" y1="0" x2="0" y2="1">
                <stop
                  offset="5%"
                  stopColor="var(--color-blue-500)"
                  stopOpacity={0.2}
                />
                <stop
                  offset="95%"
                  stopColor="var(--color-blue-500)"
                  stopOpacity={0}
                />
              </linearGradient>
            </defs>

            {/* width=0 / height=0 so axes take zero layout space → chart area = full div */}
            <XAxis
              type="number"
              dataKey="index"
              domain={[0, Math.max(chartData.length - 1, 1)]}
              hide
              height={0}
            />
            <YAxis hide width={0} domain={[0, "auto"]} />

            <Tooltip
              content={<PanoramaTooltip />}
              cursor={{ stroke: "rgba(255,255,255,0.15)", strokeWidth: 1 }}
              isAnimationActive={false}
            />

            {/* Thin stacked bars in background */}
            <Bar
              dataKey="insertions"
              stackId="c"
              fill="var(--color-green-500)"
              fillOpacity={0.5}
              barSize={2}
              isAnimationActive={false}
            />
            <Bar
              dataKey="deletions"
              stackId="c"
              fill="var(--color-red-400)"
              fillOpacity={0.5}
              barSize={2}
              isAnimationActive={false}
            />

            {/* Rolling average area + line */}
            <Area
              type="monotone"
              dataKey="activity"
              stroke="none"
              fill="url(#panoramaGradient)"
              isAnimationActive={false}
              activeDot={false}
            />
            <Line
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
                x={headIndex}
                stroke="var(--color-green-500)"
                strokeWidth={1.5}
                strokeDasharray="3 2"
              />
            )}
          </ComposedChart>
        </ResponsiveContainer>
      </div>

      {/* Viewport overlay — absolutely positioned on top of chart area, DOM-mutated */}
      <div
        ref={viewportRef}
        className="pointer-events-none absolute inset-y-0"
        style={{
          left: 0,
          width: 0,
          zIndex: 20,
          backgroundColor: "rgba(0,0,0,0.12)",
          borderLeft: "2px solid rgba(0,0,0,0.85)",
          borderRight: "2px solid rgba(0,0,0,0.85)",
          borderTop: "1px solid rgba(0,0,0,0.3)",
          borderBottom: "1px solid rgba(0,0,0,0.3)",
        }}
        onClick={(e) => e.stopPropagation()}
      >
        <div
          className="pointer-events-auto absolute inset-y-0 left-0 w-2 cursor-ew-resize"
          onMouseDown={(e) => startDrag(e, "left")}
        />
        <div
          className="pointer-events-auto absolute inset-0 cursor-grab active:cursor-grabbing"
          style={{ marginLeft: 8, marginRight: 8 }}
          onMouseDown={(e) => startDrag(e, "body")}
        />
        <div
          className="pointer-events-auto absolute inset-y-0 right-0 w-2 cursor-ew-resize"
          onMouseDown={(e) => startDrag(e, "right")}
        />
      </div>
    </div>
  );
};

export default PanoramaChart;

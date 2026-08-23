import { cn } from "@gitru/ui/lib/utils";
import { useLayoutEffect, useRef } from "react";
import { create } from "zustand";
import { persist } from "zustand/middleware";

const clamp = (val: number, min: number, max: number) =>
  Math.min(max, Math.max(min, val));

export const ResizableLayout = ({
  id,
  children,
  minWidth = 200,
  maxWidth = 600,
  leftPanelClassName,
  rightPanelClassName,
}: {
  id: string;
  children: [React.ReactNode, React.ReactNode];
  minWidth?: number;
  maxWidth?: number;
  leftPanelClassName?: string;
  rightPanelClassName?: string;
}) => {
  const width = useLayoutStore((s) => s.layouts[id] ?? 300);
  const setWidth = useLayoutStore((s) => s.setWidth);

  const containerRef = useRef<HTMLDivElement>(null);
  const leftRef = useRef<HTMLDivElement>(null);
  const rightRef = useRef<HTMLDivElement>(null);

  const draggingRef = useRef(false);
  const startXRef = useRef(0);
  const startWidthRef = useRef(0);

  const onPointerDown = (e: React.PointerEvent<HTMLDivElement>) => {
    draggingRef.current = true;
    startXRef.current = e.clientX;
    startWidthRef.current = width;

    const onMove = (e: PointerEvent) => {
      if (!draggingRef.current) return;

      const delta = e.clientX - startXRef.current;
      const next = clamp(startWidthRef.current + delta, minWidth, maxWidth);

      setWidth(id, next);
    };

    const onUp = () => {
      draggingRef.current = false;
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
      document.documentElement.classList.remove("dragging");
    };

    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
    document.documentElement.classList.add("dragging");
  };

  useLayoutEffect(() => {
    const el = containerRef.current;
    const left = leftRef.current;
    const right = rightRef.current;

    if (!el || !left || !right) return;

    const update = () => {
      const containerRect = el.getBoundingClientRect();

      const leftWidth = left.offsetWidth;
      const rightWidth = containerRect.width - leftWidth;

      el.style.setProperty("--layout-width", `${containerRect.width}px`);
      el.style.setProperty("--layout-height", `${containerRect.height}px`);

      el.style.setProperty("--left-width", `${leftWidth}px`);
      el.style.setProperty("--right-width", `${rightWidth}px`);
    };

    update();

    const ro = new ResizeObserver(update);
    ro.observe(el);
    ro.observe(left);

    return () => ro.disconnect();
  }, []);

  return (
    <div
      ref={containerRef}
      data-layout-id={id}
      data-dragging={draggingRef.current ? "true" : "false"}
      className="grid h-full w-full"
      style={{
        gridTemplateColumns: `${width}px 1fr`,
      }}
    >
      {/* Left */}
      <div
        ref={leftRef}
        className={cn("relative border-r", leftPanelClassName)}
      >
        {children[0]}

        <ResizeHandle
          onPointerDown={onPointerDown}
          onResize={(delta) =>
            setWidth(id, clamp(width + delta, minWidth, maxWidth))
          }
        />
      </div>

      {/* Right */}
      <div className={cn(rightPanelClassName)} ref={rightRef}>
        {children[1]}
      </div>
    </div>
  );
};

const ResizeHandle = ({
  onPointerDown,
  onResize,
}: {
  onPointerDown: (e: React.PointerEvent<HTMLDivElement>) => void;
  onResize: (delta: number) => void;
}) => {
  return (
    <div
      role="separator"
      aria-orientation="vertical"
      tabIndex={0}
      className="absolute top-0 right-0 h-full w-1 cursor-col-resize data-[dragging=true]:bg-black/20 _hover:bg-black/20"
      onPointerDown={onPointerDown}
      onKeyDown={(e) => {
        if (e.key === "ArrowLeft") onResize(-10);
        if (e.key === "ArrowRight") onResize(10);
      }}
    />
  );
};

type LayoutState = {
  layouts: Record<string, number>;
  setWidth: (id: string, width: number) => void;
};

export const useLayoutStore = create<LayoutState>()(
  persist(
    (set) => ({
      layouts: {},
      setWidth: (id, width) =>
        set((state) => ({
          layouts: {
            ...state.layouts,
            [id]: width,
          },
        })),
    }),
    {
      name: "layout-storage",
    },
  ),
);

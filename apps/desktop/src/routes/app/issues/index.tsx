import { createFileRoute, redirect } from "@tanstack/react-router";
import { useRef } from "react";
import PageLayout from "@/components/page-layout";

export const Route = createFileRoute("/app/issues/")({
  beforeLoad: () => {
    if (!import.meta.env.DEV) {
      throw redirect({ to: "/app/git" });
    }
  },
  component: RouteComponent,
});

const ROWS = 100;

const dataSet = Array.from({ length: ROWS });

/** DEV ONLY: CSS subgrid / scroll interaction prototype. */
function RouteComponent() {
  const scrollRef = useRef<HTMLDivElement>(null);

  return (
    <PageLayout className="p-4">
      <div
        style={{
          display: "grid",
          gridTemplateColumns: "minmax(min-content, max-content) 400px 1fr",
          overflowX: "hidden",
          width: "100%",
        }}
        ref={scrollRef}
      >
        <div
          style={{
            gridRow: `span ${ROWS}`,
            gridColumn: 1,
            display: "grid",
            gridTemplateRows: "subgrid",
            position: "sticky",
            left: 0,
            zIndex: 3,
            backgroundColor: "lightpink",
          }}
        >
          {dataSet.map((_, i) => (
            <div
              className="min-h-8 px-4 flex items-center justify-end tabular-nums"
              key={i}
            >
              {i}
            </div>
          ))}
        </div>

        <div
          style={{
            gridRow: `span ${ROWS}`,
            gridColumn: 2,
            display: "grid",
            gridTemplateRows: "subgrid",
            overflowX: "auto",
            minWidth: 0,
            WebkitOverflowScrolling: "touch",
          }}
          className="overflow-x-auto overflow-y-hidden"
          onWheel={(e: React.WheelEvent<HTMLDivElement>) => {
            const parent = scrollRef.current;
            if (!parent) return;

            const isVerticalIntent = Math.abs(e.deltaY) > Math.abs(e.deltaX);

            if (isVerticalIntent) {
              parent.scrollTop += e.deltaY;
              e.preventDefault();
              e.stopPropagation();
            }
          }}
        >
          {dataSet.map((_, i) => (
            <div
              className="relative min-h-8 flex items-center font-mono pl-2 text-nowrap"
              key={i}
            >
              {i % 7 === 0 ? (
                <div className="flex items-center gap-3">
                  <p>
                    Lorem ipsum, dolor sit amet consectetur adipisicing elit.
                    Vel, laboriosam eius odit architecto asperiores veniam
                    pariatur nam praesentium hic quibusdam. Iure recusandae,
                    eius quis temporibus et aliquid enim quas tempore.
                  </p>
                  <div className="size-6 bg-red-400 sticky left-0 right-0" />
                  <p>
                    Lorem ipsum, dolor sit amet consectetur adipisicing elit.
                    Vel, laboriosam eius odit architecto asperiores veniam
                    pariatur nam praesentium hic quibusdam. Iure recusandae,
                    eius quis temporibus et aliquid enim quas tempore.
                  </p>
                </div>
              ) : (
                i
              )}
            </div>
          ))}
        </div>

        <div
          style={{
            gridRow: `span ${ROWS}`,
            gridColumn: 3,
            display: "grid",
            gridTemplateRows: "subgrid",
            backgroundColor: "lightgreen",
          }}
        >
          {dataSet.map((_, i) => (
            <div className="min-h-8 px-4 flex items-center" key={i}>
              {i}
            </div>
          ))}
        </div>
      </div>
    </PageLayout>
  );
}

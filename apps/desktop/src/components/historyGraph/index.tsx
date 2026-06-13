import { useCallback, useMemo, useRef } from "react";
import LoaderIndicator from "@/components/loaderIndicator";
import { useGitHistoryGraph, useGitHistoryOverview } from "@/hooks";
import GraphBody from "./body";
import OverviewChart from "./OverviewChart";

const HistoryGraph = () => {
  const query = useMemo(
    () => ({
      limit: 200,
      search: undefined,
      include_local: true,
      include_remotes: true,
      include_tags: true,
      include_stash: true,
    }),
    [],
  );

  const { data, fetchNextPage, hasNextPage, isFetchingNextPage, isLoading } =
    useGitHistoryGraph(query);

  const rows = data?.pages.flatMap((page) => page.rows) ?? [];

  // Dedicated lightweight overview (full series under same filters, no graph/lens data).
  const { data: overview, isLoading: isOverviewLoading } =
    useGitHistoryOverview(query);

  // Ref to the list scroller (for chart drag to scroll the list via DOM).
  const listScrollerRef = useRef<HTMLDivElement | null>(null);

  // Ref to the brush highlight band div (for list scroll to update the highlight via cheap DOM style mutation, avoiding React re-renders).
  const highlightBandRef = useRef<HTMLDivElement | null>(null);

  // Called by the chart when the user drags the range highlight band or uses other controls in the chart.
  // We scroll the list using direct DOM mutation (no state update in this path to keep things light).
  const handleRangeRequest = useCallback(
    (range: { start: number; end: number }) => {
      const scroller = listScrollerRef.current;
      if (scroller) {
        const rowH = 32;
        const target = Math.max(0, range.start * rowH);
        const maxScroll = Math.max(
          0,
          scroller.scrollHeight - scroller.clientHeight,
        );
        scroller.scrollTop = Math.min(target, maxScroll);
      }

      // Trigger loading more if needed (this is fine, it only happens on user action in the chart, not on every scroll tick).
      if (range.end >= rows.length - 5 && hasNextPage && !isFetchingNextPage) {
        fetchNextPage();
      }
    },
    [rows.length, hasNextPage, isFetchingNextPage],
  );

  return (
    <div className="flex h-full max-h-[calc(var(--layout-height)-(--spacing(14)))] overflow-y-auto flex-col bg-secondary/50">
      <div className="h-19.5 border-b">
        <OverviewChart
          data={overview}
          isLoading={isOverviewLoading}
          onRangeRequest={handleRangeRequest}
          scrollerRef={listScrollerRef}
          highlightBandRef={highlightBandRef}
          total={overview?.total}
        />
      </div>
      {isLoading ? (
        <div className="p-3">
          <LoaderIndicator />
        </div>
      ) : (
        <GraphBody
          rows={rows}
          fetchNextPage={fetchNextPage}
          hasNextPage={hasNextPage}
          isFetchingNextPage={isFetchingNextPage}
          scrollerRef={listScrollerRef}
          highlightBandRef={highlightBandRef}
          total={overview?.total ?? 0}
        />
      )}
    </div>
  );
};

export default HistoryGraph;

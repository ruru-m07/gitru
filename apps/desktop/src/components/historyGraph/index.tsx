import { useMemo, useRef } from "react";
import LoaderIndicator from "@/components/loaderIndicator";
import { useCommitActivity, useGitHistoryGraph } from "@/hooks";
import GraphBody from "./body";
import PanoramaChart from "./PanoramaChart";

type FilterKey = "local" | "remotes" | "tags" | "stashes";
type Filters = Record<FilterKey, boolean>;

const defaultFilters: Filters = {
  local: true,
  remotes: true,
  tags: true,
  stashes: true,
};

const HistoryGraph = () => {
  const filters = defaultFilters;
  const search = "";
  const scrollRef = useRef<HTMLDivElement>(null);
  const query = useMemo(
    () => ({
      limit: 200,
      search: search.length > 0 ? search : undefined,
      include_local: filters.local,
      include_remotes: filters.remotes,
      include_tags: filters.tags,
      include_stash: filters.stashes,
    }),
    [filters.local, filters.remotes, filters.tags, filters.stashes, search],
  );

  const activityQuery = useMemo(
    () => ({
      include_local: filters.local,
      include_remotes: filters.remotes,
      include_tags: filters.tags,
      include_stash: filters.stashes,
    }),
    [filters.local, filters.remotes, filters.tags, filters.stashes],
  );

  const { data, fetchNextPage, hasNextPage, isFetchingNextPage, isLoading } =
    useGitHistoryGraph(query);

  const { data: activityData } = useCommitActivity(activityQuery);

  const rows = data?.pages.flatMap((page) => page.rows) ?? [];
  const activityItems = activityData?.items ?? [];

  return (
    <div className="flex h-full max-h-[calc(var(--layout-height)-(--spacing(14)))] overflow-y-auto flex-col">
      <div className="h-19.25 border-b shrink-0">
        <PanoramaChart
          items={activityItems}
          headIndex={activityData?.head_index}
          totalCommits={activityData?.total ?? activityItems.length}
          scrollRef={scrollRef}
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
          scrollRef={scrollRef}
        />
      )}
    </div>
  );
};

export default HistoryGraph;

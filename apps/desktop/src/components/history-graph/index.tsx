import { useEffect, useMemo, useRef } from "react";
import LoaderIndicator from "@/components/loader-indicator";
import {
  useCommitActivity,
  useGetCurrentBranch,
  useGetStatusAheadBehind,
  useGitHistoryGraph,
} from "@/hooks";
import {
  createHistoryGraphController,
  type HistoryGraphController,
  type HistoryGraphControllerData,
} from "./controller";
import PanoramaChart from "./panorama-chart";

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
  const listRef = useRef<HTMLDivElement>(null);
  const controllerRef = useRef<HistoryGraphController | null>(null);
  const initialDataRef = useRef<HistoryGraphControllerData | null>(null);
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
  const fetchNextPageRef = useRef(fetchNextPage);

  const { data: activityData } = useCommitActivity(activityQuery);
  const { data: currentBranch } = useGetCurrentBranch();
  const { data: statusAheadBehind } = useGetStatusAheadBehind();

  const rows = useMemo(
    () => data?.pages.flatMap((page) => page.rows) ?? [],
    [data],
  );
  const activityItems = useMemo(
    () => activityData?.items ?? [],
    [activityData?.items],
  );
  const controllerData: HistoryGraphControllerData = useMemo(
    () => ({
      rows,
      currentBranch: currentBranch ?? null,
      pushEnabled: (statusAheadBehind?.ahead ?? 0) > 0,
      hasNextPage,
      isFetchingNextPage,
    }),
    [
      rows,
      currentBranch,
      statusAheadBehind?.ahead,
      hasNextPage,
      isFetchingNextPage,
    ],
  );
  initialDataRef.current = controllerData;
  fetchNextPageRef.current = fetchNextPage;

  useEffect(() => {
    if (isLoading || !listRef.current) return;
    const initialData = initialDataRef.current;
    if (!initialData) return;
    const controller = createHistoryGraphController({
      listRoot: listRef.current,
      initialData,
      fetchNextPage: () => void fetchNextPageRef.current(),
    });
    controllerRef.current = controller;
    return () => {
      controller.dispose();
      controllerRef.current = null;
    };
  }, [isLoading]);

  useEffect(() => {
    controllerRef.current?.update(controllerData);
  }, [controllerData]);

  return (
    <div className="flex h-full max-h-[calc(var(--layout-height)-(--spacing(14)))] overflow-y-auto flex-col">
      <div className="h-19.25 border-b shrink-0">
        <PanoramaChart
          key={isLoading ? "loading" : "ready"}
          items={activityItems}
          headIndex={activityData?.head_index}
          totalCommits={activityData?.total ?? activityItems.length}
          scrollRef={listRef}
        />
      </div>
      {isLoading ? (
        <div className="p-3">
          <LoaderIndicator />
        </div>
      ) : (
        <div className="flex min-h-0 flex-1 flex-col">
          <div ref={listRef} className="h-full flex-1 overflow-y-auto" />
        </div>
      )}
    </div>
  );
};

export default HistoryGraph;

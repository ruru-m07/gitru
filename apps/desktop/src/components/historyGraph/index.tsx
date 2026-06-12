import { Input } from "@gitru/ui/components/input";
import {
  ToggleGroup,
  ToggleGroupItem,
} from "@gitru/ui/components/toggle-group";
import { useCallback, useMemo, useRef, useState } from "react";
import LoaderIndicator from "@/components/loaderIndicator";
import { useGitHistoryGraph, useGitHistoryOverview } from "@/hooks";
import GraphBody from "./body";
import OverviewChart from "./OverviewChart";

type FilterKey = "local" | "remotes" | "tags" | "stashes";
type Filters = Record<FilterKey, boolean>;

const defaultFilters: Filters = {
  local: true,
  remotes: true,
  tags: true,
  stashes: true,
};

const HistoryGraph = () => {
  const [filters, setFilters] = useState<Filters>(defaultFilters);
  const [searchInput, setSearchInput] = useState("");

  const search = searchInput.trim();
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

  const { data, fetchNextPage, hasNextPage, isFetchingNextPage, isLoading } =
    useGitHistoryGraph(query);

  const rows = data?.pages.flatMap((page) => page.rows) ?? [];

  // Dedicated lightweight overview (full series under same filters, no graph/lens data).
  const {
    data: overview,
    isLoading: isOverviewLoading,
  } = useGitHistoryOverview(query);

  // Live visible range reported by the list scroller (indices into `rows`, which start at global 0 for the filter).
  const [visibleRange, setVisibleRange] = useState<{ start: number; end: number } | undefined>(undefined);

  // Ref owned by parent so chart can drive scrolling the list.
  const listScrollerRef = useRef<HTMLDivElement | null>(null);

  // Handler for when the chart wants to change the visible range (drag on the highlight band).
  // We optimistically update the chart highlight and scroll the list.
  // If the requested range is beyond currently loaded rows we trigger more pages.
  const handleRangeRequest = useCallback((range: { start: number; end: number }) => {
    setVisibleRange(range);

    const scroller = listScrollerRef.current;
    if (scroller) {
      const rowH = 32;
      const target = Math.max(0, range.start * rowH);
      // Clamp to current content height; more content may arrive after fetchNextPage.
      const maxScroll = Math.max(0, scroller.scrollHeight - scroller.clientHeight);
      scroller.scrollTop = Math.min(target, maxScroll);
    }

    // Trigger loading more history if the user dragged beyond what we have loaded.
    if (range.end >= rows.length - 5 && hasNextPage && !isFetchingNextPage) {
      fetchNextPage();
    }
  }, [rows.length, hasNextPage, isFetchingNextPage]);

  return (
    <div className="flex h-full max-h-[calc(var(--layout-height)-(--spacing(14)))] overflow-y-auto flex-col">
      {/* <GraphHeader
          filters={filters}
          searchInput={searchInput}
          onSearchChange={setSearchInput}
          onFiltersChange={setFilters}
          /> */}
      {/* Lightweight overview: single activity line + bidirectional thin bars + HEAD indicator.
          Uses a dedicated optimized IPC (no swimlanes/refs/files). Range highlight + interactions
          wired in subsequent phases. */}
      <div className="h-[78px] border-b">
        <OverviewChart
          data={overview}
          isLoading={isOverviewLoading}
          visibleRange={visibleRange}
          onRangeRequest={handleRangeRequest}
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
          onVisibleRangeChange={setVisibleRange}
          scrollerRef={listScrollerRef}
        />
      )}
    </div>
  );
};

export default HistoryGraph;

type GraphHeaderProps = {
  filters: Filters;
  searchInput: string;
  onSearchChange: (value: string) => void;
  onFiltersChange: (next: Filters) => void;
};

const GraphHeader = ({
  filters,
  searchInput,
  onSearchChange,
  onFiltersChange,
}: GraphHeaderProps) => {
  const activeFilters = useMemo(
    () => (Object.keys(filters) as FilterKey[]).filter((k) => filters[k]),
    [filters],
  );

  return (
    <div className="border-b bg-background px-3 py-2">
      <div className="flex flex-wrap items-center gap-2">
        <div className="min-w-55 flex-1">
          <Input
            placeholder="Search commits, refs, author..."
            value={searchInput}
            onChange={(e) => onSearchChange(e.currentTarget.value)}
            type="search"
          />
        </div>
        <ToggleGroup
          multiple
          value={activeFilters}
          onValueChange={(values) =>
            onFiltersChange({
              local: values.includes("local"),
              remotes: values.includes("remotes"),
              tags: values.includes("tags"),
              stashes: values.includes("stashes"),
            })
          }
          className="flex-wrap"
        >
          <ToggleGroupItem value="local">Local</ToggleGroupItem>
          <ToggleGroupItem value="remotes">Remote</ToggleGroupItem>
          <ToggleGroupItem value="tags">Tags</ToggleGroupItem>
          <ToggleGroupItem value="stashes">Stash</ToggleGroupItem>
        </ToggleGroup>
      </div>
    </div>
  );
};

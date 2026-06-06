import {
  ChartConfig,
  ChartContainer,
  ChartTooltip,
  ChartTooltipContent,
} from "@gitru/ui/components/chart";
import { Input } from "@gitru/ui/components/input";
import {
  ToggleGroup,
  ToggleGroupItem,
} from "@gitru/ui/components/toggle-group";
import { useMemo, useState } from "react";
import { CartesianGrid, Line, LineChart, ResponsiveContainer } from "recharts";
import LoaderIndicator from "@/components/loaderIndicator";
import { useGitHistoryGraph } from "@/hooks";
import GraphBody from "./body";

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

  const chartData = rows.slice(0, 28).map((r) => ({
    oid: r.oid,
    insertions: r.commit.stats.insertions,
    deletions: -r.commit.stats.deletions,
  }));

  const chartConfig = {
    insertions: {
      label: "Insertions",
      color: "var(--color-green-600)",
    },
    deletions: {
      label: "Deletions",
      color: "var(--color-red-600)",
    },
  } satisfies ChartConfig;

  return (
    <div className="flex h-full max-h-[calc(var(--layout-height)-(--spacing(14)))] overflow-y-auto flex-col">
      {/* <GraphHeader
          filters={filters}
          searchInput={searchInput}
          onSearchChange={setSearchInput}
          onFiltersChange={setFilters}
          /> */}
      <div className="h-19.25 border-b">
        <ChartContainer config={chartConfig}>
          <ResponsiveContainer width="100%" height="100%">
            <LineChart data={chartData}>
              <CartesianGrid horizontal={false} />
              <ChartTooltip
                cursor={false}
                content={<ChartTooltipContent hideLabel />}
              />
              <Line
                dataKey="insertions"
                type="monotone"
                stroke="var(--color-insertions)"
                strokeWidth={2}
                dot={false}
                animateNewValues={false}
              />
              <Line
                dataKey="deletions"
                type="monotone"
                stroke="var(--color-deletions)"
                strokeWidth={2}
                dot={false}
                animateNewValues={false}
              />
            </LineChart>
          </ResponsiveContainer>
        </ChartContainer>
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

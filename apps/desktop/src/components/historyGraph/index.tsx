import type { GraphRef, GraphRow } from "@gitru/commands";
import { Badge } from "@gitru/ui/components/badge";
import { Input } from "@gitru/ui/components/input";
import {
  ToggleGroup,
  ToggleGroupItem,
} from "@gitru/ui/components/toggle-group";
import { useMemo, useRef, useState } from "react";
import { useOnInView } from "react-intersection-observer";
import LoaderIndicator from "@/components/loaderIndicator";
import { useGitHistoryGraph } from "@/hooks";
import GraphLane from "./lane";

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

  return (
    <div className="flex h-[calc(calc(var(--main-window-height)-calc(var(--spacing)*21))-var(--main-actual-content-padding))] overflow-y-auto flex-col">
      <GraphHeader
        filters={filters}
        searchInput={searchInput}
        onSearchChange={setSearchInput}
        onFiltersChange={setFilters}
      />
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

type GraphBodyProps = {
  rows: GraphRow[];
  fetchNextPage: () => void;
  hasNextPage: boolean;
  isFetchingNextPage: boolean;
};

const GraphBody = ({
  rows,
  fetchNextPage,
  hasNextPage,
  isFetchingNextPage,
}: GraphBodyProps) => {
  const scrollRef = useRef<HTMLDivElement>(null);

  const bottomRef = useOnInView(
    (inView) => {
      if (inView && hasNextPage && !isFetchingNextPage) {
        fetchNextPage();
      }
    },
    {
      root: scrollRef.current,
      threshold: 0,
      rootMargin: "500px",
    },
  );

  const layout = useMemo(() => computeGraphLayout(rows), [rows]);

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div ref={scrollRef} className="h-full flex-1 overflow-y-auto">
        {rows.map((row) => (
          <GraphRowItem key={row.oid} row={row} maxLane={layout.maxLane} />
        ))}

        {/* Sentinel element */}
        <div className="w-full flex justify-center">
          {isFetchingNextPage && "Loading more..."}
        </div>
        <div ref={bottomRef} className="h-4" />
      </div>
    </div>
  );
};

// ─── Row ─────────────────────────────────────────────────────────────────────

type GraphRowItemProps = {
  row: GraphRow;
  maxLane: number;
};

const GraphRowItem = ({ row, maxLane }: GraphRowItemProps) => {
  const refs = useMemo(() => buildRefList(row), [row]);

  return (
    <div className="grid grid-cols-[auto_1fr_auto] items-center gap-3 hover:bg-muted/30">
      <GraphLane row={row} maxLane={maxLane} />
      <div className="flex min-w-0 flex-col justify-center py-1">
        <div className="flex flex-wrap items-center gap-2">
          {/* <span className="truncate text-sm font-medium text-green-600 tabular-nums font-mono">
            {`+${row.commit.stats.insertions || 0}`}
          </span>
          <span className="truncate text-sm font-medium text-red-600 tabular-nums font-mono">
            {`-${row.commit.stats.deletions || 0}`}
          </span> */}
          <span className="truncate text-sm font-medium">
            {row.commit.summary || "(no subject)"}
          </span>
          {refs.length > 0 && (
            <div className="mt-0.5 flex flex-wrap items-center gap-1">
              {refs.map((r) => (
                <Badge
                  key={`${r.kind}-${r.name}`}
                  variant={badgeVariantForRef(r)}
                  size="sm"
                >
                  {r.name}
                </Badge>
              ))}
            </div>
          )}
        </div>
        {/* <div className="mt-0.5 flex flex-wrap items-center gap-2 text-xs text-muted-foreground">
          <span>{row.commit.authors.author.name}</span>
          <span>•</span>
          <span>
            +{row.commit.stats.insertions} -{row.commit.stats.deletions}
          </span>
          <span>•</span>
          <span>{row.parents.length} parents</span>
        </div> */}
      </div>
    </div>
  );
};

// ─── Graph layout computation ────────────────────────────────────────────────

type GraphLayout = {
  maxLane: number;
};

function computeGraphLayout(rows: GraphRow[]): GraphLayout {
  let maxLane = 0;

  for (const row of rows) {
    const inputMax = row.input_swimlanes.length - 1;
    const outputMax = row.output_swimlanes.length - 1;
    if (inputMax > maxLane) maxLane = inputMax;
    if (outputMax > maxLane) maxLane = outputMax;
  }

  return { maxLane };
}

function buildRefList(row: GraphRow): GraphRef[] {
  const prioritized = [
    ...row.heads,
    ...row.tags,
    ...row.remotes,
    ...row.stashes,
  ];

  const seen = new Set<string>();
  const refs: GraphRef[] = [];

  for (const ref of prioritized) {
    const key = `${ref.kind}-${ref.name}`;
    if (!seen.has(key)) {
      seen.add(key);
      refs.push(ref);
    }
  }
  for (const ref of row.refs) {
    const key = `${ref.kind}-${ref.name}`;
    if (!seen.has(key)) {
      seen.add(key);
      refs.push(ref);
    }
  }

  return refs.slice(0, 6);
}

function badgeVariantForRef(ref: GraphRef) {
  switch (ref.kind) {
    case "Local":
      return "secondary";
    case "Remote":
      return "info";
    case "Tag":
      return "outline";
    case "Stash":
      return "warning";
    default:
      return "default";
  }
}

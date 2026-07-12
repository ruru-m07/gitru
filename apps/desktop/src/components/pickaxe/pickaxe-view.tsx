import { Button } from "@gitru/ui/components/button";
import { Input } from "@gitru/ui/components/input";
import { ScrollArea } from "@gitru/ui/components/scroll-area";
import { WorkerPoolContextProvider } from "@pierre/diffs/react";
import {
  ChevronDown,
  ChevronUp,
  ListFilterPlus,
  Loader2,
  X,
} from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import LoaderIndicator from "@/components/loaderIndicator";
import { type PickaxeHit, usePickaxe } from "@/hooks/use-pickaxe";
import { useHighlightNavigation } from "@/hooks/useHighlightNavigation";
import { diffWorkerFactory } from "@/lib/diffWorkerFactory";
import {
  DEFAULT_PICKAXE_SEARCH_OPTIONS,
  type PickaxeSearchOptions,
} from "@/lib/pickaxe-search-options";
import { clearAllSearchHighlights } from "@/lib/searchTextHighlight";
import { useAppStore } from "@/store/useAppStore";
import { PickaxeDiffPreview } from "./pickaxe-diff-preview";
import { PickaxeQueryInput } from "./pickaxe-query-input";

const SEARCH_DEBOUNCE_MS = 350;

function hitDedupeKey(hit: PickaxeHit) {
  return `${hit.commitHash}:${hit.filePath}`;
}

function dedupeHits(hits: PickaxeHit[]) {
  const seen = new Set<string>();
  const unique: PickaxeHit[] = [];

  for (const hit of hits) {
    const key = hitDedupeKey(hit);
    if (seen.has(key)) {
      continue;
    }

    seen.add(key);
    unique.push(hit);
  }

  return unique;
}

export function PickaxeView() {
  const setMainWindowView = useAppStore((state) => state.setMainWindowView);
  const { hits, status, startSearch, cancelSearch } = usePickaxe();

  const [query, setQuery] = useState("");
  const [searchOptions, setSearchOptions] = useState<PickaxeSearchOptions>(
    DEFAULT_PICKAXE_SEARCH_OPTIONS,
  );
  const [author, setAuthor] = useState("");
  const [since, setSince] = useState("");
  const [until, setUntil] = useState("");
  const [filePatterns, setFilePatterns] = useState("");
  const [showFilters, setShowFilters] = useState(false);
  const resultsScrollRef = useRef<HTMLDivElement>(null);

  const uniqueHits = useMemo(() => dedupeHits(hits), [hits]);
  const { goToNext, goToPrevious, hasMatches } = useHighlightNavigation(
    query,
    searchOptions,
    resultsScrollRef,
    Boolean(query.trim()),
  );

  useEffect(() => {
    return () => {
      clearAllSearchHighlights();
    };
  }, []);

  useEffect(() => {
    const trimmedQuery = query.trim();
    if (!trimmedQuery) {
      clearAllSearchHighlights();
      void cancelSearch();
      return;
    }

    const timeoutId = window.setTimeout(() => {
      void startSearch({
        query,
        ...searchOptions,
        author,
        since,
        until,
        filePatterns,
      });
    }, SEARCH_DEBOUNCE_MS);

    return () => {
      window.clearTimeout(timeoutId);
    };
  }, [
    author,
    cancelSearch,
    filePatterns,
    query,
    searchOptions,
    since,
    startSearch,
    until,
  ]);

  return (
    <div className="flex max-h-[calc(var(--layout-height)-(--spacing(14)))] min-h-0 flex-col">
      <div className="shrink-0 p-1">
        <div className="flex items-center gap-1">
          <Button
            size="icon-sm"
            variant="outline"
            onClick={() => setShowFilters((open) => !open)}
          >
            <ListFilterPlus />
          </Button>
          <PickaxeQueryInput
            value={query}
            onChange={setQuery}
            searchOptions={searchOptions}
            onSearchOptionsChange={setSearchOptions}
            onKeyDown={(event) => {
              if (event.key === "Enter" && !event.shiftKey) {
                event.preventDefault();
                goToNext();
              } else if (event.key === "Enter" && event.shiftKey) {
                event.preventDefault();
                goToPrevious();
              }
            }}
          />
          <Button
            size="icon-sm"
            variant="outline"
            aria-label="Previous match"
            disabled={!hasMatches}
            onClick={goToPrevious}
          >
            <ChevronUp className="size-4" />
          </Button>
          <Button
            size="icon-sm"
            variant="outline"
            aria-label="Next match"
            disabled={!hasMatches}
            onClick={goToNext}
          >
            <ChevronDown className="size-4" />
          </Button>
          <Button
            size="icon-sm"
            variant="outline"
            aria-label="Close pickaxe"
            onClick={() => setMainWindowView(null)}
          >
            <X />
          </Button>
        </div>

        {/* {hasMatches ? (
          <div className="flex items-center gap-2 text-xs text-muted-foreground">
            <span>
              {activeIndex >= 0
                ? `Match ${activeIndex + 1} of ${matchCount}`
                : `${matchCount} matches`}
            </span>
            <span className="text-muted-foreground/60">·</span>
            {statusLabel ? (
              <div
                className={cn(
                  "flex items-center gap-2 text-xs text-muted-foreground",
                  error && "text-destructive",
                )}
              >
                {status === "running" ? (
                  <Loader2 className="size-3.5 animate-spin" />
                ) : null}
                <span>{statusLabel}</span>
              </div>
            ) : null}
          </div>
        ) : null} */}

        {showFilters ? (
          <div className="grid grid-cols-2 gap-2">
            <Input
              aria-label="Filter by author"
              placeholder="Author"
              size="sm"
              value={author}
              onChange={(event) => setAuthor(event.target.value)}
            />
            <Input
              aria-label="Filter by file type"
              placeholder="File patterns (*.ts, *.tsx)"
              size="sm"
              value={filePatterns}
              onChange={(event) => setFilePatterns(event.target.value)}
            />
            <Input
              aria-label="Filter commits since"
              placeholder="Since (e.g. 2024-01-01)"
              size="sm"
              value={since}
              onChange={(event) => setSince(event.target.value)}
            />
            <Input
              aria-label="Filter commits until"
              placeholder="Until (e.g. 2025-12-31)"
              size="sm"
              value={until}
              onChange={(event) => setUntil(event.target.value)}
            />
          </div>
        ) : null}
      </div>

      <ScrollArea
        ref={resultsScrollRef}
        className="min-h-0 dark:bg-black bg-secondary/20 max-h-[calc(var(--layout-height)-(--spacing(14)))] flex-1 overflow-auto"
        scrollFade
      >
        {!query.trim() ? (
          <div className="flex h-full items-center justify-center p-6 text-center text-sm text-muted-foreground">
            Search across all commits and files in this repository.
          </div>
        ) : status === "running" && uniqueHits.length === 0 ? (
          <div className="p-4">
            <LoaderIndicator />
          </div>
        ) : uniqueHits.length === 0 ? (
          <div className="flex h-full items-center justify-center p-6 text-center text-sm text-muted-foreground">
            No matches found. Try a different query or loosen your filters.
          </div>
        ) : (
          <WorkerPoolContextProvider
            poolOptions={{
              workerFactory: diffWorkerFactory,
              poolSize: 4,
            }}
            highlighterOptions={{
              theme: {
                dark: "github-dark",
                light: "github-light",
              },
              langs: [
                "typescript",
                "tsx",
                "javascript",
                "jsx",
                "rust",
                "json",
                "css",
                "html",
                "markdown",
                "toml",
                "yaml",
              ],
            }}
          >
            <div className="space-y-4">
              {uniqueHits.map((hit) => (
                <PickaxeDiffPreview
                  key={hitDedupeKey(hit)}
                  hit={hit}
                  searchQuery={query}
                  searchOptions={searchOptions}
                />
              ))}
              {status === "running" ? (
                <div className="flex items-center justify-center gap-2 py-4 text-xs text-muted-foreground">
                  <Loader2 className="size-3.5 animate-spin" />
                  <span>Loading more results...</span>
                </div>
              ) : null}
            </div>
          </WorkerPoolContextProvider>
        )}
      </ScrollArea>
    </div>
  );
}

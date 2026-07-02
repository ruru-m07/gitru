import { Button } from "@gitru/ui/components/button";
import { Input } from "@gitru/ui/components/input";
import { Kbd } from "@gitru/ui/components/kbd";
import { Label } from "@gitru/ui/components/label";
import { Switch } from "@gitru/ui/components/switch";
import { cn } from "@gitru/ui/lib/utils";
import { WorkerPoolContextProvider } from "@pierre/diffs/react";
import { ChevronDown, ChevronUp, Loader2, Search, X } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import LoaderIndicator from "@/components/loaderIndicator";
import { useHighlightNavigation } from "@/hooks/useHighlightNavigation";
import type { TimelineSearchMode } from "@/hooks/useTimelineSearch";
import {
  type TimelineSearchHit,
  useTimelineSearch,
} from "@/hooks/useTimelineSearch";
import { diffWorkerFactory } from "@/lib/diffWorkerFactory";
import { clearAllSearchHighlights } from "@/lib/searchTextHighlight";
import { useAppStore } from "@/store/useAppStore";
import { TimelineSearchDiffPreview } from "./TimelineSearchDiffPreview";
import { TimelineSearchResultCard } from "./TimelineSearchResultCard";

const SEARCH_DEBOUNCE_MS = 350;

function hitDedupeKey(hit: TimelineSearchHit) {
  return `${hit.commitHash}:${hit.filePath}`;
}

function dedupeHits(hits: TimelineSearchHit[]) {
  const seen = new Set<string>();
  const unique: TimelineSearchHit[] = [];

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

export function TimelineSearchView() {
  const setMainWindowView = useAppStore((state) => state.setMainWindowView);
  const {
    hits,
    commitsScanned,
    status,
    statusMessage,
    error,
    startSearch,
    cancelSearch,
  } = useTimelineSearch();

  const [query, setQuery] = useState("");
  const [isRegex, setIsRegex] = useState(false);
  const [mode, setMode] = useState<TimelineSearchMode>("pickaxe");
  const [author, setAuthor] = useState("");
  const [since, setSince] = useState("");
  const [until, setUntil] = useState("");
  const [filePatterns, setFilePatterns] = useState("");
  const [showFilters, setShowFilters] = useState(false);
  const resultsScrollRef = useRef<HTMLDivElement>(null);

  const uniqueHits = useMemo(() => dedupeHits(hits), [hits]);
  const { activeIndex, matchCount, goToNext, goToPrevious, hasMatches } =
    useHighlightNavigation(
      query,
      isRegex,
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
        isRegex,
        mode,
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
    isRegex,
    mode,
    query,
    since,
    startSearch,
    until,
  ]);

  const statusLabel = useMemo(() => {
    if (error) {
      return error;
    }

    if (status === "running") {
      return (
        statusMessage ??
        `Searching... ${commitsScanned} commits scanned, ${uniqueHits.length} results`
      );
    }

    if (status === "finished") {
      return `Found ${uniqueHits.length} results across ${commitsScanned} commits`;
    }

    if (status === "cancelled") {
      return "Search cancelled";
    }

    return null;
  }, [commitsScanned, error, status, statusMessage, uniqueHits.length]);

  return (
    <div className="flex h-[var(--layout-height)] min-h-0 flex-col">
      <div className="flex h-9.25 shrink-0 items-center justify-between border-b px-3">
        <div className="flex items-center gap-2 text-sm font-medium">
          <Search className="size-4" />
          Timeline Search
        </div>
        <Button
          size="icon-xs"
          variant="outline"
          aria-label="Close timeline search"
          onClick={() => setMainWindowView(null)}
        >
          <X />
        </Button>
      </div>

      <div className="shrink-0 space-y-3 border-b p-3">
        <div className="flex items-center gap-2">
          <Input
            aria-label="Timeline search query"
            placeholder="Search across commits and files..."
            className="flex-1"
            size="sm"
            value={query}
            onChange={(event) => setQuery(event.target.value)}
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
          <div className="flex items-center gap-1">
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
          </div>
          <Button
            size="sm"
            variant="outline"
            onClick={() => setShowFilters((open) => !open)}
          >
            Filters
          </Button>
        </div>

        {hasMatches ? (
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
        ) : null}

        <div className="flex flex-wrap items-center gap-4 text-sm">
          <div className="flex items-center gap-2">
            <Switch
              id="timeline-search-regex"
              checked={isRegex}
              onCheckedChange={setIsRegex}
            />
            <Label htmlFor="timeline-search-regex">Regex</Label>
          </div>

          <div className="flex items-center gap-1 rounded-md border p-0.5">
            <Button
              size="sm"
              variant={mode === "pickaxe" ? "secondary" : "ghost"}
              onClick={() => setMode("pickaxe")}
            >
              Pickaxe
            </Button>
            <Button
              size="sm"
              variant={mode === "fullContent" ? "secondary" : "ghost"}
              onClick={() => setMode("fullContent")}
            >
              Full content
            </Button>
          </div>
        </div>

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

      <div ref={resultsScrollRef} className="min-h-0 flex-1 overflow-auto">
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
                <TimelineSearchDiffPreview
                  key={hitDedupeKey(hit)}
                  hit={hit}
                  searchQuery={query}
                  isRegex={isRegex}
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
      </div>
    </div>
  );
}

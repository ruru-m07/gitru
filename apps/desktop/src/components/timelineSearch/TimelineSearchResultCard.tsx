import type { TimelineSearchHit } from "@/hooks/useTimelineSearch";
import { FileLevelStatusBarLeft } from "@/routes/app/git";
import { TimelineSearchDiffPreview } from "./TimelineSearchDiffPreview";

export function TimelineSearchResultCard({
  hit,
  searchQuery,
  isRegex,
}: {
  hit: TimelineSearchHit;
  searchQuery: string;
  isRegex: boolean;
}) {
  return (
    <article className="overflow-hidden rounded-lg border bg-card">
      <FileLevelStatusBarLeft
        resolvedSelection={{
          state: "timeline",
          hit,
        }}
      />

      <TimelineSearchDiffPreview
        hit={hit}
        searchQuery={searchQuery}
        isRegex={isRegex}
      />
    </article>
  );
}

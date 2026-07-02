import type { TimelineSearchHit } from "@/hooks/useTimelineSearch";
import { FileLevelStatusBarLeft } from "@/routes/app/git/components/file-level-status-bar-left";
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
    <article className="overflow-hidden">
      <TimelineSearchDiffPreview
        hit={hit}
        searchQuery={searchQuery}
        isRegex={isRegex}
      />
    </article>
  );
}

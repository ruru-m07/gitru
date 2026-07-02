import type { FileDiffMetadata } from "@pierre/diffs";
import { Minus, MoveHorizontal, Plus } from "lucide-react";
import { getStatusIcon } from "@/components/getStatusIcon";
import { fileDiffTypeToStatus } from "@/lib/timelineSearchStatus";

function renderPath(path: string) {
  const parts = path.split("/");
  const fileName = parts.pop();
  const dir = parts.join("/");

  return (
    <span>
      {dir ? <span className="text-muted-foreground/75">{dir}/</span> : null}
      {fileName}
    </span>
  );
}

function countDiffStats(fileDiff: FileDiffMetadata) {
  let additions = 0;
  let deletions = 0;

  for (const hunk of fileDiff.hunks) {
    additions += hunk.additionCount;
    deletions += hunk.deletionCount;
  }

  return { additions, deletions };
}

export function TimelineSearchDiffHeader({
  fileDiff,
}: {
  fileDiff: FileDiffMetadata;
}) {
  const { additions, deletions } = countDiffStats(fileDiff);
  const displayPath = fileDiff.name || fileDiff.prevName || "untitled";

  return (
    <div className="flex min-h-9 items-center justify-between gap-3 border-b bg-muted/20 px-3 py-2 text-xs">
      <div className="flex min-w-0 flex-wrap items-center gap-x-2 gap-y-1">
        {getStatusIcon(fileDiffTypeToStatus(fileDiff.type), 16)}
        {fileDiff.prevName && fileDiff.prevName !== fileDiff.name ? (
          <>
            <span className="truncate">{renderPath(fileDiff.prevName)}</span>
            <MoveHorizontal
              className="shrink-0 text-muted-foreground opacity-70"
              size={14}
            />
            <span className="truncate font-medium">{renderPath(displayPath)}</span>
          </>
        ) : (
          <span className="truncate font-medium">{renderPath(displayPath)}</span>
        )}
      </div>
      <div className="flex shrink-0 items-center gap-2 text-muted-foreground">
        {additions > 0 ? (
          <span className="flex items-center gap-0.5 text-green-600 dark:text-green-400">
            <Plus size={12} />
            {additions}
          </span>
        ) : null}
        {deletions > 0 ? (
          <span className="flex items-center gap-0.5 text-red-600 dark:text-red-400">
            <Minus size={12} />
            {deletions}
          </span>
        ) : null}
      </div>
    </div>
  );
}
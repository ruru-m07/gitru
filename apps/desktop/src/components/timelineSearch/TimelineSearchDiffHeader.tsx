import { CopyButton } from "@gitru/ui/components/copy-button";
import type { FileDiffMetadata } from "@pierre/diffs";
import { ChevronDown, MoveHorizontal } from "lucide-react";
import { useMemo, useState } from "react";
import { getStatusIcon } from "@/components/getStatusIcon";
import { TimelineSearchHit } from "@/hooks";
import { timeAgoFromUnixSeconds } from "@/lib/time";
import { fileDiffTypeToStatus } from "@/lib/timelineSearchStatus";
import { DiffStat } from "../diffBoxes";

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
  collapsed,
  fileDiff,
  toggleCollapsed,
  hit,
}: {
  collapsed: boolean;
  fileDiff: FileDiffMetadata;
  toggleCollapsed(): unknown;
  hit: TimelineSearchHit;
}) {
  // const { additions, deletions } = countDiffStats(fileDiff);
  const displayPath = fileDiff.name || fileDiff.prevName || "untitled";

  const { additions, deletions } = useMemo(() => {
    let additions = 0;
    let deletions = 0;
    for (const hunk of fileDiff.hunks) {
      additions += hunk.additionLines;
      deletions += hunk.deletionLines;
    }
    return { additions, deletions };
  }, [fileDiff]);

  return (
    <div className="flex min-h-9 items-center justify-between gap-3 mb-1 bg-muted/20 px-3 py-2 text-xs">
      {/* <div className="flex min-w-0 flex-wrap items-center gap-x-2 gap-y-1 ml-1">
        {getStatusIcon(fileDiffTypeToStatus(fileDiff.type), 20)}
        {fileDiff.prevName && fileDiff.prevName !== fileDiff.name ? (
          <>
            <span className="truncate">{renderPath(fileDiff.prevName)}</span>
            <MoveHorizontal
              className="shrink-0 text-muted-foreground opacity-70"
              size={14}
            />
            <span className="truncate font-medium">
              {renderPath(displayPath)}
            </span>
          </>
        ) : (
          <div className="group flex items-center">
            <span className="truncate font-normal text-base">
              {renderPath(displayPath)}
            </span>
            <div className="ml-1 opacity-0 transition-opacity group-hover:opacity-100 text-xs text-muted-foreground">
              <CopyButton size={"xs"} variant="ghost" text={displayPath} />
            </div>
          </div>
        )}
      </div> */}
      <div className="flex min-w-0 items-center gap-3">
        {/* <button
          type="button"
          onClick={toggleCollapsed}
          aria-label={collapsed ? "Expand file" : "Collapse file"}
          aria-pressed={collapsed}
          className={`inline-flex size-8 shrink-0 cursor-pointer items-center justify-center rounded-sm text-white/70 transition ${collapsed ? "bg-white/15 hover:bg-white/20" : "bg-[#F05138] hover:bg-[#F05138]/80 "}`}
        >
          <ChevronDown
            className={`transition-transform ${collapsed ? "-rotate-90" : ""}`}
          />
        </button> */}

        <div className="min-w-0">
          <div className="flex min-w-0 flex-wrap items-center gap-x-2 gap-y-1">
            {getStatusIcon(fileDiffTypeToStatus(fileDiff.type), 20)}
            {fileDiff.prevName && fileDiff.prevName !== fileDiff.name ? (
              <>
                <span className="truncate">
                  {renderPath(fileDiff.prevName)}
                </span>
                <MoveHorizontal
                  className="shrink-0 text-muted-foreground opacity-70"
                  size={14}
                />
                <span className="truncate font-medium">
                  {renderPath(displayPath)}
                </span>
              </>
            ) : (
              <div className="group flex items-center">
                <span className="truncate font-[450] text-sm">
                  {renderPath(displayPath)}
                </span>
                <div className="ml-1 opacity-0 transition-opacity group-hover:opacity-100 text-xs text-muted-foreground">
                  <CopyButton size={"xs"} variant="ghost" text={displayPath} />
                </div>
              </div>
            )}
          </div>
          <div className="flex flex-wrap items-center gap-x-2 gap-y-1 text-xs text-muted-foreground">
            <span className="font-mono">{hit.commitHash.slice(0, 7)}</span>
            <span>{hit.authorName}</span>
            <span>{timeAgoFromUnixSeconds(hit.commitTime)}</span>
            {hit.matchLine ? <span>line {hit.matchLine}</span> : null}
          </div>
        </div>
      </div>
      <div className="flex items-center justify-end gap-2">
        <span className="flex text-sm gap-2 font-mono">
          <span className="tabular-nums text-green-600">+{additions}</span>
          <span className="tabular-nums text-red-600">-{deletions}</span>
        </span>
        <DiffStat
          stats={{
            deletions: deletions,
            insertions: additions,
          }}
        />
      </div>
    </div>
  );
}

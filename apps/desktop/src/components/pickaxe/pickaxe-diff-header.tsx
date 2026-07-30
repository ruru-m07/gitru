import {
  Avatar,
  AvatarFallback,
  AvatarImage,
} from "@gitru/ui/components/avatar";
import {
  Tooltip,
  TooltipPopup,
  TooltipTrigger,
} from "@gitru/ui/components/tooltip";
import type { FileDiffMetadata } from "@pierre/diffs";
import { MoveHorizontal } from "lucide-react";
import { useMemo } from "react";
import { getStatusIcon } from "@/components/get-status-icon";
import { PickaxeHit } from "@/hooks";
import { fileDiffTypeToStatus } from "@/lib/pickaxe-status";
import { timeAgoFromUnixSeconds } from "@/lib/time";
import { DiffStat } from "@/components/diff-boxes";

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

export function PickaxeDiffHeader({
  fileDiff,
  hit,
}: {
  fileDiff: FileDiffMetadata;
  hit: PickaxeHit;
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
    <div className="flex min-h-9 items-center justify-between gap-3 mb-1 bg-muted/20 pl-3.5 px-3 py-2 text-xs border-t">
      <div className="flex min-w-0 items-center gap-1.5">
        <div className="min-w-0">
          <div className="flex min-w-0 flex-wrap items-center gap-x-2.5">
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
              </div>
            )}
          </div>
        </div>
        <span className="text-muted-foreground/50">•</span>
        <div className="gap-1 flex items-center">
          <div className="flex group">
            <Tooltip>
              <TooltipTrigger
                style={{
                  zIndex: (hit.commit.authors.co_authors.length || 0) + 1,
                }}
              >
                <Avatar className="ring-2 ring-background rounded-sm size-4">
                  <AvatarImage
                    alt={hit.commit.authors.author.name}
                    src={`https://avatars.githubusercontent.com/u/e?email=${hit.commit.authors.author.email}&s=64`}
                  />
                  <AvatarFallback>
                    {hit.commit.authors.author.name
                      .split(" ")
                      .map((namePart) => namePart[0])
                      .join("")
                      .toUpperCase()}
                  </AvatarFallback>
                </Avatar>
              </TooltipTrigger>
              <TooltipPopup side="bottom">
                {hit.commit.authors.author.name}
              </TooltipPopup>
            </Tooltip>
            {hit.commit.authors.co_authors.map((coAuthor, idx) => (
              <Tooltip key={`${idx}-tooltip-coauthor`}>
                <TooltipTrigger
                  style={{
                    zIndex: hit.commit.authors.co_authors.length - idx,
                  }}
                  key={`${idx}-tooltip-trigger-coauthor`}
                >
                  <Avatar className="ring-2 ring-background rounded-sm size-4 ml-[-0.2rem] group-hover:ml-0.5 transition-all duration-100">
                    <AvatarImage
                      alt="U1"
                      src={`https://avatars.githubusercontent.com/u/e?email=${coAuthor.email}&s=64`}
                    />
                    <AvatarFallback>
                      {coAuthor.name
                        .split(" ")
                        .map((namePart) => namePart[0])
                        .join("")
                        .toUpperCase()}
                    </AvatarFallback>
                  </Avatar>
                </TooltipTrigger>
                <TooltipPopup side="bottom">{coAuthor.name}</TooltipPopup>
              </Tooltip>
            ))}
          </div>
          <span className="font-normal">{hit.commit.authors.author.name}</span>
          <span className="text-xs">
            ( {timeAgoFromUnixSeconds(hit.commit.timestamp)} )
          </span>
        </div>
        <span className="text-muted-foreground/50">•</span>
        <span className="font-mono text-xs text-muted-foreground">
          {hit.commit.summary}
        </span>
      </div>
      <div className="flex items-center justify-end gap-2">
        <div className="flex items-center gap-1 mr-1">
          <span className="font-mono text-xs text-muted-foreground opacity-60">
            {hit.commitHash.slice(0, 7)}
          </span>
        </div>
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

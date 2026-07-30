import type { GraphRow } from "@gitru/commands";
import {
  Avatar,
  AvatarFallback,
  AvatarImage,
} from "@gitru/ui/components/avatar";
import { Badge } from "@gitru/ui/components/badge";
import { Label } from "@gitru/ui/components/label";
import {
  Tooltip,
  TooltipPopup,
  TooltipProvider,
  TooltipTrigger,
} from "@gitru/ui/components/tooltip";
import { Files, Tags } from "lucide-react";
import { useRef } from "react";
import { useOnInView } from "react-intersection-observer";
import {
  formatUnixSecondsToDateTime,
  timeAgoFromUnixSeconds,
} from "@/lib/time";

export const HistoryCommitInfiniteList = ({
  rows,
  onOpenCommit,
  fetchNextPage,
  hasNextPage,
  isFetchingNextPage,
  isLoading,
}: {
  rows: GraphRow[];
  onOpenCommit: (commitHash: string) => void;
  fetchNextPage: () => void;
  hasNextPage: boolean;
  isFetchingNextPage: boolean;
  isLoading: boolean;
}) => {
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

  if (isLoading) {
    return (
      <div className="w-full h-full flex items-center justify-center">
        <span className="text-sm text-muted-foreground">Loading...</span>
      </div>
    );
  }

  return (
    <div ref={scrollRef} className="flex-1 min-h-0 overflow-y-auto">
      {rows.map((row) => (
        <div
          className="w-full p-2 border-b hover:bg-accent cursor-pointer"
          key={row.oid}
          onClick={() => onOpenCommit(row.commit.id)}
        >
          <p className="truncate text-sm">{row.commit.summary}</p>
          <div className="flex mt-1 items-center justify-between w-full">
            <TooltipProvider>
              <div className="flex items-center">
                <div className="flex -mt-0.5 group">
                  <Tooltip>
                    <TooltipTrigger
                      style={{
                        zIndex: row.commit.authors.co_authors.length + 1,
                      }}
                    >
                      <Avatar className="ring-2 ring-background rounded-sm size-4">
                        <AvatarImage
                          alt={row.commit.authors.author.name}
                          src={`https://avatars.githubusercontent.com/u/e?email=${row.commit.authors.author.email}&s=64`}
                        />
                        <AvatarFallback>
                          {row.commit.authors.author.name
                            .split(" ")
                            .map((n) => n[0])
                            .join("")
                            .toUpperCase()}
                        </AvatarFallback>
                      </Avatar>
                    </TooltipTrigger>
                    <TooltipPopup side="bottom">
                      {row.commit.authors.author.name}
                    </TooltipPopup>
                  </Tooltip>
                  {row.commit.authors.co_authors.map((coAuthor, idx) => (
                    <Tooltip key={`${row.oid}-${idx}-tooltip-coauthor`}>
                      <TooltipTrigger
                        style={{
                          zIndex: row.commit.authors.co_authors.length - idx,
                        }}
                        key={`${row.oid}-${idx}-tooltip-trigger-coauthor`}
                      >
                        <Avatar className="ring-2 ring-background rounded-sm size-4 -ml-[0.2rem] group-hover:ml-0.5 transition-all duration-100">
                          <AvatarImage
                            alt={coAuthor.name}
                            src={`https://avatars.githubusercontent.com/u/e?email=${coAuthor.email}&s=64`}
                          />
                          <AvatarFallback>
                            {coAuthor.name
                              .split(" ")
                              .map((n) => n[0])
                              .join("")
                              .toUpperCase()}
                          </AvatarFallback>
                        </Avatar>
                      </TooltipTrigger>
                      <TooltipPopup side="bottom">{coAuthor.name}</TooltipPopup>
                    </Tooltip>
                  ))}
                </div>
                <Label className="ml-1 text-muted-foreground text-xs font-light">
                  <Tooltip>
                    <TooltipTrigger>
                      {row.commit.authors.author.name}
                    </TooltipTrigger>
                    <TooltipPopup side="bottom">
                      {row.commit.authors.author.email}
                    </TooltipPopup>
                  </Tooltip>
                  <span className="-mx-1">{" • "}</span>
                  <Tooltip>
                    <TooltipTrigger>
                      {timeAgoFromUnixSeconds(row.commit.timestamp)}{" "}
                    </TooltipTrigger>
                    <TooltipPopup side="bottom">
                      {formatUnixSecondsToDateTime(row.commit.timestamp)}
                    </TooltipPopup>
                  </Tooltip>
                </Label>
              </div>
              <div className="flex gap-1 items-center">
                {row.tags.length > 0 && (
                  <>
                    <Tooltip>
                      <TooltipTrigger className={"flex gap-0.5"}>
                        <Tags
                          className="size-3.5 text-muted-foreground"
                          aria-label={`${row.tags.length} tags`}
                        />
                        <span className="text-xs text-muted-foreground tabular-nums font-normal">
                          {row.tags.length}
                        </span>
                      </TooltipTrigger>
                      <TooltipPopup>
                        {row.tags.map((tag) => (
                          <Badge key={tag.name}>{tag.name}</Badge>
                        ))}
                      </TooltipPopup>
                    </Tooltip>
                    <span className="text-xs text-muted-foreground">/</span>
                  </>
                )}

                <span className="text-xs text-muted-foreground flex items-center gap-1">
                  <Files className="size-3.5" />
                  {row?.commit.stats?.files_changed ?? 0}
                </span>

                <span className="text-xs text-muted-foreground">/</span>

                <span className="text-xs text-green-600 tabular-nums font-normal">
                  +{row?.commit.stats?.insertions ?? 0}
                </span>
                <span className="text-xs text-red-600 tabular-nums font-normal">
                  -{row?.commit.stats?.deletions ?? 0}
                </span>
              </div>
            </TooltipProvider>
          </div>
        </div>
      ))}
      <div className="w-full flex justify-center p-2 text-xs text-muted-foreground">
        {isFetchingNextPage ? "Loading more..." : null}
      </div>
      <div ref={bottomRef} className="h-4" />
    </div>
  );
};

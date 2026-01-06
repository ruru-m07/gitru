import {
  Avatar,
  AvatarFallback,
  AvatarImage,
} from "@gitru/ui/components/avatar";
import { Badge } from "@gitru/ui/components/badge";
import {
  Popover,
  PopoverPopup,
  PopoverTitle,
  PopoverTrigger,
} from "@gitru/ui/components/popover";
import { Separator } from "@gitru/ui/components/separator";
import {
  Tooltip,
  TooltipPopup,
  TooltipTrigger,
} from "@gitru/ui/components/tooltip";
import {
  ArrowDown,
  File,
  GitBranch,
  GitCommitVertical,
  RefreshCw,
  Settings,
} from "lucide-react";
import {
  useGetCommitById,
  useGetCurrentBranch,
  useGetLastCommit,
  useGetRepositoryOrigin,
  useGetStatus,
} from "@/hooks";
import { timeAgoFromUnixSeconds } from "@/lib/time";
import { CommitInfo } from "@/tauri";
// import { GithubDark } from "../ui/svgs/githubDark";
import { GithubLight } from "./svgs/githubLight";

const StatusBar = () => {
  const { data: lastCommit } = useGetLastCommit();
  const { data: currentBranch } = useGetCurrentBranch();
  const { data: repositoryOrigin } = useGetRepositoryOrigin();
  const { data: status } = useGetStatus();

  return (
    <div className="border-t overflow-hidden h-7 flex justify-between items-center ">
      {/* left side */}
      <div className="h-full flex">
        <a
          target="_blank"
          href={
            repositoryOrigin?.provider === "github"
              ? `https://github.com/${repositoryOrigin?.owner}/${repositoryOrigin?.repo}`
              : "#"
          }
        >
          <Badge
            variant={"outline"}
            className="h-full rounded-none border-0 border-r px-2 flex items-center cursor-pointer hover:bg-muted border-b border-b-transparent hover:border-b-border"
          >
            <span className="flex items-center gap-1">
              <span className="text-muted-foreground font-normal">
                Origin:{" "}
              </span>
              <GithubLight />
              <span className="text-muted-foreground">
                {repositoryOrigin?.owner} /{" "}
              </span>
              <span className="text-foreground">{repositoryOrigin?.repo}</span>
            </span>
          </Badge>
        </a>
        <Badge
          variant={"outline"}
          className="text-muted-foreground h-full rounded-none border-0 border-r px-2 flex items-center font-normal cursor-pointer hover:bg-muted border-b border-b-transparent hover:border-b-border"
        >
          <GitBranch />
          <span className="ml-1 text-foreground">
            {currentBranch?.display_name}
            {status?.files && status?.files.length > 0 ? "*" : ""}
          </span>
        </Badge>
        <Badge
          variant={"outline"}
          className="text-muted-foreground h-full rounded-none border-0 border-r px-2 flex items-center cursor-pointer hover:bg-muted border-b border-b-transparent hover:border-b-border"
        >
          <RefreshCw />
        </Badge>
        {lastCommit ? <LastCommitBox lastCommit={lastCommit} /> : null}
        {/* <Badge
          variant={"outline"}
          className="text-muted-foreground h-full rounded-none border-0 border-r flex items-center font-normal tabular-nums px-1.5"
        >
          <ArrowUp />
          <span className="tabular-nums text-foreground">6 </span>
          <span>ahead</span>
        </Badge> */}
        <Badge
          variant={"outline"}
          className="text-muted-foreground h-full rounded-none border-0 border-r flex items-center font-normal tabular-nums px-1.5 cursor-pointer hover:bg-muted border-b border-b-transparent hover:border-b-border"
        >
          <ArrowDown />
          <span className="tabular-nums text-foreground">3 </span>
          <span>behind</span>
        </Badge>
      </div>
      {/* right side */}
      <div className="h-full flex">
        <Badge
          variant={"outline"}
          className="h-full rounded-none border-0 border-l px-2 flex items-center"
        >
          <span>
            <span className="text-muted-foreground font-normal">Channel: </span>
            <span className="text-foreground">development </span>
          </span>
        </Badge>
        <Badge
          variant={"outline"}
          className="h-full rounded-none border-0 border-l px-2 flex items-center"
        >
          <span className="text-muted-foreground font-mono font-normal tabular-nums">
            v0.0.1-beta.1
          </span>
        </Badge>
        <Badge
          variant={"outline"}
          className="text-muted-foreground h-full rounded-none border-0 border-l px-2 flex items-center"
        >
          <Settings />
        </Badge>
      </div>
    </div>
  );
};

export default StatusBar;

const LastCommitBox = ({ lastCommit }: { lastCommit: CommitInfo }) => {
  const { data: fullCommit } = useGetCommitById(lastCommit.id);
  return (
    <Popover>
      <PopoverTrigger
        render={
          <Badge
            variant={"outline"}
            className="h-full rounded-none border-0 border-r flex items-center cursor-pointer hover:bg-muted border-b border-b-transparent hover:border-b-border"
          >
            <GitCommitVertical className="size-4" strokeWidth={1} />
            <div className="flex group">
              <Tooltip>
                <TooltipTrigger
                  style={{
                    zIndex: lastCommit.authors.co_authors.length + 1,
                  }}
                >
                  <Avatar className="ring-2 ring-background rounded-sm size-4">
                    <AvatarImage
                      alt={lastCommit.authors.author.name}
                      src={`https://avatars.githubusercontent.com/u/e?email=${lastCommit.authors.author.email}&s=64`}
                    />
                    <AvatarFallback>
                      {lastCommit.authors.author.name
                        .split(" ")
                        .map((n) => n[0])
                        .join("")
                        .toUpperCase()}
                    </AvatarFallback>
                  </Avatar>
                </TooltipTrigger>
                <TooltipPopup side="bottom">
                  {lastCommit.authors.author.name}
                </TooltipPopup>
              </Tooltip>
              {lastCommit.authors.co_authors.map((coAuthor, idx) => (
                <Tooltip key={`${idx}-tooltip-coauthor`}>
                  <TooltipTrigger
                    style={{
                      zIndex: lastCommit.authors.co_authors.length - idx,
                    }}
                    key={`${idx}-tooltip-trigger-coauthor`}
                  >
                    <Avatar className="ring-2 ring-background rounded-sm size-4 -ml-[0.2rem] group-hover:ml-0.5 transition-all duration-100">
                      <AvatarImage
                        alt="U1"
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
            <span className="font-normal">
              {lastCommit?.authors.author.name}
            </span>
            <span className="text-muted-foreground font-light text-xs">
              ( {timeAgoFromUnixSeconds(lastCommit?.timestamp)} )
            </span>
          </Badge>
        }
      ></PopoverTrigger>
      <PopoverPopup className="min-w-80 max-w-96 relative [&>div]:p-2">
        <div>
          <div className="flex group">
            <Tooltip>
              <TooltipTrigger
                style={{
                  zIndex: lastCommit.authors.co_authors.length + 1,
                }}
              >
                <Avatar className="ring-2 ring-background rounded-sm size-6">
                  <AvatarImage
                    alt={lastCommit.authors.author.name}
                    src={`https://avatars.githubusercontent.com/u/e?email=${lastCommit.authors.author.email}&s=64`}
                  />
                  <AvatarFallback>
                    {lastCommit.authors.author.name
                      .split(" ")
                      .map((n) => n[0])
                      .join("")
                      .toUpperCase()}
                  </AvatarFallback>
                </Avatar>
              </TooltipTrigger>
              <TooltipPopup side="bottom">
                {lastCommit.authors.author.name}
              </TooltipPopup>
            </Tooltip>
            {lastCommit.authors.co_authors.map((coAuthor, idx) => (
              <Tooltip key={`${idx}-tooltip-coauthor`}>
                <TooltipTrigger
                  style={{
                    zIndex: lastCommit.authors.co_authors.length - idx,
                  }}
                  key={`${idx}-tooltip-trigger-coauthor`}
                >
                  <Avatar className="ring-2 ring-background rounded-sm size-6 -ml-[0.2rem] group-hover:ml-0.5 transition-all duration-100">
                    <AvatarImage
                      alt="U1"
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
          <PopoverTitle className="text-xs font-normal mt-2 p-0">
            <span>
              <span>{lastCommit.authors.author.name}</span>
              <span className="ml-1 text-muted-foreground">
                ( {lastCommit.authors.author.email} )
              </span>
            </span>
          </PopoverTitle>
          <Separator className={"my-1"} />
          <div className="text-sm">
            <div className="truncate">{lastCommit.summary}</div>
            <span className="line-clamp-5 text-xs text-muted-foreground mt-1">
              <pre>{lastCommit.body}</pre>
            </span>
          </div>
          <div className="absolute top-0 right-0 px-2 py-1 flex items-center gap-2">
            <span className="text-xs text-muted-foreground flex items-center gap-1">
              <File size={12} />
              {fullCommit?.stats.files_changed}
            </span>
            <div className="flex h-3.5 items-center justify-center">
              <Separator orientation="vertical" />
            </div>
            <span className="text-xs text-green-600">
              +{fullCommit?.stats.insertions}
            </span>
            <span className="text-xs text-red-600">
              -{fullCommit?.stats.deletions}
            </span>
          </div>
        </div>
      </PopoverPopup>
    </Popover>
  );
};

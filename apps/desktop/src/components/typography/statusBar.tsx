import { Badge } from "@gitru/ui/components/badge";
import {
  ArrowDown,
  GitBranch,
  GitCommitVertical,
  RefreshCw,
  Settings,
} from "lucide-react";
// import { GithubDark } from "../ui/svgs/githubDark";
import { GithubLight } from "../ui/svgs/githubLight";

const StatusBar = () => {
  return (
    <div className="border-t overflow-hidden h-7 flex justify-between items-center ">
      {/* left side */}
      <div className="h-full flex">
        <a target="_blank" href="https://github.com/ruru-m07/gitru">
          <Badge
            variant={"outline"}
            className="h-full rounded-none border-0 border-r px-2 flex items-center"
          >
            <span className="flex items-center gap-1">
              <span className="text-muted-foreground font-normal">
                Origin:{" "}
              </span>
              <GithubLight />
              <span className="text-muted-foreground">ruru-m07 / </span>
              <span className="text-foreground">gitru</span>
            </span>
          </Badge>
        </a>
        <Badge
          variant={"outline"}
          className="text-muted-foreground h-full rounded-none border-0 border-r px-2 flex items-center font-normal"
        >
          <GitBranch />
          <span className="ml-1 text-foreground">main*</span>
        </Badge>
        <Badge
          variant={"outline"}
          className="text-muted-foreground h-full rounded-none border-0 border-r px-2 flex items-center"
        >
          <RefreshCw />
        </Badge>
        <Badge
          variant={"outline"}
          className="h-full rounded-none border-0 border-r flex items-center"
        >
          <GitCommitVertical className="size-4" strokeWidth={1} />
          <span className="font-normal">ruru</span>
          <span className="text-muted-foreground font-light text-xs">
            ( 2 months ago )
          </span>
        </Badge>
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
          className="text-muted-foreground h-full rounded-none border-0 border-r flex items-center font-normal tabular-nums px-1.5"
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

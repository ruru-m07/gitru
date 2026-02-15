import { AheadBehindStatus, gitVersion } from "@gitru/commands";
import { Git } from "@gitru/icon";
import {
  Avatar,
  AvatarFallback,
  AvatarImage,
} from "@gitru/ui/components/avatar";
import { Badge } from "@gitru/ui/components/badge";
import { useCommandNavigation } from "@gitru/ui/components/command";
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
import { getVersion } from "@tauri-apps/api/app";
import {
  ArrowDown,
  ArrowUp,
  CloudUpload,
  File,
  GitBranch,
  GitCommitVertical,
  Loader2,
  RefreshCw,
  RotateCw,
} from "lucide-react";
import { motion, useAnimate } from "motion/react";
import React, { useEffect, useRef } from "react";
import {
  useGetCommitById,
  useGetCurrentBranch,
  useGetLastCommit,
  useGetStatus,
  useGetStatusAheadBehind,
  useGitFetch,
  useGitPull,
  useGitPush,
} from "@/hooks";
import { getAvatarByProvider } from "@/lib/getAvatarByGitProvider";
import { parseOrigin } from "@/lib/parseOrigin";
import { timeAgoFromUnixSeconds } from "@/lib/time";
import { appState } from "@/state";
import { useAppStore } from "@/store/useAppStore";

const StatusBar = () => {
  const { data: statusAheadBehind } = useGetStatusAheadBehind();

  return (
    <div className="border-t overflow-hidden h-7 flex justify-between items-center">
      <div className="flex h-full">
        <OriginBadge />
        <CurrentBranchBadge />
        <FetchBadge />
        <AheadBadge statusAheadBehind={statusAheadBehind || undefined} />
        <BehindBadge statusAheadBehind={statusAheadBehind || undefined} />
        <LastCommitBox />
      </div>
      <div className="flex h-full">
        <EnvironmentBadge />
        <VersionBadge />
        <InvalidateAllBadge />
        <GitVersion />
      </div>
    </div>
  );
};

export default StatusBar;

const LastCommitBox = () => {
  const { data: lastCommit } = useGetLastCommit();
  const { data: fullCommit } = useGetCommitById(lastCommit?.id || "");

  return (
    <Popover>
      <PopoverTrigger
        render={
          <Badge
            variant={"outline"}
            className="h-full rounded-none border-t-transparent border-l-transparent flex items-center cursor-pointer hover:bg-muted!"
          >
            <GitCommitVertical className="size-4" strokeWidth={1} />
            <div className="flex group">
              <Tooltip>
                <TooltipTrigger
                  style={{
                    zIndex: (lastCommit?.authors.co_authors.length || 0) + 1,
                  }}
                >
                  <Avatar className="ring-2 ring-background rounded-sm size-4">
                    <AvatarImage
                      alt={lastCommit?.authors.author.name}
                      src={`https://avatars.githubusercontent.com/u/e?email=${lastCommit?.authors.author.email}&s=64`}
                    />
                    <AvatarFallback>
                      {lastCommit?.authors.author.name
                        .split(" ")
                        .map((n) => n[0])
                        .join("")
                        .toUpperCase()}
                    </AvatarFallback>
                  </Avatar>
                </TooltipTrigger>
                <TooltipPopup side="bottom">
                  {lastCommit?.authors.author.name}
                </TooltipPopup>
              </Tooltip>
              {lastCommit?.authors.co_authors.map((coAuthor, idx) => (
                <Tooltip key={`${idx}-tooltip-coauthor`}>
                  <TooltipTrigger
                    style={{
                      zIndex: lastCommit?.authors.co_authors.length - idx,
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
            <span className="text-muted-foreground! font-light text-xs pr-2">
              ( {timeAgoFromUnixSeconds(lastCommit?.timestamp || 0)} ) {}
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
                  zIndex: (lastCommit?.authors.co_authors.length || 0) + 1,
                }}
              >
                <Avatar className="ring-2 ring-background rounded-sm size-6">
                  <AvatarImage
                    alt={lastCommit?.authors.author.name}
                    src={`https://avatars.githubusercontent.com/u/e?email=${lastCommit?.authors.author.email}&s=64`}
                  />
                  <AvatarFallback>
                    {lastCommit?.authors.author.name
                      .split(" ")
                      .map((n) => n[0])
                      .join("")
                      .toUpperCase()}
                  </AvatarFallback>
                </Avatar>
              </TooltipTrigger>
              <TooltipPopup side="bottom">
                {lastCommit?.authors.author.name}
              </TooltipPopup>
            </Tooltip>
            {lastCommit?.authors.co_authors.map((coAuthor, idx) => (
              <Tooltip key={`${idx}-tooltip-coauthor`}>
                <TooltipTrigger
                  style={{
                    zIndex: lastCommit?.authors.co_authors.length - idx,
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
              <span>{lastCommit?.authors.author.name}</span>
              <span className="ml-1 text-muted-foreground!">
                ( {lastCommit?.authors.author.email} )
              </span>
            </span>
          </PopoverTitle>
          <Separator className={"my-1"} />
          <div className="text-sm">
            <div className="truncate">{lastCommit?.summary}</div>
            <span className="line-clamp-5 text-xs text-muted-foreground! mt-1">
              <pre>{lastCommit?.body}</pre>
            </span>
          </div>
          <div className="absolute top-0.5 right-0 px-2 py-1 flex items-center gap-2">
            <span className="text-xs flex items-center gap-1">
              <File size={12} />
              {fullCommit?.stats.files_changed}
            </span>
            <div className="flex h-3.5 items-center justify-center">
              <Separator orientation="vertical" />
            </div>
            <span className="text-xs text-green-600! dark:text-green-500!">
              +{fullCommit?.stats.insertions}
            </span>
            <span className="text-xs text-red-600! dark:text-red-500!">
              -{fullCommit?.stats.deletions}
            </span>
          </div>
        </div>
      </PopoverPopup>
    </Popover>
  );
};

const EnvironmentBadge = () => {
  if (import.meta.env.MODE) {
    return (
      <Badge
        variant={"outline"}
        className="h-full rounded-none border-0 border-l px-2 flex items-center"
      >
        <span>
          <span className="text-muted-foreground! font-normal">Channel: </span>
          <span className="text-foreground!">{import.meta.env.MODE} </span>
        </span>
      </Badge>
    );
  }
};

const GitVersion = () => {
  const [version, setVersion] = React.useState<string>("");

  React.useEffect(() => {
    gitVersion().then((v) => setVersion(v));
  }, []);

  if (!version) {
    return null;
  }

  return (
    <Tooltip>
      <TooltipTrigger>
        <Badge
          variant={"outline"}
          className="h-full rounded-none border-0 border-l px-2 flex items-center"
        >
          <Git className="size-3.5" />
        </Badge>
      </TooltipTrigger>
      <TooltipPopup side="top" align="end">
        <span className="text-muted-foreground! font-mono font-normal tabular-nums">
          git version{" "}
          <span className="text-foreground font-semibold">
            v{version.split("git version ")[1]}
          </span>
        </span>
      </TooltipPopup>
    </Tooltip>
  );
};

const VersionBadge = () => {
  const [version, setVersion] = React.useState<string>("");

  React.useEffect(() => {
    getVersion().then((v) => setVersion(v));
  }, []);

  if (!version) {
    return null;
  }

  return (
    <Badge
      variant={"outline"}
      className="h-full rounded-none border-0 border-l px-2 flex items-center"
    >
      <span className="text-muted-foreground! font-mono font-normal tabular-nums">
        v{version}
      </span>
    </Badge>
  );
};

const FetchBadge = () => {
  const { mutateAsync: fetch, isPending } = useGitFetch();
  const [scope, animate] = useAnimate();
  const rotationRef = useRef(0);
  const animationRef = useRef<any>(null);

  useEffect(() => {
    if (isPending) {
      if (animationRef.current) {
        animationRef.current.stop();
      }

      const startRotation = rotationRef.current;
      animationRef.current = animate(
        scope.current,
        { rotate: startRotation + 360 },
        {
          duration: 1,
          repeat: Infinity,
          ease: "linear",
          onUpdate: (latest) => {
            rotationRef.current = latest;
          },
        },
      );
    } else {
      if (animationRef.current) {
        animationRef.current.stop();
      }

      const currentRotation = rotationRef.current % 360;
      const nextCheckpoint = Math.ceil(currentRotation / 180) * 180;
      const targetRotation =
        rotationRef.current - currentRotation + nextCheckpoint;

      animationRef.current = animate(
        scope.current,
        { rotate: targetRotation },
        {
          duration: ((nextCheckpoint - currentRotation) / 360) * 1,
          ease: "easeOut",
          onComplete: () => {
            rotationRef.current = targetRotation % 360;
          },
        },
      );
    }

    return () => {
      if (animationRef.current) {
        animationRef.current.stop();
      }
    };
  }, [isPending, animate, scope]);

  return (
    <Badge
      variant={"outline"}
      className="text-muted-foreground! h-full rounded-none border-0 border-r px-2 flex items-center cursor-pointer hover:bg-muted! border-b border-b-transparent hover:border-b-border"
      onClick={async () => await fetch()}
    >
      <motion.div ref={scope}>
        <RefreshCw />
      </motion.div>
    </Badge>
  );
};

const AheadBadge = ({
  statusAheadBehind,
}: {
  statusAheadBehind: AheadBehindStatus | undefined;
}) => {
  const { mutateAsync: push, isPending } = useGitPush();

  return (
    <>
      {statusAheadBehind ? (
        <>
          {!statusAheadBehind.is_published ? (
            <Badge
              variant={"outline"}
              className="h-full rounded-none border-0 border-r flex items-center font-normal tabular-nums px-1.5 cursor-pointer hover:bg-muted! border-b border-b-transparent hover:border-b-border"
              onClick={async () => {
                await push();
              }}
            >
              <CloudUpload />
              <span>Publish Branch</span>
            </Badge>
          ) : statusAheadBehind?.ahead && statusAheadBehind.ahead > 0 ? (
            <Badge
              variant={"outline"}
              className="text-muted-foreground! h-full rounded-none border-0 border-r flex items-center font-normal tabular-nums px-1.5 cursor-pointer hover:bg-muted! border-b border-b-transparent hover:border-b-border"
              onClick={async () => {
                await push();
              }}
            >
              {isPending ? <Loader2 className="animate-spin" /> : <ArrowUp />}
              <span className="tabular-nums text-foreground!">
                {statusAheadBehind.ahead}{" "}
              </span>
              <span>ahead</span>
            </Badge>
          ) : null}
        </>
      ) : null}
    </>
  );
};

const BehindBadge = ({
  statusAheadBehind,
}: {
  statusAheadBehind: AheadBehindStatus | undefined;
}) => {
  const { mutateAsync: pull, isPending } = useGitPull();

  return (
    <>
      {statusAheadBehind?.behind && statusAheadBehind.behind > 0 ? (
        <Badge
          variant={"outline"}
          className="text-muted-foreground! h-full rounded-none border-0 border-r flex items-center font-normal tabular-nums px-1.5 cursor-pointer hover:bg-muted! border-b border-b-transparent hover:border-b-border"
          onClick={async () => {
            await pull();
          }}
        >
          {isPending ? <Loader2 className="animate-spin" /> : <ArrowDown />}
          <span className="tabular-nums text-foreground!">
            {statusAheadBehind.behind}{" "}
          </span>
          <span>behind</span>
        </Badge>
      ) : null}
    </>
  );
};

const OriginBadge = () => {
  const { selectedRepository } = useAppStore();

  if (!selectedRepository?.origin) {
    return null;
  }

  const origin = parseOrigin(selectedRepository?.origin);
  const icon = getAvatarByProvider(origin?.provider);

  return (
    <>
      {origin ? (
        <a target="_blank" href={origin.href} rel="noreferrer">
          <Badge
            variant={"outline"}
            className="h-full rounded-none px-2 flex items-center cursor-pointer hover:bg-muted! border-t-0"
          >
            <span className="flex items-center gap-1">
              <span className="text-muted-foreground! font-normal">
                Origin:{" "}
              </span>
              {icon}
              <span className="text-muted-foreground!">{origin.owner} / </span>
              <span className="text-foreground">{origin.repo}</span>
            </span>
          </Badge>
        </a>
      ) : null}
    </>
  );
};

const CurrentBranchBadge = () => {
  const { data: currentBranch } = useGetCurrentBranch();
  const { data: status } = useGetStatus();
  const navigation = useCommandNavigation();

  return (
    <>
      {currentBranch?.display_name ? (
        <Badge
          variant={"outline"}
          className="text-muted-foreground! h-full rounded-none hover:bg-muted! px-2 flex items-center font-normal cursor-pointer border-t-0 border-l-0 "
          onClick={() => {
            navigation.setOpen(true);
            navigation.push("branch-list");
          }}
        >
          <GitBranch />
          <span className="ml-1 text-foreground!">
            {currentBranch?.display_name}
            {status?.files && status?.files.length > 0 ? "*" : ""}
          </span>
        </Badge>
      ) : null}
    </>
  );
};

const InvalidateAllBadge = () => {
  const repo = appState.repository;

  return (
    <>
      {repo ? (
        <Badge
          variant={"outline"}
          className="text-muted-foreground! h-full rounded-none border-0 border-l px-2 flex items-center font-normal cursor-pointer hover:bg-muted! border-b border-b-transparent hover:border-b-border"
          onClick={async () => {
            await repo.invalidateAll();
          }}
        >
          <RotateCw />
        </Badge>
      ) : null}
    </>
  );
};

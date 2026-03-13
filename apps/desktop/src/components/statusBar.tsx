import {
  AheadBehindStatus,
  cancelCloneRepository,
  gitVersion,
} from "@gitru/commands";
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
import { cn } from "@gitru/ui/lib/utils";
import { getVersion } from "@tauri-apps/api/app";
import {
  ArrowDown,
  ArrowUp,
  CloudUpload,
  CopyPlus,
  File,
  GitBranch,
  GitCommitVertical,
  Loader2,
  RefreshCw,
  RotateCw,
  X,
} from "lucide-react";
import React from "react";
import {
  useCloneProgress,
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
    <div className="border-t overflow-hidden h-fit flex justify-between items-center">
      <div className="flex">
        <OriginBadge />
        <CurrentBranchBadge />
        <FetchBadge />
        <AheadBadge statusAheadBehind={statusAheadBehind || undefined} />
        <BehindBadge statusAheadBehind={statusAheadBehind || undefined} />
        <LastCommitBox />
      </div>
      <div className="flex">
        {/* <CloneProgressBadge /> */}
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
            className="py-2.5 rounded-none flex items-center cursor-pointer hover:bg-muted! border-transparent border-r-border"
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
        className="py-2.5 rounded-none border-0 border-l px-2 flex items-center"
      >
        <span>
          <span className="text-muted-foreground! font-normal">Channel: </span>
          <span className="text-foreground!">{import.meta.env.MODE} </span>
        </span>
      </Badge>
    );
  }
};

const CloneProgressBadge = () => {
  const { event, status } = useCloneProgress();
  const [isCancelling, setIsCancelling] = React.useState(false);

  const handleCancel = async () => {
    if (!event?.operationId) return;

    setIsCancelling(true);
    try {
      await cancelCloneRepository({ operationId: event.operationId });
    } catch (error) {
      console.error("Failed to cancel clone:", error);
    } finally {
      setIsCancelling(false);
    }
  };

  if (!event || status === "idle") {
    return null;
  }

  const percent = Math.round(event.percent ?? 0);
  const label =
    event.phase === "Error" || event.phase === "Cancelled"
      ? event.line || "Clone failed"
      : event.phase === "Finished"
        ? "Clone completed"
        : `${event.status || "Cloning"}${event.percent != null ? ` ${percent}%` : ""}`;

  return (
    <Tooltip>
      <TooltipTrigger>
        <Badge
          variant={
            event.phase === "Error" || event.phase === "Cancelled"
              ? "destructive"
              : "outline"
          }
          className="py-2.5 rounded-none border-0 border-l px-2 flex items-center gap-1.5"
        >
          {status === "running" ? (
            <Loader2 className="size-3.5 animate-spin" />
          ) : (
            <CopyPlus className="size-3.5" />
          )}
          <span className="text-xs tabular-nums">{label}</span>
          {status === "running" && (
            <button
              type="button"
              onClick={(e) => {
                e.stopPropagation();
                handleCancel();
              }}
              disabled={isCancelling}
              className="ml-1 hover:opacity-70 disabled:opacity-50"
              title="Cancel clone"
            >
              <X className="size-3.5" />
            </button>
          )}
        </Badge>
      </TooltipTrigger>
      <TooltipPopup side="top" align="end">
        <span className="max-w-96 break-all text-xs">
          {event.line || label}
        </span>
      </TooltipPopup>
    </Tooltip>
  );
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
          className="py-2.5 rounded-none border-0 border-l px-2 flex items-center"
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
      className="py-2.5 rounded-none border-0 border-l px-2 flex items-center"
    >
      <span className="text-muted-foreground! font-mono font-normal tabular-nums">
        v{version}
      </span>
    </Badge>
  );
};

const FetchBadge = () => {
  const { mutateAsync: fetch, isPending } = useGitFetch();
  const [spinState, setSpinState] = React.useState<
    "idle" | "spinning" | "completing"
  >("idle");
  const iconRef = React.useRef<SVGSVGElement | null>(null);

  React.useEffect(() => {
    if (isPending) {
      setSpinState("spinning");
      return;
    }

    setSpinState((prev) => (prev === "spinning" ? "completing" : "idle"));
  }, [isPending]);

  React.useLayoutEffect(() => {
    const icon = iconRef.current;

    if (!icon) {
      return;
    }

    if (spinState === "spinning") {
      icon.style.animation = "";
      icon.style.transition = "";
      icon.style.transform = "";
      return;
    }

    if (spinState !== "completing") {
      return;
    }

    const transform = window.getComputedStyle(icon).transform;
    let angle = 0;

    if (transform && transform !== "none") {
      const match = transform.match(/^matrix\((.+)\)$/);

      if (match?.[1]) {
        const values = match[1]
          .split(",")
          .map((value) => Number.parseFloat(value.trim()));

        if (values.length >= 2) {
          angle = (Math.atan2(values[1], values[0]) * 180) / Math.PI;
        }
      }
    }

    if (angle < 0) {
      angle += 360;
    }

    const remaining = angle === 0 ? 0 : 360 - angle;

    if (remaining < 0.1) {
      icon.style.animation = "";
      icon.style.transition = "";
      icon.style.transform = "";
      setSpinState("idle");
      return;
    }

    icon.style.animation = "none";
    icon.style.transition = "none";
    icon.style.transform = `rotate(${angle}deg)`;

    void icon.getBoundingClientRect();

    icon.style.transition = `transform ${(remaining / 360) * 0.8}s linear`;
    icon.style.transform = `rotate(${angle + remaining}deg)`;

    const handleTransitionEnd = (event: TransitionEvent) => {
      if (event.propertyName !== "transform") {
        return;
      }

      icon.style.animation = "";
      icon.style.transition = "";
      icon.style.transform = "";
      setSpinState("idle");
    };

    icon.addEventListener("transitionend", handleTransitionEnd, { once: true });

    return () => {
      icon.removeEventListener("transitionend", handleTransitionEnd);
    };
  }, [spinState]);

  return (
    <Badge
      variant={"outline"}
      className={cn(
        "py-2.5 text-muted-foreground! rounded-none px-2 flex items-center cursor-pointer hover:bg-muted! border-transparent border-r-border",
        isPending ? "pointer-events-none opacity-75" : "",
      )}
      onClick={async () => {
        if (!isPending) {
          await fetch();
        }
      }}
    >
      <RefreshCw
        ref={iconRef}
        className={
          spinState !== "idle" ? "animate-spin animation-duration-[0.8s]" : ""
        }
      />
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
              className="py-2.5 rounded-none border-0 border-r flex items-center font-normal tabular-nums px-1.5 cursor-pointer hover:bg-muted! border-b border-b-transparent hover:border-b-border"
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
              className="py-2.5 text-muted-foreground! rounded-none border-0 border-r flex items-center font-normal tabular-nums px-1.5 cursor-pointer hover:bg-muted! border-b border-b-transparent hover:border-b-border"
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
          className="py-2.5 text-muted-foreground! rounded-none border-0 border-r flex items-center font-normal tabular-nums px-1.5 cursor-pointer hover:bg-muted! border-b border-b-transparent hover:border-b-border"
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
  const selectedRepository = useAppStore((state) => state.selectedRepository);

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
            className="rounded-none py-2.5 px-2 flex items-center cursor-pointer hover:bg-muted! border-transparent border-r-border"
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

  const hasUnstaged = status?.files?.some((f) =>
    f.status.some((s) => s.startsWith("Worktree") || s === "Conflicted"),
  );
  const hasStaged = status?.files?.some((f) =>
    f.status.some((s) => s.startsWith("Index")),
  );
  const statusIndicator = `${hasUnstaged ? "*" : ""}${hasStaged ? "+" : ""}`;

  return (
    <>
      {currentBranch?.display_name ? (
        <Badge
          variant={"outline"}
          className="py-2.5 text-muted-foreground! rounded-none hover:bg-muted! px-2 flex items-center font-normal cursor-pointer border-transparent border-r-border"
          onClick={() => {
            navigation.setOpen(true);
            navigation.push("branch-list");
          }}
        >
          <GitBranch />
          <span className="ml-1 text-foreground!">
            {currentBranch?.display_name}
            {statusIndicator}
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
          className="py-2.5 text-muted-foreground! rounded-none border-0 border-l px-2 flex items-center font-normal cursor-pointer hover:bg-muted! border-b border-b-transparent hover:border-b-border"
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

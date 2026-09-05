import {
  type AheadBehindStatus,
  gitVersion,
  type RepoOperation,
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
  File,
  GitBranch,
  GitCommitVertical,
  Loader2,
  RefreshCw,
  RotateCw,
} from "lucide-react";
import React from "react";
import { useTabContext } from "@/context/tab-context-provider";
import {
  useGetCommitById,
  useGetCurrentBranch,
  useGetLastCommit,
  useGetRepoOperation,
  useGetStatus,
  useGetStatusAheadBehind,
  useGitFetch,
  useGitPull,
  useGitPush,
} from "@/hooks";
import { githubCommitterAvatarUrl } from "@/lib/external-content";
import { getAvatarByProvider } from "@/lib/get-avatar-by-git-provider";
import { openExternalUrlSafely } from "@/lib/open-external-url";
import { parseOrigin } from "@/lib/parse-origin";
import { timeAgoFromUnixSeconds } from "@/lib/time";
import { useActiveRepositoryState } from "@/state/use-active-repository-state";
import { selectActiveRepository, useAppStore } from "@/store/use-app-store";

const StatusBar = () => {
  const { data: statusAheadBehind } = useGetStatusAheadBehind();

  return (
    <div className="border-t overflow-hidden h-6 flex justify-between items-center">
      <div className="flex">
        <OriginBadge />
        <CurrentBranchBadge />
        <FetchBadge />
        <AheadBadge statusAheadBehind={statusAheadBehind || undefined} />
        <BehindBadge statusAheadBehind={statusAheadBehind || undefined} />
        <LastCommitBox />
        <RebaseBadge />
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
                      src={githubCommitterAvatarUrl(
                        lastCommit?.authors.author.email,
                      )}
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
                        src={githubCommitterAvatarUrl(coAuthor.email)}
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
                    src={githubCommitterAvatarUrl(
                      lastCommit?.authors.author.email,
                    )}
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
                      src={githubCommitterAvatarUrl(coAuthor.email)}
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

const GitVersion = () => {
  const { contextId } = useTabContext();
  const [version, setVersion] = React.useState<string>("");

  React.useEffect(() => {
    if (!contextId) {
      setVersion("");
      return;
    }

    gitVersion({ contextId }).then((v) => setVersion(v));
  }, [contextId]);

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

const RebaseBadge = () => {
  const { data: operation } = useGetRepoOperation();
  if (!operation?.isRebasing) return null;

  // The branch badge already says "(Rebasing)", so this one carries progress.
  const label =
    operation.current != null && operation.total != null
      ? `${operation.current}/${operation.total}`
      : "Rebasing";

  return (
    <Tooltip>
      <TooltipTrigger
        render={
          <Badge
            variant={"warning"}
            className="py-2.5 rounded-none border-0 border-r border-warning/30 px-2 flex items-center"
          />
        }
      >
        <RotateCw className="size-3" />
        <span className="ml-1 font-normal">{label}</span>
      </TooltipTrigger>
      <TooltipPopup side="top">
        {operation.label || "Repository is rebasing"}
        {operation.pauseReason ? ` · paused (${operation.pauseReason})` : ""}
      </TooltipPopup>
    </Tooltip>
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

  if (statusAheadBehind?.is_detached) {
    return null;
  }

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
          className="rounded-none py-2.5 px-2 flex items-center cursor-pointer hover:bg-muted! border-transparent border-r-border"
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
  const activeRepository = useAppStore(selectActiveRepository);

  if (!activeRepository?.origin) {
    return null;
  }

  const origin = parseOrigin(activeRepository?.origin);
  const icon = getAvatarByProvider(origin?.provider);

  return (
    <>
      {origin ? (
        <button
          type="button"
          onClick={() => {
            if (origin.href) void openExternalUrlSafely(origin.href);
          }}
        >
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
        </button>
      ) : null}
    </>
  );
};

/** Label for an in-progress operation, matching VS Code's `(Rebasing)` suffix. */
const operationLabel = (operation: RepoOperation | null | undefined) => {
  if (!operation) return null;
  if (operation.isRebasing) return "Rebasing";

  switch (operation.kind) {
    case "merge":
      return "Merging";
    case "cherryPick":
      return "Cherry Picking";
    case "revert":
      return "Reverting";
    case "bisect":
      return "Bisecting";
    default:
      return null;
  }
};

const CurrentBranchBadge = () => {
  const { data: currentBranch } = useGetCurrentBranch();
  const { data: status } = useGetStatus();
  const { data: operation } = useGetRepoOperation();
  const navigation = useCommandNavigation();

  const hasUnstaged = status?.files?.some((f) =>
    f.status.some((s) => s.startsWith("Worktree")),
  );
  const hasStaged = status?.files?.some((f) =>
    f.status.some((s) => s.startsWith("Index")),
  );
  const hasConflicts = status?.files?.some((f) =>
    f.status.some((s) => s === "Conflicted"),
  );
  const statusIndicator = `${hasUnstaged ? "*" : ""}${hasStaged ? "+" : ""}${
    hasConflicts ? "!" : ""
  }`;

  if (!currentBranch?.display_name) {
    return null;
  }

  const detached = currentBranch.is_detached;
  const label = operationLabel(operation);

  return (
    <Badge
      variant={"outline"}
      className="py-2.5 text-muted-foreground! rounded-none hover:bg-muted! px-2 flex items-center font-normal cursor-pointer border-transparent border-r-border"
      onClick={() => {
        navigation.setOpen(true);
        navigation.push("branch-list");
      }}
    >
      {detached ? <GitCommitVertical /> : <GitBranch />}
      <span className="ml-1 text-foreground!">
        {currentBranch.display_name}
        {statusIndicator}
      </span>
      {label ? (
        <span className="ml-1 text-muted-foreground!">({label})</span>
      ) : null}
    </Badge>
  );
};

const InvalidateAllBadge = () => {
  const repo = useActiveRepositoryState();

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

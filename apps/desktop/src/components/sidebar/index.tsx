import { Git, Inbox, Issue, PullRequest } from "@gitru/icon";
import {
  Avatar,
  AvatarFallback,
  AvatarImage,
} from "@gitru/ui/components/avatar";
import { Button } from "@gitru/ui/components/button";
import { ScrollArea } from "@gitru/ui/components/scroll-area";
import {
  Tooltip,
  TooltipPopup,
  TooltipProvider,
  TooltipTrigger,
} from "@gitru/ui/components/tooltip";
import { Download, Plus, RotateCcw } from "lucide-react";
import { useAppStore } from "@/store/use-app-store";
import SideBarItems from "./items";
import { useUpdateState } from "./update-state";

const CIRCLE_RADIUS = 12;
const CIRCLE_CIRCUMFERENCE = 2 * Math.PI * CIRCLE_RADIUS;

const Sidebar = () => {
  const updateChannel = useAppStore((s) => s.updateChannel);
  const {
    updateStatus,
    availableUpdate,
    downloadProgress,
    startDownloadAndInstall,
    restartApp,
  } = useUpdateState(updateChannel);

  const showUpdateAction =
    updateStatus === "available" ||
    updateStatus === "downloading" ||
    updateStatus === "downloaded";

  const progressPercent = downloadProgress?.percent ?? 0;
  const versionLabel = availableUpdate?.version ?? "latest";
  const channelLabel = availableUpdate?.channel ?? updateChannel;

  const updateTooltip =
    updateStatus === "downloaded"
      ? "Update installed. Restart app"
      : updateStatus === "downloading"
        ? `${Math.round(progressPercent)}% · ${versionLabel} · ${channelLabel}`
        : `Update ${versionLabel} on ${channelLabel}`;

  const handleUpdateAction = async () => {
    if (updateStatus === "available") {
      await startDownloadAndInstall();
      return;
    }

    if (updateStatus === "downloaded") {
      await restartApp();
    }
  };

  return (
    <div className="flex flex-col justify-between items-center -mr-2 ml-0.5">
      <div className="w-full _m-2 _mt-4 flex flex-col items-center">
        <TooltipProvider>
          <div className="flex flex-col items-center gap-1">
            <SideBarItems
              items={[
                {
                  icon: Inbox,
                  name: "Inbox",
                  href: "/app/inbox",
                  badge: "5",
                },
                {
                  icon: PullRequest,
                  name: "pull requests",
                  href: "/app/pulls",
                },
                {
                  icon: Issue,
                  name: "Issues",
                  href: "/app/issues",
                },
                {
                  icon: Git,
                  name: "Local Git",
                  href: "/app/git",
                },
              ]}
            />
            <div className="w-full flex justify-center items-center">
              <div className="my-1 h-px w-7 bg-muted-foreground/20" />
            </div>
            {/* // TODO: we will forward to do some kindof sortcuts */}
            <ScrollArea className="w-full max-h-[calc(100vh-3rem-4rem)]">
              <div className="flex flex-col items-center gap-1">
                {[
                  "legions-developer",
                  "ruru-m07",
                  "vercel",
                  "tauri-apps",
                  "aceternity",
                  "shadcn-ui",
                  "pierrecomputer",
                  "raycast",
                  "gitru-app",
                ].map((v) => (
                  <Button
                    className="size-8 p-0"
                    key={v}
                    variant="ghost"
                    size="icon"
                  >
                    <Avatar className="rounded-md size-7">
                      <AvatarImage
                        src={`https://github.com/${v}.png`}
                        alt={v}
                      />
                      <AvatarFallback></AvatarFallback>
                    </Avatar>
                  </Button>
                ))}
                <Button variant="outline" size="icon" className="size-8 p-0">
                  <Plus
                    className="opacity-60"
                    size={14}
                    strokeWidth={2}
                    aria-hidden="true"
                  />
                </Button>
              </div>
            </ScrollArea>

            {/* <div className="my-1 h-px w-full bg-border" /> */}
            {/* <SideBarItems
              items={[
                {
                  icon: Star,
                  href: "/starred",
                  name: "Starred",
                },
                {
                  icon: BookMarked,
                  href: "/repositories",
                  name: "Repositories",
                },
                {
                  icon: Settings,
                  href: "/settings",
                  name: "Settings",
                },
              ]}
            /> */}
          </div>
        </TooltipProvider>
      </div>

      <div className="flex flex-col items-center gap-1 pb-1">
        {showUpdateAction ? (
          <TooltipProvider>
            <Tooltip>
              <TooltipTrigger
                render={
                  <Button
                    variant={
                      updateStatus === "available" ||
                      updateStatus === "downloaded"
                        ? "default"
                        : "ghost"
                    }
                    size="icon-sm"
                    className="p-0 mb-1"
                    onClick={handleUpdateAction}
                    disabled={updateStatus === "downloading"}
                    aria-label={updateTooltip}
                  />
                }
              >
                {updateStatus === "downloading" ? (
                  <DownloadProgressIcon percent={progressPercent} />
                ) : updateStatus === "downloaded" ? (
                  <RotateCcw />
                ) : (
                  <Download />
                )}
              </TooltipTrigger>
              <TooltipPopup side="right">{updateTooltip}</TooltipPopup>
            </Tooltip>
          </TooltipProvider>
        ) : null}

        <Avatar className="rounded-md size-7">
          <AvatarImage alt="User" src="https://github.com/ruru-m07.png" />
          <AvatarFallback>AV</AvatarFallback>
        </Avatar>
        {/* <AvatarDropdown rateLimit={rateLimit} user={session?.user} /> */}
      </div>
    </div>
  );
};

export default Sidebar;

const DownloadProgressIcon = ({ percent }: { percent: number }) => {
  const clampedPercent = Math.max(0, Math.min(100, percent));
  const dashOffset =
    CIRCLE_CIRCUMFERENCE - (clampedPercent / 100) * CIRCLE_CIRCUMFERENCE;

  const progressPercent = Math.round(clampedPercent);

  return (
    <svg viewBox="0 0 28 28" aria-hidden="true" className="scale-170">
      <circle
        cx="14"
        cy="14"
        r={CIRCLE_RADIUS}
        fill="none"
        stroke="currentColor"
        strokeOpacity="0.2"
        strokeWidth="2"
      />
      <circle
        cx="14"
        cy="14"
        r={CIRCLE_RADIUS}
        fill="none"
        stroke="var(--color-primary)"
        strokeLinecap="round"
        strokeWidth="2"
        transform="rotate(-90 14 14)"
        strokeDasharray={CIRCLE_CIRCUMFERENCE}
        strokeDashoffset={dashOffset}
      />
      <text
        x="14"
        y="14"
        textAnchor="middle"
        dominantBaseline="middle"
        className="text-[11px] tabular-nums"
        fill="currentColor"
      >
        {progressPercent === 100 ? "99" : `${progressPercent}`}
      </text>
    </svg>
  );
};

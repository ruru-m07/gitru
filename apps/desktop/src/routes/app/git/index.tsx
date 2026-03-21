import type { FileStatusKind } from "@gitru/commands";
import { DiffViewer } from "@gitru/diff";
import { Button } from "@gitru/ui/components/button";
import { CopyButton } from "@gitru/ui/components/copy-button";
import { Group, GroupSeparator } from "@gitru/ui/components/group";
import { Kbd, KbdGroup } from "@gitru/ui/components/kbd";
import { Label } from "@gitru/ui/components/label";
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@gitru/ui/components/popover";
import { Separator } from "@gitru/ui/components/separator";
import { Switch } from "@gitru/ui/components/switch";
import { cn } from "@gitru/ui/lib/utils";
import { createFileRoute } from "@tanstack/react-router";
import {
  ArrowUpFromLine,
  ChevronDown,
  ChevronsUp,
  CircleAlertIcon,
  Diff,
  GitBranch,
  Loader2,
  MoveHorizontal,
  Settings,
  TextWrap,
  X,
} from "lucide-react";
// import { useTheme } from "next-themes";
import { ImageDiffViewer } from "@/components/diff/image/ImageDiffViewer";
import { useDiffViewerSettings } from "@/components/diff/useDiffViewSettingStore";
import { getStatusIcon } from "@/components/getStatusIcon";
import HistoryGraph from "@/components/historyGraph";
import LoaderIndicator from "@/components/loaderIndicator";
import { GitruBorderedSVG } from "@/components/svgs/gitru-borderd";
import {
  useGetCommitById,
  useGetCurrentBranch,
  useGetCurrentBranchStash,
  useGetDiff,
  useGetDiffBlame,
  useGetStatus,
  useGetStatusAheadBehind,
  useGitPush,
  useStashList,
  useStashShow,
} from "@/hooks";
import { useAppStore } from "@/store/useAppStore";
import { SplitSVG } from "../../../components/svgs/splitSVG";
import { UnifiedSVG } from "../../../components/svgs/unifiedSVG";
import {
  type ResolvedFileSelection,
  resolveFileSelection,
} from "../../../lib/gitSelectionResolver";

export const Route = createFileRoute("/app/git/")({
  component: App,
});

function App() {
  const mainWindowView = useAppStore((state) => state.mainWindowView);

  return (
    <>
      <MainActionBar />
      {mainWindowView === null && <EmptyStateScreen />}
      {mainWindowView === "FileDiff" && <DiffBoxBody />}
      {mainWindowView === "HistoryGraph" && <HistoryGraph />}
    </>
  );
}

const DiffBoxBody = () => {
  const selectedRepository = useAppStore((state) => state.selectedRepository);
  const selectionByRepo = useAppStore((state) => state.selectionByRepo);
  const gitViewByRepo = useAppStore((state) => state.gitViewByRepo);
  const { data: status } = useGetStatus();
  const { data: currentBranchStash } = useGetCurrentBranchStash();
  const { data: stashes } = useStashList();

  const repoPath = selectedRepository?.path ?? "";
  const gitViewState = gitViewByRepo[repoPath];
  const activeSource =
    gitViewState?.leftPanelView === "stash"
      ? "stash"
      : gitViewState?.leftPanelView === "history"
        ? "history"
        : "worktree";
  const activeStashReference =
    activeSource === "stash"
      ? gitViewState?.stashViewMode === "branch"
        ? (currentBranchStash?.reference ?? null)
        : (gitViewState?.selectedStashReference ?? null)
      : null;
  const activeHistoryCommitHash =
    activeSource === "history"
      ? (gitViewState?.selectedHistoryCommitHash ?? null)
      : null;

  const { data: stashShow } = useStashShow(activeStashReference);
  const { data: historyCommit } = useGetCommitById(
    activeHistoryCommitHash ?? "",
  );

  const activeSelection =
    activeSource === "stash"
      ? activeStashReference
        ? (selectionByRepo[repoPath]?.stashByReference[activeStashReference] ??
          null)
        : null
      : activeSource === "history"
        ? activeHistoryCommitHash
          ? (selectionByRepo[repoPath]?.historyByCommit?.[
              activeHistoryCommitHash
            ] ?? null)
          : null
        : (selectionByRepo[repoPath]?.worktree ?? null);

  const resolvedSelection = resolveFileSelection({
    selection: activeSelection,
    files:
      activeSource === "stash"
        ? (stashShow?.files ?? [])
        : activeSource === "history"
          ? (historyCommit?.files ?? [])
          : (status?.files ?? []),
    context: {
      source: activeSource,
      stashReference: activeStashReference,
      availableStashReferences: (stashes ?? []).map((stash) => stash.reference),
      historyCommitHash: activeHistoryCommitHash,
    },
  });

  return (
    <>
      {resolvedSelection.state === "valid" ? (
        <>
          <FileLevelStatusBar resolvedSelection={resolvedSelection} />
          <DiffArea
            filePath={resolvedSelection.file.path}
            fileNewPath={resolvedSelection.file.new_path ?? null}
            status={resolvedSelection.file.status}
            stashReference={
              resolvedSelection.identity.source === "stash"
                ? (resolvedSelection.identity.stashReference ?? null)
                : null
            }
            commitHash={
              resolvedSelection.identity.source === "history"
                ? (resolvedSelection.identity.historyCommitHash ?? null)
                : null
            }
          />
        </>
      ) : (
        <EmptyStateScreen />
      )}
    </>
  );
};

const MainActionBar = () => {
  const { data: currentBranch } = useGetCurrentBranch();
  const { data: statusAheadBehind } = useGetStatusAheadBehind();

  const { mutateAsync: push, isPending } = useGitPush();

  return (
    <div className="w-full justify-between min-h-14 max-h-14 h-14 border-b flex">
      <div className="min-h-14 max-h-14 h-14 flex w-full">
        <Button
          className="flex justify-between items-center min-h-full rounded-none border-x-0 max-w-72 w-full"
          variant="ghost"
        >
          <div className="flex items-center gap-4 min-w-0 flex-1">
            <GitBranch className="size-7.5" strokeWidth={1.5} />

            <div className="flex flex-col items-start min-w-0 flex-1">
              <span className="text-xs text-muted-foreground font-normal">
                Current Branch
              </span>
              <span className="truncate block w-full text-left">
                {currentBranch?.display_name}
              </span>
            </div>
          </div>

          <ChevronDown size={18} />
        </Button>
        <Separator orientation="vertical" className={"border-0"} />
        {statusAheadBehind && statusAheadBehind.is_published ? (
          (statusAheadBehind && statusAheadBehind.ahead > 0) ||
          (statusAheadBehind && statusAheadBehind.behind > 0) ? (
            <>
              <Button
                className="flex justify-between items-center min-h-full rounded-none border-x-0 w-72"
                variant={"ghost"}
                onClick={async () => {
                  await push();
                }}
              >
                <div className="flex items-center justify-center gap-4">
                  {isPending ? (
                    <Loader2
                      className="animate-spin size-7.5"
                      strokeWidth={1.5}
                    />
                  ) : (
                    <ChevronsUp className="size-8" />
                  )}
                  <div className="flex-col flex items-start">
                    <span className="text-xs text-muted-foreground font-normal">
                      {statusAheadBehind.ahead > 0
                        ? "Push to Remote"
                        : "Pull from Remote"}
                    </span>
                    <span>
                      {statusAheadBehind
                        ? `${statusAheadBehind.ahead} / ${statusAheadBehind.behind}`
                        : "0 / 0"}
                    </span>
                  </div>
                </div>
                <ChevronDown size={18} />
              </Button>
              <Separator orientation="vertical" className={"border-0"} />
            </>
          ) : null
        ) : (
          <>
            <Button
              className="flex border-x-0 justify-between items-center min-h-full rounded-none max-w-60 w-60"
              variant={"ghost"}
              onClick={async () => {
                await push();
              }}
            >
              <div className="flex items-center gap-4 min-w-0 flex-1">
                {isPending ? (
                  <Loader2
                    className="animate-spin size-7.5"
                    strokeWidth={1.5}
                  />
                ) : (
                  <ArrowUpFromLine className="size-7.5" strokeWidth={1.5} />
                )}
                <div className="flex flex-col flex-1 items-start min-w-0">
                  <span className="text-xs text-muted-foreground font-normal">
                    Publish Branch
                  </span>
                  <span className="truncate block w-full text-left">
                    Published as {currentBranch?.name}
                  </span>
                </div>
              </div>
            </Button>
            <Separator orientation="vertical" className={"border-0"} />
            <Button
              className="flex border-x-0 justify-between items-center min-h-full rounded-none"
              variant={"ghost"}
              onClick={async () => {}}
            >
              <ChevronDown size={18} />
            </Button>
            <Separator orientation="vertical" className={"border-0"} />
          </>
        )}
      </div>
      <div></div>
    </div>
  );
};

const FileLevelStatusBar = ({
  resolvedSelection,
}: {
  resolvedSelection: ResolvedFileSelection;
}) => {
  const clearWorktreeSelectionForRepo = useAppStore(
    (state) => state.clearWorktreeSelectionForRepo,
  );
  const clearStashSelectionForRepo = useAppStore(
    (state) => state.clearStashSelectionForRepo,
  );
  const clearHistorySelectionForRepo = useAppStore(
    (state) => state.clearHistorySelectionForRepo,
  );
  const setMainWindowView = useAppStore((state) => state.setMainWindowView);

  return (
    <div className="w-full h-9.25 border-b flex justify-between items-center">
      <FileLevelStatusBarLeft resolvedSelection={resolvedSelection} />
      <div className="flex items-center gap-2 pr-2">
        <Button
          size="icon-xs"
          variant="outline"
          className="relative"
          aria-label="Open notifications"
          onClick={() => {
            setMainWindowView(null);
            if (resolvedSelection.state === "none") {
              return;
            }

            const selection = resolvedSelection.identity;
            if (selection.source === "stash" && selection.stashReference) {
              clearStashSelectionForRepo(selection.stashReference);
              return;
            }

            if (selection.source === "history" && selection.historyCommitHash) {
              clearHistorySelectionForRepo(selection.historyCommitHash);
              return;
            }

            clearWorktreeSelectionForRepo();
          }}
        >
          <X />
        </Button>

        <SettingsPopover />
      </div>
    </div>
  );
};

const DiffArea = ({
  filePath,
  fileNewPath,
  status,
  stashReference,
  commitHash,
}: {
  filePath: string;
  fileNewPath: string | null;
  status: FileStatusKind[];
  stashReference: string | null;
  commitHash: string | null;
}) => {
  const { data: diffData, isLoading } = useGetDiff(filePath, {
    fileNewPath,
    status,
    stashReference,
    commitHash,
    parentIndex: commitHash ? 1 : undefined,
  });
  const { data: blameData } = useGetDiffBlame(filePath, {
    fileNewPath,
    status,
    stashReference,
    commitHash,
    parentIndex: commitHash ? 1 : undefined,
  });

  // const { diffStyle, overflow } = useDiffViewerSettings();
  // const { theme } = useTheme();
  const assetKind = String(diffData?.asset_diff?.kind ?? "").toLowerCase();
  const isImageAssetDiff = assetKind === "image";
  const imageAssetDiff = isImageAssetDiff
    ? (diffData?.asset_diff ?? null)
    : null;

  return (
    <div
      className={cn(
        "bg-background max-h-[calc(100vh-calc(var(--spacing)*14)-calc(var(--spacing)*9)-calc(var(--spacing)*12)-calc(var(--spacing)*6))] h-full w-full relative overflow-y-auto _bg-[color-mix(in_oklab,var(--color-secondary)_70%,var(--color-background))]",
      )}
    >
      {isLoading ? (
        <div className="p-2.5">
          <LoaderIndicator />
        </div>
      ) : (
        <>
          {imageAssetDiff ? <ImageDiffViewer diff={imageAssetDiff} /> : null}
          {diffData?.patch && !isImageAssetDiff && (
            // <div className="relative w-full flex overflow-auto py-1">
            //   <DiffViewer
            //     patch={diffData?.patch}
            //     options={{
            //       maxChangeRatio: 0.45,
            //       maxDiffDistance: 1,
            //       inlineMaxCharEdits: 0,
            //       mergeModifiedLines: true,
            //     }}
            //   />
            // </div>
            <>
              <div className="relative! w-full flex overflow-auto py-1">
                <DiffViewer
                  patch={diffData?.patch}
                  options={{
                    maxChangeRatio: 0.45,
                    maxDiffDistance: 1,
                    inlineMaxCharEdits: 0,
                    mergeModifiedLines: true,
                  }}
                  oldBlame={blameData?.oldBlame}
                  newBlame={blameData?.newBlame}
                />
              </div>
              {/* <span
                onClick={() => {
                  navigator.clipboard.writeText(diffData.patch);
                }}
              >
                {diffData.patch.split("\n").length} lines of patch
              </span>
              <br />
              <span
                onClick={() => {
                  navigator.clipboard.writeText(
                    JSON.stringify(blameData?.oldBlame),
                  );
                }}
              >
                {blameData?.oldBlame?.length}
              </span>
              <br />
              <span
                onClick={() => {
                  navigator.clipboard.writeText(
                    JSON.stringify(blameData?.newBlame),
                  );
                }}
              >
                {blameData?.newBlame?.length}
              </span> */}
            </>
          )}
        </>
      )}
    </div>
  );
};

const FileLevelStatusBarLeft = ({
  resolvedSelection,
}: {
  resolvedSelection: ResolvedFileSelection;
}) => {
  if (resolvedSelection.state === "none") {
    return null;
  }

  const selectedFile =
    resolvedSelection.state === "valid"
      ? {
          filePath: resolvedSelection.file.path,
          fileNewPath: resolvedSelection.file.new_path,
          status: resolvedSelection.file.status,
        }
      : {
          filePath: resolvedSelection.identity.filePath,
          fileNewPath: resolvedSelection.identity.fileNewPath,
          status: undefined,
        };

  return (
    <div className="items-center h-full px-2 flex gap-2">
      {selectedFile?.status && selectedFile?.filePath ? (
        <>
          {getStatusIcon(selectedFile?.status)}
          <span className="group flex items-center">
            {renderPath(selectedFile?.filePath)}
            <div className="ml-1 opacity-0 group-hover:opacity-100 transition-opacity text-xs text-muted-foreground">
              <CopyButton
                size={"xs"}
                variant="ghost"
                text={selectedFile?.filePath || ""}
              />
            </div>
          </span>
        </>
      ) : null}
      {resolvedSelection.state === "stale" ? (
        <span className="text-xs text-amber-600 flex items-center gap-1">
          <CircleAlertIcon size={14} />
          Unavailable
        </span>
      ) : null}
      {selectedFile?.fileNewPath ? (
        <div>
          <MoveHorizontal
            className="text-muted-foreground opacity-70"
            size={16}
          />
        </div>
      ) : null}
      {selectedFile?.fileNewPath ? (
        <span>{renderPath(selectedFile.fileNewPath)}</span>
      ) : null}
    </div>
  );
};

const renderPath = (path: string) => {
  const parts = path.split("/");
  const fileName = parts.pop();
  const dir = parts.join("/");

  return (
    <span>
      {dir && (
        <>
          <span className="text-muted-foreground/75">{dir}/</span>
        </>
      )}
      {fileName}
    </span>
  );
};

const SettingsPopover = () => {
  return (
    <Popover>
      <PopoverTrigger
        render={
          <Button
            size="icon-xs"
            variant="outline"
            className="relative"
            aria-label="Open notifications"
          />
        }
      >
        <Settings size={16} aria-hidden="true" />
      </PopoverTrigger>
      <SettingsPopoverContent />
    </Popover>
  );
};

const SettingsPopoverContent = () => {
  const { setDiffStyle, diffStyle, overflow, setOverflow } =
    useDiffViewerSettings();

  return (
    <PopoverContent className="fit mr-4 px-0 mt-0.5 w-96">
      <Label className="text-muted-foreground">Settings</Label>
      <Separator className={"mt-2 mb-3"} />
      <div className="flex flex-col gap-4 mt-1">
        <div>
          <Label className="flex items-center gap-2 mb-3">
            <Diff size={16} />
            Diff Style
          </Label>
          <div className="flex items-center justify-center ">
            <Group className="flex w-full h-full">
              <Button
                className={cn(
                  "rounded-none w-32 h-32! shadow-none first:rounded-s-md last:rounded-e-md focus-visible:z-10",
                  diffStyle === "unified" &&
                    "border-primary/40 bg-primary/10! hover:bg-primary/13!",
                )}
                variant="outline"
                onClick={() => {
                  setDiffStyle("unified");
                }}
              >
                <UnifiedSVG />
              </Button>
              <GroupSeparator className="bg-primary/40" />
              <Button
                className={cn(
                  "rounded-none w-32 h-32! shadow-none rounded-r-md border-l-0 focus-visible:z-10",
                  diffStyle === "split" &&
                    "border-primary/40 bg-primary/10! hover:bg-primary/13!",
                )}
                variant="outline"
                onClick={() => {
                  setDiffStyle("split");
                }}
              >
                <SplitSVG />
              </Button>
            </Group>
          </div>
        </div>
        <Separator />
        <div className="flex justify-between">
          <Label htmlFor="wrapping" className="flex items-center gap-2">
            <TextWrap size={16} />
            Wrapping
          </Label>
          <Switch
            checked={overflow === "wrap"}
            onCheckedChange={(checked) => {
              setOverflow(checked ? "wrap" : "scroll");
            }}
            id="wrapping"
          />
        </div>
      </div>
    </PopoverContent>
  );
};

const EmptyStateScreen = () => {
  return (
    <div className="w-full flex justify-center h-full bg-background border-r">
      <div className="w-full h-full flex flex-col items-center justify-center -mt-20">
        <GitruBorderedSVG />
        <div className="flex flex-col gap-0.5 w-60 select-none">
          <div className="flex items-center justify-between">
            <span className="text-muted-foreground text-sm font-light">
              Command Pannel
            </span>
            <span>
              <KbdGroup>
                <Kbd>⌘</Kbd>
                <Kbd>K</Kbd>
              </KbdGroup>
            </span>
          </div>
          <div className="flex items-center justify-between">
            <span className="text-muted-foreground text-sm font-light">
              New Branch
            </span>
            <span>
              <KbdGroup>
                <Kbd>⌘</Kbd>
                <Kbd>⇧</Kbd>
                <Kbd>N</Kbd>
              </KbdGroup>
            </span>
          </div>
          <div className="flex items-center justify-between">
            <span className="text-muted-foreground text-sm font-light">
              Pull Changes
            </span>
            <span>
              <KbdGroup>
                <Kbd>⌘</Kbd>
                <Kbd>⇧</Kbd>
                <Kbd>P</Kbd>
              </KbdGroup>
            </span>
          </div>
        </div>
      </div>
    </div>
  );
};

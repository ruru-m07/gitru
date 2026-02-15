import { DiffViewer } from "@gitru/diff";
import { Button } from "@gitru/ui/components/button";
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
  Diff,
  GitBranch,
  MoveHorizontal,
  Settings,
  TextWrap,
  X,
} from "lucide-react";
import { useDiffViewerSettings } from "@/components/diff/useDiffViewSettingStore";
import { getStatusIcon } from "@/components/getStatusIcon";
import HistoryGraph from "@/components/historyGraph";
import LoaderIndicator from "@/components/loaderIndicator";
import { GitruBorderedSVG } from "@/components/svgs/gitru-borderd";
import {
  useGetCurrentBranch,
  useGetDiff,
  useGetStatusAheadBehind,
  useInvalidateAll,
} from "@/hooks";
import { SelectedFile, useAppStore } from "@/store/useAppStore";
import { SplitSVG } from "../../../components/svgs/splitSVG";
import { UnifiedSVG } from "../../../components/svgs/unifiedSVG";

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
  const selectedFileByRepo = useAppStore((state) => state.selectedFileByRepo);

  const selectedFile = selectedFileByRepo[selectedRepository?.path || ""];

  return (
    <>
      {selectedFile?.filePath ? (
        <>
          <FileLevelStatusBar selectedFile={selectedFile} />
          <DiffArea selectedFile={selectedFile} />
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

  const { mutateAsync: invalidateAll } = useInvalidateAll();

  return (
    <div className="w-full justify-between min-h-14 max-h-14 h-14 border-b flex">
      <div className="min-h-14 max-h-14 h-14 flex">
        <Button
          className="flex justify-between items-center h-full rounded-none border-x-0 min-w-72"
          variant={"ghost"}
        >
          <div className="flex items-center justify-center gap-4">
            <GitBranch className="size-6" />
            <div className="flex-col flex items-start">
              <span className="text-xs text-muted-foreground font-normal">
                Current Branch
              </span>
              <span>{currentBranch?.display_name}</span>
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
                className="flex justify-between items-center h-full rounded-none border-x-0 w-72"
                variant={"ghost"}
                onClick={async () => {
                  await invalidateAll();
                }}
              >
                <div className="flex items-center justify-center gap-4">
                  <ChevronsUp className="size-8" />
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
              className="flex border-x-0 justify-between items-center h-full rounded-none min-w-60"
              variant={"ghost"}
              onClick={async () => {
                await invalidateAll();
              }}
            >
              <div className="flex items-center justify-center gap-4">
                <ArrowUpFromLine className="size-7" />
                <div className="flex-col flex items-start">
                  <span className="text-xs text-muted-foreground font-normal">
                    Published Branch
                  </span>
                  <span>Published as {currentBranch?.name}</span>
                </div>
              </div>
            </Button>
            <Separator orientation="vertical" className={"border-0"} />
            <Button
              className="flex border-x-0 justify-between items-center h-full rounded-none"
              variant={"ghost"}
              onClick={async () => {
                await invalidateAll();
              }}
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
  selectedFile,
}: {
  selectedFile: SelectedFile;
}) => {
  return (
    <div className="w-full h-9.25 border-b flex justify-between items-center">
      <FileLevelStatusBarLeft selectedFile={selectedFile} />
      <SettingsPopover />
    </div>
  );
};

const DiffArea = ({ selectedFile }: { selectedFile: SelectedFile }) => {
  const { data: diffData, isLoading } = useGetDiff(
    selectedFile?.filePath || null,
  );
  const { diffStyle, overflow } = useDiffViewerSettings();

  return (
    <div
      className={cn(
        "max-h-[calc(100vh-calc(var(--spacing)*14)-calc(var(--spacing)*9)-calc(var(--spacing)*12)-calc(var(--spacing)*7))] w-full relative overflow-y-auto pl-px",
      )}
    >
      {isLoading ? (
        <div className="p-2.5">
          <LoaderIndicator />
        </div>
      ) : (
        <>
          {diffData?.patch && (
            <DiffViewer
              patch={diffData?.patch}
              options={{
                diffStyle,
                overflow,
                disableFileHeader: true,
                theme: {
                  dark: "vesper",
                  light: "vesper-light",
                },
              }}
            />
          )}
        </>
      )}
    </div>
  );
};

const FileLevelStatusBarLeft = ({
  selectedFile,
}: {
  selectedFile: SelectedFile;
}) => {
  if (!selectedFile) {
    return null;
  }

  return (
    <div className="items-center h-full px-2 flex gap-2">
      {selectedFile?.status && selectedFile?.filePath ? (
        <>
          {getStatusIcon(selectedFile?.status)}
          <span className="flex items-center">
            {renderPath(selectedFile?.filePath)}
          </span>
        </>
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
    <span className="flex items-center">
      {dir && <span className="text-muted-foreground/75">{dir}/</span>}
      <span>{fileName}</span>
    </span>
  );
};

const SettingsPopover = () => {
  const setSelectedFileForRepo = useAppStore(
    (state) => state.setSelectedFileForRepo,
  );
  const setMainWindowView = useAppStore((state) => state.setMainWindowView);

  return (
    <div>
      <Button
        size="icon"
        variant="ghost"
        className="relative"
        aria-label="Open notifications"
        onClick={() => {
          setMainWindowView(null);
          setSelectedFileForRepo({
            filePath: undefined,
            status: undefined,
            fileNewPath: undefined,
          });
        }}
      >
        <X />
      </Button>
      <Popover>
        <PopoverTrigger
          render={
            <Button
              size="icon"
              variant="ghost"
              className="relative"
              aria-label="Open notifications"
            />
          }
        >
          <Settings size={16} aria-hidden="true" />
        </PopoverTrigger>
        <SettingsPopoverContent />
      </Popover>
    </div>
  );
};

const SettingsPopoverContent = () => {
  const { setDiffStyle, diffStyle, overflow, setOverflow } =
    useDiffViewerSettings();

  return (
    <PopoverContent className="fit mr-4 px-0 mt-0.5">
      <Label className="text-muted-foreground">Settings</Label>
      <Separator className={"mt-2 mb-3"} />
      <div className="flex flex-col gap-4 mt-1">
        <div>
          <Label className="flex items-center gap-2 mb-3">
            <Diff size={16} />
            Diff Style
          </Label>
          <div className="flex items-center justify-center ">
            <Group className="flex">
              <Button
                className={cn(
                  "rounded-none w-32 h-32 shadow-none first:rounded-s-md last:rounded-e-md focus-visible:z-10",
                  diffStyle === "unified" &&
                    "border-primary/40 bg-primary/10! hover:bg-primary/13!",
                )}
                variant="outline"
                size="icon"
                onClick={() => {
                  setDiffStyle("unified");
                }}
              >
                <UnifiedSVG />
              </Button>
              <GroupSeparator className="bg-primary/40" />
              <Button
                className={cn(
                  "rounded-none w-32 h-32 shadow-none rounded-r-md border-l-0 focus-visible:z-10",
                  diffStyle === "split" &&
                    "border-primary/40 bg-primary/10! hover:bg-primary/13!",
                )}
                variant="outline"
                size="icon"
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
        <div className="flex flex-col gap-0.5 w-60">
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

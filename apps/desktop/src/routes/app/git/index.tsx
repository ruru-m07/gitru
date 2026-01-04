import { Button } from "@gitru/ui/components/button";
import { Group, GroupSeparator } from "@gitru/ui/components/group";
import { Label } from "@gitru/ui/components/label";
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@gitru/ui/components/popover";
import { Radio, RadioGroup } from "@gitru/ui/components/radio-group";
import { ScrollArea } from "@gitru/ui/components/scroll-area";
import { Separator } from "@gitru/ui/components/separator";
import { Switch } from "@gitru/ui/components/switch";
import { cn } from "@gitru/ui/lib/utils";
import { LineDiffTypes, MultiFileDiff } from "@pierre/diffs/react";
import { createFileRoute } from "@tanstack/react-router";
import {
  ChevronDown,
  ChevronsUp,
  Diff,
  GitBranch,
  MoveHorizontal,
  Settings,
  TextWrap,
} from "lucide-react";
import { useTheme } from "next-themes";
import { useDiffViewerSettings } from "@/components/diff/useDiffViewSettingStore";
import { useDiffViewStore } from "@/components/diff/useDiffViewStore";
import { getStatusIcon } from "@/components/getStatusIcon";
import { useGetCurrentBranch, useGetDiff } from "@/hooks";
import { EmptyGitDiffSVG } from "../../../components/svgs/EmptyGitDiffSVG";
import { SplitSVG } from "../../../components/svgs/splitSVG";
import { UnifiedSVG } from "../../../components/svgs/unifiedSVG";

export const Route = createFileRoute("/app/git/")({
  component: App,
});

function App() {
  return (
    <>
      <MainActionBar />
      <DiffBoxBody />
    </>
  );
}

const DiffBoxBody = () => {
  const { selectedFilePath, selectedFileStatus } = useDiffViewStore();

  return (
    <>
      {selectedFilePath && selectedFileStatus ? (
        <>
          <FileLevelStatusBar />
          <DiffArea />
        </>
      ) : (
        <>
          <div className="w-full flex items-center justify-center h-full bg-background">
            <div className="w-full h-[85%] flex flex-col items-center justify-center">
              <EmptyGitDiffSVG />
              <span className="text-muted-foreground text-base">
                Select a file to see the changes
              </span>
            </div>
          </div>
        </>
      )}
    </>
  );
};

const MainActionBar = () => {
  const { data: currentBranch } = useGetCurrentBranch();

  return (
    <div className="w-full min-h-14 max-h-14 h-14 border-b flex">
      <Button
        className="flex border-0 border-t border-t-transparent hover:border-t-border justify-between items-center h-full rounded-none min-w-72"
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
      <Button
        className="flex border-0 border-t border-t-transparent hover:border-t-border justify-between items-center h-full rounded-none w-72"
        variant={"ghost"}
      >
        <div className="flex items-center justify-center gap-4">
          <ChevronsUp className="size-8" />
          <div className="flex-col flex items-start">
            <span className="text-xs text-muted-foreground font-normal">
              ruru/fix/whatever/sucks
            </span>
            <span>Push 3 Commits</span>
          </div>
        </div>
        <ChevronDown size={18} />
      </Button>
      <Separator orientation="vertical" className={"border-0"} />
    </div>
  );
};

const FileLevelStatusBar = () => {
  return (
    <div className="w-full h-9.25 border-b flex justify-between items-center">
      <FileLevelStatusBarLeft />
      <SettingsPopover />
    </div>
  );
};

const DiffArea = () => {
  const { selectedFilePath } = useDiffViewStore();
  const { diffStyle, overflow, lineDiffType } = useDiffViewerSettings();
  const { data: diffData } = useGetDiff(selectedFilePath?.path || null);
  const { theme } = useTheme();

  return (
    <ScrollArea
      className={cn(
        "h-[calc(100vh-calc(var(--spacing)*14)-calc(var(--spacing)*9)-calc(var(--spacing)*12)-calc(var(--spacing)*7))] w-full relative",
      )}
      scrollFade
    >
      {/* {diffData?.patch ? (
        <PatchDiff
          patch={diffData.patch}
          options={{
            disableFileHeader: true,
            theme: { dark: "vesper", light: "vesper-light" },
            themeType: theme?.startsWith("dark-") ? "dark" : "light",
            diffStyle,
            overflow,
            lineDiffType,
            unsafeCSS: `
              pre {
                --diffs-light-bg: transparent !important;
              }
            `,
          }}
        />
      ) : null} */}
      {diffData ? (
        <MultiFileDiff
          newFile={{
            contents: diffData.workdir?.content || "",
            name: diffData.file_path,
          }}
          oldFile={{
            contents: diffData.head?.content || "",
            name: diffData.file_path,
          }}
          options={{
            disableFileHeader: true,
            theme: { dark: "vesper", light: "vesper-light" },
            themeType: theme?.startsWith("dark-") ? "dark" : "light",
            diffStyle,
            overflow,
            lineDiffType,
            unsafeCSS: `
              pre {
                // --diffs-light-bg: transparent !important;
              }
            `,
          }}
        />
      ) : null}
    </ScrollArea>
  );
};

const FileLevelStatusBarLeft = () => {
  const { selectedFilePath, selectedFileStatus } = useDiffViewStore();

  if (!selectedFilePath || !selectedFileStatus) {
    return null;
  }

  return (
    <div className="items-center h-full px-2 flex gap-2">
      {getStatusIcon(selectedFileStatus)}
      <span className="flex items-center">
        <span className="text-muted-foreground/75">
          {selectedFilePath?.path?.slice(
            0,
            selectedFilePath?.path?.lastIndexOf("/"),
          )}
          /
        </span>
        <span>{selectedFilePath?.path?.split("/").pop()}</span>
      </span>
      {selectedFilePath?.newPath ? (
        <div>
          <MoveHorizontal
            className="text-muted-foreground opacity-70"
            size={16}
          />
        </div>
      ) : null}
      {selectedFilePath?.newPath ? (
        <span className="flex items-center">
          <span className="text-muted-foreground/75">
            {selectedFilePath?.newPath?.slice(
              0,
              selectedFilePath?.newPath?.lastIndexOf("/"),
            )}
            /
          </span>
          <span>{selectedFilePath?.newPath?.split("/").pop()}</span>
        </span>
      ) : null}
    </div>
  );
};

const SettingsPopover = () => {
  return (
    <div>
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
  const {
    setDiffStyle,
    diffStyle,
    overflow,
    setOverflow,
    lineDiffType,
    setLineDiffType,
  } = useDiffViewerSettings();

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
                    "border-primary/40 bg-primary/10 hover:bg-primary/13!",
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
                    "border-primary/40 bg-primary/10 hover:bg-primary/13!",
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
        <div>
          <Label>Line Diff Type</Label>
          <RadioGroup
            defaultValue={lineDiffType}
            className="gap-2 mt-1 "
            onValueChange={(v) => {
              setLineDiffType(v as LineDiffTypes);
            }}
          >
            <Label>
              <Radio value="word-alt" />
              Word-Alt
            </Label>
            <Label>
              <Radio value="word" />
              Word
            </Label>
            <Label>
              <Radio value="char" />
              Char
            </Label>
            <Label>
              <Radio value="none" />
              None
            </Label>
          </RadioGroup>
        </div>
      </div>
    </PopoverContent>
  );
};

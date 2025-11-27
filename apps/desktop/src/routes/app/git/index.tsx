import { Button } from "@gitru/ui/components/button";
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@gitru/ui/components/popover";
import { ScrollArea } from "@gitru/ui/components/scroll-area";
import { cn } from "@gitru/ui/lib/utils";
import { createFileRoute } from "@tanstack/react-router";
import {
  ChevronDown,
  ChevronsUp,
  GitBranch,
  MoveHorizontal,
  Settings,
} from "lucide-react";
import { useEffect, useState } from "react";
import { DiffViewer } from "@/components/diff/diff-viewer";
import { useDiffViewStore } from "@/components/diff/useDiffViewStore";
import { useFileDiff, useGitRepository } from "@/lib/git";
import { useAppStore } from "@/store/useAppStore";
import {
  currentBranch,
  type GetDiffResponse,
  getDiff,
  listBranch,
} from "@/tauri";
import { EmptyGitDiffSVG } from "../../../components/svgs/EmptyGitDiffSVG";
import { SplitSVG } from "../../../components/svgs/splitSVG";
import { UnifiedSVG } from "../../../components/svgs/unifiedSVG";
import { getStatusIcon } from "./route";

export const Route = createFileRoute("/app/git/")({
  component: App,
});

function App() {
  const { selectedFilePath, selectedFileStatus, setViewMode } =
    useDiffViewStore();
  const { selectedRepository } = useAppStore();

  const repo = useGitRepository(
    selectedRepository?.path || null,
    selectedRepository?.name,
  );

  const { diff } = useFileDiff(repo, selectedFilePath?.path || null);

  const [diffData, setDiffData] = useState<GetDiffResponse | null>(null);

  useEffect(() => {
    if (!selectedFilePath || !selectedRepository || !diff) {
      setDiffData(null);
      return;
    }

    let isCancelled = false;

    (async () => {
      try {
        const data = await getDiff({
          filePath: selectedFilePath.path,
          repoPath: selectedRepository.path,
        });

        if (!isCancelled) {
          setDiffData(data);
        }
      } catch (error) {
        console.error("Failed to fetch diff structure", error);
        if (!isCancelled) {
          setDiffData(null);
        }
      }
    })();

    return () => {
      isCancelled = true;
    };
  }, [selectedFilePath, selectedRepository, diff]);

  return (
    <>
      <div className="w-full min-h-14 max-h-14 h-14 border-b flex">
        <Button
          className="flex border-r-border! justify-between items-center h-full rounded-none hover:border-t w-72"
          variant={"ghost"}
          onClick={async () => {
            const data = await currentBranch({
              repoPath: selectedRepository?.path || "",
            });
            const data2 = await listBranch({
              repoPath: selectedRepository?.path || "",
            });
            console.log({ data, data2 });
          }}
        >
          <div className="flex items-center justify-center gap-4">
            <GitBranch className="size-6" />
            <div className="flex-col flex items-start">
              <span className="text-xs text-muted-foreground font-normal">
                Current Branch
              </span>
              <span>{"ruru/perf/diff/render/i1"}</span>
            </div>
          </div>
          <ChevronDown size={18} />
        </Button>
        <Button
          className="flex border-r-border! justify-between items-center h-full rounded-none hover:border-t w-72"
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
      </div>
      {selectedFilePath && selectedFileStatus ? (
        <>
          <div className="w-full h-[37px] border-b flex justify-between items-center">
            <div className="items-center h-full px-2 flex gap-2">
              {getStatusIcon(selectedFileStatus)}
              <span className="flex items-center">
                <span className="text-muted-foreground/75">
                  {selectedFilePath
                    ? selectedFilePath?.path?.slice(
                        0,
                        selectedFilePath?.path?.lastIndexOf("/"),
                      )
                    : ""}
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
                    {selectedFilePath &&
                      selectedFilePath?.newPath?.slice(
                        0,
                        selectedFilePath?.newPath?.lastIndexOf("/"),
                      )}
                    /
                  </span>
                  <span>{selectedFilePath?.newPath?.split("/").pop()}</span>
                </span>
              ) : null}
            </div>
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
                <PopoverContent className="w-80 mr-4 py-0 px-0 mt-0.5">
                  <div className="flex items-center justify-center">
                    <div className="flex flex-col items-center gap-2 w-full">
                      <Button
                        className="rounded-none size-full h-32 shadow-none first:rounded-s-md last:rounded-e-md focus-visible:z-10"
                        variant="outline"
                        size="icon"
                        aria-label="Flip Horizontal"
                        onClick={() => {
                          setViewMode("unified");
                        }}
                      >
                        <UnifiedSVG />
                      </Button>
                      <span className="text-sm text-muted-foreground">
                        Unified
                      </span>
                    </div>
                    <div className="flex flex-col items-center gap-2 w-full">
                      <Button
                        className="rounded-none size-full h-32 shadow-none rounded-r-md border-l-0 focus-visible:z-10"
                        variant="outline"
                        size="icon"
                        aria-label="Flip Vertical"
                        onClick={() => {
                          setViewMode("split");
                        }}
                      >
                        <SplitSVG />
                      </Button>
                      <span className="text-sm text-muted-foreground">
                        Split
                      </span>
                    </div>
                  </div>
                </PopoverContent>
              </Popover>
            </div>
          </div>
          <ScrollArea
            className={cn(
              "h-[calc(100vh-calc(var(--spacing)*14)-calc(var(--spacing)*9)-calc(var(--spacing)*12))] w-full",
            )}
          >
            <DiffViewer
              diff={diffData}
              filePath={selectedFilePath.path}
              status={selectedFileStatus}
            />
          </ScrollArea>
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
}

import {
  Avatar,
  AvatarFallback,
  AvatarImage,
} from "@gitru/ui/components/avatar";
import { Button } from "@gitru/ui/components/button";
import { CopyButton } from "@gitru/ui/components/copy-button";
import { Group } from "@gitru/ui/components/group";
import { Input } from "@gitru/ui/components/input";
import {
  Tooltip,
  TooltipPopup,
  TooltipTrigger,
} from "@gitru/ui/components/tooltip";
import { ChevronLeftIcon, Files } from "lucide-react";
import { useEffect, useState } from "react";
import { useFileSelectionStore } from "@/components/diff/use-file-selection-store";
import { VirtualizedFileList } from "@/components/virtualized-file-list";
import { useGetCommitById } from "@/hooks";
import { resolveFileSelection } from "@/lib/git-selection-resolver";
import { timeAgoFromUnixSeconds } from "@/lib/time";
import {
  selectActiveRepository,
  selectActiveSessionRepoKey,
  useAppStore,
} from "@/store/use-app-store";
import { matchesSearchQuery } from "../lib/matches-search-query";

export const HistoryDetailView = ({ onBack }: { onBack: () => void }) => {
  const [query, setQuery] = useState("");

  const activeRepository = useAppStore(selectActiveRepository);
  const repoStateKey = useAppStore(selectActiveSessionRepoKey);
  const repoPath = activeRepository?.path ?? "";
  const repoGitViewState = useAppStore((state) =>
    repoStateKey ? state.gitViewByRepo[repoStateKey] : undefined,
  );
  const repoSelectionState = useAppStore((state) =>
    repoStateKey ? state.selectionByRepo[repoStateKey] : undefined,
  );
  const setHistorySelectionForRepo = useAppStore(
    (state) => state.setHistorySelectionForRepo,
  );
  const clearHistorySelectionForRepo = useAppStore(
    (state) => state.clearHistorySelectionForRepo,
  );
  const handleFileClick = useFileSelectionStore(
    (state) => state.handleFileClick,
  );

  const selectedCommitHash =
    repoGitViewState?.selectedHistoryCommitHash ?? null;
  const { data: commitDetails, isLoading: isCommitLoading } = useGetCommitById(
    selectedCommitHash ?? "",
  );

  const selectedFileForCurrentRepo =
    selectedCommitHash && repoPath
      ? (repoSelectionState?.historyByCommit?.[selectedCommitHash] ?? null)
      : null;
  const resolvedHistorySelection = resolveFileSelection({
    selection: selectedFileForCurrentRepo,
    files: commitDetails?.files ?? [],
    context: {
      source: "history",
      historyCommitHash: selectedCommitHash,
    },
  });

  useEffect(() => {
    if (!selectedCommitHash || !commitDetails?.files?.length) {
      return;
    }

    if (resolvedHistorySelection.state === "valid") {
      return;
    }

    const firstFile = commitDetails.files[0];
    setHistorySelectionForRepo(selectedCommitHash, {
      filePath: firstFile.path,
      fileNewPath: firstFile.new_path,
      source: "history",
      historyCommitHash: selectedCommitHash,
      selectedAt: Date.now(),
    });
  }, [
    commitDetails?.files,
    resolvedHistorySelection.state,
    selectedCommitHash,
    setHistorySelectionForRepo,
  ]);

  const selectedHistoryFileForList =
    resolvedHistorySelection.state === "valid"
      ? resolvedHistorySelection.identity
      : selectedFileForCurrentRepo;
  const filteredFiles = (commitDetails?.files ?? []).filter((file) =>
    matchesSearchQuery(file.path, query),
  );

  return (
    <div className="h-full flex flex-col">
      <div className="px-2 py-1 border-b">
        <div className="group flex-1 flex h-6 items-center">
          <span className="text-sm truncate">
            {commitDetails?.summary ?? ""}
          </span>
          <Tooltip>
            <TooltipTrigger
              render={
                <CopyButton
                  size={"xs"}
                  variant="ghost"
                  text={selectedCommitHash ?? ""}
                  className="group-hover:flex hidden"
                />
              }
            ></TooltipTrigger>
            <TooltipPopup>{selectedCommitHash}</TooltipPopup>
          </Tooltip>
        </div>
        <div className="flex justify-between w-full flex-1">
          <div className="flex items-center gap-1 flex-1 mr-1">
            <div className="flex group">
              <Tooltip>
                <TooltipTrigger
                  style={{
                    zIndex: (commitDetails?.authors.co_authors.length || 0) + 1,
                  }}
                >
                  <Avatar className="ring-2 ring-background rounded-sm size-4">
                    <AvatarImage
                      alt={commitDetails?.authors.author.name}
                      src={`https://avatars.githubusercontent.com/u/e?email=${commitDetails?.authors.author.email}&s=64`}
                    />
                    <AvatarFallback>
                      {commitDetails?.authors.author.name
                        .split(" ")
                        .map((n) => n[0])
                        .join("")
                        .toUpperCase()}
                    </AvatarFallback>
                  </Avatar>
                </TooltipTrigger>
                <TooltipPopup side="bottom">
                  {commitDetails?.authors.author.name}
                </TooltipPopup>
              </Tooltip>
              {commitDetails?.authors.co_authors.map((coAuthor, idx) => (
                <Tooltip key={`${idx}-tooltip-coauthor`}>
                  <TooltipTrigger
                    style={{
                      zIndex: commitDetails?.authors.co_authors.length - idx,
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
            <span className="font-normal text-sm text-nowrap">
              {commitDetails?.authors.author.name}
            </span>
            <span className="text-muted-foreground font-light text-xs text-nowrap flex-1 truncate">
              ( {timeAgoFromUnixSeconds(commitDetails?.timestamp || 0)} ) {}
            </span>
          </div>
          <div className="flex gap-1 items-center">
            <span className="text-xs text-muted-foreground flex items-center gap-1">
              <Files className="size-3.5" />
              {commitDetails?.stats?.files_changed ?? 0}
            </span>

            <span className="text-xs text-muted-foreground">/</span>

            <span className="text-xs text-green-600 tabular-nums font-normal">
              +{commitDetails?.stats?.insertions ?? 0}
            </span>
            <span className="text-xs text-red-600 tabular-nums font-normal">
              -{commitDetails?.stats?.deletions ?? 0}
            </span>
          </div>
        </div>
      </div>
      <div className="p-1.5 min-h-10 border-b flex items-center gap-2">
        <Button size="icon-sm" variant="outline" onClick={onBack}>
          <ChevronLeftIcon className="size-4" />
        </Button>
        <Group aria-label="History file actions" className="w-full">
          <Input
            aria-label="Filter history files"
            placeholder="Filter files..."
            size={"sm"}
            value={query}
            onChange={(e) => setQuery(e.target.value)}
          />
        </Group>
      </div>
      <div className="flex-1 min-h-0 overflow-y-auto custom-scroll **:data-[slot=file-row]:mr-2!">
        {isCommitLoading ? (
          <div className="w-full h-full flex items-center justify-center">
            <span className="text-sm text-muted-foreground">Loading...</span>
          </div>
        ) : !selectedCommitHash ? (
          <div className="w-full h-full flex items-center justify-center">
            <span className="text-sm text-muted-foreground">
              No commit selected
            </span>
          </div>
        ) : filteredFiles.length > 0 ? (
          <VirtualizedFileList
            sectionMode="flat"
            searchQuery={query}
            sections={[
              {
                id: "history-files",
                name: "Changed Files",
                type: "custom",
                files: filteredFiles,
              },
            ]}
            onFileClick={handleFileClick}
            setSelectedFilePath={(file) => {
              if (!selectedCommitHash) return;
              if (!file) {
                clearHistorySelectionForRepo(selectedCommitHash);
                return;
              }
              setHistorySelectionForRepo(selectedCommitHash, file);
            }}
            selectedFilePath={
              selectedHistoryFileForList
                ? {
                    path: selectedHistoryFileForList.filePath,
                    newPath: selectedHistoryFileForList.fileNewPath,
                  }
                : undefined
            }
            className="h-full"
          />
        ) : (
          <div className="w-full h-full flex items-center justify-center">
            <span className="text-sm text-muted-foreground">
              No files changed in this commit
            </span>
          </div>
        )}
      </div>
    </div>
  );
};

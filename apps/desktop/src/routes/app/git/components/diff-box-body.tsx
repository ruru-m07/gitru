import { useGetCommitById, useGetCurrentBranchStash, useGetStatus, useStashList, useStashShow } from "@/hooks";
import { resolveFileSelection } from "@/lib/gitSelectionResolver";
import { selectActiveSessionRepoKey, useAppStore } from "@/store/useAppStore";
import { DiffArea } from "./diff-area";
import { EmptyStateScreen } from "./empty-state-screen";
import { FileLevelStatusBar } from "./file-level-status-bar";

export function DiffBoxBody() {
  const repoStateKey = useAppStore(selectActiveSessionRepoKey);
  const repoSelectionState = useAppStore((state) =>
    repoStateKey ? state.selectionByRepo[repoStateKey] : undefined,
  );
  const gitViewState = useAppStore((state) =>
    repoStateKey ? state.gitViewByRepo[repoStateKey] : undefined,
  );
  const { data: status } = useGetStatus();
  const { data: currentBranchStash } = useGetCurrentBranchStash();
  const { data: stashes } = useStashList();

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
        ? (repoSelectionState?.stashByReference[activeStashReference] ?? null)
        : null
      : activeSource === "history"
        ? activeHistoryCommitHash
          ? (repoSelectionState?.historyByCommit?.[activeHistoryCommitHash] ??
            null)
          : null
        : (repoSelectionState?.worktree ?? null);

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
            worktreeScope={
              resolvedSelection.identity.source === "worktree"
                ? resolvedSelection.identity.worktreeScope
                : undefined
            }
          />
        </>
      ) : (
        <EmptyStateScreen />
      )}
    </>
  );
};

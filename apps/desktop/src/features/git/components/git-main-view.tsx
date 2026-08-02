import HistoryGraph from "@/components/history-graph";
import { RebaseModeView } from "@/features/git/rebase";
import { ConflictUnresolvedViewer } from "@/features/git/rebase/conflict-unresolved-viewer";
import { useGetRepoOperation } from "@/hooks";
import { selectActiveSessionRepoKey, useAppStore } from "@/store/use-app-store";
import { DiffBoxBody } from "./diff-box-body";
import { EmptyStateScreen } from "./empty-state-screen";
import { MainActionBar } from "./main-action-bar";

export function GitMainView() {
  const mainWindowView = useAppStore((state) => state.mainWindowView);
  const { data: operation } = useGetRepoOperation();
  const repoStateKey = useAppStore(selectActiveSessionRepoKey);
  const worktreeSelection = useAppStore((state) =>
    repoStateKey
      ? (state.selectionByRepo[repoStateKey]?.worktree ?? null)
      : null,
  );

  const isRebasing = !!operation?.isRebasing;
  const selectedPath = worktreeSelection?.filePath ?? null;
  const isConflictSelection =
    worktreeSelection?.worktreeScope === "conflicted" ||
    (!!selectedPath &&
      (operation?.conflictPaths.includes(selectedPath) ?? false));

  return (
    <div className="flex flex-col h-full min-h-0">
      <MainActionBar />
      {isRebasing && operation ? (
        selectedPath ? (
          <div className="flex-1 min-h-0 grid grid-cols-2">
            <div className="min-h-0 h-full flex flex-col border-r overflow-hidden">
              {isConflictSelection ? (
                <ConflictUnresolvedViewer filePath={selectedPath} />
              ) : (
                <DiffBoxBody />
              )}
            </div>
            <div className="min-h-0 h-full flex flex-col overflow-hidden">
              <RebaseModeView operation={operation} />
            </div>
          </div>
        ) : (
          <div className="flex-1 min-h-0 flex flex-col overflow-hidden">
            <RebaseModeView operation={operation} />
          </div>
        )
      ) : (
        <>
          {mainWindowView === null && <EmptyStateScreen />}
          {mainWindowView === "FileDiff" && <DiffBoxBody />}
          {mainWindowView === "HistoryGraph" && <HistoryGraph />}
        </>
      )}
    </div>
  );
}

import { Button } from "@gitru/ui/components/button";
import { X } from "lucide-react";
import type { ResolvedFileSelection } from "@/lib/gitSelectionResolver";
import { useAppStore } from "@/store/useAppStore";
import { DiffSettingsPopover } from "./diff-settings-popover";
import { FileLevelStatusBarLeft } from "./file-level-status-bar-left";

export function FileLevelStatusBar({
  resolvedSelection,
}: {
  resolvedSelection: ResolvedFileSelection;
}) {
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
            if (
              resolvedSelection.state === "none" ||
              resolvedSelection.state === "pickaxe"
            ) {
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

        <DiffSettingsPopover />
      </div>
    </div>
  );
}
import { CopyButton } from "@gitru/ui/components/copy-button";
import { CircleAlertIcon, MoveHorizontal } from "lucide-react";
import { getStatusIcon } from "@/components/get-status-icon";
import type { ResolvedFileSelection } from "@/lib/git-selection-resolver";
import { renderPath } from "./render-path";

export const FileLevelStatusBarLeft = ({
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

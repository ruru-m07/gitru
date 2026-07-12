import { CopyButton } from "@gitru/ui/components/copy-button";
import { CircleAlertIcon, MoveHorizontal } from "lucide-react";
import { getStatusIcon } from "@/components/getStatusIcon";
import type { ResolvedFileSelection } from "@/lib/gitSelectionResolver";
import { timeAgoFromUnixSeconds } from "@/lib/time";
import { inferPickaxeHitStatus } from "@/lib/pickaxe-status";
import { renderPath } from "./render-path";

export function FileLevelStatusBarLeft({
  resolvedSelection,
}: {
  resolvedSelection: ResolvedFileSelection;
}) {
  if (resolvedSelection.state === "none") {
    return null;
  }

  if (resolvedSelection.state === "pickaxe") {
    const hit = resolvedSelection.hit;

    return (
      <div className="w-full space-y-1 border-b px-4 py-3">
        <div className="text-sm font-medium">
          {hit.commitSubject || "Untitled commit"}
        </div>
        <div className="flex flex-wrap items-center gap-x-2 gap-y-1 text-xs">
          {getStatusIcon(inferPickaxeHitStatus(hit), 16)}
          <span className="group flex items-center">
            {renderPath(hit.filePath)}
            <div className="ml-1 opacity-0 transition-opacity group-hover:opacity-100 text-xs text-muted-foreground">
              <CopyButton size={"xs"} variant="ghost" text={hit.filePath} />
            </div>
          </span>
          {hit.fileNewPath ? (
            <>
              <MoveHorizontal
                className="text-muted-foreground opacity-70"
                size={16}
              />
              <span>{renderPath(hit.fileNewPath)}</span>
            </>
          ) : null}
        </div>
        <div className="flex flex-wrap items-center gap-x-2 gap-y-1 text-xs text-muted-foreground">
          <span className="font-mono">{hit.commitHash.slice(0, 7)}</span>
          <span>{hit.authorName}</span>
          <span>{timeAgoFromUnixSeconds(hit.commitTime)}</span>
          {hit.matchLine ? <span>line {hit.matchLine}</span> : null}
        </div>
      </div>
    );
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
}
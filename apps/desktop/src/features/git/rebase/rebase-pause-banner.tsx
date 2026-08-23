import type { RepoOperation } from "@gitru/commands";
import { cn } from "@gitru/ui/lib/utils";
import { AlertTriangle } from "lucide-react";

export function RebasePauseBanner({ operation }: { operation: RepoOperation }) {
  const progress =
    operation.current != null && operation.total != null
      ? `${operation.current}/${operation.total}`
      : null;
  const remaining =
    operation.remaining != null ? `${operation.remaining} remaining` : null;
  const paused =
    operation.pausedAt != null
      ? operation.pausedAt.slice(0, 7)
      : "current step";
  const reason = operation.pauseReason ? ` ${operation.pauseReason}` : "";

  return (
    // <div
    //   className={cn(
    //     "flex justify-between items-center gap-2 px-2 h-9.25 text-sm",
    //     "bg-warning/10 text-warning _border-b border-warning/30",
    //   )}
    // >
    //   <div className="flex items-center gap-2">
    //     <AlertTriangle className="size-4 shrink-0" />
    //     <span className="font-medium tabular-nums">
    //       Rebase paused at {paused} due to
    //       {operation.conflictPaths.length > 0 ? (
    //         <span className="ml-1 tabular-nums">
    //           {operation.conflictPaths.length} conflict
    //           {operation.conflictPaths.length === 1 ? "" : "s"}
    //         </span>
    //       ) : (
    //         reason
    //       )}
    //     </span>
    //   </div>
    //   <div className="flex items-center gap-2">
    //     <span className="font-medium opacity-75">
    //       {remaining ? `${remaining}` : ""}
    //     </span>
    //     <span className="font-medium">{progress ? `(${progress})` : ""}</span>
    //   </div>
    // </div>
    <div className="flex flex-col gap-2 w-full p-1.5">
      <div
        className={cn(
          "flex justify-between w-full items-center gap-2 rounded-md pl-2.5 pr-2 h-9 text-sm",
          "bg-warning text-background",
        )}
      >
        <div className="flex items-center gap-2">
          <AlertTriangle className="size-4 shrink-0" />
          <span className="font-medium tabular-nums">
            Rebase paused at {paused}
            {operation.pauseReason === "conflict" ? (
              <>
                due to
                {operation.conflictPaths.length > 0 ? (
                  <span className="ml-1 tabular-nums">
                    {operation.conflictPaths.length} conflict
                    {operation.conflictPaths.length === 1 ? "" : "s"}
                  </span>
                ) : (
                  reason
                )}
              </>
            ) : null}
          </span>
        </div>
        <div className="flex items-center gap-2">
          <span className="font-medium opacity-75">
            {remaining ? `${remaining}` : ""}
          </span>
          <span className="font-medium">{progress ? `(${progress})` : ""}</span>
        </div>
      </div>
    </div>
  );
}

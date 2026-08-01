import type { RepoOperation } from "@gitru/commands";
import { cn } from "@gitru/ui/lib/utils";
import { AlertTriangle } from "lucide-react";

export function RebasePauseBanner({
  operation,
}: {
  operation: RepoOperation;
}) {
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
  const reason = operation.pauseReason
    ? ` · ${operation.pauseReason}`
    : "";

  return (
    <div
      className={cn(
        "flex items-center gap-2 border-b px-3 py-2 text-sm",
        "bg-amber-500/15 text-amber-950 dark:text-amber-100 border-amber-500/30",
      )}
    >
      <AlertTriangle className="size-4 shrink-0 text-amber-600 dark:text-amber-400" />
      <span className="font-medium">
        Rebase paused at {paused}
        {progress ? ` (${progress})` : ""}
        {remaining ? ` · ${remaining}` : ""}
        {reason}
      </span>
      {operation.conflictPaths.length > 0 ? (
        <span className="ml-auto text-xs opacity-80">
          {operation.conflictPaths.length} conflict
          {operation.conflictPaths.length === 1 ? "" : "s"}
        </span>
      ) : null}
    </div>
  );
}

import type { RebaseTodoEntry } from "@gitru/commands";
import { Button } from "@gitru/ui/components/button";
import {
  Menu,
  MenuItem,
  MenuPopup,
  MenuTrigger,
} from "@gitru/ui/components/menu";
import { cn } from "@gitru/ui/lib/utils";
import { ChevronDown } from "lucide-react";
import { timeAgoFromUnixSeconds } from "@/lib/time";

type RebaseAction = RebaseTodoEntry["action"];

const ACTIONS: { value: RebaseAction; label: string; hint: string }[] = [
  { value: "pick", label: "Pick", hint: "Use commit" },
  { value: "reword", label: "Reword", hint: "Use commit, edit message" },
  { value: "edit", label: "Edit", hint: "Use commit, stop for amending" },
  {
    value: "squash",
    label: "Squash",
    hint: "Meld into previous, edit message",
  },
  {
    value: "fixup",
    label: "Fixup",
    hint: "Meld into previous, discard message",
  },
  { value: "drop", label: "Drop", hint: "Remove commit" },
];

function actionLabel(action: RebaseAction): string {
  return ACTIONS.find((a) => a.value === action)?.label ?? action;
}

function formatAuthoredAt(authoredAt: string | undefined): string | null {
  if (!authoredAt?.trim()) return null;
  const asUnix = Number(authoredAt);
  if (Number.isFinite(asUnix) && asUnix > 0) {
    return timeAgoFromUnixSeconds(asUnix);
  }
  return authoredAt;
}

export function RebaseTodoList({
  entries,
  editable,
  pauseReason,
  pausedAt,
  onActionChange,
}: {
  entries: RebaseTodoEntry[];
  editable?: boolean;
  /** When pause is a conflict, the paused commit row uses the conflict indicator. */
  pauseReason?: string | null;
  pausedAt?: string | null;
  onActionChange?: (index: number, action: RebaseAction) => void;
}) {
  const conflictPaused = pauseReason === "conflict";

  return (
    <div className="flex flex-col min-h-0 h-full">
      <div className="flex-1 overflow-auto">
        {entries.length === 0 ? (
          <div className="p-6 text-sm text-muted-foreground text-center">
            No commits in this rebase todo.
          </div>
        ) : (
          <div className="grid grid-cols-[fit-content(100px)_1fr_fit-content(200px)_fit-content(200px)_fit-content(200px)] px-1.5">
            {entries.map((entry, index) => {
              const isDone = entry.status === "done";
              const isCurrent = entry.status === "current";
              const isPending =
                entry.status === "pending" || entry.status === "skipped";
              const isConflict =
                conflictPaused &&
                (isCurrent ||
                  (!!pausedAt &&
                    (entry.commit.startsWith(pausedAt) ||
                      pausedAt.startsWith(entry.commit) ||
                      entry.shortCommit === pausedAt.slice(0, 7))));
              const canEditAction =
                !!editable && !!onActionChange && entry.status === "pending";
              const authoredLabel = formatAuthoredAt(entry.authoredAt);

              return (
                <div
                  className={cn(
                    "grid grid-cols-subgrid col-span-5 h-9 items-center border border-transparent transition-all duration-150 rounded-md [--indicator-bg:var(--color-background)]",
                    isCurrent &&
                      "[--indicator-bg:color-mix(in_oklab,var(--indicator-color)_10%,var(--color-background))] border-(--indicator-color)/50 bg-(--indicator-bg)",
                    isDone &&
                      !isConflict &&
                      "[--indicator-color:var(--color-green-600)]",
                    isConflict && "[--indicator-color:var(--color-red-600)]",
                    isPending &&
                      !isConflict &&
                      "[--indicator-color:var(--color-muted-foreground)]",
                    isCurrent &&
                      !isConflict &&
                      "[--indicator-color:var(--color-amber-500)]",
                  )}
                >
                  <div
                    className={cn(
                      "flex items-center justify-center relative px-1.5",
                    )}
                  >
                    {index !== 0 ? (
                      <div
                        className={cn(
                          "w-0.5 absolute left-1/2 top-0 bg-(--indicator-color) h-10 rounded-full -translate-x-1/2 -translate-y-1/2",
                          (entry.action === "drop" ||
                            entries[index - 1].action === "drop") &&
                            "[--indicator-color:var(--color-red-600)]",
                        )}
                      />
                    ) : null}
                    <div
                      className={cn(
                        "size-5 bg-background relative flex items-center justify-center ring-3 z-10 ring-(--indicator-bg) rounded-full",
                        entry.action === "drop" && "opacity-0",
                      )}
                    >
                      <svg
                        className="absolute inset-0 size-full will-change-auto bg-(--indicator-bg)"
                        viewBox="0 0 20 20"
                        aria-hidden="true"
                        shapeRendering="geometricPrecision"
                      >
                        <circle
                          cx="10"
                          cy="10"
                          r="9"
                          fill="none"
                          stroke="var(--indicator-color)"
                          strokeWidth="2"
                          strokeDasharray={
                            isPending && !isConflict ? "4 4" : "57 0"
                          }
                          strokeDashoffset={0}
                          strokeLinecap="round"
                          className="will-change-auto"
                          style={{
                            transition:
                              "stroke 150ms ease, stroke-dasharray 1000ms ease",
                          }}
                        />
                      </svg>

                      <div className="relative z-10 flex items-center justify-center">
                        <div
                          className={cn(
                            "w-2 h-2 will-change-auto transition-all opacity-100 duration-300 bg-(--indicator-color) rounded-full",
                            isConflict && "h-0.75",
                            isPending && "w-0 h-0",
                          )}
                        />
                      </div>
                    </div>
                  </div>

                  <div
                    className={cn(
                      "truncate pl-0.5 pr-2 text-sm",
                      isDone && !isConflict && "text-muted-foreground",
                      entry.action === "drop" && "opacity-80 line-through",
                    )}
                  >
                    {entry.message || "(no message)"}
                  </div>

                  <div className="px-2 text-sm text-right text-muted-foreground tabular-nums">
                    {authoredLabel ?? "—"}
                  </div>

                  <div className="px-2 font-mono text-sm text-muted-foreground">
                    {entry.shortCommit}
                  </div>

                  <div className="flex items-center gap-1 pl-2 pr-1 justify-end">
                    {canEditAction ? (
                      <Menu>
                        <MenuTrigger
                          render={<Button variant="secondary" size="sm" />}
                        >
                          {actionLabel(entry.action)}
                          <ChevronDown />
                        </MenuTrigger>
                        <MenuPopup align="end">
                          {ACTIONS.map((action) => (
                            <MenuItem
                              key={action.value}
                              closeOnClick
                              onClick={() =>
                                onActionChange?.(entry.index, action.value)
                              }
                            >
                              <span className="flex flex-col items-start gap-0.5">
                                <span>{action.label}</span>
                                <span className="text-xs text-muted-foreground font-normal">
                                  {action.hint}
                                </span>
                              </span>
                            </MenuItem>
                          ))}
                        </MenuPopup>
                      </Menu>
                    ) : (
                      <span className="text-sm text-muted-foreground px-2">
                        {actionLabel(entry.action)}
                      </span>
                    )}
                  </div>
                </div>
              );
            })}
          </div>
        )}
      </div>
    </div>
  );
}

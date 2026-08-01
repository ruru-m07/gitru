import type { RebaseTodoEntry } from "@gitru/commands";
import { Button } from "@gitru/ui/components/button";
import {
  Select,
  SelectItem,
  SelectPopup,
  SelectTrigger,
  SelectValue,
} from "@gitru/ui/components/select";
import { cn } from "@gitru/ui/lib/utils";

type RebaseAction = RebaseTodoEntry["action"];

const ACTIONS: { value: RebaseAction; label: string; hint: string }[] = [
  { value: "pick", label: "pick", hint: "Use commit" },
  { value: "reword", label: "reword", hint: "Use commit, edit message" },
  { value: "edit", label: "edit", hint: "Use commit, stop for amending" },
  {
    value: "squash",
    label: "squash",
    hint: "Meld into previous, edit message",
  },
  {
    value: "fixup",
    label: "fixup",
    hint: "Meld into previous, discard message",
  },
  { value: "drop", label: "drop", hint: "Remove commit" },
];

export function RebaseTodoList({
  entries,
  editable,
  onActionChange,
  onMove,
}: {
  entries: RebaseTodoEntry[];
  editable?: boolean;
  onActionChange?: (index: number, action: RebaseAction) => void;
  onMove?: (index: number, direction: -1 | 1) => void;
}) {
  return (
    <div className="flex flex-col min-h-0 h-full">
      <div className="flex-1 overflow-auto">
        {entries.map((entry) => {
          const isCurrent = entry.status === "current";
          const isDone = entry.status === "done";
          return (
            <div
              key={`${entry.index}-${entry.commit}`}
              className={cn(
                "group flex items-center gap-2 px-3 py-1.5 border-b border-border/40 text-sm",
                isCurrent && "bg-amber-500/15",
                isDone && "opacity-55",
              )}
            >
              <div className="w-24 shrink-0">
                {editable && onActionChange ? (
                  <Select
                    value={entry.action}
                    onValueChange={(value) => {
                      if (typeof value === "string") {
                        onActionChange(entry.index, value as RebaseAction);
                      }
                    }}
                  >
                    <SelectTrigger size="sm" className="h-7 min-w-0 w-full">
                      <SelectValue />
                    </SelectTrigger>
                    <SelectPopup>
                      {ACTIONS.map((a) => (
                        <SelectItem key={a.value} value={a.value}>
                          {a.label}
                        </SelectItem>
                      ))}
                    </SelectPopup>
                  </Select>
                ) : (
                  <span className="font-mono text-xs text-muted-foreground">
                    {entry.action}
                  </span>
                )}
              </div>

              <div className="flex flex-col items-center shrink-0 w-3">
                <div
                  className={cn(
                    "size-2 rounded-full border",
                    isCurrent
                      ? "bg-amber-500 border-amber-600"
                      : isDone
                        ? "bg-muted-foreground/40 border-transparent"
                        : "bg-background border-muted-foreground/50",
                  )}
                />
              </div>

              <div className="flex-1 min-w-0 truncate font-medium">
                {entry.message || "(no message)"}
              </div>

              {entry.authoredAt ? (
                <span className="text-xs text-muted-foreground shrink-0 hidden sm:inline">
                  {entry.authoredAt}
                </span>
              ) : null}

              <span className="font-mono text-xs text-muted-foreground shrink-0">
                {entry.shortCommit}
              </span>

              {editable && onMove ? (
                <div className="flex opacity-0 group-hover:opacity-100 transition-opacity shrink-0">
                  <Button
                    variant="ghost"
                    size="sm"
                    className="h-6 px-1"
                    onClick={() => onMove(entry.index, -1)}
                    aria-label="Move up"
                  >
                    ↑
                  </Button>
                  <Button
                    variant="ghost"
                    size="sm"
                    className="h-6 px-1"
                    onClick={() => onMove(entry.index, 1)}
                    aria-label="Move down"
                  >
                    ↓
                  </Button>
                </div>
              ) : null}
            </div>
          );
        })}
        {entries.length === 0 ? (
          <div className="p-6 text-sm text-muted-foreground text-center">
            No commits in this rebase todo.
          </div>
        ) : null}
      </div>
    </div>
  );
}

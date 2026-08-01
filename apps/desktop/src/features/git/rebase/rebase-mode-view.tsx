import type {
  RebasePlanEntry,
  RebaseTodoEntry,
  RepoOperation,
} from "@gitru/commands";
import { GitBranch } from "lucide-react";
import { useRebaseUpdateTodo } from "@/hooks";
import { RebaseActionsBar } from "./rebase-actions-bar";
import { RebasePauseBanner } from "./rebase-pause-banner";
import { RebaseTodoList } from "./rebase-todo-list";

type RebaseAction = RebaseTodoEntry["action"];

/** Rebase chrome for GitMainView — todo list + pause banner + actions. */
export function RebaseModeView({ operation }: { operation: RepoOperation }) {
  const { mutateAsync: updateTodo } = useRebaseUpdateTodo();

  const editable =
    operation.engine === "gitru" &&
    operation.todo.some((e) => e.status === "pending");

  const handleActionChange = async (index: number, action: RebaseAction) => {
    const entries: RebasePlanEntry[] = operation.todo.map((e) => ({
      action: e.index === index ? action : e.action,
      commit: e.commit,
      message: e.message,
    }));
    await updateTodo(entries);
  };

  const handleMove = async (index: number, direction: -1 | 1) => {
    const sorted = [...operation.todo].sort((a, b) => a.index - b.index);
    const from = sorted.findIndex((e) => e.index === index);
    const to = from + direction;
    if (from < 0 || to < 0 || to >= sorted.length) return;
    if (sorted[from].status !== "pending" || sorted[to].status !== "pending") {
      return;
    }
    const next = [...sorted];
    const [item] = next.splice(from, 1);
    next.splice(to, 0, item);
    const entries: RebasePlanEntry[] = next.map((e) => ({
      action: e.action,
      commit: e.commit,
      message: e.message,
    }));
    await updateTodo(entries);
  };

  return (
    <div className="flex-1 min-h-0 flex flex-col">
      <header className="flex gap-2 justify-between px-3 py-2 border-b shrink-0">
        <div className="min-w-0 flex items-center gap-2">
          <div className="flex items-center gap-2 font-normal">
            <GitBranch className="size-4" />
            <span className="truncate">
              {operation.headName
                ?.replace(/^refs\/heads\//, "")
                .replace(/^refs\/remotes\//, "")}
            </span>
            <span className="text-muted-foreground">onto</span>
            <span className="truncate font-mono">
              {operation.onto?.slice(0, 7)}
            </span>
          </div>
        </div>
        <RebaseActionsBar operation={operation} />
      </header>

      {operation.pauseReason ? (
        <RebasePauseBanner operation={operation} />
      ) : null}

      <div className="flex-1 min-h-0 flex flex-col">
        <RebaseTodoList
          entries={operation.todo}
          editable={editable}
          onActionChange={handleActionChange}
          onMove={handleMove}
        />
      </div>
    </div>
  );
}

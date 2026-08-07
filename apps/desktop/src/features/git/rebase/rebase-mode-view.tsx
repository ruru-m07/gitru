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

  // Pending commits can have their action edited for both Gitru and native
  // interactive rebases (rewrites `.git/rebase-merge/git-rebase-todo`).
  const editable = operation.todo.some((e) => e.status === "pending");

  const handleActionChange = async (index: number, action: RebaseAction) => {
    const entries: RebasePlanEntry[] = operation.todo.map((e) => ({
      action: e.index === index ? action : e.action,
      commit: e.commit,
      message: e.message,
    }));
    await updateTodo(entries);
  };

  return (
    <div className="flex-1 min-h-0 flex flex-col">
      <header className="flex h-9.25 borde-b gap-2 justify-between px-2 items-center border-b shrink-0">
        <div className="min-w-0 flex items-center gap-2">
          <div className="flex items-center gap-2 font-normal">
            <GitBranch className="size-4" />
            <span className="flex items-center gap-1">
              <span className="truncate">
                {operation.headName
                  ?.replace(/^refs\/heads\//, "")
                  .replace(/^refs\/remotes\//, "")}
              </span>
              <span className="text-muted-foreground">onto</span>
              <span className="truncate font-mono">
                {operation.onto?.slice(0, 7)}
              </span>
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
          pauseReason={operation.pauseReason}
          pausedAt={operation.pausedAt}
          onActionChange={handleActionChange}
        />
      </div>
    </div>
  );
}

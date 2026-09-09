export type DiffSource = "worktree" | "stash" | "history";

export type WorktreeDiffScope = "staged" | "unstaged" | "conflicted";

export const resolveDiffActionAvailability = ({
  source,
  worktreeScope,
}: {
  source: DiffSource;
  worktreeScope?: WorktreeDiffScope;
}) => ({
  canStageOrDiscard: source === "worktree" && worktreeScope === "unstaged",
  canUnstage: source === "worktree" && worktreeScope === "staged",
});

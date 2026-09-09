import { describe, expect, test } from "vitest";
import { resolveDiffActionAvailability } from "../src/features/git/lib/diff-action-availability";

describe("diff action predicates", () => {
  test("allows stage and discard only for unstaged worktree changes", () => {
    expect(
      resolveDiffActionAvailability({
        source: "worktree",
        worktreeScope: "unstaged",
      }),
    ).toEqual({ canStageOrDiscard: true, canUnstage: false });
  });

  test("allows unstage only for staged worktree changes", () => {
    expect(
      resolveDiffActionAvailability({
        source: "worktree",
        worktreeScope: "staged",
      }),
    ).toEqual({ canStageOrDiscard: false, canUnstage: true });
  });

  test.each([
    "stash",
    "history",
  ] as const)("keeps %s diffs read-only even when given a worktree scope", (source) => {
    expect(
      resolveDiffActionAvailability({ source, worktreeScope: "unstaged" }),
    ).toEqual({ canStageOrDiscard: false, canUnstage: false });
    expect(
      resolveDiffActionAvailability({ source, worktreeScope: "staged" }),
    ).toEqual({ canStageOrDiscard: false, canUnstage: false });
  });

  test.each([
    undefined,
    "conflicted",
  ] as const)("does not expose a patch action for %s worktree scope", (worktreeScope) => {
    expect(
      resolveDiffActionAvailability({ source: "worktree", worktreeScope }),
    ).toEqual({ canStageOrDiscard: false, canUnstage: false });
  });
});

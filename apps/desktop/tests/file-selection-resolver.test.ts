import type { FileStatus } from "@gitru/commands";
import { describe, expect, test } from "vitest";
import { resolveFileSelection } from "../src/lib/git-selection-resolver";
import type { FileSelectionIdentity } from "../src/types/store";

const selectedAt = 1_700_000_000_000;

const selection = (
  overrides: Partial<FileSelectionIdentity> = {},
): FileSelectionIdentity => ({
  filePath: "src/app.tsx",
  source: "worktree",
  selectedAt,
  ...overrides,
});

const file = (overrides: Partial<FileStatus> = {}): FileStatus => ({
  path: "src/app.tsx",
  status: ["WorktreeModified"],
  ...overrides,
});

describe("stale file selection", () => {
  test("marks a selection stale when the active source changes", () => {
    const identity = selection();

    expect(
      resolveFileSelection({
        selection: identity,
        files: [file()],
        context: { source: "history", historyCommitHash: "abc123" },
      }),
    ).toEqual({ state: "stale", identity, reason: "source_mismatch" });
  });

  test("marks a selection stale when the file disappears", () => {
    const identity = selection();

    expect(
      resolveFileSelection({
        selection: identity,
        files: [],
        context: { source: "worktree" },
      }),
    ).toEqual({ state: "stale", identity, reason: "missing" });
  });

  test("does not reuse a worktree selection after its scope changes", () => {
    const identity = selection({ worktreeScope: "staged" });

    expect(
      resolveFileSelection({
        selection: identity,
        files: [file({ status: ["WorktreeModified"] })],
        context: { source: "worktree" },
      }),
    ).toEqual({ state: "stale", identity, reason: "missing" });
  });

  test("distinguishes a removed stash from a different active stash", () => {
    const identity = selection({
      source: "stash",
      stashReference: "stash@{1}",
    });

    expect(
      resolveFileSelection({
        selection: identity,
        files: [file()],
        context: {
          source: "stash",
          stashReference: "stash@{1}",
          availableStashReferences: ["stash@{0}"],
        },
      }),
    ).toEqual({ state: "stale", identity, reason: "stash_removed" });

    expect(
      resolveFileSelection({
        selection: identity,
        files: [file()],
        context: {
          source: "stash",
          stashReference: "stash@{0}",
          availableStashReferences: ["stash@{0}", "stash@{1}"],
        },
      }),
    ).toEqual({ state: "stale", identity, reason: "source_mismatch" });
  });

  test("does not carry a file selection between history commits", () => {
    const identity = selection({
      source: "history",
      historyCommitHash: "abc123",
    });

    expect(
      resolveFileSelection({
        selection: identity,
        files: [file()],
        context: { source: "history", historyCommitHash: "def456" },
      }),
    ).toEqual({ state: "stale", identity, reason: "source_mismatch" });
  });

  test.each([
    selection({ filePath: "src/old-name.tsx" }),
    selection({
      filePath: "src/old-name.tsx",
      fileNewPath: "src/new-name.tsx",
    }),
    selection({ filePath: "src/new-name.tsx" }),
  ])("keeps renamed-file identities valid and canonical: %o", (identity) => {
    const renamedFile = file({
      path: "src/old-name.tsx",
      new_path: "src/new-name.tsx",
      status: ["IndexRenamed"],
    });

    expect(
      resolveFileSelection({
        selection: identity,
        files: [renamedFile],
        context: { source: "worktree" },
      }),
    ).toEqual({
      state: "valid",
      identity: {
        ...identity,
        filePath: "src/old-name.tsx",
        fileNewPath: "src/new-name.tsx",
      },
      file: renamedFile,
    });
  });
});

import { describe, expect, test } from "vitest";
import {
  isSyncControlVisible,
  resolveSyncState,
} from "../src/features/git/components/sync-state";

const published = {
  ahead: 0,
  behind: 0,
  isPublished: true,
  isDetached: false,
  localBranch: "main",
  upstreamBranch: "origin/main",
};

describe("sync decisions", () => {
  test("models the loading state", () => {
    const state = resolveSyncState({});
    expect(state).toMatchObject({
      kind: "loading",
      primaryAction: null,
    });
  });

  test("models an unpublished branch as publish", () => {
    expect(
      resolveSyncState({ status: { ...published, isPublished: false } }),
    ).toMatchObject({ kind: "unpublished", primaryAction: "publish" });
  });

  test("models an ahead branch as push", () => {
    expect(
      resolveSyncState({ status: { ...published, ahead: 2 } }),
    ).toMatchObject({ kind: "ahead", primaryAction: "push" });
  });

  test("models a behind branch as pull", () => {
    expect(
      resolveSyncState({ status: { ...published, behind: 3 } }),
    ).toMatchObject({ kind: "behind", primaryAction: "pull" });
  });

  test("does not guess a primary action for diverged branches", () => {
    expect(
      resolveSyncState({
        status: { ...published, ahead: 2, behind: 3 },
      }),
    ).toMatchObject({ kind: "diverged", primaryAction: null });
  });

  test("models a synced branch as fetch", () => {
    const state = resolveSyncState({ status: published });
    expect(state).toMatchObject({
      kind: "synced",
      label: "Fetch",
      primaryAction: "fetch",
    });
  });

  test("models detached HEAD without a publish action", () => {
    expect(resolveSyncState({ isDetached: true })).toMatchObject({
      kind: "detached",
      primaryAction: null,
    });
  });

  test("blocks syncing while any git operation is in progress", () => {
    expect(
      resolveSyncState({ status: published, operationKind: "merge" }),
    ).toMatchObject({ kind: "inProgress", primaryAction: null });
  });

  test("prioritizes an in-progress operation over detached and remote state", () => {
    expect(
      resolveSyncState({
        status: {
          ...published,
          ahead: 2,
          behind: 1,
          isDetached: true,
        },
        operationKind: "rebaseInteractive",
        isDetached: true,
      }),
    ).toEqual({
      kind: "inProgress",
      label: "Rebase in progress",
      detail: "Finish or abort it before syncing",
      primaryAction: null,
    });
  });

  test("treats a clean operation as idle", () => {
    expect(
      resolveSyncState({
        status: { ...published, ahead: 1 },
        operationKind: "clean",
      }),
    ).toMatchObject({ kind: "ahead", primaryAction: "push" });
  });

  test("prioritizes publishing before ahead and behind counts", () => {
    expect(
      resolveSyncState({
        status: {
          ...published,
          ahead: 4,
          behind: 2,
          isPublished: false,
        },
      }),
    ).toMatchObject({ kind: "unpublished", primaryAction: "publish" });
  });

  test("uses singular and plural commit details deterministically", () => {
    expect(
      resolveSyncState({ status: { ...published, ahead: 1 } }).detail,
    ).toBe("1 commit ahead");
    expect(
      resolveSyncState({ status: { ...published, behind: 2 } }).detail,
    ).toBe("2 commits behind");
  });

  test("falls back safely for an unknown git operation", () => {
    expect(
      resolveSyncState({ status: published, operationKind: "futureOperation" }),
    ).toMatchObject({
      kind: "inProgress",
      label: "Git operation in progress",
      primaryAction: null,
    });
  });
});

describe("sync action predicates", () => {
  test.each([
    ["loading", resolveSyncState({}), false],
    ["detached", resolveSyncState({ isDetached: true }), false],
    [
      "operation in progress",
      resolveSyncState({ status: published, operationKind: "merge" }),
      true,
    ],
    [
      "unpublished",
      resolveSyncState({ status: { ...published, isPublished: false } }),
      true,
    ],
    ["ahead", resolveSyncState({ status: { ...published, ahead: 1 } }), true],
    ["behind", resolveSyncState({ status: { ...published, behind: 1 } }), true],
    [
      "diverged",
      resolveSyncState({ status: { ...published, ahead: 1, behind: 1 } }),
      true,
    ],
    ["synced", resolveSyncState({ status: published }), true],
  ])("shows the control for %s state: %s", (_name, state, expected) => {
    expect(isSyncControlVisible(state)).toBe(expected);
  });
});

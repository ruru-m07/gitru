import { describe, expect, test } from "bun:test";
import { resolveSyncState } from "../src/features/git/components/sync-state";

const published = {
  ahead: 0,
  behind: 0,
  isPublished: true,
  isDetached: false,
  localBranch: "main",
  upstreamBranch: "origin/main",
};

describe("resolveSyncState", () => {
  test("models the loading state", () => {
    expect(resolveSyncState({})).toMatchObject({
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

  test("models a synced branch without an action", () => {
    expect(resolveSyncState({ status: published })).toMatchObject({
      kind: "synced",
      primaryAction: null,
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
});

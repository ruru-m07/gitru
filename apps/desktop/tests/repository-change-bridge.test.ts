import { QueryClient, QueryObserver } from "@tanstack/react-query";
import { describe, expect, test, vi } from "vitest";
import {
  invalidateRepositoryQueries,
  type RepositoryChangeKind,
  shouldInvalidateRepositoryQuery,
} from "../src/state/core/repository-change-bridge";
import {
  queryClient,
  refreshActiveQueriesAfterNativeFocus,
} from "../src/state/core/state-manager";

const repoKey = (...suffix: unknown[]) => [
  "repository",
  "context-a",
  "/repos/alpha",
  ...suffix,
];

const affected = (change: RepositoryChangeKind, ...suffix: unknown[]) =>
  shouldInvalidateRepositoryQuery(repoKey(...suffix), {
    contextId: "context-a",
    changes: [change],
  });

describe("repository filesystem change query mapping", () => {
  test("worktree changes refresh only worktree state", () => {
    expect(affected("worktree", "status")).toBe(true);
    expect(affected("worktree", "diff", "worktree", "Worktree")).toBe(true);
    expect(affected("worktree", "branches", "hasUncommittedChanges")).toBe(
      true,
    );

    expect(affected("worktree", "diff", "commit:abc:p1")).toBe(false);
    expect(affected("worktree", "commit", "history")).toBe(false);
    expect(
      shouldInvalidateRepositoryQuery(
        ["worktree-file", "context-a", "src/conflicted.ts", 0],
        { contextId: "context-a", changes: ["worktree"] },
      ),
    ).toBe(true);
    expect(
      shouldInvalidateRepositoryQuery(
        ["worktree-file", "context-b", "src/conflicted.ts", 0],
        { contextId: "context-a", changes: ["worktree"] },
      ),
    ).toBe(false);
    expect(
      shouldInvalidateRepositoryQuery(
        ["worktree-file", "context-a", "src/conflicted.ts", 0],
        { contextId: "context-a", changes: ["index"] },
      ),
    ).toBe(false);
  });

  test("index changes also refresh operation state", () => {
    expect(affected("index", "status")).toBe(true);
    expect(affected("index", "diff", "worktree", "Staged")).toBe(true);
    expect(affected("index", "operation")).toBe(true);
    expect(affected("index", "branches", "list")).toBe(false);
  });

  test("HEAD changes refresh branch, timeline, status, and worktree data", () => {
    for (const suffix of [
      ["branches", "list"],
      ["branches", "current"],
      ["branches", "statusAheadBehind"],
      ["branches", "hasUncommittedChanges"],
      ["branches", "currentBranchStash"],
      ["commit", "last"],
      ["commit", "historyGraph"],
      ["status"],
      ["diff", "worktree"],
    ]) {
      expect(affected("head", ...suffix)).toBe(true);
    }

    expect(affected("head", "stash", "list")).toBe(false);
    expect(affected("head", "commit", "getCommitById", "abc")).toBe(false);
  });

  test("ref changes refresh branch and commit timeline queries", () => {
    expect(affected("refs", "branches", "list")).toBe(true);
    expect(affected("refs", "branches", "statusAheadBehind")).toBe(true);
    expect(affected("refs", "branches", "hasUncommittedChanges")).toBe(true);
    expect(affected("refs", "commit", "history")).toBe(true);
    expect(affected("refs", "status")).toBe(true);
    expect(affected("refs", "diff", "worktree")).toBe(true);

    expect(affected("refs", "branches", "current")).toBe(false);
    expect(affected("refs", "diff", "commit:abc:p1")).toBe(false);
  });

  test("stash changes refresh stash identities and stash-aware timelines", () => {
    expect(affected("stash", "stash", "list")).toBe(true);
    expect(affected("stash", "diff", "stash:stash@{0}")).toBe(true);
    expect(affected("stash", "branches", "currentBranchStash")).toBe(true);
    expect(
      affected("stash", "commit", "historyGraph", {
        include_stash: true,
      }),
    ).toBe(true);

    expect(
      affected("stash", "commit", "historyGraph", {
        include_stash: false,
      }),
    ).toBe(false);
    expect(affected("stash", "diff", "worktree")).toBe(false);
  });

  test("operation and config changes stay within their affected domains", () => {
    expect(affected("operation", "operation")).toBe(true);
    expect(affected("operation", "status")).toBe(true);
    expect(affected("operation", "diff", "worktree")).toBe(true);
    expect(affected("operation", "branches", "current")).toBe(true);
    expect(affected("operation", "branches", "hasUncommittedChanges")).toBe(
      true,
    );
    expect(affected("operation", "branches", "list")).toBe(false);

    expect(affected("config", "origin")).toBe(true);
    expect(affected("config", "branches", "list")).toBe(true);
    expect(affected("config", "branches", "statusAheadBehind")).toBe(true);
    expect(affected("config", "branches", "hasUncommittedChanges")).toBe(true);
    expect(affected("config", "status")).toBe(true);
    expect(affected("config", "diff", "worktree")).toBe(true);
    expect(affected("config", "diff", "commit:abc:p1")).toBe(true);
    expect(affected("config", "diff", "stash:stash@{0}")).toBe(true);
  });

  test("never invalidates another repository context", () => {
    expect(
      shouldInvalidateRepositoryQuery(
        ["repository", "context-b", "/repos/beta", "status"],
        { contextId: "context-a", changes: ["worktree", "head"] },
      ),
    ).toBe(false);
  });

  test("marks only matching cached queries stale", async () => {
    const client = new QueryClient();
    const matching = repoKey("status");
    const unrelated = repoKey("commit", "getCommitById", "abc");
    const otherContext = ["repository", "context-b", "/repos/beta", "status"];

    client.setQueryData(matching, "status");
    client.setQueryData(unrelated, "commit");
    client.setQueryData(otherContext, "other");

    await invalidateRepositoryQueries(client, {
      contextId: "context-a",
      changes: ["worktree"],
    });

    expect(client.getQueryState(matching)?.isInvalidated).toBe(true);
    expect(client.getQueryState(unrelated)?.isInvalidated).toBe(false);
    expect(client.getQueryState(otherContext)?.isInvalidated).toBe(false);
  });

  test("restarts an active initial fetch so an old result cannot win", async () => {
    const client = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    });
    const key = repoKey("status");
    let resolveFirst!: (value: string) => void;
    const firstResult = new Promise<string>((resolve) => {
      resolveFirst = resolve;
    });
    let calls = 0;
    const observer = new QueryObserver(client, {
      queryKey: key,
      queryFn: () => {
        calls += 1;
        return calls === 1 ? firstResult : Promise.resolve("fresh");
      },
    });
    const unsubscribe = observer.subscribe(() => {});

    await vi.waitFor(() => expect(calls).toBe(1));
    const refresh = invalidateRepositoryQueries(client, {
      contextId: "context-a",
      changes: ["worktree"],
    });

    await vi.waitFor(() => expect(calls).toBe(2));
    resolveFirst("stale");
    await refresh;

    expect(client.getQueryData(key)).toBe("fresh");
    expect(client.getQueryState(key)?.isInvalidated).toBe(false);
    unsubscribe();
  });
});

test("global queries keep focus recovery without interval polling", () => {
  const options = queryClient.getDefaultOptions().queries;

  expect(options?.refetchOnWindowFocus).toBe(true);
  expect(options?.refetchInterval).toBeUndefined();
});

test("focus recovery clears native caches before refetching", async () => {
  const calls: string[] = [];
  const client = {
    cancelQueries: vi.fn(async () => {
      calls.push("cancel");
    }),
    invalidateQueries: vi.fn(async () => {
      calls.push("refetch");
    }),
  } as unknown as QueryClient;

  await refreshActiveQueriesAfterNativeFocus(client, async () => {
    calls.push("backend");
  });

  expect(calls).toEqual(["cancel", "backend", "refetch"]);
});

test("focus recovery still refetches if native cache clearing fails", async () => {
  const client = {
    cancelQueries: vi.fn(async () => {}),
    invalidateQueries: vi.fn(async () => {}),
  } as unknown as QueryClient;

  await refreshActiveQueriesAfterNativeFocus(client, async () => {
    throw new Error("native bridge unavailable");
  });

  expect(client.invalidateQueries).toHaveBeenCalledWith({
    refetchType: "active",
  });
});

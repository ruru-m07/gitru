import { QueryClient } from "@tanstack/react-query";
import { describe, expect, test, vi } from "vitest";
import { RepositoryManager } from "../src/state/domains/repository-manager";

describe("RepositoryManager disposal", () => {
  test("cancels and removes every query for a disposed context", async () => {
    const client = new QueryClient();
    const manager = new RepositoryManager(client);
    const alphaStatus = ["repository", "context-a", "/repos/alpha", "status"];
    const alphaDiff = [
      "repository",
      "context-a",
      "/repos/alpha",
      "diff",
      "worktree",
    ];
    const betaStatus = ["repository", "context-b", "/repos/beta", "status"];
    const alphaWorktreeFile = [
      "worktree-file",
      "context-a",
      "src/conflicted.ts",
      0,
    ];

    manager.for("/repos/alpha", "context-a");
    manager.for("/repos/beta", "context-b");
    client.setQueryData(alphaStatus, "alpha-status");
    client.setQueryData(alphaDiff, "alpha-diff");
    client.setQueryData(alphaWorktreeFile, "file contents");
    client.setQueryData(betaStatus, "beta-status");
    const cancel = vi.spyOn(client, "cancelQueries");

    await manager.disposeContext("context-a");

    expect(cancel).toHaveBeenCalledWith({
      queryKey: ["repository", "context-a"],
    });
    expect(cancel).toHaveBeenCalledWith({
      queryKey: ["worktree-file", "context-a"],
    });
    expect(client.getQueryData(alphaStatus)).toBeUndefined();
    expect(client.getQueryData(alphaDiff)).toBeUndefined();
    expect(client.getQueryData(alphaWorktreeFile)).toBeUndefined();
    expect(client.getQueryData(betaStatus)).toBe("beta-status");
  });

  test("repository disposal removes cache instead of invalidating it", async () => {
    const client = new QueryClient();
    const manager = new RepositoryManager(client);
    const status = ["repository", "context-a", "/repos/alpha", "status"];
    const invalidate = vi.spyOn(client, "invalidateQueries");

    manager.for("/repos/alpha/", "context-a");
    client.setQueryData(status, "status");

    await manager.dispose("/repos/alpha", "context-a");

    expect(invalidate).not.toHaveBeenCalled();
    expect(client.getQueryData(status)).toBeUndefined();
  });
});

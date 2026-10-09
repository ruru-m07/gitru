import { QueryClient, QueryObserver } from "@tanstack/react-query";
import { expect, it, vi } from "vitest";
import { refreshActiveQueriesAfterNativeFocus } from "./state-manager";

it("native Git focus refresh leaves collaboration reads to their revision bridge", async () => {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  const gitRead = vi.fn(async () => "git");
  const collaborationRead = vi.fn(async () => "cached");
  const git = new QueryObserver(client, {
    queryKey: ["RepositoryState", "status"],
    queryFn: gitRead,
  });
  const remote = new QueryObserver(client, {
    queryKey: ["collaboration", "account", "a"],
    queryFn: collaborationRead,
  });
  const stopGit = git.subscribe(() => {});
  const stopRemote = remote.subscribe(() => {});
  await Promise.all([git.refetch(), remote.refetch()]);
  const before = collaborationRead.mock.calls.length;
  const invalidateNative = vi.fn(async () => {});
  await refreshActiveQueriesAfterNativeFocus(client, invalidateNative);
  expect(invalidateNative).toHaveBeenCalledOnce();
  expect(collaborationRead.mock.calls.length).toBe(before);
  expect(gitRead.mock.calls.length).toBeGreaterThan(1);
  stopGit();
  stopRemote();
  client.clear();
});

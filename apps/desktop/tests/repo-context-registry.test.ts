import { describe, expect, test, vi } from "vitest";
import {
  createRepoContextOwnerId,
  RepoContextRegistry,
} from "../src/state/core/repo-context-registry";

const deferred = <T>() => {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((resolvePromise) => {
    resolve = resolvePromise;
  });
  return { promise, resolve };
};

const createDependencies = () => ({
  createContext: vi.fn(
    async ({ repoId }: { repoId: string; ownerId: string }) =>
      `context-${repoId}`,
  ),
  disposeContext: vi.fn(async (_contextId: string) => {}),
  disposeRepositoryState: vi.fn(async (_contextId: string) => {}),
});

describe("RepoContextRegistry lifecycle", () => {
  test("builds collision-safe owners from the webview and scope", () => {
    expect(createRepoContextOwnerId("main", "tab-a")).toBe('["main","tab-a"]');
    expect(createRepoContextOwnerId("main:tab", "a")).not.toBe(
      createRepoContextOwnerId("main", "tab:a"),
    );
  });

  test("passes the runtime-specific owner to the backend", async () => {
    const dependencies = createDependencies();
    const registry = new RepoContextRegistry(dependencies);
    const ownerId = createRepoContextOwnerId("main", "tab-a");

    await expect(
      registry.ensureScopeContext("tab-a", "repo-a", ownerId),
    ).resolves.toBe("context-repo-a");

    expect(dependencies.createContext).toHaveBeenCalledWith({
      repoId: "repo-a",
      ownerId,
    });
  });

  test("replaces a context when the same scope moves to another runtime", async () => {
    const dependencies = createDependencies();
    dependencies.createContext
      .mockResolvedValueOnce("context-root")
      .mockResolvedValueOnce("context-child");
    const registry = new RepoContextRegistry(dependencies);
    const rootOwner = createRepoContextOwnerId("main", "tab-a");
    const childOwner = createRepoContextOwnerId("tab-webview:tab-a", "tab-a");

    await registry.ensureScopeContext("tab-a", "repo-a", rootOwner);
    await registry.ensureScopeContext("tab-a", "repo-a", childOwner);

    expect(dependencies.disposeContext).toHaveBeenCalledWith("context-root");
    expect(registry.getScopeContext("tab-a")).toEqual({
      repoId: "repo-a",
      contextId: "context-child",
      ownerId: childOwner,
    });
  });

  test("serializes rapid repository switches without leaking the late context", async () => {
    const firstContext = deferred<string>();
    const dependencies = createDependencies();
    dependencies.createContext.mockImplementation(
      async ({ repoId }: { repoId: string; ownerId: string }) => {
        if (repoId === "repo-a") return firstContext.promise;
        return "context-repo-b";
      },
    );
    const registry = new RepoContextRegistry(dependencies);

    const ownerId = createRepoContextOwnerId("tab-webview:tab-a", "tab-a");
    const first = registry.ensureScopeContext("tab-a", "repo-a", ownerId);
    const second = registry.ensureScopeContext("tab-a", "repo-b", ownerId);

    await vi.waitFor(() => {
      expect(dependencies.createContext).toHaveBeenCalledTimes(1);
    });
    firstContext.resolve("context-repo-a");

    await expect(first).resolves.toBe("context-repo-a");
    await expect(second).resolves.toBe("context-repo-b");

    expect(dependencies.disposeContext).toHaveBeenCalledWith("context-repo-a");
    expect(dependencies.disposeRepositoryState).toHaveBeenCalledWith(
      "context-repo-a",
    );
    expect(registry.getScopeContext("tab-a")).toEqual({
      repoId: "repo-b",
      contextId: "context-repo-b",
      ownerId,
    });
  });

  test("queues scope disposal behind an in-flight context creation", async () => {
    const createdContext = deferred<string>();
    const dependencies = createDependencies();
    dependencies.createContext.mockImplementation(async () => {
      return createdContext.promise;
    });
    const registry = new RepoContextRegistry(dependencies);

    const ensure = registry.ensureScopeContext(
      "tab-a",
      "repo-a",
      createRepoContextOwnerId("main", "tab-a"),
    );
    const dispose = registry.disposeScope("tab-a");

    createdContext.resolve("context-repo-a");
    await ensure;
    await dispose;

    expect(registry.getScopeContext("tab-a")).toBeNull();
    expect(dependencies.disposeContext).toHaveBeenCalledWith("context-repo-a");
    expect(dependencies.disposeRepositoryState).toHaveBeenCalledWith(
      "context-repo-a",
    );
  });

  test("disposeAll includes scopes whose context creation is still pending", async () => {
    const createdContext = deferred<string>();
    const dependencies = createDependencies();
    dependencies.createContext.mockImplementation(async () => {
      return createdContext.promise;
    });
    const registry = new RepoContextRegistry(dependencies);

    const ensure = registry.ensureScopeContext(
      "tab-a",
      "repo-a",
      createRepoContextOwnerId("main", "tab-a"),
    );
    const disposeAll = registry.disposeAll();

    createdContext.resolve("context-repo-a");
    await ensure;
    await disposeAll;

    expect(registry.getScopeContext("tab-a")).toBeNull();
    expect(dependencies.disposeContext).toHaveBeenCalledWith("context-repo-a");
  });
});

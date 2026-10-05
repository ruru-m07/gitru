import type {
  AccountSnapshot,
  CapabilitySnapshot,
  CapabilityTarget,
  ChangePage,
  ContextualCapabilitySnapshot,
  DetailSnapshot,
  ItemPage,
  ItemSnapshot,
  RemoteAccount,
  RemoteItem,
  RepositorySnapshot,
  ResourceLocator,
  ResourceResolution,
} from "@gitru/commands";
import { QueryClient, QueryObserver } from "@tanstack/react-query";
import { describe, expect, it, vi } from "vitest";
import { StaleAuthorizationError } from "./authorization-fence";
import {
  CollaborationClient,
  type CollaborationTransport,
  collaborationErrorMessage,
  collaborationKeys,
} from "./client";

const account: RemoteAccount = {
  id: "account-a",
  provider: "github",
  host: "https://github.com",
  actor_id: "1",
  login: "fixture",
  display_name: null,
  authorization_epoch: "1",
  state: "active",
  notifications_supported: true,
};
const snapshot: AccountSnapshot = {
  accounts: [account],
  revision: "1",
  authorization_view: "1",
};
const page: ItemPage = {
  items: [],
  revision: "1",
  authorization_view: "1",
  next_cursor: null,
  coverage: {
    state: "complete",
    validated_at: "2026-10-02T00:00:00Z",
    remote_has_more: false,
  },
  sync: {
    state: "idle",
    last_success_at: null,
    next_retry_at: null,
    error: null,
  },
};
const changePage = (
  revision: string,
  view = "1",
  changes: ChangePage["changes"] = [],
): ChangePage => ({
  revision,
  authorization_view: view,
  reset_required: false,
  has_more: false,
  changes,
});

function transport(
  overrides: Partial<CollaborationTransport> = {},
): CollaborationTransport {
  const unexpected = async () => {
    throw new Error("Unexpected transport operation");
  };
  return {
    accounts: unexpected,
    connectGithub: unexpected,
    connectGitlab: unexpected,
    connectBitbucketCloud: unexpected,
    discoverGithubCli: unexpected,
    connectGithubCli: unexpected,
    disconnect: unexpected,
    repositories: unexpected,
    selectRepository: unexpected,
    items: unexpected,
    item: unexpected,
    refresh: unexpected,
    changesSince: unexpected,
    saveDraft: unexpected,
    draft: unexpected,
    capabilities: unexpected,
    contextualCapabilities: unexpected,
    resolveResource: unexpected,
    detail: unexpected,
    hydrateDetail: unexpected,
    notificationSubject: unexpected,
    discoverNotificationSubject: unexpected,
    demandActivity: unexpected,
    acquireDemand: unexpected,
    renewDemand: unexpected,
    releaseDemand: unexpected,
    listenDemandActivity: unexpected,
    localLinks: unexpected,
    confirmLocalLink: unexpected,
    removeLocalLink: unexpected,
    saveTransportBinding: unexpected,
    removeTransportBinding: unexpected,
    localClones: unexpected,
    validateLocalNavigation: unexpected,
    listenLocalChanges: async () => () => {},
    listen: unexpected,
    ...overrides,
  };
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((finish) => {
    resolve = finish;
  });
  return { promise, resolve };
}

const locator: ResourceLocator = {
  instance_id: "github:https://github.com/",
  kind: "repository",
  locator_kind: "repository_path",
  value: "owner/project",
  repository_path: null,
};
const capability: CapabilitySnapshot = {
  account_id: account.id,
  instance: {
    id: locator.instance_id,
    provider: "github",
    base_url: "https://github.com/",
  },
  facets: [{ facet: "inbox", state: "supported", reason: null }],
  inbox_semantics: "native_notifications",
  revision: "1",
  authorization_view: "1",
};
const resolution: ResourceResolution = {
  state: "unresolved",
  resource: null,
  candidates: [],
  revision: "1",
  authorization_view: "1",
};

const privateItem: RemoteItem = {
  id: "issue:7",
  account_id: account.id,
  repository_id: "repository:1",
  provider_id: "7",
  kind: "issue",
  number: "7",
  title: "Private issue",
  body: "old private body",
  body_omitted: false,
  author: "fixture",
  web_url: "https://github.com/owner/project/issues/7",
  state: "open",
  updated_at: "2026-10-02T00:00:00Z",
  head_oid: null,
  is_draft: null,
  reason: null,
  unread: null,
};
const itemQuery = {
  kind: "issue" as const,
  repository_id: privateItem.repository_id,
  state: null,
  search: null,
  cursor: null,
  limit: 50,
};

describe("CollaborationClient", () => {
  it("fences old Bitbucket repository reads after accepted reconnect while keeping UUID actors and epoch keys distinct", async () => {
    const bitbucket: RemoteAccount = {
      ...account,
      id: "bitbucket-account-a",
      provider: "bitbucket_cloud",
      host: "bitbucket.org",
      actor_id: "11111111-1111-4111-8111-111111111111",
      notifications_supported: false,
    };
    const peer: RemoteAccount = {
      ...bitbucket,
      id: "bitbucket-account-b",
      actor_id: "22222222-2222-4222-8222-222222222222",
    };
    const replacement = {
      ...bitbucket,
      authorization_epoch: "9007199254740994",
    };
    const repositories: RepositorySnapshot = {
      repositories: [],
      revision: "1",
      authorization_view: "1",
      coverage: page.coverage,
      sync: page.sync,
    };
    const delayed = deferred<RepositorySnapshot>();
    let connected = false;
    const connectBitbucketCloud = vi.fn(async () => {
      connected = true;
      return replacement;
    });
    const client = new CollaborationClient(
      transport({
        connectBitbucketCloud,
        accounts: async () => ({
          ...snapshot,
          accounts: [connected ? replacement : bitbucket, peer, account],
        }),
        repositories: () => delayed.promise,
        listen: async () => () => {},
        changesSince: async () => changePage("1"),
      }),
    );
    const cache = new QueryClient();
    const stop = client.installBridge(cache);
    await client.wake();
    await client.accounts();
    const key = collaborationKeys.repositories(bitbucket);
    cache.setQueryData(key, repositories);
    const pending = client.forAccount(bitbucket).repositories();
    expect(
      await client.connectBitbucketCloud("synthetic-bitbucket-api-token"),
    ).toBe(replacement);
    expect(connectBitbucketCloud).toHaveBeenCalledExactlyOnceWith(
      "synthetic-bitbucket-api-token",
    );
    expect(cache.getQueryData(key)).toBeUndefined();
    delayed.resolve(repositories);
    await expect(pending).rejects.toBeInstanceOf(StaleAuthorizationError);
    expect(replacement.actor_id).toBe(bitbucket.actor_id);
    expect(replacement.authorization_epoch).toBe("9007199254740994");
    expect((await client.accounts()).accounts).toEqual([
      replacement,
      peer,
      account,
    ]);
    const keys = [bitbucket, replacement, peer, account].map((actor) =>
      JSON.stringify(collaborationKeys.repositories(actor)),
    );
    expect(new Set(keys).size).toBe(4);
    expect(peer.login).toBe(bitbucket.login);
    stop();
    cache.clear();
  });

  it("preserves cached repositories and an in-flight saved read when Bitbucket token verification fails", async () => {
    const repositories: RepositorySnapshot = {
      repositories: [],
      revision: "1",
      authorization_view: "1",
      coverage: page.coverage,
      sync: page.sync,
    };
    const delayed = deferred<RepositorySnapshot>();
    const failure = { code: "permission_denied" };
    const connectBitbucketCloud = vi.fn().mockRejectedValue(failure);
    const client = new CollaborationClient(
      transport({
        connectBitbucketCloud,
        accounts: async () => snapshot,
        repositories: () => delayed.promise,
        listen: async () => () => {},
        changesSince: async () => changePage("1"),
      }),
    );
    const cache = new QueryClient();
    const stop = client.installBridge(cache);
    await client.wake();
    await client.accounts();
    const version = client.getVersion();
    const key = collaborationKeys.repositories(account);
    cache.setQueryData(key, repositories);
    const pending = client.forAccount(account).repositories();
    await expect(
      client.connectBitbucketCloud("synthetic-rejected-api-token"),
    ).rejects.toBe(failure);
    expect(client.getVersion()).toBe(version);
    expect(cache.getQueryData(key)).toBe(repositories);
    delayed.resolve(repositories);
    await expect(pending).resolves.toEqual(repositories);
    expect(connectBitbucketCloud).toHaveBeenCalledExactlyOnceWith(
      "synthetic-rejected-api-token",
    );
    stop();
    cache.clear();
  });

  it("connects GitLab through its own transport and fences pending old-account projections after accepted cutover", async () => {
    const gitlab: RemoteAccount = {
      ...account,
      id: "gitlab-account",
      provider: "gitlab",
      host: "gitlab.com",
      actor_id: "9007199254740993",
      login: account.login,
      notifications_supported: false,
    };
    const delayed = deferred<ItemPage>();
    const connectGitlab = vi.fn(async () => gitlab);
    const client = new CollaborationClient(
      transport({
        connectGitlab,
        accounts: async () => snapshot,
        items: () => delayed.promise,
        listen: async () => () => {},
        changesSince: async () => changePage("1"),
      }),
    );
    const cache = new QueryClient();
    const stop = client.installBridge(cache);
    await client.wake();
    await client.accounts();
    const oldKey = collaborationKeys.item(account, "private-item");
    cache.setQueryData(oldKey, { private: true });
    const pending = client.forAccount(account).items(itemQuery);
    expect(await client.connectGitlab("synthetic-gitlab-pat")).toEqual(gitlab);
    expect(connectGitlab).toHaveBeenCalledExactlyOnceWith(
      "synthetic-gitlab-pat",
    );
    expect(cache.getQueryData(oldKey)).toBeUndefined();
    delayed.resolve(page);
    await expect(pending).rejects.toBeInstanceOf(StaleAuthorizationError);
    expect(gitlab.actor_id).toBe("9007199254740993");
    expect(collaborationKeys.repositories(gitlab)).not.toEqual(
      collaborationKeys.repositories(account),
    );
    stop();
    cache.clear();
  });

  it("keeps current saved projections and pending reads when GitLab credential validation fails", async () => {
    const delayed = deferred<ItemPage>();
    const failure = { code: "permission_denied" };
    const connectGitlab = vi.fn().mockRejectedValue(failure);
    const client = new CollaborationClient(
      transport({
        connectGitlab,
        accounts: async () => snapshot,
        items: () => delayed.promise,
        listen: async () => () => {},
        changesSince: async () => changePage("1"),
      }),
    );
    const cache = new QueryClient();
    const stop = client.installBridge(cache);
    await client.wake();
    await client.accounts();
    const version = client.getVersion();
    const key = collaborationKeys.item(account, "saved-item");
    cache.setQueryData(key, { private: true });
    const pending = client.forAccount(account).items(itemQuery);
    await expect(client.connectGitlab("synthetic-rejected-pat")).rejects.toBe(
      failure,
    );
    expect(client.getVersion()).toBe(version);
    expect(cache.getQueryData(key)).toEqual({ private: true });
    delayed.resolve(page);
    await expect(pending).resolves.toEqual(page);
    stop();
    cache.clear();
  });

  it("binds contextual requests to epoch and canonical target and cancels initial pending policy on same-epoch changes", async () => {
    const target: CapabilityTarget = {
      kind: "resource",
      instance_id: "github:https://github.com/",
      repository_id: null,
      resource_id: "pull:67",
      resource_kind: "pull_request",
    };
    const old: ContextualCapabilitySnapshot = {
      ...capability,
      authorization_epoch: account.authorization_epoch,
      target,
      facets: [],
    };
    const current: ContextualCapabilitySnapshot = {
      ...old,
      revision: "2",
      facets: [
        {
          facet: "reviews",
          saved_read: { state: "unsupported", reason: "not_implemented" },
          synchronize: { state: "unsupported", reason: "not_implemented" },
          remote_write: { state: "unsupported", reason: "not_implemented" },
          observation: "unknown",
          sync: page.sync,
          can_recheck_access: false,
        },
      ],
    };
    let next = changePage("1");
    const delayed = deferred<ContextualCapabilitySnapshot>();
    const read = vi
      .fn()
      .mockImplementationOnce(() => delayed.promise)
      .mockResolvedValue(current);
    const client = new CollaborationClient(
      transport({
        listen: async () => () => {},
        changesSince: async () => next,
        contextualCapabilities: read,
      }),
    );
    const cache = new QueryClient();
    const stop = client.installBridge(cache);
    await client.wake();
    const key = collaborationKeys.contextualCapabilities(account, target);
    expect(key).not.toEqual(
      collaborationKeys.contextualCapabilities(
        { ...account, authorization_epoch: "2" },
        target,
      ),
    );
    expect(key).not.toEqual(
      collaborationKeys.contextualCapabilities(account, {
        ...target,
        resource_id: "pull:68",
      }),
    );
    const unrelated = collaborationKeys.contextualCapabilities(
      { ...account, id: "actor-b" },
      target,
    );
    cache.setQueryData(unrelated, current);
    const draft = collaborationKeys.draft(account, "draft");
    cache.setQueryData(draft, "authored text");
    let signal!: AbortSignal;
    const observer = new QueryObserver(cache, {
      queryKey: key,
      queryFn: ({ signal: abort }) => {
        signal ??= abort;
        return client.forAccount(account).contextualCapabilities(target, abort);
      },
      staleTime: Infinity,
      retry: false,
    });
    const unsubscribe = observer.subscribe(() => {});
    expect(read).toHaveBeenCalledWith({
      account_id: account.id,
      authorization_epoch: "1",
      target,
    });
    expect(cache.getQueryData(key)).toBeUndefined();
    next = changePage("2", "1", [
      {
        revision: "2",
        account_id: account.id,
        scope: "detail:pull:67:reviews",
        reset: false,
      },
    ]);
    await client.wake();
    await vi.waitFor(() => expect(cache.getQueryData(key)).toEqual(current));
    expect(read).toHaveBeenCalledTimes(2);
    expect(signal.aborted).toBe(true);
    delayed.resolve(old);
    await delayed.promise;
    await Promise.resolve();
    expect(cache.getQueryData(key)).toEqual(current);
    expect(cache.getQueryState(unrelated)?.isInvalidated).toBe(false);
    expect(cache.getQueryState(draft)?.isInvalidated).toBe(false);
    unsubscribe();
    stop();
    cache.clear();
  });

  it("removes contextual eligibility on an epoch reset and rejects a late prior-actor response", async () => {
    const target: CapabilityTarget = {
      kind: "account",
      instance_id: null,
      repository_id: null,
      resource_id: null,
      resource_kind: null,
    };
    const old: ContextualCapabilitySnapshot = {
      ...capability,
      authorization_epoch: "1",
      target,
      facets: [],
    };
    const delayed = deferred<ContextualCapabilitySnapshot>();
    let next = changePage("1");
    const client = new CollaborationClient(
      transport({
        listen: async () => () => {},
        changesSince: async () => next,
        contextualCapabilities: () => delayed.promise,
      }),
    );
    const cache = new QueryClient();
    const stop = client.installBridge(cache);
    await client.wake();
    const key = collaborationKeys.contextualCapabilities(account, target);
    const observer = new QueryObserver(cache, {
      queryKey: key,
      queryFn: ({ signal }) =>
        client.forAccount(account).contextualCapabilities(target, signal),
      staleTime: Infinity,
      retry: false,
    });
    const unsubscribe = observer.subscribe(() => {});
    next = changePage("2", "2", [
      { revision: "2", account_id: account.id, scope: "account", reset: true },
    ]);
    await client.wake();
    delayed.resolve(old);
    await delayed.promise;
    await Promise.resolve();
    expect(cache.getQueryState(key)).toBeUndefined();
    expect(cache.getQueryData(key)).toBeUndefined();
    unsubscribe();
    stop();
    cache.clear();
  });
  it("binds cache-only detail reads and explicit hydration without starting refresh from a read", async () => {
    const saved: DetailSnapshot = {
      subject_id: "pull",
      body: { state: "known", text: null },
      metadata: null,
      entries: [],
      next_cursor: null,
      revision: "1",
      authorization_view: "1",
      evidence: {
        facet: "body",
        availability: "ready",
        coverage: {
          state: "complete",
          validated_at: "2026-10-03T00:00:00Z",
          remote_has_more: false,
        },
        freshness: "stale",
        stale_at: "2026-10-03T00:01:00Z",
        facet_revision: "1",
        authorization_epoch: "1",
        access_reason: null,
        source: {
          source: "fixture/body/v1",
          adapter_version: 1,
          field_mask: ["body"],
          provider_updated_at: null,
          observed_at: "2026-10-03T00:00:00Z",
        },
        value_source: null,
        saved_empty: null,
        observed_state: "known",
        sync: page.sync,
      },
    };
    const detail = vi.fn().mockResolvedValue(saved);
    const hydrateDetail = vi.fn().mockResolvedValue({ job_id: "coalesced" });
    const refresh = vi.fn();
    const client = new CollaborationClient(
      transport({ detail, hydrateDetail, refresh }),
    );
    const handle = client.forAccount(account);
    const query = {
      subject_id: "pull",
      facet: "body" as const,
      cursor: null,
      limit: 100,
    };
    expect(await handle.detail(query)).toEqual(saved);
    expect(detail).toHaveBeenCalledWith({ ...query, account_id: account.id });
    expect(hydrateDetail).not.toHaveBeenCalled();
    expect(refresh).not.toHaveBeenCalled();
    expect(
      await handle.hydrateDetail({ subject_id: "pull", facet: "body" }),
    ).toEqual({ job_id: "coalesced" });
    expect(hydrateDetail).toHaveBeenCalledWith({
      subject_id: "pull",
      facet: "body",
      account_id: account.id,
      authorization_epoch: account.authorization_epoch,
    });
  });

  it("captures hydration authorization from the account handle and rejects late receipts after reconnect", async () => {
    const oldReceipt = deferred<{ job_id: string }>();
    const replacement = { ...account, authorization_epoch: "2" };
    const hydrateDetail = vi
      .fn()
      .mockImplementationOnce(() => oldReceipt.promise)
      .mockRejectedValue({ code: "stale_view" });
    const client = new CollaborationClient(
      transport({ hydrateDetail, connectGithub: async () => replacement }),
    );
    const oldHandle = client.forAccount(account);
    const request = { subject_id: "pull", facet: "body" as const };
    const pending = oldHandle.hydrateDetail(request);
    await client.connectGithub("fixture");
    oldReceipt.resolve({ job_id: "old-grant" });
    await expect(pending).rejects.toBeInstanceOf(StaleAuthorizationError);
    await expect(oldHandle.hydrateDetail(request)).rejects.toEqual({
      code: "stale_view",
    });
    expect(hydrateDetail).toHaveBeenLastCalledWith({
      ...request,
      account_id: account.id,
      authorization_epoch: "1",
    });
  });

  it("cancels initial pending detail reads on matching facet changes and preserves other facets and drafts", async () => {
    let next = changePage("1");
    const old = deferred<DetailSnapshot>();
    const saved: DetailSnapshot = {
      subject_id: "pull",
      body: { state: "not_loaded", text: null },
      metadata: null,
      entries: [],
      next_cursor: null,
      revision: "2",
      authorization_view: "1",
      evidence: {
        facet: "comments",
        availability: "ready",
        coverage: {
          state: "complete",
          validated_at: "2026-10-03T00:00:00Z",
          remote_has_more: false,
        },
        freshness: "stale",
        stale_at: null,
        facet_revision: "2",
        authorization_epoch: "1",
        access_reason: null,
        source: null,
        value_source: null,
        saved_empty: null,
        observed_state: "known",
        sync: page.sync,
      },
    };
    const detail = vi
      .fn<() => Promise<DetailSnapshot>>()
      .mockImplementationOnce(() => old.promise)
      .mockResolvedValue(saved);
    const client = new CollaborationClient(
      transport({
        detail,
        listen: async () => () => {},
        changesSince: async () => next,
      }),
    );
    const cache = new QueryClient();
    const stop = client.installBridge(cache);
    await client.wake();
    const query = {
      account_id: account.id,
      subject_id: "pull",
      facet: "comments" as const,
      cursor: null,
      limit: 100,
    };
    const key = collaborationKeys.detail(account, query);
    const other = collaborationKeys.detail(account, {
      ...query,
      facet: "reviews",
    });
    const draft = collaborationKeys.draft(account, "pull");
    cache.setQueryData(other, "saved other facet");
    cache.setQueryData(draft, "authored draft");
    const observer = new QueryObserver(cache, {
      queryKey: key,
      queryFn: ({ signal }) => client.forAccount(account).detail(query, signal),
      staleTime: Infinity,
      retry: false,
    });
    const unsubscribe = observer.subscribe(() => {});
    next = changePage("2", "1", [
      {
        revision: "2",
        account_id: account.id,
        scope: "detail:pull:comments",
        reset: false,
      },
    ]);
    await client.wake();
    await vi.waitFor(() => expect(cache.getQueryData(key)).toEqual(saved));
    old.resolve({
      ...saved,
      revision: "1",
      evidence: {
        ...saved.evidence,
        availability: "missing",
        facet_revision: null,
      },
    });
    await old.promise;
    await Promise.resolve();
    expect(detail).toHaveBeenCalledTimes(2);
    expect(cache.getQueryData(key)).toEqual(saved);
    expect(cache.getQueryState(other)?.isInvalidated).toBe(false);
    expect(cache.getQueryState(draft)?.isInvalidated).toBe(false);
    unsubscribe();
    stop();
    cache.clear();
  });
  it.each([
    "repositories",
    "items",
    "item",
  ] as const)("refetches an initial pending %s projection on a same-epoch update", async (projection) => {
    type ProviderSnapshot = RepositorySnapshot | ItemPage | ItemSnapshot;
    const beforeRepository: RepositorySnapshot = {
      repositories: [
        {
          id: "repository:1",
          account_id: account.id,
          provider_id: "1",
          full_name: "owner/project",
          name: "project",
          description: "old private description",
          web_url: "https://github.com/owner/project",
          default_branch: "main",
          selected: true,
        },
      ],
      revision: "1",
      authorization_view: "1",
      coverage: page.coverage,
      sync: page.sync,
    };
    const before =
      projection === "repositories"
        ? beforeRepository
        : projection === "items"
          ? { ...page, items: [privateItem] }
          : { item: privateItem, revision: "1", authorization_view: "1" };
    const currentItem = { ...privateItem, body: "current private body" };
    const current =
      projection === "repositories"
        ? {
            ...beforeRepository,
            revision: "2",
            repositories: beforeRepository.repositories.map((repository) => ({
              ...repository,
              full_name: "owner/renamed-project",
              description: "current private description",
            })),
          }
        : projection === "items"
          ? { ...page, revision: "2", items: [currentItem] }
          : { item: currentItem, revision: "2", authorization_view: "1" };
    let next = changePage("1");
    const oldRead = deferred<ProviderSnapshot>();
    const read = vi
      .fn<() => Promise<ProviderSnapshot>>()
      .mockImplementationOnce(() => oldRead.promise)
      .mockResolvedValue(current);
    const client = new CollaborationClient(
      transport({
        listen: async () => () => {},
        changesSince: async () => next,
        repositories: () => read() as Promise<RepositorySnapshot>,
        items: () => read() as Promise<ItemPage>,
        item: () => read() as Promise<ItemSnapshot>,
      }),
    );
    const cache = new QueryClient();
    const stop = client.installBridge(cache);
    await client.wake();
    const key =
      projection === "repositories"
        ? collaborationKeys.repositories(account)
        : projection === "items"
          ? collaborationKeys.items(account, {
              ...itemQuery,
              account_id: account.id,
            })
          : collaborationKeys.item(account, privateItem.id);
    const observer = new QueryObserver<ProviderSnapshot>(cache, {
      queryKey: key,
      queryFn: ({ signal }) => {
        const handle = client.forAccount(account);
        return projection === "repositories"
          ? handle.repositories(signal)
          : projection === "items"
            ? handle.items(itemQuery, signal)
            : handle.item(privateItem.id, signal);
      },
      staleTime: Infinity,
      retry: false,
    });
    const unsubscribe = observer.subscribe(() => {});
    expect(read).toHaveBeenCalledTimes(1);
    expect(cache.getQueryData(key)).toBeUndefined();
    next = changePage("2", "1", [
      {
        revision: "2",
        account_id: account.id,
        scope: "repositories",
        reset: false,
      },
    ]);
    await client.wake();
    await vi.waitFor(() => expect(cache.getQueryData(key)).toEqual(current));
    oldRead.resolve(before);
    await oldRead.promise;
    await Promise.resolve();
    expect(read).toHaveBeenCalledTimes(2);
    expect(cache.getQueryData(key)).toEqual(current);
    unsubscribe();
    stop();
    cache.clear();
  });

  it("drops a delayed private item when a same-epoch scope denial resets the authorization view", async () => {
    let next = changePage("1");
    const oldRead = deferred<ItemSnapshot>();
    const client = new CollaborationClient(
      transport({
        listen: async () => () => {},
        changesSince: async () => next,
        item: () => oldRead.promise,
      }),
    );
    const cache = new QueryClient();
    const stop = client.installBridge(cache);
    await client.wake();
    const key = collaborationKeys.item(account, privateItem.id);
    const observer = new QueryObserver(cache, {
      queryKey: key,
      queryFn: ({ signal }) =>
        client.forAccount(account).item(privateItem.id, signal),
      staleTime: Infinity,
      retry: false,
    });
    const unsubscribe = observer.subscribe(() => {});
    next = changePage("2", "2", [
      {
        revision: "2",
        account_id: account.id,
        scope: `repo:${privateItem.repository_id}:issue`,
        reset: true,
      },
    ]);
    await client.wake();
    oldRead.resolve({
      item: privateItem,
      revision: "1",
      authorization_view: "1",
    });
    await oldRead.promise;
    await Promise.resolve();
    expect(cache.getQueryData(key)).toBeUndefined();
    expect(cache.getQueryState(key)).toBeUndefined();
    unsubscribe();
    stop();
    cache.clear();
  });

  it.each([
    "capabilities",
    "resource",
  ] as const)("cancels the initial %s fetch before same-epoch invalidation and keeps other accounts and drafts intact", async (projection) => {
    let next = changePage("1");
    const oldRead = deferred<CapabilitySnapshot | ResourceResolution>();
    const current =
      projection === "capabilities"
        ? {
            ...capability,
            revision: "2",
            facets: [
              {
                facet: "inbox" as const,
                state: "unavailable" as const,
                reason: "permission_denied" as const,
              },
            ],
          }
        : {
            ...resolution,
            revision: "2",
            state: "resolved" as const,
            resource: {
              account_id: account.id,
              instance_id: locator.instance_id,
              id: "repository:7",
              kind: "repository" as const,
              provider_id: "7",
            },
          };
    const read = vi
      .fn<() => Promise<CapabilitySnapshot | ResourceResolution>>()
      .mockImplementationOnce(() => oldRead.promise)
      .mockResolvedValue(current);
    const client = new CollaborationClient(
      transport({
        listen: async () => () => {},
        changesSince: async () => next,
        capabilities: () => read() as Promise<CapabilitySnapshot>,
        resolveResource: () => read() as Promise<ResourceResolution>,
      }),
    );
    const cache = new QueryClient();
    const stop = client.installBridge(cache);
    await client.wake();
    const key =
      projection === "capabilities"
        ? collaborationKeys.capabilities(account)
        : collaborationKeys.resource(account, locator);
    const otherAccount = { ...account, id: "account-b" };
    const otherKey =
      projection === "capabilities"
        ? collaborationKeys.capabilities(otherAccount)
        : collaborationKeys.resource(otherAccount, locator);
    const draftKey = collaborationKeys.draft(account, "draft");
    cache.setQueryData(otherKey, "other account");
    cache.setQueryData(draftKey, "authored draft");
    let initialSignal!: AbortSignal;
    const observer = new QueryObserver<CapabilitySnapshot | ResourceResolution>(
      cache,
      {
        queryKey: key,
        queryFn: ({ signal }) => {
          initialSignal ??= signal;
          return projection === "capabilities"
            ? client.forAccount(account).capabilities(signal)
            : client.forAccount(account).resolveResource(locator, signal);
        },
        staleTime: Infinity,
        networkMode: "always",
        retry: false,
      },
    );
    const unsubscribe = observer.subscribe(() => {});
    expect(read).toHaveBeenCalledTimes(1);
    expect(cache.getQueryData(key)).toBeUndefined();
    next = changePage("2", "1", [
      {
        revision: "2",
        account_id: account.id,
        scope: projection === "capabilities" ? "notifications" : "repositories",
        reset: false,
      },
    ]);
    await client.wake();
    await vi.waitFor(() => expect(cache.getQueryData(key)).toEqual(current));
    expect(read).toHaveBeenCalledTimes(2);
    expect(initialSignal.aborted).toBe(true);
    oldRead.resolve(projection === "capabilities" ? capability : resolution);
    await oldRead.promise;
    await Promise.resolve();
    expect(cache.getQueryData(key)).toEqual(current);
    expect(cache.getQueryState(key)?.isInvalidated).toBe(false);
    expect(cache.getQueryState(otherKey)?.isInvalidated).toBe(false);
    expect(cache.getQueryData(otherKey)).toBe("other account");
    expect(cache.getQueryState(draftKey)?.isInvalidated).toBe(false);
    expect(cache.getQueryData(draftKey)).toBe("authored draft");
    unsubscribe();
    stop();
    cache.clear();
  });

  it("removes a pending capability query after an authorization reset and ignores its late response", async () => {
    let next = changePage("1");
    const oldRead = deferred<CapabilitySnapshot>();
    const client = new CollaborationClient(
      transport({
        listen: async () => () => {},
        changesSince: async () => next,
        capabilities: () => oldRead.promise,
      }),
    );
    const cache = new QueryClient();
    const stop = client.installBridge(cache);
    await client.wake();
    const key = collaborationKeys.capabilities(account);
    const observer = new QueryObserver(cache, {
      queryKey: key,
      queryFn: ({ signal }) => client.forAccount(account).capabilities(signal),
      staleTime: Infinity,
      retry: false,
    });
    const unsubscribe = observer.subscribe(() => {});
    next = changePage("2", "2", [
      {
        revision: "2",
        account_id: account.id,
        scope: "account",
        reset: true,
      },
    ]);
    await client.wake();
    oldRead.resolve(capability);
    await oldRead.promise;
    await Promise.resolve();
    expect(cache.getQueryData(key)).toBeUndefined();
    expect(cache.getQueryState(key)).toBeUndefined();
    unsubscribe();
    stop();
    cache.clear();
  });

  it("binds capability and local resolver reads to the account and fences late lifecycle responses", async () => {
    const locator: ResourceLocator = {
      instance_id: "github:https://github.com/",
      kind: "pull_request",
      locator_kind: "repository_number",
      value: "67",
      repository_path: "owner/project",
    };
    const capability: CapabilitySnapshot = {
      account_id: account.id,
      instance: {
        id: locator.instance_id,
        provider: "github",
        base_url: "https://github.com/",
      },
      facets: [],
      inbox_semantics: "native_notifications",
      revision: "1",
      authorization_view: "1",
    };
    const resolution: ResourceResolution = {
      state: "unresolved",
      resource: null,
      candidates: [],
      revision: "1",
      authorization_view: "1",
    };
    let finish!: (result: ResourceResolution) => void;
    const capabilities = vi.fn().mockResolvedValue(capability);
    const resolveResource = vi.fn(
      () =>
        new Promise<ResourceResolution>((resolve) => {
          finish = resolve;
        }),
    );
    const client = new CollaborationClient(
      transport({ capabilities, resolveResource, disconnect: async () => "2" }),
    );
    const handle = client.forAccount(account);
    expect(await handle.capabilities()).toEqual(capability);
    expect(capabilities).toHaveBeenCalledWith(account.id);
    const pending = handle.resolveResource(locator);
    expect(resolveResource).toHaveBeenCalledWith(account.id, locator);
    await client.disconnect(account.id);
    finish(resolution);
    await expect(pending).rejects.toBeInstanceOf(StaleAuthorizationError);
  });

  it("refreshes unresolved identities and capability observations on provider changes without invalidating drafts", async () => {
    let next = changePage("1");
    const client = new CollaborationClient(
      transport({
        listen: async () => () => {},
        changesSince: async () => next,
      }),
    );
    const cache = new QueryClient();
    const stop = client.installBridge(cache);
    await client.wake();
    const locator: ResourceLocator = {
      instance_id: "github:https://github.com/",
      kind: "repository",
      locator_kind: "repository_path",
      value: "owner/project",
      repository_path: null,
    };
    const resourceKey = collaborationKeys.resource(account, locator);
    const capabilityKey = collaborationKeys.capabilities(account);
    const draftKey = collaborationKeys.draft(account, "draft");
    for (const key of [resourceKey, capabilityKey, draftKey])
      cache.setQueryData(key, "cached");
    next = changePage("2", "1", [
      {
        revision: "2",
        account_id: account.id,
        scope: "repositories",
        reset: false,
      },
    ]);
    await client.wake();
    expect(cache.getQueryState(resourceKey)?.isInvalidated).toBe(true);
    expect(cache.getQueryState(capabilityKey)?.isInvalidated).toBe(true);
    expect(cache.getQueryState(draftKey)?.isInvalidated).toBe(false);
    stop();
    cache.clear();
  });
  it("discovers only CLI metadata without changing the saved authorization view", async () => {
    const discovery = {
      status: "available" as const,
      accounts: [
        {
          id: "opaque-candidate",
          login: "fixture",
          host: "github.com",
          active: true,
          availability: "ready" as const,
        },
      ],
    };
    const discoverGithubCli = vi.fn().mockResolvedValue(discovery);
    const client = new CollaborationClient(transport({ discoverGithubCli }));
    expect(await client.discoverGithubCli()).toEqual(discovery);
    expect(discoverGithubCli).toHaveBeenCalledWith();
    expect(client.getVersion()).toBe(0);
  });

  it("connects a CLI account using its opaque candidate and fences obsolete private reads", async () => {
    let finish!: (snapshot: typeof page) => void;
    const connectGithubCli = vi.fn().mockResolvedValue(account);
    const client = new CollaborationClient(
      transport({
        connectGithubCli,
        items: () =>
          new Promise((resolve) => {
            finish = resolve;
          }),
      }),
    );
    const oldRead = client.forAccount(account).items({
      kind: "issue",
      repository_id: null,
      state: null,
      search: null,
      cursor: null,
      limit: 50,
    });
    expect(await client.connectGithubCli("opaque-candidate")).toBe(account);
    expect(connectGithubCli).toHaveBeenCalledWith("opaque-candidate");
    expect(client.getVersion()).toBe(1);
    finish(page);
    await expect(oldRead).rejects.toBeInstanceOf(StaleAuthorizationError);
  });

  it("keeps local account views intact when CLI credential import fails", async () => {
    const client = new CollaborationClient(
      transport({
        connectGithubCli: async () => {
          throw { code: "auth_required" };
        },
      }),
    );
    await expect(client.connectGithubCli("expired-candidate")).rejects.toEqual({
      code: "auth_required",
    });
    expect(client.getVersion()).toBe(0);
  });

  it("creates inert account handles and reads locally without provider refresh", async () => {
    const items = vi.fn().mockResolvedValue(page);
    const refresh = vi.fn();
    const client = new CollaborationClient(
      transport({ accounts: async () => snapshot, items, refresh }),
    );
    const handle = client.forAccount(account);
    expect(items).not.toHaveBeenCalled();
    await client.accounts();
    await handle.items({
      kind: "issue",
      repository_id: null,
      state: "open",
      search: null,
      cursor: null,
      limit: 50,
    });
    expect(items).toHaveBeenCalledWith({
      account_id: account.id,
      kind: "issue",
      repository_id: null,
      state: "open",
      search: null,
      cursor: null,
      limit: 50,
    });
    expect(refresh).not.toHaveBeenCalled();
  });

  it("partitions query keys by account authorization epoch", () => {
    expect(collaborationKeys.item(account, "item")).not.toEqual(
      collaborationKeys.item({ ...account, authorization_epoch: "2" }, "item"),
    );
    expect(collaborationKeys.item(account, "item")).not.toEqual(
      collaborationKeys.item({ ...account, id: "account-b" }, "item"),
    );
  });

  it("removes account projections immediately and fences late reads on disconnect", async () => {
    let finishDisconnect!: (value: string) => void;
    let finishRead!: (value: ItemPage) => void;
    const client = new CollaborationClient(
      transport({
        accounts: async () => snapshot,
        listen: async () => () => {},
        changesSince: async () => changePage("1"),
        disconnect: () =>
          new Promise((resolve) => {
            finishDisconnect = resolve;
          }),
        items: () =>
          new Promise((resolve) => {
            finishRead = resolve;
          }),
      }),
    );
    const cache = new QueryClient();
    const stop = client.installBridge(cache);
    await client.wake();
    await client.accounts();
    cache.setQueryData(collaborationKeys.item(account, "private-item"), {
      private: true,
    });
    const pendingRead = client.forAccount(account).items({
      kind: "issue",
      repository_id: null,
      state: null,
      search: null,
      cursor: null,
      limit: 50,
    });
    const disconnected = client.disconnect(account.id);
    expect(
      cache.getQueryData(collaborationKeys.item(account, "private-item")),
    ).toBeUndefined();
    finishRead(page);
    await expect(pendingRead).rejects.toBeInstanceOf(StaleAuthorizationError);
    finishDisconnect("2");
    await disconnected;
    stop();
    cache.clear();
  });

  it("rejects an obsolete authorization view and does not render raw error data", async () => {
    const client = new CollaborationClient(
      transport({
        accounts: async () => snapshot,
        items: async () => ({
          ...page,
          authorization_view: "0",
          revision: "0",
        }),
      }),
    );
    await client.accounts();
    await expect(
      client.forAccount(account).items({
        kind: "issue",
        repository_id: null,
        state: null,
        search: null,
        cursor: null,
        limit: 50,
      }),
    ).rejects.toBeInstanceOf(StaleAuthorizationError);
    expect(
      collaborationErrorMessage({
        code: "provider",
        message: "private token ghp_something",
      }),
    ).not.toContain("ghp_");
    expect(collaborationErrorMessage("secret payload")).not.toContain("secret");
  });

  it("invalidates only matching repository item scopes and leaves drafts untouched", async () => {
    let next = changePage("1");
    const client = new CollaborationClient(
      transport({
        listen: async () => () => {},
        changesSince: async () => next,
      }),
    );
    const cache = new QueryClient();
    const stop = client.installBridge(cache);
    await client.wake();
    const issueKey = collaborationKeys.items(account, {
      account_id: account.id,
      kind: "issue",
      repository_id: "repo-a",
      state: null,
      search: null,
      cursor: null,
      limit: 50,
    });
    const otherKey = collaborationKeys.items(account, {
      account_id: account.id,
      kind: "issue",
      repository_id: "repo-b",
      state: null,
      search: null,
      cursor: null,
      limit: 50,
    });
    const draftKey = collaborationKeys.draft(account, "draft");
    cache.setQueryData(issueKey, page);
    cache.setQueryData(otherKey, page);
    cache.setQueryData(draftKey, "draft");
    next = changePage("2", "1", [
      {
        revision: "2",
        account_id: account.id,
        scope: "repo:repo-a:issue",
        reset: false,
      },
    ]);
    await client.wake();
    expect(cache.getQueryState(issueKey)?.isInvalidated).toBe(true);
    expect(cache.getQueryState(otherKey)?.isInvalidated).toBe(false);
    expect(cache.getQueryState(draftKey)?.isInvalidated).toBe(false);
    stop();
    cache.clear();
  });

  it("fences draft commit responses after an account lifecycle reset", async () => {
    let finish!: (draft: {
      account_id: string;
      subject_id: string;
      body: string;
      generation: string;
    }) => void;
    const client = new CollaborationClient(
      transport({
        disconnect: async () => "2",
        saveDraft: () =>
          new Promise((resolve) => {
            finish = resolve;
          }),
      }),
    );
    const saved = client.forAccount(account).saveDraft({
      subject_id: "item",
      body: "private draft",
      generation: "0",
    });
    await client.disconnect(account.id);
    finish({
      account_id: account.id,
      subject_id: "item",
      body: "private draft",
      generation: "1",
    });
    await expect(saved).rejects.toBeInstanceOf(StaleAuthorizationError);
  });
});

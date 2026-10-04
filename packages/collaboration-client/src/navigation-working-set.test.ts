import type {
  CapabilityTarget,
  ChangePage,
  ContextualCapabilitySnapshot,
  DetailSnapshot,
  ItemSnapshot,
  RemoteAccount,
} from "@gitru/commands";
import {
  onlineManager,
  QueryClient,
  QueryObserver,
} from "@tanstack/react-query";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  CollaborationClient,
  type CollaborationTransport,
  collaborationKeys,
} from "./client";
import { DemandCoordinator } from "./demand-coordinator";
import {
  NAVIGATION_PREFETCH_LIMITS as limits,
  type NavigationResource,
  type NavigationSource,
  NavigationWorkingSet,
} from "./navigation-working-set";

const account: RemoteAccount = {
  id: "account-a",
  actor_id: "actor-a",
  authorization_epoch: "1",
  provider: "github",
  host: "github.com",
  state: "active",
  login: "fixture",
  display_name: null,
  notifications_supported: true,
};
const instanceId = "github:https://github.com/";
const stamp = { revision: "1", authorization_view: "1" };
const target = (resource: NavigationResource): CapabilityTarget => ({
  kind: "resource",
  instance_id: resource.instanceId,
  repository_id: null,
  resource_id: resource.subjectId,
  resource_kind: resource.kind,
});
const bodyQuery = (subjectId: string) => ({
  subject_id: subjectId,
  facet: "body" as const,
  cursor: null,
  limit: 50,
});
const keys = (resource: NavigationResource) => ({
  context: collaborationKeys.contextualCapabilities(
    resource.account,
    target(resource),
  ),
  item: collaborationKeys.item(resource.account, resource.subjectId),
  body: collaborationKeys.detail(resource.account, {
    ...bodyQuery(resource.subjectId),
    account_id: resource.account.id,
  }),
});
const bodyKey = (subjectId: string, actor = account) =>
  keys({ account: actor, instanceId, kind: "issue", subjectId }).body;
const sync = {
  state: "idle" as const,
  last_success_at: null,
  next_retry_at: null,
  error: null,
};
const context = (
  actor: RemoteAccount,
  resourceTarget: CapabilityTarget,
): ContextualCapabilitySnapshot => ({
  ...stamp,
  account_id: actor.id,
  authorization_epoch: actor.authorization_epoch,
  instance: {
    id: instanceId,
    provider: actor.provider,
    base_url: "https://github.com/",
  },
  target: resourceTarget,
  inbox_semantics: "native_notifications",
  facets: ["issues", "issue_details", "pull_requests", "pull_details"].map(
    (facet) => ({
      facet: facet as ContextualCapabilitySnapshot["facets"][number]["facet"],
      saved_read: { state: "supported", reason: null },
      synchronize: { state: "supported", reason: null },
      observation: "complete",
      remote_write: { state: "unsupported", reason: "not_implemented" },
      can_recheck_access: false,
      observed_at: null,
      recheck_at: null,
      sync,
    }),
  ),
});
const item = (subjectId: string, actor = account): ItemSnapshot => ({
  ...stamp,
  item: {
    id: subjectId,
    account_id: actor.id,
    repository_id: "repo:1",
    provider_id: subjectId,
    kind: "issue",
    number: "7",
    title: "Private saved issue",
    body: null,
    body_omitted: true,
    author: "fixture",
    web_url: null,
    state: "open",
    updated_at: "2026-10-04T00:00:00Z",
    head_oid: null,
    is_draft: null,
    reason: null,
    unread: null,
  },
});
const detail = (
  subjectId: string,
  text: string | null = "Saved body",
): DetailSnapshot => ({
  ...stamp,
  subject_id: subjectId,
  body: { state: "known", text },
  metadata: null,
  entries: [],
  next_cursor: null,
  evidence: {
    facet: "body",
    availability: "ready",
    coverage: { state: "complete", validated_at: null, remote_has_more: false },
    freshness: "fresh",
    stale_at: null,
    facet_revision: "1",
    authorization_epoch: "1",
    access_reason: null,
    source: null,
    value_source: null,
    saved_empty: text === null,
    observed_state: "known",
    sync,
  },
});
const deferred = <T>() => {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((finish) => {
    resolve = finish;
  });
  return { promise, resolve };
};
const flush = async () => {
  for (let i = 0; i < 50; i += 1) await Promise.resolve();
};
const resources: (() => void)[] = [];
beforeEach(() => {
  vi.useFakeTimers();
  vi.setSystemTime(0);
});
afterEach(async () => {
  for (const stop of resources.splice(0)) stop();
  onlineManager.setOnline(true);
  await flush();
  vi.useRealTimers();
});

function fixture(
  overrides: Partial<CollaborationTransport> = {},
  install = false,
) {
  let activityListener:
    | ((activity: { generation: string; active: boolean }) => void)
    | null = null;
  let changePage: ChangePage = {
    ...stamp,
    changes: [],
    has_more: false,
    reset_required: false,
  };
  let sequence = 0;
  const unexpected = vi.fn(async () => {
    throw new Error("Unexpected durable or provider operation");
  });
  const transport: CollaborationTransport = {
    accounts: async () => ({ ...stamp, accounts: [account] }),
    connectGithub: unexpected,
    connectGitlab: unexpected,
    discoverGithubCli: unexpected,
    connectGithubCli: unexpected,
    repositories: unexpected,
    selectRepository: unexpected,
    items: unexpected,
    refresh: unexpected,
    saveDraft: unexpected,
    draft: unexpected,
    capabilities: unexpected,
    resolveResource: unexpected,
    hydrateDetail: unexpected,
    notificationSubject: unexpected,
    discoverNotificationSubject: unexpected,
    localLinks: unexpected,
    confirmLocalLink: unexpected,
    removeLocalLink: unexpected,
    saveTransportBinding: unexpected,
    removeTransportBinding: unexpected,
    localClones: unexpected,
    validateLocalNavigation: unexpected,
    disconnect: async () => "2",
    listenLocalChanges: async () => () => {},
    listen: async () => () => {},
    changesSince: async () => changePage,
    item: vi.fn(async (accountId, subjectId) =>
      item(subjectId, { ...account, id: accountId }),
    ),
    detail: vi.fn(async (query) => detail(query.subject_id)),
    contextualCapabilities: vi.fn(async (request) =>
      context(
        {
          ...account,
          id: request.account_id,
          authorization_epoch: request.authorization_epoch,
        },
        request.target,
      ),
    ),
    demandActivity: vi.fn(async () => ({ generation: "1", active: true })),
    listenDemandActivity: vi.fn(async (listener) => {
      activityListener = listener;
      return () => {};
    }),
    acquireDemand: vi.fn(async (request) => ({
      lease_id: `lease-${++sequence}`,
      owner_generation: request.owner_generation,
      expires_in_seconds: 45,
      renew_after_seconds: 15,
    })),
    renewDemand: unexpected,
    releaseDemand: vi.fn(async () => {}),
    ...overrides,
  };
  const client = new CollaborationClient(transport);
  const cache = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  if (install) {
    const stop = client.installBridge(cache);
    resources.push(() => {
      stop();
      cache.clear();
    });
    return {
      client,
      cache,
      transport,
      emit: (generation: string, active: boolean) =>
        activityListener?.({ generation, active }),
      setChangePage: (next: ChangePage) => {
        changePage = next;
      },
    };
  }
  const coordinator = new DemandCoordinator(transport);
  coordinator.attach();
  const source: NavigationSource = {
    keys,
    read: (resource, projection, signal) => {
      const actor = client.forAccount(resource.account);
      return projection === "context"
        ? actor.contextualCapabilities(target(resource), signal)
        : projection === "item"
          ? actor.item(resource.subjectId, signal)
          : actor.detail(bodyQuery(resource.subjectId), signal);
    },
    retain: (resource) =>
      coordinator.retain(resource.account, {
        kind: "detail",
        repository_id: null,
        subject_id: resource.subjectId,
        facet: "body",
      }),
    observeActivity: (listener) => coordinator.observeActivity(listener),
  };
  const manager = new NavigationWorkingSet(cache, source);
  resources.push(() => {
    manager.stop();
    coordinator.stop();
    cache.clear();
  });
  return {
    client,
    cache,
    transport,
    manager,
    emit: (generation: string, active: boolean) =>
      activityListener?.({ generation, active }),
    setChangePage: (next: ChangePage) => {
      changePage = next;
    },
  };
}
const input = { account, instanceId, kind: "issue" as const };

describe("bounded navigation working set with actual SDK reads", () => {
  it("coalesces pointer, focus and recent selection and warms the ordinary cache without durable hydration", async () => {
    const { manager, cache, transport } = fixture();
    const first = manager!.scope(input);
    const second = manager!.scope({ ...input, account: { ...account } });
    first.enter("issue:7", "pointer");
    second.enter("issue:7", "focus");
    await flush();
    expect(transport.detail).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(limits.dwellMs);
    first.visit("issue:7");
    await flush();
    expect(transport.contextualCapabilities).toHaveBeenCalledTimes(1);
    expect(transport.item).toHaveBeenCalledTimes(1);
    expect(transport.detail).toHaveBeenCalledTimes(1);
    expect(transport.acquireDemand).toHaveBeenCalledTimes(1);
    expect(transport.listenDemandActivity).toHaveBeenCalledTimes(1);
    expect(transport.demandActivity).toHaveBeenCalledTimes(1);
    const saved = await cache.fetchQuery({
      queryKey: bodyKey("issue:7"),
      queryFn: () => {
        throw new Error("Cache-first selection performed a read");
      },
      staleTime: Infinity,
    });
    expect(saved).toEqual(detail("issue:7"));
    expect(transport.hydrateDetail).not.toHaveBeenCalled();
    expect(transport.refresh).not.toHaveBeenCalled();
    first.dispose();
    expect(cache.getQueryData(bodyKey("issue:7"))).toBeDefined();
    second.dispose();
    await vi.advanceTimersByTimeAsync(0);
    expect(transport.releaseDemand).toHaveBeenCalledTimes(1);
    expect(manager!.stats().entries).toBe(1);
    await vi.advanceTimersByTimeAsync(limits.cacheMs);
    expect(manager!.stats().entries).toBe(0);
  });

  it("bounds a 10,000-row sweep, queues, metadata and two uncancellable native reads", async () => {
    const held = deferred<ContextualCapabilitySnapshot>();
    const { manager, transport } = fixture({
      contextualCapabilities: vi.fn(() => held.promise),
    });
    const scope = manager!.scope(input);
    for (let i = 0; i < 10_000; i += 1) {
      scope.enter(`issue:${i}`, "pointer");
      scope.leave(`issue:${i}`, "pointer");
    }
    await vi.advanceTimersByTimeAsync(limits.dwellMs);
    expect(transport.contextualCapabilities).not.toHaveBeenCalled();
    scope.visit("issue:first");
    scope.visit("issue:second");
    await flush();
    for (let i = 0; i < 10_000; i += 1) scope.enter(`issue:${i}`, "focus");
    await vi.advanceTimersByTimeAsync(limits.dwellMs);
    expect(transport.contextualCapabilities).toHaveBeenCalledTimes(2);
    expect(manager!.stats().entries).toBe(limits.entries);
    expect(manager!.stats().reads).toBe(limits.reads);
    expect(manager!.stats().bytes).toBeLessThanOrEqual(limits.bytes);
    scope.dispose();
    expect(manager!.stats().reads).toBe(2);
    expect(manager!.stats().entries).toBe(2);
    const replacement = manager!.scope(input);
    replacement.visit("issue:new");
    await flush();
    expect(transport.contextualCapabilities).toHaveBeenCalledTimes(2);
    held.resolve(
      context(account, target({ ...input, subjectId: "issue:first" })),
    );
    await flush();
    expect(manager!.stats().reads).toBe(0);
    expect(transport.item).not.toHaveBeenCalled();
  });

  it("rejects oversized projections, stays within byte/reservation bounds, and retains no per-key query defaults", async () => {
    const { manager, cache } = fixture({
      detail: vi.fn(async (query) =>
        detail(query.subject_id, "x".repeat(2 * 1024 * 1024)),
      ),
    });
    const scope = manager!.scope(input);
    for (let i = 0; i < 60; i += 1) {
      scope.visit(`issue:${i}`);
      await flush();
    }
    expect(manager!.stats().bytes).toBeLessThanOrEqual(limits.bytes);
    expect(manager!.stats().entries).toBe(0);
    expect(cache.getQueryCache().getAll()).toHaveLength(0);
    expect(cache.getQueryDefaults(bodyKey("issue:59"))).toEqual({});
  });

  // This allocation/admission stress case traverses 200 large receipts. Its
  // deadline covers slower CI CPUs; every count/byte/IPC assertion stays exact.
  it(
    "bounds resident Body bytes while admitting newer navigation and owns only eight scope descriptors",
    { timeout: 15_000 },
    async () => {
      const { manager, cache, transport } = fixture({
        detail: vi.fn(async (query) =>
          detail(query.subject_id, "x".repeat(400_000)),
        ),
      });
      const scope = manager!.scope(input);
      for (let i = 0; i < 200; i += 1) {
        scope.visit(`issue:${i}`);
        await flush();
        expect(manager!.stats().bytes).toBeLessThanOrEqual(limits.bytes);
        expect(manager!.stats().entries).toBeLessThanOrEqual(limits.entries);
        expect(manager!.stats().reads).toBeLessThanOrEqual(limits.reads);
      }
      expect(cache.getQueryData(bodyKey("issue:199"))).toBeDefined();
      expect(cache.getQueryData(bodyKey("issue:0"))).toBeUndefined();
      expect(cache.getQueryCache().getAll().length).toBeLessThanOrEqual(
        limits.entries * 3,
      );
      const scopes = Array.from({ length: limits.scopes - 1 }, () =>
        manager!.scope(input),
      );
      const rejected = manager!.scope(input);
      const previous = vi.mocked(transport.contextualCapabilities).mock.calls
        .length;
      rejected.visit("issue:over-scope-limit");
      await flush();
      expect(transport.contextualCapabilities).toHaveBeenCalledTimes(previous);
      scopes[0].dispose();
      const admitted = manager!.scope(input);
      admitted.visit("issue:new-scope");
      await flush();
      expect(transport.contextualCapabilities).toHaveBeenCalledTimes(
        previous + 1,
      );
      for (const retained of scopes) retained.dispose();
      admitted.dispose();
    },
  );

  it("retires cached resource capability interest on a real QueryClient invalidation", async () => {
    const { manager, cache, transport } = fixture();
    const scope = manager!.scope(input);
    scope.visit("issue:7");
    await flush();
    await cache.invalidateQueries({
      queryKey: keys({ ...input, subjectId: "issue:7" }).context,
      exact: true,
    });
    await vi.advanceTimersByTimeAsync(0);
    expect(cache.getQueryData(bodyKey("issue:7"))).toBeUndefined();
    expect(manager!.stats().interests).toBe(0);
    expect(transport.releaseDemand).toHaveBeenCalledTimes(1);
    scope.visit("issue:7");
    await flush();
    expect(transport.contextualCapabilities).toHaveBeenCalledTimes(2);
  });

  it.each([
    false,
    true,
  ])("preserves a selected observer mounted while speculative Body is held (selected completes first=%s)", async (selectedCompletesFirst) => {
    const speculative = deferred<DetailSnapshot>();
    const selected = deferred<DetailSnapshot>();
    const { manager, cache, client, transport } = fixture({
      detail: vi
        .fn()
        .mockImplementationOnce(() => speculative.promise)
        .mockImplementationOnce(() => selected.promise),
    });
    const scope = manager!.scope(input);
    scope.visit("issue:7");
    await flush();
    const observer = new QueryObserver(cache, {
      queryKey: bodyKey("issue:7"),
      queryFn: ({ signal }) =>
        client.forAccount(account).detail(bodyQuery("issue:7"), signal),
      staleTime: Infinity,
    });
    const unsubscribe = observer.subscribe(() => {});
    expect(transport.detail).toHaveBeenCalledTimes(2);
    if (selectedCompletesFirst) {
      selected.resolve(detail("issue:7", "Current selected view"));
      await flush();
    }
    speculative.resolve(detail("issue:7", "Old speculative snapshot"));
    await flush();
    expect(
      cache.getQueryData<DetailSnapshot>(bodyKey("issue:7"))?.body.text,
    ).toBe(selectedCompletesFirst ? "Current selected view" : undefined);
    if (!selectedCompletesFirst) {
      selected.resolve(detail("issue:7", "Current selected view"));
      await flush();
    }
    expect(
      cache.getQueryData<DetailSnapshot>(bodyKey("issue:7"))?.body.text,
    ).toBe("Current selected view");
    scope.dispose();
    expect(cache.getQueryData(bodyKey("issue:7"))).toBeDefined();
    unsubscribe();
  });

  it("gives a real disabled selected observer ownership of its upcoming Body read", async () => {
    const { manager, cache, client, transport } = fixture();
    const options = {
      queryKey: bodyKey("issue:7"),
      queryFn: ({ signal }: { signal: AbortSignal }) =>
        client.forAccount(account).detail(bodyQuery("issue:7"), signal),
      staleTime: Infinity,
    };
    const observer = new QueryObserver(cache, { ...options, enabled: false });
    const unsubscribe = observer.subscribe(() => {});
    const scope = manager!.scope(input);
    scope.visit("issue:7");
    await flush();
    expect(transport.detail).not.toHaveBeenCalled();
    expect(transport.acquireDemand).not.toHaveBeenCalled();
    observer.setOptions({ ...options, enabled: true });
    await flush();
    expect(transport.detail).toHaveBeenCalledTimes(1);
    expect(cache.getQueryData(bodyKey("issue:7"))).toEqual(detail("issue:7"));
    unsubscribe();
  });

  it("evicts deterministic LRU/TTL speculative entries and never removes an active selected query or private draft", async () => {
    const { manager, cache } = fixture();
    const scope = manager!.scope(input);
    scope.visit("issue:0");
    await flush();
    const observer = new QueryObserver(cache, {
      queryKey: bodyKey("issue:0"),
      staleTime: Infinity,
    });
    const unsubscribe = observer.subscribe(() => {});
    const draftKey = collaborationKeys.draft(account, "issue:0");
    cache.setQueryData(draftKey, { text: "Unsaved private draft" });
    for (let i = 1; i <= limits.entries; i += 1) {
      scope.visit(`issue:${i}`);
      await flush();
    }
    expect(manager!.stats().entries).toBe(limits.entries);
    expect(cache.getQueryData(bodyKey("issue:0"))).toBeDefined();
    expect(cache.getQueryData(bodyKey("issue:1"))).toBeUndefined();
    await vi.advanceTimersByTimeAsync(limits.cacheMs);
    expect(cache.getQueryData(bodyKey("issue:0"))).toBeDefined();
    expect(cache.getQueryData(bodyKey("issue:24"))).toBeUndefined();
    expect(cache.getQueryData(draftKey)).toEqual({
      text: "Unsaved private draft",
    });
    unsubscribe();
    expect(cache.getQueryData(bodyKey("issue:0"))).toBeUndefined();
    expect(manager!.stats().entries).toBe(0);
  });

  it("caps ephemeral native interests, releases on leave/expiry/inactive and ignores late inactive completion", async () => {
    const { manager, transport, cache, emit } = fixture();
    const scope = manager!.scope(input);
    for (let i = 0; i < 10; i += 1) {
      scope.visit(`issue:${i}`);
      await flush();
    }
    expect(manager!.stats().interests).toBe(limits.interests);
    expect(transport.acquireDemand).toHaveBeenCalledTimes(limits.interests);
    scope.enter("issue:0", "pointer");
    scope.leave("issue:0", "pointer");
    await vi.advanceTimersByTimeAsync(0);
    expect(transport.releaseDemand).toHaveBeenCalledTimes(1);
    await vi.advanceTimersByTimeAsync(limits.interestMs + 1);
    expect(manager!.stats().interests).toBe(0);
    expect(transport.releaseDemand).toHaveBeenCalledTimes(limits.interests);
    const held = deferred<DetailSnapshot>();
    vi.mocked(transport.detail).mockImplementationOnce(() => held.promise);
    scope.visit("issue:held");
    await flush();
    emit("2", false);
    expect(manager!.stats().reads).toBe(1);
    held.resolve(detail("issue:held"));
    await flush();
    expect(cache.getQueryData(bodyKey("issue:held"))).toBeUndefined();
    expect(manager!.stats().reads).toBe(0);
    scope.visit("issue:hidden");
    await flush();
    expect(transport.detail).toHaveBeenCalledTimes(11);
  });

  it("keeps known empty and offline missing evidence, while denied capability forbids item/body reads", async () => {
    onlineManager.setOnline(false);
    const { manager, cache, transport } = fixture({
      detail: vi.fn(
        async (query): Promise<DetailSnapshot> => ({
          ...detail(query.subject_id, null),
          body: { state: "not_loaded", text: null },
          evidence: {
            ...detail(query.subject_id).evidence,
            availability: "missing",
            freshness: "unknown",
            saved_empty: false,
            observed_state: "not_loaded",
            sync: { ...sync, state: "offline" },
          },
        }),
      ),
    });
    const scope = manager!.scope(input);
    scope.visit("issue:missing");
    await flush();
    expect(
      cache.getQueryData<DetailSnapshot>(bodyKey("issue:missing"))?.body.state,
    ).toBe("not_loaded");
    expect(
      cache.getQueryData<DetailSnapshot>(bodyKey("issue:missing"))?.evidence
        .sync.state,
    ).toBe("offline");
    vi.mocked(transport.detail).mockResolvedValueOnce(
      detail("issue:empty", null),
    );
    scope.visit("issue:empty");
    await flush();
    expect(
      cache.getQueryData<DetailSnapshot>(bodyKey("issue:empty"))?.body,
    ).toEqual({ state: "known", text: null });
    vi.mocked(transport.contextualCapabilities).mockImplementationOnce(
      async (request) => ({
        ...context(account, request.target),
        facets: context(account, request.target).facets.map((facet) => ({
          ...facet,
          saved_read: { state: "unavailable", reason: "permission_denied" },
        })),
      }),
    );
    scope.visit("issue:denied");
    await flush();
    expect(transport.item).toHaveBeenCalledTimes(2);
    expect(transport.detail).toHaveBeenCalledTimes(2);
    expect(transport.hydrateDetail).not.toHaveBeenCalled();
  });

  it("separates account epoch/instance identity and revokes removed-account scope descriptors", async () => {
    const { manager, cache, transport } = fixture();
    const first = manager!.scope(input);
    first.visit("issue:7");
    await flush();
    const actor = { ...account, authorization_epoch: "2" };
    vi.mocked(transport.detail).mockResolvedValueOnce({
      ...detail("issue:7"),
      evidence: { ...detail("issue:7").evidence, authorization_epoch: "2" },
    });
    const second = manager!.scope({ ...input, account: actor });
    second.visit("issue:7");
    await flush();
    expect(transport.detail).toHaveBeenCalledTimes(2);
    expect(cache.getQueryData(bodyKey("issue:7", actor))).toBeDefined();
    manager!.clear(account.id);
    first.visit("issue:8");
    second.visit("issue:8");
    await flush();
    expect(transport.detail).toHaveBeenCalledTimes(2);
    expect(cache.getQueryData(bodyKey("issue:7"))).toBeUndefined();
    expect(cache.getQueryData(bodyKey("issue:7", actor))).toBeUndefined();
  });
});

describe("ordinary client revision/authorization integration", () => {
  it.each([
    "disconnect",
    "reset",
    "revision",
  ] as const)("fences an actual held Body IPC across %s without cache repopulation", async (incident) => {
    const held = deferred<DetailSnapshot>();
    const { client, cache, transport, setChangePage } = fixture(
      { detail: vi.fn(() => held.promise) },
      true,
    );
    await client.wake();
    const scope = client.navigationScope(input);
    scope.visit("issue:7");
    await flush();
    expect(transport.detail).toHaveBeenCalledTimes(1);
    if (incident === "disconnect") await client.disconnect(account.id);
    else {
      setChangePage({
        revision: "2",
        authorization_view: incident === "reset" ? "2" : "1",
        reset_required: incident === "reset",
        has_more: false,
        changes:
          incident === "revision"
            ? [
                {
                  revision: "2",
                  account_id: account.id,
                  scope: "detail:issue:7:body",
                  reset: false,
                },
              ]
            : [],
      });
      await client.wake();
    }
    held.resolve(detail("issue:7"));
    await flush();
    expect(cache.getQueryData(bodyKey("issue:7"))).toBeUndefined();
    expect(transport.hydrateDetail).not.toHaveBeenCalled();
    scope.dispose();
  });
});

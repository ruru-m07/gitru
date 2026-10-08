import type {
  DetailSnapshot,
  NotificationSubjectSnapshot,
  RemoteAccount,
} from "@gitru/commands";
import { QueryClient, QueryObserver } from "@tanstack/react-query";
import { afterEach, describe, expect, it, vi } from "vitest";
import { StaleAuthorizationError } from "./authorization-fence";
import {
  CollaborationClient,
  type CollaborationTransport,
  collaborationKeys,
} from "./client";

const account: RemoteAccount = {
  id: "account-a",
  actor_id: "9007199254740993",
  authorization_epoch: "9007199254740994",
  provider: "github",
  host: "https://github.com",
  state: "active",
  login: "fixture",
  display_name: null,
  notifications_supported: true,
};
const snapshot = (generation = "1"): NotificationSubjectSnapshot => ({
  revision: generation,
  authorization_view: "1",
  authorization_epoch: account.authorization_epoch,
  state: "not_cached",
  reason: "not_cached",
  selector_generation: generation,
  subject: null,
  fallback_web_url: "https://github.com/owner/project/pull/67",
  discovery: {
    support: "supported",
    admission: true,
    paused: false,
    retry_at: null,
    attempts: 0,
    sync: {
      state: "idle",
      next_retry_at: null,
      last_success_at: null,
      error: null,
    },
  },
});
const body = (text: string): DetailSnapshot => ({
  pending_intent: null,

  subject_id: "canonical-pr",
  body: { state: "known", text },
  metadata: null,
  entries: [],
  next_cursor: null,
  revision: "1",
  authorization_view: "1",
  evidence: {
    facet: "body",
    availability: "ready",
    coverage: { state: "complete", validated_at: null, remote_has_more: false },
    freshness: "fresh",
    stale_at: null,
    facet_revision: "1",
    authorization_epoch: account.authorization_epoch,
    access_reason: null,
    source: null,
    value_source: null,
    saved_empty: false,
    observed_state: "known",
    sync: {
      state: "idle",
      last_success_at: null,
      next_retry_at: null,
      error: null,
    },
  },
});
function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((done) => {
    resolve = done;
  });
  return { promise, resolve };
}
async function flush() {
  for (let n = 0; n < 24; n += 1) await Promise.resolve();
}
const stops: (() => void)[] = [];
const caches: QueryClient[] = [];
afterEach(() => {
  for (const stop of stops.splice(0)) stop();
  for (const cache of caches.splice(0)) cache.clear();
});

function fixture() {
  let revision = "1";
  let view = "1";
  let quotaPaused = false;
  let changes: {
    account_id: string;
    revision: string;
    scope: string;
    reset: boolean;
  }[] = [];
  const unexpected = vi.fn(async () => {
    throw new Error("Unexpected provider or authored operation");
  });
  const transport = {
    commandRecoveryList: async () => {
      throw new Error("Unexpected recovery operation");
    },
    commandRecoveryDetail: async () => {
      throw new Error("Unexpected recovery operation");
    },
    commandRecoveryAction: async () => {
      throw new Error("Unexpected recovery operation");
    },
    commandRecoveryReplace: async () => {
      throw new Error("Unexpected recovery operation");
    },
    providerInboxActions: async () => {
      throw new Error("Unexpected provider inbox query");
    },
    queueProviderInboxAction: async () => {
      throw new Error("Unexpected provider inbox action");
    },
    commandRecoveryExport: async () => {
      throw new Error("Unexpected recovery operation");
    },
    textEditSnapshot: unexpected,
    submitTextEdit: unexpected,
    workflowStateSnapshot: unexpected,
    submitWorkflowState: unexpected,
    guardedMergeSnapshot: unexpected,
    previewGuardedMerge: unexpected,
    submitGuardedMerge: unexpected,
    commentDraft: unexpected,
    commentDrafts: unexpected,
    saveCommentDraft: unexpected,
    sendComment: unexpected,
    createdComments: unexpected,
    reviewDraft: vi.fn(),
    reviewDrafts: vi.fn(),
    saveReviewDraft: vi.fn(),
    submitReview: vi.fn(),
    submittedReviews: vi.fn(),
    pullDraft: unexpected,
    pullDrafts: unexpected,
    savePullDraft: unexpected,
    previewPullCreation: unexpected,
    submitPull: unexpected,
    issueDraft: unexpected,
    issueDrafts: unexpected,
    saveIssueDraft: unexpected,
    submitIssue: unexpected,
    accounts: async () => ({
      accounts: [account],
      revision,
      authorization_view: view,
    }),
    diagnostics: unexpected,
    exportDiagnostics: unexpected,
    connectGithub: unexpected,
    connectGitlab: unexpected,
    connectBitbucketCloud: unexpected,
    connectGithubCli: unexpected,
    discoverGithubCli: unexpected,
    disconnect: async () => "2",
    repositories: unexpected,
    selectRepository: unexpected,
    items: unexpected,
    inbox: unexpected,
    setLocalInboxState: unexpected,
    item: unexpected,
    refresh: unexpected,
    saveDraft: unexpected,
    draft: unexpected,
    drafts: unexpected,
    exportDraft: unexpected,
    capabilities: unexpected,
    contextualCapabilities: unexpected,
    resolveResource: unexpected,
    detail: vi.fn(async () => body("current saved body")),
    pullCommits: unexpected,
    pullFiles: unexpected,
    pullFileArtifact: unexpected,
    hydratePullFile: unexpected,
    loadLocalPullFile: unexpected,
    hydrateDetail: unexpected,
    notificationSubject: vi.fn(async () => {
      const current = snapshot(revision);
      return {
        ...current,
        authorization_view: view,
        discovery: {
          ...current.discovery,
          paused: quotaPaused,
          retry_at: quotaPaused ? "2099-10-03T12:00:00Z" : null,
        },
      };
    }),
    discoverNotificationSubject: vi.fn(async () => ({
      job_id: "native-finite-intent",
    })),
    planPullCheckout: unexpected,
    executePullCheckout: unexpected,
    openLocalPullCommit: unexpected,
    localLinks: unexpected,
    confirmLocalLink: unexpected,
    removeLocalLink: unexpected,
    saveTransportBinding: unexpected,
    removeTransportBinding: unexpected,
    localClones: unexpected,
    validateLocalNavigation: unexpected,
    listenRuntimeReset: async () => () => {},
    listenLocalChanges: async () => () => {},
    demandActivity: unexpected,
    acquireDemand: unexpected,
    renewDemand: unexpected,
    releaseDemand: unexpected,
    listenDemandActivity: unexpected,
    listen: async () => () => {},
    changesSince: async () => ({
      revision,
      authorization_view: view,
      changes,
      reset_required: false,
      has_more: false,
    }),
  } satisfies CollaborationTransport;
  const client = new CollaborationClient(transport);
  const cache = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  caches.push(cache);
  stops.push(client.installBridge(cache));
  return {
    client,
    cache,
    transport,
    unexpected,
    change: async (scope: string, withdrawal = false) => {
      revision = (BigInt(revision) + 1n).toString();
      if (withdrawal) view = (BigInt(view) + 1n).toString();
      if (scope === "provider:rest") quotaPaused = true;
      changes = [{ account_id: account.id, revision, scope, reset: false }];
      await client.wake();
    },
  };
}

describe("cached notification subject projection and explicit discovery", () => {
  it("isolates account/actor/epoch/thread query keys and reads without any discovery or provider work", async () => {
    const { client, transport, unexpected } = fixture();
    await client.forAccount(account).notificationSubject("thread-a");
    expect(transport.notificationSubject).toHaveBeenCalledExactlyOnceWith({
      account_id: account.id,
      authorization_epoch: account.authorization_epoch,
      notification_id: "thread-a",
    });
    expect(transport.discoverNotificationSubject).not.toHaveBeenCalled();
    expect(unexpected).not.toHaveBeenCalled();
    const key = collaborationKeys.notificationSubject(account, "thread-a");
    for (const actor of [
      { ...account, id: "other" },
      { ...account, actor_id: "other" },
      { ...account, authorization_epoch: "2" },
    ])
      expect(
        collaborationKeys.notificationSubject(actor, "thread-a"),
      ).not.toEqual(key);
    expect(
      collaborationKeys.notificationSubject(account, "thread-b"),
    ).not.toEqual(key);
  });
  it("captures immutable account/epoch and sends only inspected selector generation on explicit discovery", async () => {
    const { client, transport, unexpected } = fixture();
    const mutable = { ...account };
    const handle = client.forAccount(mutable);
    mutable.id = "replacement-account";
    mutable.actor_id = "replacement-actor";
    mutable.authorization_epoch = "replacement-epoch";
    await handle.notificationSubject("thread-a");
    await handle.discoverNotificationSubject("thread-a", "9007199254740995");
    expect(transport.notificationSubject).toHaveBeenCalledExactlyOnceWith({
      account_id: account.id,
      authorization_epoch: account.authorization_epoch,
      notification_id: "thread-a",
    });
    expect(
      transport.discoverNotificationSubject,
    ).toHaveBeenCalledExactlyOnceWith({
      account_id: account.id,
      authorization_epoch: account.authorization_epoch,
      notification_id: "thread-a",
      selector_generation: "9007199254740995",
    });
    expect(unexpected).not.toHaveBeenCalled();
  });
  it("rejects an old epoch snapshot even before its native change hint arrives", async () => {
    const { client, cache, transport } = fixture();
    await client.wake();
    const key = collaborationKeys.notificationSubject(account, "thread-a");
    const current = snapshot();
    cache.setQueryData(key, current);
    const version = client.getVersion();
    transport.notificationSubject.mockResolvedValueOnce({
      ...snapshot("2"),
      authorization_view: "2",
      authorization_epoch: "older",
    });
    await expect(
      client.forAccount(account).notificationSubject("thread-a"),
    ).rejects.toBeInstanceOf(StaleAuthorizationError);
    expect(client.getVersion()).toBe(version);
    expect(cache.getQueryData(key)).toEqual(current);
  });
  it.each([
    "notifications",
    "notification_subject:thread-a",
  ])("cancels initial pending resolver and body observers before %s invalidation, preserving authored cache", async (scope) => {
    const { client, cache, transport, change, unexpected } = fixture();
    await client.wake();
    const oldResolver = deferred<NotificationSubjectSnapshot>();
    const oldBody = deferred<DetailSnapshot>();
    transport.notificationSubject.mockImplementationOnce(
      () => oldResolver.promise,
    );
    transport.detail.mockImplementationOnce(() => oldBody.promise);
    const resolver = new QueryObserver(cache, {
      queryKey: collaborationKeys.notificationSubject(account, "thread-a"),
      queryFn: ({ signal }) =>
        client.forAccount(account).notificationSubject("thread-a", signal),
      staleTime: Infinity,
      retry: false,
    });
    const detailQuery = {
      account_id: account.id,
      subject_id: "canonical-pr",
      facet: "body" as const,
      cursor: null,
      limit: 50,
    };
    const detail = new QueryObserver(cache, {
      queryKey: collaborationKeys.detail(account, detailQuery),
      queryFn: ({ signal }) =>
        client.forAccount(account).detail(detailQuery, signal),
      staleTime: Infinity,
      retry: false,
    });
    const draftKey = collaborationKeys.draft(account, "canonical-pr");
    const draft = {
      account_id: account.id,
      subject_id: "canonical-pr",
      body: "private text",
      generation: "7",
    };
    cache.setQueryData(draftKey, draft);
    stops.push(
      resolver.subscribe(() => {}),
      detail.subscribe(() => {}),
    );
    await flush();
    await change(scope);
    await flush();
    expect(transport.notificationSubject).toHaveBeenCalledTimes(2);
    expect(transport.detail).toHaveBeenCalledTimes(2);
    oldResolver.resolve({
      ...snapshot(),
      state: "resolved",
      subject: {
        account_id: account.id,
        instance_id: "github:https://github.com/",
        id: "old-target",
        kind: "pull_request",
        provider_id: "67",
      },
    });
    oldBody.resolve(body("retired body"));
    await flush();
    expect(resolver.getCurrentResult().data?.selector_generation).toBe("2");
    expect(resolver.getCurrentResult().data?.subject).toBeNull();
    expect(detail.getCurrentResult().data?.body.text).toBe(
      "current saved body",
    );
    expect(cache.getQueryData(draftKey)).toEqual(draft);
    expect(unexpected).not.toHaveBeenCalled();
  });
  it.each([
    "pending",
    "cached",
  ] as const)("updates %s resolver quota metadata on provider:rest without a discovery or content request", async (initial) => {
    const { client, cache, transport, change, unexpected } = fixture();
    await client.wake();
    const old = deferred<NotificationSubjectSnapshot>();
    if (initial === "pending")
      transport.notificationSubject.mockImplementationOnce(() => old.promise);
    const observer = new QueryObserver(cache, {
      queryKey: collaborationKeys.notificationSubject(account, "thread-a"),
      queryFn: ({ signal }) =>
        client.forAccount(account).notificationSubject("thread-a", signal),
      staleTime: Infinity,
      retry: false,
    });
    stops.push(observer.subscribe(() => {}));
    await flush();
    if (initial === "cached")
      expect(observer.getCurrentResult().data?.discovery.paused).toBe(false);
    await change("provider:rest");
    await flush();
    old.resolve(snapshot());
    await flush();
    expect(transport.notificationSubject).toHaveBeenCalledTimes(2);
    expect(observer.getCurrentResult().data?.revision).toBe("2");
    expect(observer.getCurrentResult().data?.discovery).toMatchObject({
      paused: true,
      admission: true,
      retry_at: "2099-10-03T12:00:00Z",
    });
    expect(transport.discoverNotificationSubject).not.toHaveBeenCalled();
    expect(transport.detail).not.toHaveBeenCalled();
    expect(unexpected).not.toHaveBeenCalled();
  });

  it("rejects delayed explicit receipts and resolver reads across authorization withdrawal", async () => {
    const { client, transport, change } = fixture();
    await client.wake();
    const pendingResolver = deferred<NotificationSubjectSnapshot>();
    const pendingAction = deferred<{ job_id: string }>();
    transport.notificationSubject.mockImplementationOnce(
      () => pendingResolver.promise,
    );
    transport.discoverNotificationSubject.mockImplementationOnce(
      () => pendingAction.promise,
    );
    const reading = client.forAccount(account).notificationSubject("thread-a");
    const discovering = client
      .forAccount(account)
      .discoverNotificationSubject("thread-a", "1");
    const rejectedRead = expect(reading).rejects.toBeInstanceOf(
      StaleAuthorizationError,
    );
    const rejectedIntent = expect(discovering).rejects.toBeInstanceOf(
      StaleAuthorizationError,
    );
    await change("notifications", true);
    pendingResolver.resolve(snapshot());
    pendingAction.resolve({ job_id: "obsolete-receipt" });
    await Promise.all([rejectedRead, rejectedIntent]);
  });
  it("does not reread notification identity for authored draft changes", async () => {
    const { client, cache, transport, change } = fixture();
    await client.wake();
    const observer = new QueryObserver(cache, {
      queryKey: collaborationKeys.notificationSubject(account, "thread-a"),
      queryFn: ({ signal }) =>
        client.forAccount(account).notificationSubject("thread-a", signal),
      staleTime: Infinity,
      retry: false,
    });
    stops.push(observer.subscribe(() => {}));
    await flush();
    await change("drafts");
    await flush();
    expect(transport.notificationSubject).toHaveBeenCalledTimes(1);
  });
});

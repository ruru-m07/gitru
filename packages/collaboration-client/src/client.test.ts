import type {
  AccountSnapshot,
  CapabilitySnapshot,
  CapabilityTarget,
  ChangePage,
  CommandRecoveryDetail,
  CommentDraftSnapshot,
  ContextualCapabilitySnapshot,
  CreatedCommentPage,
  DetailSnapshot,
  InboxPage,
  IssueDraftSnapshot,
  ItemPage,
  ItemSnapshot,
  PullCheckoutPlan,
  CollaborationPullCheckoutReceipt as PullCheckoutReceipt,
  PullCommitSnapshot,
  RemoteAccount,
  RemoteItem,
  RepositorySnapshot,
  ResourceLocator,
  ResourceResolution,
  SendCommentRequest,
  SetLocalInboxStateRequest,
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
  total_count: 0,
  pending_intents: [],

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
const inboxPage: InboxPage = {
  total_count: 0,
  pending_intents: [],

  entries: [],
  revision: "1",
  authorization_view: "1",
  next_cursor: null,
  coverage: page.coverage,
  sync: page.sync,
  evaluated_at: "2026-10-07T00:00:00Z",
  next_local_change_at: null,
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
const checkoutPlan: PullCheckoutPlan = {
  plan_id: "opaque-plan",
  local_repository_id: "registered-a",
  local_repository_name: "project",
  source_repository: "owner/project",
  source_remote: "origin",
  source_branch: "feature",
  expected_oid: "a".repeat(40),
  local_branch: "review/42",
  metadata_validated_at: "2026-10-05T00:00:00Z",
  metadata_stale: false,
  inspection: {
    current_branch: "main",
    current_head_oid: "b".repeat(40),
    detached: false,
    dirty: false,
    operation: "clean",
    object_available: true,
    action: "create_branch",
  },
};
const checkoutReceipt: PullCheckoutReceipt = {
  local_repository_id: checkoutPlan.local_repository_id,
  branch: checkoutPlan.local_branch,
  oid: checkoutPlan.expected_oid,
  fetched: false,
  git_reported_failure: false,
};

function transport(
  overrides: Partial<CollaborationTransport> = {},
): CollaborationTransport {
  const unexpected = async () => {
    throw new Error("Unexpected transport operation");
  };
  return {
    commandRecoveryList: unexpected,
    commandRecoveryDetail: unexpected,
    commandRecoveryAction: unexpected,
    commandRecoveryReplace: unexpected,
    commandRecoveryExport: unexpected,
    textEditSnapshot: unexpected,
    submitTextEdit: unexpected,
    workflowStateSnapshot: unexpected,
    submitWorkflowState: unexpected,
    commentDraft: unexpected,
    commentDrafts: unexpected,
    saveCommentDraft: unexpected,
    sendComment: unexpected,
    createdComments: unexpected,
    issueDraft: unexpected,
    issueDrafts: unexpected,
    saveIssueDraft: unexpected,
    submitIssue: unexpected,
    accounts: unexpected,
    diagnostics: unexpected,
    exportDiagnostics: unexpected,
    connectGithub: unexpected,
    connectGitlab: unexpected,
    connectBitbucketCloud: unexpected,
    discoverGithubCli: unexpected,
    connectGithubCli: unexpected,
    disconnect: unexpected,
    repositories: unexpected,
    selectRepository: unexpected,
    items: unexpected,
    inbox: unexpected,
    setLocalInboxState: unexpected,
    item: unexpected,
    refresh: unexpected,
    changesSince: unexpected,
    saveDraft: unexpected,
    draft: unexpected,
    drafts: unexpected,
    exportDraft: unexpected,
    capabilities: unexpected,
    contextualCapabilities: unexpected,
    resolveResource: unexpected,
    detail: unexpected,
    pullCommits: unexpected,
    pullFiles: unexpected,
    pullFileArtifact: unexpected,
    hydratePullFile: unexpected,
    loadLocalPullFile: unexpected,
    hydrateDetail: unexpected,
    notificationSubject: unexpected,
    discoverNotificationSubject: unexpected,
    planPullCheckout: unexpected,
    executePullCheckout: unexpected,
    openLocalPullCommit: unexpected,
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
    listenRuntimeReset: async () => () => {},
    listenLocalChanges: async () => () => {},
    listen: unexpected,
    ...overrides,
  };
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((finish, fail) => {
    resolve = finish;
    reject = fail;
  });
  return { promise, resolve, reject };
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
      pending_intent: null,

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
      pending_intent: null,

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
    {
      changeScope: "detail:pull:body",
      source: "body-context",
    },
    {
      changeScope: "repo:repo-1:pull_request",
      source: "repository pull-request",
    },
  ])("hides a superseded pull-commit generation before its $source refetch resolves", async ({
    changeScope,
  }) => {
    let next = changePage("1");
    const refreshed = deferred<PullCommitSnapshot>();
    const pullCommits = vi.fn(() => refreshed.promise);
    const client = new CollaborationClient(
      transport({
        pullCommits,
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
      cursor: null,
      limit: 50,
    };
    const key = collaborationKeys.pullCommits(account, query);
    const stale: PullCommitSnapshot = {
      subject_id: "pull",
      context: {
        base_oid: "b".repeat(40),
        head_oid: "a".repeat(40),
        source_repository_provider_id: "fork-1",
        metadata_facet_revision: "1",
      },
      commits: [
        {
          oid: "a".repeat(40),
          position: 0,
          summary: "superseded",
          message: { state: "known", text: "superseded" },
          author: { name: "Author", provider: null },
          committer: null,
          authored_at: null,
          committed_at: null,
          parent_oids: ["b".repeat(40)],
          web_url: null,
        },
      ],
      next_cursor: null,
      completeness: { state: "complete", reason: null },
      coverage: page.coverage,
      sync: page.sync,
      freshness: "fresh",
      facet_revision: "1",
      revision: "1",
      authorization_view: "1",
    };
    cache.setQueryData(key, stale);
    const observer = new QueryObserver(cache, {
      queryKey: key,
      queryFn: ({ signal }) =>
        client.forAccount(account).pullCommits(query, signal),
      staleTime: Infinity,
      retry: false,
    });
    const unsubscribe = observer.subscribe(() => {});

    next = changePage("2", "1", [
      {
        revision: "2",
        account_id: account.id,
        scope: changeScope,
        reset: false,
      },
    ]);
    await client.wake();

    expect(cache.getQueryData(key)).toBeUndefined();
    expect(observer.getCurrentResult().data).toBeUndefined();
    expect(pullCommits).toHaveBeenCalled();
    refreshed.resolve({
      ...stale,
      context: null,
      commits: [],
      completeness: { state: "missing", reason: null },
      freshness: "unknown",
      facet_revision: null,
      revision: "2",
    });
    await refreshed.promise;
    unsubscribe();
    stop();
    cache.clear();
  });
  it("invalidates file list and artifact reads when selected file evidence changes", async () => {
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
    const context = {
      base_oid: "a".repeat(40),
      head_oid: "b".repeat(40),
      merge_base_oid: null,
      base_repository_provider_id: "target",
      source_repository_provider_id: "source",
      body_metadata_facet_revision: "1",
    };
    const query = {
      account_id: account.id,
      subject_id: "pull",
      cursor: null,
      limit: 100,
    };
    const request = {
      account_id: account.id,
      authorization_epoch: account.authorization_epoch,
      subject_id: "pull",
      file_facet_revision: "2",
      context,
      file_key: "file-a",
    };
    const listKey = collaborationKeys.pullFiles(account, query);
    const artifactKey = collaborationKeys.pullFileArtifact(account, request);
    cache.setQueryData(listKey, "saved file list");
    cache.setQueryData(artifactKey, "saved selected artifact");

    next = changePage("2", "1", [
      {
        revision: "2",
        account_id: account.id,
        scope: "detail:pull:files",
        reset: false,
      },
    ]);
    await client.wake();

    expect(cache.getQueryData(listKey)).toBe("saved file list");
    expect(cache.getQueryData(artifactKey)).toBe("saved selected artifact");
    expect(cache.getQueryState(listKey)?.isInvalidated).toBe(true);
    expect(cache.getQueryState(artifactKey)?.isInvalidated).toBe(true);
    stop();
    cache.clear();
  });
  it.each([
    { scope: "detail:pull:body", source: "body context" },
    {
      scope: "repo:repo-1:pull_request",
      source: "repository context",
    },
  ])("removes superseded file projections before a $source refresh", async ({
    scope,
  }) => {
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
    const context = {
      base_oid: "a".repeat(40),
      head_oid: "b".repeat(40),
      merge_base_oid: null,
      base_repository_provider_id: "target",
      source_repository_provider_id: "source",
      body_metadata_facet_revision: "1",
    };
    const listKey = collaborationKeys.pullFiles(account, {
      account_id: account.id,
      subject_id: "pull",
      cursor: null,
      limit: 100,
    });
    const artifactKey = collaborationKeys.pullFileArtifact(account, {
      account_id: account.id,
      authorization_epoch: account.authorization_epoch,
      subject_id: "pull",
      file_facet_revision: "2",
      context,
      file_key: "file-a",
    });
    cache.setQueryData(listKey, "superseded file list");
    cache.setQueryData(artifactKey, "superseded selected artifact");

    next = changePage("2", "1", [
      {
        revision: "2",
        account_id: account.id,
        scope,
        reset: false,
      },
    ]);
    await client.wake();
    await vi.waitFor(() => expect(cache.getQueryData(listKey)).toBeUndefined());
    expect(cache.getQueryData(artifactKey)).toBeUndefined();
    stop();
    cache.clear();
  });
  it.each([
    {
      changeScope: "detail:pull:body",
      source: "Body revision",
    },
    {
      changeScope: "repo:repo-1:pull_request",
      source: "source repository",
    },
  ])("hides an exact-head check result before its $source refetch resolves", async ({
    changeScope,
  }) => {
    let next = changePage("1");
    const refreshed = deferred<DetailSnapshot>();
    const detail = vi.fn(() => refreshed.promise);
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
    const head = "a".repeat(40);
    const query = {
      account_id: account.id,
      subject_id: "pull",
      facet: "checks" as const,
      cursor: null,
      limit: 50,
    };
    const key = [
      ...collaborationKeys.detail(account, query),
      "body-context",
      "1",
      head,
    ] as const;
    const exactGreen: DetailSnapshot = {
      subject_id: "pull",
      pending_intent: null,
      body: { state: "not_loaded", text: null },
      metadata: null,
      entries: [
        {
          id: "github-check-run:1",
          provider_id: "check-run:1",
          author: null,
          title: null,
          state: null,
          body: { state: "not_loaded", text: null },
          observed_body_state: "not_loaded",
          updated_at: null,
          head_oid: head,
          native: {
            kind: "check.v1",
            value: {
              kind: "check_run",
              name: "build",
              state: {
                kind: "check_run",
                status: "completed",
                conclusion: "success",
              },
              description: { state: "omitted", text: null },
              producer: "fixture-ci",
              started_at: null,
              completed_at: null,
              updated_at: null,
              allow_failure: null,
            },
          },
          field_mask: ["check", "head_oid"],
          field_validations: [],
        },
      ],
      next_cursor: null,
      revision: "1",
      authorization_view: "1",
      evidence: {
        facet: "checks",
        availability: "ready",
        coverage: page.coverage,
        freshness: "fresh",
        stale_at: null,
        facet_revision: "1",
        authorization_epoch: "1",
        access_reason: null,
        source: null,
        value_source: null,
        saved_empty: false,
        observed_state: "not_loaded",
        sync: page.sync,
      },
    };
    cache.setQueryData(key, exactGreen);
    const observer = new QueryObserver<DetailSnapshot>(cache, {
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
        scope: changeScope,
        reset: false,
      },
    ]);
    await client.wake();

    expect(cache.getQueryData(key)).toBeUndefined();
    expect(observer.getCurrentResult().data).toBeUndefined();
    expect(detail).toHaveBeenCalled();
    refreshed.resolve({
      ...exactGreen,
      entries: [],
      revision: "2",
      evidence: {
        ...exactGreen.evidence,
        freshness: "stale",
        coverage: {
          state: "partial",
          validated_at: exactGreen.evidence.coverage.validated_at,
          remote_has_more: false,
        },
        facet_revision: "2",
      },
    });
    await refreshed.promise;
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
          : {
              pending_intent: null,
              item: privateItem,
              revision: "1",
              authorization_view: "1",
            };
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
          : {
              pending_intent: null,
              item: currentItem,
              revision: "2",
              authorization_view: "1",
            };
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
      pending_intent: null,

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

  it("binds local inbox reads and writes to the selected account epoch", async () => {
    const read = vi.fn(async () => inboxPage);
    const write = vi.fn(async (_request: SetLocalInboxStateRequest) => ({
      state: {
        disposition: "done" as const,
        effective_disposition: "done" as const,
        bookmarked: false,
        snoozed_until: null,
        activity_updated_at: "2026-10-07T00:00:00Z",
        superseded_by_activity: false,
        generation: "1",
      },
      revision: "2",
      authorization_view: "1",
    }));
    const client = new CollaborationClient(
      transport({ inbox: read, setLocalInboxState: write }),
    );
    const handle = client.forAccount(account);
    await expect(
      handle.inbox({
        remote_state: "unread",
        local_state: "inbox",
        search: null,
        cursor: null,
        limit: 50,
      }),
    ).resolves.toEqual(inboxPage);
    expect(read).toHaveBeenCalledExactlyOnceWith({
      account_id: account.id,
      remote_state: "unread",
      local_state: "inbox",
      search: null,
      cursor: null,
      limit: 50,
    });
    await handle.setLocalInboxState({
      notification_id: "notification-a",
      expected_activity_updated_at: "2026-10-07T00:00:00Z",
      mutation: "disposition",
      disposition: "done",
      bookmarked: null,
      snoozed_until: null,
      expected_generation: "0",
    });
    expect(write).toHaveBeenCalledExactlyOnceWith({
      account_id: account.id,
      authorization_epoch: account.authorization_epoch,
      notification_id: "notification-a",
      expected_activity_updated_at: "2026-10-07T00:00:00Z",
      mutation: "disposition",
      disposition: "done",
      bookmarked: null,
      snoozed_until: null,
      expected_generation: "0",
    });
  });

  it("fences a late local inbox write receipt after account cutover", async () => {
    let finish!: (receipt: {
      state: {
        disposition: "done";
        effective_disposition: "done";
        bookmarked: boolean;
        snoozed_until: null;
        activity_updated_at: string;
        superseded_by_activity: boolean;
        generation: string;
      };
      revision: string;
      authorization_view: string;
    }) => void;
    const client = new CollaborationClient(
      transport({
        disconnect: async () => "2",
        setLocalInboxState: () =>
          new Promise((resolve) => {
            finish = resolve;
          }),
      }),
    );
    const pending = client.forAccount(account).setLocalInboxState({
      notification_id: "notification-a",
      expected_activity_updated_at: "2026-10-07T00:00:00Z",
      mutation: "disposition",
      disposition: "done",
      bookmarked: null,
      snoozed_until: null,
      expected_generation: "0",
    });
    await client.disconnect(account.id);
    finish({
      state: {
        disposition: "done",
        effective_disposition: "done",
        bookmarked: false,
        snoozed_until: null,
        activity_updated_at: "2026-10-07T00:00:00Z",
        superseded_by_activity: false,
        generation: "1",
      },
      revision: "2",
      authorization_view: "2",
    });
    await expect(pending).rejects.toBeInstanceOf(StaleAuthorizationError);
  });

  it("invalidates local inbox projections for local intents and provider activity", async () => {
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
    const inboxKey = collaborationKeys.inbox(account, {
      account_id: account.id,
      remote_state: null,
      local_state: "inbox",
      search: null,
      cursor: null,
      limit: 50,
    });
    const issueKey = collaborationKeys.items(account, {
      account_id: account.id,
      kind: "issue",
      repository_id: null,
      state: null,
      search: null,
      cursor: null,
      limit: 50,
    });
    cache.setQueryData(inboxKey, inboxPage);
    cache.setQueryData(issueKey, page);
    next = changePage("2", "1", [
      {
        revision: "2",
        account_id: account.id,
        scope: "local_inbox:notification-a",
        reset: false,
      },
    ]);
    await client.wake();
    expect(cache.getQueryState(inboxKey)?.isInvalidated).toBe(true);
    expect(cache.getQueryState(issueKey)?.isInvalidated).toBe(false);
    cache.setQueryData(inboxKey, inboxPage);
    next = changePage("3", "1", [
      {
        revision: "3",
        account_id: account.id,
        scope: "notifications",
        reset: false,
      },
    ]);
    await client.wake();
    expect(cache.getQueryState(inboxKey)?.isInvalidated).toBe(true);
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

  it("injects a captured account epoch and forwards only checkout planning identifiers", async () => {
    const planPullCheckout = vi.fn(async () => checkoutPlan);
    const executePullCheckout = vi.fn(async () => checkoutReceipt);
    const client = new CollaborationClient(
      transport({ planPullCheckout, executePullCheckout }),
    );
    const mutable = { ...account };
    const checkout = client.forAccount(mutable);
    mutable.id = "mutated-account";
    mutable.authorization_epoch = "mutated-epoch";
    const request = {
      instance_id: "github:https://github.com/",
      subject_id: "pull-42",
      local_repository_id: "registered-a",
      link_id: "link-a",
      link_generation: "7",
      local_branch: "review/42",
      path: "/caller-controlled/path",
      url: "https://attacker.invalid/repository",
      remote: "caller-remote",
      oid: "c".repeat(40),
    };

    await expect(checkout.planPullCheckout(request)).resolves.toBe(
      checkoutPlan,
    );
    expect(planPullCheckout).toHaveBeenCalledExactlyOnceWith({
      account_id: account.id,
      authorization_epoch: account.authorization_epoch,
      instance_id: request.instance_id,
      subject_id: request.subject_id,
      local_repository_id: request.local_repository_id,
      link_id: request.link_id,
      link_generation: request.link_generation,
      local_branch: request.local_branch,
    });
    await expect(checkout.executePullCheckout("opaque-plan")).resolves.toBe(
      checkoutReceipt,
    );
    expect(executePullCheckout).toHaveBeenCalledExactlyOnceWith({
      plan_id: "opaque-plan",
    });
  });

  it("fences a delayed plan but preserves an authoritative native execution receipt", async () => {
    const pendingPlan = deferred<PullCheckoutPlan>();
    const pendingReceipt = deferred<PullCheckoutReceipt>();
    const client = new CollaborationClient(
      transport({
        disconnect: async () => "2",
        planPullCheckout: () => pendingPlan.promise,
        executePullCheckout: () => pendingReceipt.promise,
      }),
    );
    const checkout = client.forAccount(account);
    const planned = checkout.planPullCheckout({
      instance_id: "github:https://github.com/",
      subject_id: "pull-42",
      local_repository_id: "registered-a",
      link_id: "link-a",
      link_generation: "7",
    });
    const executed = checkout.executePullCheckout("opaque-plan");
    const stalePlan = expect(planned).rejects.toBeInstanceOf(
      StaleAuthorizationError,
    );
    const authoritativeReceipt =
      expect(executed).resolves.toBe(checkoutReceipt);

    await client.disconnect(account.id);
    pendingPlan.resolve(checkoutPlan);
    pendingReceipt.resolve(checkoutReceipt);

    await stalePlan;
    await authoritativeReceipt;
  });
});

describe("authored draft recovery", () => {
  it("redacts cached issue authority before disconnect while retaining authored text", async () => {
    let finishDisconnect!: (value: string) => void;
    const client = new CollaborationClient(
      transport({
        disconnect: () =>
          new Promise((resolve) => {
            finishDisconnect = resolve;
          }),
      }),
    );
    const cache = new QueryClient();
    const stop = client.installBridge(cache);
    const publishedKey = collaborationKeys.issueDraft(account, {
      account_id: account.id,
      draft_id: "123e4567-e89b-42d3-a456-426614174000",
      repository_id: "repo",
    });
    const confirmedSubmission = {
      command_id: "223e4567-e89b-42d3-a456-426614174000",
      draft_generation: "1",
      state: "confirmed",
      attempt_count: 1,
      quarantined: false,
      attention: null,
    };
    cache.setQueryData<IssueDraftSnapshot>(publishedKey, {
      account_id: account.id,
      draft_id: "123e4567-e89b-42d3-a456-426614174000",
      repository_id: "repo",
      title: "Retained title",
      body: "Retained body",
      generation: "1",
      context: null,
      availability: "unavailable",
      reason: "already_submitted",
      submission: confirmedSubmission,
      published: {
        subject_id: "github:issue:99",
        provider_id: "99",
        number: "4",
        url: "https://github.com/owner/repo/issues/4",
        command_id: confirmedSubmission.command_id,
      },
      revision: "1",
      authorization_view: "1",
    });
    const availableKey = collaborationKeys.issueDraft(account, {
      account_id: account.id,
      draft_id: "323e4567-e89b-42d3-a456-426614174000",
      repository_id: "repo",
    });
    cache.setQueryData<IssueDraftSnapshot>(availableKey, {
      account_id: account.id,
      draft_id: "323e4567-e89b-42d3-a456-426614174000",
      repository_id: "repo",
      title: "Available title",
      body: "Available body",
      generation: "2",
      context: {
        account_id: account.id,
        repository_id: "repo",
        authorization_epoch: account.authorization_epoch,
        authorization_view: "1",
        review_token: "a".repeat(64),
      },
      availability: "available",
      reason: null,
      submission: null,
      published: null,
      revision: "1",
      authorization_view: "1",
    });
    const disconnecting = client.disconnect(account.id);
    expect(cache.getQueryData(publishedKey)).toMatchObject({
      title: "Retained title",
      body: "Retained body",
      generation: "1",
      context: null,
      availability: "unavailable",
      reason: "account_unavailable",
      submission: confirmedSubmission,
      published: null,
    });
    expect(cache.getQueryData(availableKey)).toMatchObject({
      title: "Available title",
      body: "Available body",
      generation: "2",
      context: null,
      availability: "unavailable",
      reason: "account_unavailable",
      submission: null,
      published: null,
    });
    expect(cache.getQueryState(publishedKey)?.isInvalidated).toBe(true);
    expect(cache.getQueryState(availableKey)?.isInvalidated).toBe(true);
    finishDisconnect("2");
    await disconnecting;
    stop();
    cache.clear();
  });

  it("redacts issue authority on a runtime reset and fences an older native read", async () => {
    let reset: (() => void) | undefined;
    const oldRead = deferred<IssueDraftSnapshot>();
    const client = new CollaborationClient(
      transport({
        listen: async () => () => {},
        listenRuntimeReset: async (listener) => {
          reset = listener;
          return () => {};
        },
        changesSince: async () => changePage("1"),
        issueDraft: () => oldRead.promise,
      }),
    );
    const cache = new QueryClient();
    const stop = client.installBridge(cache);
    await client.wake();
    const key = {
      account_id: account.id,
      draft_id: "423e4567-e89b-42d3-a456-426614174000",
      repository_id: "repo",
    };
    const queryKey = collaborationKeys.issueDraft(account, key);
    const cached: IssueDraftSnapshot = {
      ...key,
      title: "Keep this title",
      body: "Keep this body",
      generation: "3",
      context: null,
      availability: "unavailable",
      reason: "already_submitted",
      submission: {
        command_id: "523e4567-e89b-42d3-a456-426614174000",
        draft_generation: "3",
        state: "confirmed",
        attempt_count: 1,
        quarantined: false,
        attention: null,
      },
      published: {
        subject_id: "github:issue:100",
        provider_id: "100",
        number: "5",
        url: "https://github.com/owner/repo/issues/5",
        command_id: "523e4567-e89b-42d3-a456-426614174000",
      },
      revision: "1",
      authorization_view: "1",
    };
    cache.setQueryData(queryKey, cached);
    const pending = client
      .forAccount(account)
      .issueDraft({ draft_id: key.draft_id, repository_id: key.repository_id });
    const rejected = expect(pending).rejects.toBeInstanceOf(
      StaleAuthorizationError,
    );

    reset?.();
    expect(cache.getQueryData(queryKey)).toEqual({
      ...cached,
      context: null,
      availability: "unavailable",
      reason: "account_unavailable",
      published: null,
    });
    expect(cache.getQueryState(queryKey)?.isInvalidated).toBe(true);
    oldRead.resolve(cached);
    await rejected;
    expect(cache.getQueryData(queryKey)).toMatchObject({
      title: "Keep this title",
      body: "Keep this body",
      generation: "3",
      published: null,
      context: null,
    });
    stop();
    cache.clear();
  });

  it("redacts cached comment authority before disconnect while retaining authored state", async () => {
    let finishDisconnect!: (value: string) => void;
    const client = new CollaborationClient(
      transport({
        disconnect: () =>
          new Promise((resolve) => {
            finishDisconnect = resolve;
          }),
      }),
    );
    const cache = new QueryClient();
    const stop = client.installBridge(cache);
    const availableKey = collaborationKeys.commentDraft(account, "issue");
    const saved = commentDraft();
    cache.setQueryData<CommentDraftSnapshot>(availableKey, saved);
    const submittedKey = collaborationKeys.commentDraft(account, "submitted");
    const submission = {
      command_id: "00000000-0000-4000-8000-000000000001",
      draft_generation: "4",
      state: "confirmed",
      attempt_count: 1,
      quarantined: false,
      attention: null,
    };
    cache.setQueryData<CommentDraftSnapshot>(submittedKey, {
      ...saved,
      subject_id: "submitted",
      body: "Submitted body",
      generation: "4",
      context: null,
      availability: "unavailable",
      reason: "already_submitted",
      submission,
    });

    const disconnecting = client.disconnect(account.id);
    expect(cache.getQueryData(availableKey)).toEqual({
      ...saved,
      context: null,
      availability: "unavailable",
      reason: "account_unavailable",
    });
    expect(cache.getQueryData(submittedKey)).toMatchObject({
      body: "Submitted body",
      generation: "4",
      context: null,
      availability: "unavailable",
      reason: "account_unavailable",
      submission,
    });
    expect(cache.getQueryState(availableKey)?.isInvalidated).toBe(true);
    expect(cache.getQueryState(submittedKey)?.isInvalidated).toBe(true);
    finishDisconnect("2");
    await disconnecting;
    stop();
    cache.clear();
  });

  it("keeps authored caches across disconnect/replacement while clearing provider projections", async () => {
    let revision = "1";
    const client = new CollaborationClient(
      transport({
        listen: async () => () => {},
        changesSince: async () => changePage(revision, revision),
        disconnect: async () => {
          revision = "2";
          return revision;
        },
        connectGithub: async () => {
          revision = "3";
          return { ...account, authorization_epoch: "3" };
        },
      }),
    );
    const cache = new QueryClient();
    const stop = client.installBridge(cache);
    await client.wake();
    const draft = {
      account_id: account.id,
      subject_id: "missing",
      body: "My text",
      generation: "1",
    };
    const key = collaborationKeys.draft(account, "missing");
    const listKey = collaborationKeys.drafts(account, {
      account_id: account.id,
      cursor: null,
      limit: 50,
    });
    const remoteKey = collaborationKeys.item(account, "private-provider-data");
    cache.setQueryData(key, draft);
    cache.setQueryData(listKey, { drafts: [], next_cursor: null });
    cache.setQueryData(remoteKey, { body: "Provider body" });
    await client.disconnect(account.id);
    expect(cache.getQueryData(key)).toEqual(draft);
    expect(cache.getQueryState(key)?.isInvalidated).toBe(true);
    expect(cache.getQueryData(listKey)).toBeDefined();
    expect(cache.getQueryState(listKey)?.isInvalidated).toBe(true);
    expect(cache.getQueryData(remoteKey)).toBeUndefined();
    await client.connectGithub("fixture");
    expect(
      collaborationKeys.draft(
        { ...account, authorization_epoch: "3" },
        "missing",
      ),
    ).toEqual(key);
    expect(
      collaborationKeys.draft(
        { ...account, id: "actor-b", actor_id: "2" },
        "missing",
      ),
    ).not.toEqual(key);
    expect(cache.getQueryData(key)).toEqual(draft);
    stop();
    cache.clear();
  });

  it("fences delayed authored reads at disconnect and when a subject observer is cancelled", async () => {
    let finish!: (value: import("@gitru/commands").LocalDraft | null) => void;
    const client = new CollaborationClient(
      transport({
        disconnect: async () => "2",
        connectGithub: async () => ({ ...account, authorization_epoch: "3" }),
        draft: () =>
          new Promise((resolve) => {
            finish = resolve;
          }),
      }),
    );
    const pending = client.forAccount(account).draft("old-subject");
    await client.disconnect(account.id);
    finish({
      account_id: account.id,
      subject_id: "old-subject",
      body: "old actor text",
      generation: "1",
    });
    await expect(pending).rejects.toBeInstanceOf(StaleAuthorizationError);
    const replacing = client.forAccount(account).draft("old-subject");
    await client.connectGithub("fixture");
    finish(null);
    await expect(replacing).rejects.toBeInstanceOf(StaleAuthorizationError);
    const abort = new AbortController();
    const cancelled = client
      .forAccount(account)
      .draft("old-subject", abort.signal);
    abort.abort();
    finish(null);
    await expect(cancelled).rejects.toBeInstanceOf(StaleAuthorizationError);
  });

  it("retains authored text but invalidates it when the durable stream requires reset", async () => {
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
    const key = collaborationKeys.draft(account, "missing");
    cache.setQueryData(key, { body: "User's retained text", generation: "1" });
    next = { ...changePage("3"), reset_required: true };
    await client.wake();
    await Promise.resolve();
    expect(cache.getQueryData(key)).toEqual({
      body: "User's retained text",
      generation: "1",
    });
    expect(cache.getQueryState(key)?.isInvalidated).toBe(true);
    stop();
    cache.clear();
  });

  it("binds local list/export arguments to the actor and invalidates only its authored projections", async () => {
    let next = changePage("1");
    const drafts = vi.fn().mockResolvedValue({ drafts: [], next_cursor: null });
    const exportDraft = vi.fn().mockResolvedValue(false);
    const client = new CollaborationClient(
      transport({
        drafts,
        exportDraft,
        listen: async () => () => {},
        changesSince: async () => next,
      }),
    );
    const cache = new QueryClient();
    const stop = client.installBridge(cache);
    await client.wake();
    const handle = client.forAccount(account);
    await handle.drafts({ cursor: "cursor", limit: 50 });
    expect(drafts).toHaveBeenCalledWith({
      account_id: account.id,
      cursor: "cursor",
      limit: 50,
    });
    expect(await handle.exportDraft("missing", "7")).toBe(false);
    expect(exportDraft).toHaveBeenCalledWith(account.id, "missing", "7");
    const own = collaborationKeys.drafts(account, {
      account_id: account.id,
      cursor: null,
      limit: 50,
    });
    const other = collaborationKeys.draft(
      { ...account, id: "actor-b" },
      "missing",
    );
    const remote = collaborationKeys.item(account, "provider");
    for (const key of [own, other, remote]) cache.setQueryData(key, "data");
    next = changePage("2", "1", [
      { revision: "2", account_id: account.id, scope: "drafts", reset: false },
    ]);
    await client.wake();
    expect(cache.getQueryState(own)?.isInvalidated).toBe(true);
    expect(cache.getQueryState(other)?.isInvalidated).toBe(false);
    expect(cache.getQueryState(remote)?.isInvalidated).toBe(false);
    stop();
    cache.clear();
  });
});

it("fences late native reads and resets provider projections across recovery without erasing unsaved draft context", async () => {
  let reset: (() => void) | undefined;
  const remove = vi.fn();
  const oldRead = deferred<CapabilitySnapshot>();
  const oldCommentRead = deferred<CommentDraftSnapshot>();
  const client = new CollaborationClient(
    transport({
      listen: async () => () => {},
      listenRuntimeReset: async (listener) => {
        reset = listener;
        return remove;
      },
      changesSince: async () => changePage("1"),
      capabilities: () => oldRead.promise,
      commentDraft: () => oldCommentRead.promise,
    }),
  );
  const cache = new QueryClient();
  const stop = client.installBridge(cache);
  await client.wake();
  const key = collaborationKeys.capabilities(account);
  const draftKey = collaborationKeys.draft(account, "retained");
  const commentKey = collaborationKeys.commentDraft(account, "issue");
  const cachedComment = commentDraft();
  cache.setQueryData(key, capability);
  cache.setQueryData(draftKey, {
    body: "Keep the author's text",
    generation: "2",
  });
  cache.setQueryData(commentKey, cachedComment);
  const pending = client.forAccount(account).capabilities();
  const pendingComment = client.forAccount(account).commentDraft("issue");
  const rejected = expect(pending).rejects.toBeInstanceOf(
    StaleAuthorizationError,
  );
  const rejectedComment = expect(pendingComment).rejects.toBeInstanceOf(
    StaleAuthorizationError,
  );
  reset?.();
  expect(cache.getQueryData(key)).toBeUndefined();
  expect(cache.getQueryData(draftKey)).toEqual({
    body: "Keep the author's text",
    generation: "2",
  });
  expect(cache.getQueryState(draftKey)?.isInvalidated).toBe(true);
  expect(cache.getQueryData(commentKey)).toEqual({
    ...cachedComment,
    context: null,
    availability: "unavailable",
    reason: "account_unavailable",
  });
  expect(cache.getQueryState(commentKey)?.isInvalidated).toBe(true);
  oldRead.resolve(capability);
  oldCommentRead.resolve(cachedComment);
  await rejected;
  await rejectedComment;
  expect(cache.getQueryData(commentKey)).toMatchObject({
    body: "Saved comment",
    generation: "3",
    context: null,
    availability: "unavailable",
    reason: "account_unavailable",
  });
  stop();
  expect(remove).toHaveBeenCalledTimes(1);
  cache.setQueryData(key, capability);
  reset?.();
  expect(cache.getQueryData(key)).toEqual(capability);
  cache.clear();
});

it("removes a recovery listener that attaches after the bridge is disposed", async () => {
  const installed = deferred<() => void>();
  const remove = vi.fn();
  const client = new CollaborationClient(
    transport({
      listen: async () => () => {},
      listenRuntimeReset: () => installed.promise,
      changesSince: async () => changePage("1"),
    }),
  );
  const cache = new QueryClient();
  const stop = client.installBridge(cache);
  stop();
  installed.resolve(remove);
  await vi.waitFor(() => expect(remove).toHaveBeenCalledTimes(1));
  cache.clear();
});

it.each([
  "reject",
  "resolve",
] as const)("restarts catch-up across native recovery and fences an old %s completion", async (completion) => {
  let reset: (() => void) | undefined;
  const oldRead = deferred<ChangePage>();
  const freshRead = deferred<ChangePage>();
  const changesSince = vi
    .fn()
    .mockResolvedValueOnce(changePage("1"))
    .mockImplementationOnce(() => oldRead.promise)
    .mockImplementationOnce(() => freshRead.promise);
  const client = new CollaborationClient(
    transport({
      listen: async () => () => {},
      listenRuntimeReset: async (listener) => {
        reset = listener;
        return () => {};
      },
      changesSince,
      capabilities: async () => ({
        ...capability,
        revision: "3",
        authorization_view: "3",
      }),
    }),
  );
  const changed = vi.fn();
  const unsubscribe = client.subscribeChanges(changed);
  const cache = new QueryClient();
  const stop = client.installBridge(cache);
  await vi.waitFor(() => expect(changesSince).toHaveBeenCalledTimes(1));
  const retired = client.wake();
  reset?.();
  await vi.waitFor(() => expect(changesSince).toHaveBeenCalledTimes(3));
  // Restart ignores the old cursor and never waits for its pending request.
  expect(changesSince.mock.calls).toEqual([["0"], ["1"], ["0"]]);
  freshRead.resolve(
    changePage("3", "3", [
      { revision: "3", account_id: account.id, scope: "drafts", reset: false },
    ]),
  );
  await vi.waitFor(() => expect(changed).toHaveBeenCalledTimes(1));
  if (completion === "reject") {
    oldRead.reject({ code: "not_ready", message: "Retired runtime" });
  } else {
    oldRead.resolve(
      changePage("2", "1", [
        {
          revision: "2",
          account_id: account.id,
          scope: "drafts",
          reset: false,
        },
      ]),
    );
  }
  await retired;
  expect(changed.mock.calls).toEqual([
    [{ revision: "3", account_id: account.id, scope: "drafts", reset: false }],
  ]);
  await expect(
    client.forAccount(account).capabilities(),
  ).resolves.toMatchObject({
    authorization_view: "3",
  });
  unsubscribe();
  stop();
  cache.clear();
});

it("invalidates effective item, list/count/search and inbox consumers across independent clients without touching provider head proofs", async () => {
  let next = changePage("1");
  const clients = [0, 1].map(
    () =>
      new CollaborationClient(
        transport({
          listen: async () => () => {},
          changesSince: async () => next,
        }),
      ),
  );
  const caches = clients.map(() => new QueryClient());
  const stops = clients.map((client, i) => client.installBridge(caches[i]));
  await Promise.all(clients.map((client) => client.wake()));
  const itemKey = collaborationKeys.item(account, "pull");
  const listKey = collaborationKeys.items(account, {
    ...itemQuery,
    account_id: account.id,
    search: "changed",
    state: "closed",
  });
  const inboxKey = collaborationKeys.inbox(account, {
    account_id: account.id,
    remote_state: "unread",
    local_state: "all",
    search: null,
    cursor: null,
    limit: 50,
  });
  const bodyKey = collaborationKeys.detail(account, {
    account_id: account.id,
    subject_id: "pull",
    facet: "body",
    cursor: null,
    limit: 50,
  });
  const commentsKey = collaborationKeys.detail(account, {
    account_id: account.id,
    subject_id: "pull",
    facet: "comments",
    cursor: null,
    limit: 50,
  });
  const otherBody = collaborationKeys.detail(account, {
    account_id: account.id,
    subject_id: "other",
    facet: "body",
    cursor: null,
    limit: 50,
  });
  const draftKey = collaborationKeys.draft(account, "pull");
  const textEditKey = collaborationKeys.textEdit(account, "pull");
  const otherActorKey = collaborationKeys.item(
    { ...account, id: "other-account" },
    "pull",
  );
  for (const cache of caches)
    for (const key of [
      itemKey,
      listKey,
      inboxKey,
      bodyKey,
      commentsKey,
      otherBody,
      draftKey,
      textEditKey,
      otherActorKey,
    ])
      cache.setQueryData(key, "saved");
  next = changePage("2", "1", [
    {
      revision: "2",
      account_id: account.id,
      scope: "effective:pull",
      reset: false,
    },
  ]);
  await Promise.all(clients.map((client) => client.wake()));
  for (const cache of caches) {
    for (const key of [itemKey, listKey, inboxKey, bodyKey, textEditKey])
      expect(cache.getQueryState(key)?.isInvalidated).toBe(true);
    for (const key of [commentsKey, otherBody, draftKey, otherActorKey])
      expect(cache.getQueryState(key)?.isInvalidated).toBe(false);
  }
  stops.forEach((stop) => stop());
  caches.forEach((cache) => cache.clear());
});

it("an effective-intent change cancels a held list response before it can overwrite committed pending values", async () => {
  let next = changePage("1");
  const held = deferred<ItemPage>();
  const saved: ItemPage = {
    ...page,
    revision: "2",
    total_count: 1,
    items: [{ ...privateItem, title: "Pending title" }],
    pending_intents: [
      {
        subject_id: privateItem.id,
        commands: [
          { command_id: "command", state: "queued", fields: ["title"] },
        ],
      },
    ],
  };
  const items = vi
    .fn<() => Promise<ItemPage>>()
    .mockImplementationOnce(() => held.promise)
    .mockResolvedValue(saved);
  const client = new CollaborationClient(
    transport({
      items,
      listen: async () => () => {},
      changesSince: async () => next,
    }),
  );
  const cache = new QueryClient(),
    stop = client.installBridge(cache);
  await client.wake();
  const key = collaborationKeys.items(account, {
    ...itemQuery,
    account_id: account.id,
  });
  const observer = new QueryObserver(cache, {
    queryKey: key,
    queryFn: ({ signal }) =>
      client.forAccount(account).items(itemQuery, signal),
    staleTime: Infinity,
    retry: false,
  });
  const unsubscribe = observer.subscribe(() => {});
  next = changePage("2", "1", [
    {
      revision: "2",
      account_id: account.id,
      scope: `effective:${privateItem.id}`,
      reset: false,
    },
  ]);
  await client.wake();
  await vi.waitFor(() => expect(cache.getQueryData(key)).toEqual(saved));
  held.resolve({ ...page, items: [privateItem] });
  await held.promise;
  await Promise.resolve();
  expect(items).toHaveBeenCalledTimes(2);
  expect(cache.getQueryData(key)).toEqual(saved);
  unsubscribe();
  stop();
  cache.clear();
});

function recoveryDetail(): CommandRecoveryDetail {
  return {
    command: {
      account_id: account.id,
      command_id: "command",
      target_id: "issue",
      target_kind: "issue",
      operation_kind: "fixture.edit",
      payload_version: 1,
      state: "conflict",
      admitted_at: "2026-10-08T00:00:00Z",
      attempt_count: 0,
      paused: false,
      quarantined: false,
      attention: null,
      replacement_id: null,
      blocked_reason: null,
    },
    context: {
      account_id: account.id,
      command_id: "command",
      expected_generation: "1",
      expected_epoch: account.authorization_epoch,
      authorization_view: "1",
      review_token: "native-proof",
    },
    fields: [],
    can_retry: false,
    can_cancel: true,
    can_pause: false,
    can_replace: false,
    reason: null,
    revision: "1",
  };
}

it("fences held recovery details after disconnect and refuses a foreign action context before IPC", async () => {
  const held = deferred<CommandRecoveryDetail>();
  const action = vi.fn(async () => {
    throw new Error("must not dispatch");
  });
  const client = new CollaborationClient(
    transport({
      accounts: async () => snapshot,
      commandRecoveryDetail: () => held.promise,
      commandRecoveryAction: action,
      disconnect: async () => "2",
    }),
  );
  await client.accounts();
  const bound = client.forAccount(account);
  const pending = bound.commandRecoveryDetail("command");
  const rejected = expect(pending).rejects.toBeInstanceOf(
    StaleAuthorizationError,
  );
  await client.disconnect(account.id);
  held.resolve(recoveryDetail());
  await rejected;
  for (const context of [
    { ...recoveryDetail().context, account_id: "other" },
    { ...recoveryDetail().context, expected_epoch: "99" },
  ])
    expect(() =>
      bound.commandRecoveryAction({
        context,
        action: "cancel",
        action_id: "action",
      }),
    ).toThrow(StaleAuthorizationError);
  expect(action).not.toHaveBeenCalled();
});

it("invalidates command review after local actions and provider changes without touching private drafts", async () => {
  let next = changePage("1");
  const client = new CollaborationClient(
    transport({ listen: async () => () => {}, changesSince: async () => next }),
  );
  const cache = new QueryClient();
  const stop = client.installBridge(cache);
  await client.wake();
  const list = collaborationKeys.commandRecovery(account, {
    account_id: account.id,
    target_id: null,
    include_terminal: false,
    cursor: null,
    limit: 50,
  });
  const detailKey = collaborationKeys.commandRecoveryDetail(account, "command");
  const draft = collaborationKeys.draft(account, "issue");
  for (const [revision, scope] of [
    ["2", "commands"],
    ["3", "detail:issue:body"],
  ]) {
    for (const key of [list, detailKey, draft])
      cache.setQueryData(key, "saved");
    next = changePage(revision, "1", [
      { revision, account_id: account.id, scope, reset: false },
    ]);
    await client.wake();
    expect(cache.getQueryState(list)?.isInvalidated).toBe(true);
    expect(cache.getQueryState(detailKey)?.isInvalidated).toBe(true);
    expect(cache.getQueryState(draft)?.isInvalidated).toBe(false);
  }
  stop();
  cache.clear();
});

function commentDraft(): CommentDraftSnapshot {
  return {
    account_id: account.id,
    subject_id: "issue",
    body: "Saved comment",
    generation: "3",
    context: {
      account_id: account.id,
      subject_id: "issue",
      authorization_epoch: account.authorization_epoch,
      authorization_view: "1",
      review_token: "a".repeat(64),
    },
    availability: "available",
    reason: null,
    submission: null,
    revision: "1",
    authorization_view: "1",
  };
}

it("fences comment draft context, exact sends and created receipt subjects", async () => {
  const send = vi.fn(async (request: SendCommentRequest) => ({
    account_id: request.context.account_id,
    command_id: request.command_id,
    admitted_revision: "2",
    duplicate: false,
  }));
  const created = vi.fn(
    async (): Promise<CreatedCommentPage> => ({
      account_id: account.id,
      subject_id: "other-issue",
      comments: [],
      next_cursor: null,
      revision: "1",
      authorization_view: "1",
    }),
  );
  const client = new CollaborationClient(
    transport({
      commentDraft: async () => commentDraft(),
      sendComment: send,
      createdComments: created,
    }),
  );
  const bound = client.forAccount(account);
  expect(await bound.commentDraft("issue")).toEqual(commentDraft());
  const request: SendCommentRequest = {
    context: commentDraft().context!,
    draft_generation: "3",
    command_id: "00000000-0000-4000-8000-000000000001",
    accept_background_delivery: true,
  };
  await expect(bound.sendComment(request)).resolves.toMatchObject({
    command_id: request.command_id,
  });
  await expect(
    bound.sendComment({
      ...request,
      context: { ...request.context, authorization_epoch: "99" },
    }),
  ).rejects.toBeInstanceOf(StaleAuthorizationError);
  expect(send).toHaveBeenCalledTimes(1);
  await expect(
    bound.createdComments({ subject_id: "issue", cursor: null, limit: 25 }),
  ).rejects.toBeInstanceOf(StaleAuthorizationError);
});

it("rejects a late comment draft snapshot after account retirement", async () => {
  const held = deferred<CommentDraftSnapshot>();
  const client = new CollaborationClient(
    transport({
      commentDraft: () => held.promise,
      disconnect: async () => "2",
    }),
  );
  const pending = client.forAccount(account).commentDraft("issue");
  const rejected = expect(pending).rejects.toBeInstanceOf(
    StaleAuthorizationError,
  );
  await client.disconnect(account.id);
  held.resolve(commentDraft());
  await rejected;
});

it("invalidates only the matching authored comment draft and created history", async () => {
  let next = changePage("1");
  const client = new CollaborationClient(
    transport({ listen: async () => () => {}, changesSince: async () => next }),
  );
  const cache = new QueryClient();
  const stop = client.installBridge(cache);
  await client.wake();
  const draft = collaborationKeys.commentDraft(account, "issue");
  const otherDraft = collaborationKeys.commentDraft(account, "other");
  const draftList = collaborationKeys.commentDrafts(account, {
    account_id: account.id,
    cursor: null,
    limit: 50,
  });
  const created = collaborationKeys.createdComments(account, {
    account_id: account.id,
    subject_id: "issue",
    cursor: null,
    limit: 25,
  });
  const otherCreated = collaborationKeys.createdComments(account, {
    account_id: account.id,
    subject_id: "other",
    cursor: null,
    limit: 25,
  });
  for (const key of [draft, otherDraft, draftList, created, otherCreated])
    cache.setQueryData(key, "saved");
  next = changePage("2", "1", [
    {
      revision: "2",
      account_id: account.id,
      scope: "comment_draft:issue",
      reset: false,
    },
  ]);
  await client.wake();
  expect(cache.getQueryState(draft)?.isInvalidated).toBe(true);
  expect(cache.getQueryState(otherDraft)?.isInvalidated).toBe(false);
  expect(cache.getQueryState(draftList)?.isInvalidated).toBe(true);
  expect(cache.getQueryState(created)?.isInvalidated).toBe(false);
  next = changePage("3", "1", [
    {
      revision: "3",
      account_id: account.id,
      scope: "created_comments:issue",
      reset: false,
    },
  ]);
  await client.wake();
  expect(cache.getQueryState(created)?.isInvalidated).toBe(true);
  expect(cache.getQueryState(otherCreated)?.isInvalidated).toBe(false);
  stop();
  cache.clear();
});

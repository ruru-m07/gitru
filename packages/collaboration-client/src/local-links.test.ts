import type {
  ChangePage,
  LocalCloneSnapshot,
  LocalLinkInspection,
  LocalNavigationReceipt,
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
  actor_id: "actor-a",
  provider: "github",
  host: "https://github.com",
  authorization_epoch: "9007199254740993",
  login: "fixture",
  state: "active",
  display_name: null,
  notifications_supported: true,
};
const inspection = (revision = "1"): LocalLinkInspection => ({
  local_repository_id: "registered-a",
  remotes: null,
  observation_error: "unavailable",
  preview_id: null,
  snapshot: {
    links: [],
    resolutions: [],
    bindings: [],
    bindings_generation: "9007199254740993",
    revision,
    authorization_view: "1",
  },
});
const receipt: LocalNavigationReceipt = {
  local_repository_id: "registered-a",
  account_id: account.id,
  instance_id: "github:https://github.com/",
  repository_id: "repo-a",
  authorization_epoch: account.authorization_epoch,
  selected: false,
};
function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((finish) => {
    resolve = finish;
  });
  return { promise, resolve };
}
const flush = async () => {
  for (let n = 0; n < 20; n += 1) await Promise.resolve();
};
const stops: (() => void)[] = [];
const caches: QueryClient[] = [];
afterEach(() => {
  for (const stop of stops.splice(0)) stop();
  for (const cache of caches.splice(0)) cache.clear();
});
function fixture() {
  let revision = "1";
  let changes: ChangePage["changes"] = [];
  let hint!: () => void;
  const unexpected = vi.fn(async () => {
    throw new Error("Unexpected provider operation");
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
      authorization_view: "1",
    }),
    diagnostics: unexpected,
    exportDiagnostics: unexpected,
    connectGithub: unexpected,
    connectGitlab: unexpected,
    connectBitbucketCloud: unexpected,
    connectGithubCli: unexpected,
    discoverGithubCli: unexpected,
    disconnect: vi.fn(async () => "2"),
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
    changesSince: async () => ({
      revision,
      authorization_view: "1",
      changes,
      has_more: false,
      reset_required: false,
    }),
    listen: async () => () => {},
    listenRuntimeReset: async () => () => {},
    listenLocalChanges: async (next: () => void) => {
      hint = next;
      return () => {};
    },
    localLinks: vi.fn(async () => inspection(revision)),
    confirmLocalLink: unexpected,
    removeLocalLink: vi.fn(async () => "2"),
    saveTransportBinding: unexpected,
    removeTransportBinding: unexpected,
    localClones: vi.fn(async () => ({ clones: [] }) as LocalCloneSnapshot),
    validateLocalNavigation: vi.fn(async () => receipt),
  } satisfies CollaborationTransport;
  const client = new CollaborationClient(transport);
  const cache = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  caches.push(cache);
  stops.push(client.installBridge(cache));
  return {
    client,
    transport,
    cache,
    unexpected,
    hint: () => hint(),
    change: async (scope: string, reset = false) => {
      revision = (BigInt(revision) + 1n).toString();
      changes = [{ account_id: account.id, revision, scope, reset }];
      await client.wake();
    },
  };
}
describe("account-independent authored links and authorized clone projections", () => {
  it("uses durable local IDs and isolates clone keys by actor, epoch, installation and repository", () => {
    expect(collaborationKeys.localLinks("registered-a", 3)).toEqual([
      "collaboration",
      "local-links",
      "registered-a",
      3,
    ]);
    const key = collaborationKeys.localClones(
      account,
      receipt.instance_id,
      "repo-a",
    );
    expect(key).not.toEqual(
      collaborationKeys.localClones(
        { ...account, actor_id: "other" },
        receipt.instance_id,
        "repo-a",
      ),
    );
    expect(key).not.toEqual(
      collaborationKeys.localClones(
        { ...account, authorization_epoch: "9007199254740994" },
        receipt.instance_id,
        "repo-a",
      ),
    );
    expect(key).not.toEqual(
      collaborationKeys.localClones(account, "other-instance", "repo-a"),
    );
    expect(key).not.toEqual(
      collaborationKeys.localClones(account, receipt.instance_id, "repo-b"),
    );
    expect(key).not.toEqual(
      collaborationKeys.localClones(
        account,
        receipt.instance_id,
        "repo-a",
        "fork-provider-id",
      ),
    );
  });
  it("captures the account epoch before clone reads even if the caller mutates its metadata object", async () => {
    const { client, transport } = fixture();
    const mutable = { ...account };
    const handle = client.forAccount(mutable);
    mutable.authorization_epoch = "changed";
    await handle.localClones(receipt.instance_id, "repo-a", "fork-provider-id");
    expect(transport.localClones).toHaveBeenCalledExactlyOnceWith({
      account_id: account.id,
      authorization_epoch: account.authorization_epoch,
      instance_id: receipt.instance_id,
      repository_id: "repo-a",
      source_repository_provider_id: "fork-provider-id",
    });
  });
  it("fences pending global multi-account inspections before an account disconnect", async () => {
    const { client, transport } = fixture();
    await client.wake();
    const pending = deferred<LocalLinkInspection>();
    transport.localLinks.mockImplementationOnce(() => pending.promise);
    const read = client.localLinks("registered-a");
    const rejected = expect(read).rejects.toBeInstanceOf(
      StaleAuthorizationError,
    );
    await client.disconnect(account.id);
    pending.resolve(inspection());
    await rejected;
  });
  it("cancels an initial pending observer before a durable local-link change and reads only the committed replacement", async () => {
    const { client, transport, cache, change, unexpected } = fixture();
    await client.wake();
    const pending = deferred<LocalLinkInspection>();
    transport.localLinks.mockImplementationOnce(() => pending.promise);
    const observer = new QueryObserver(cache, {
      queryKey: collaborationKeys.localLinks("registered-a", 0),
      queryFn: ({ signal }) => client.localLinks("registered-a", signal),
      staleTime: Infinity,
      retry: false,
    });
    const stop = observer.subscribe(() => {});
    stops.push(stop);
    await flush();
    await change("local_link:registered-a");
    await flush();
    expect(transport.localLinks).toHaveBeenCalledTimes(2);
    pending.resolve(inspection("1"));
    await flush();
    expect(observer.getCurrentResult().data?.snapshot.revision).toBe("2");
    expect(unexpected).not.toHaveBeenCalled();
  });
  it("invalidates visible safe observations on Git hints without provider refresh/hydration", async () => {
    const { client, transport, cache, hint, unexpected } = fixture();
    await client.wake();
    const observer = new QueryObserver(cache, {
      queryKey: collaborationKeys.localLinks("registered-a", 0),
      queryFn: ({ signal }) => client.localLinks("registered-a", signal),
      staleTime: Infinity,
      retry: false,
    });
    stops.push(observer.subscribe(() => {}));
    await flush();
    hint();
    await flush();
    expect(transport.localLinks).toHaveBeenCalledTimes(2);
    expect(unexpected).not.toHaveBeenCalled();
  });
  it("cancels pending inspections and clone projections for a binding committed by another webview", async () => {
    const { client, transport, cache, change, unexpected } = fixture();
    await client.wake();
    const pendingInspection = deferred<LocalLinkInspection>();
    const pendingClones = deferred<LocalCloneSnapshot>();
    transport.localLinks.mockImplementationOnce(
      () => pendingInspection.promise,
    );
    transport.localClones.mockImplementationOnce(() => pendingClones.promise);
    const inspectionObserver = new QueryObserver(cache, {
      queryKey: collaborationKeys.localLinks("registered-a", 0),
      queryFn: ({ signal }) => client.localLinks("registered-a", signal),
      staleTime: Infinity,
      retry: false,
    });
    const cloneObserver = new QueryObserver(cache, {
      queryKey: collaborationKeys.localClones(
        account,
        receipt.instance_id,
        "repo-a",
      ),
      queryFn: ({ signal }) =>
        client
          .forAccount(account)
          .localClones(receipt.instance_id, "repo-a", null, signal),
      staleTime: Infinity,
      retry: false,
    });
    stops.push(
      inspectionObserver.subscribe(() => {}),
      cloneObserver.subscribe(() => {}),
    );
    await flush();
    await change("local_transport_bindings");
    await flush();
    expect(transport.localLinks).toHaveBeenCalledTimes(2);
    expect(transport.localClones).toHaveBeenCalledTimes(2);
    pendingInspection.resolve(inspection("1"));
    pendingClones.resolve({
      clones: [
        {
          local_repository_id: "retired",
          local_repository_name: "Retired binding clone",
          link_id: "retired-link",
          generation: "1",
          state: "linked",
        },
      ],
    });
    await flush();
    expect(inspectionObserver.getCurrentResult().data?.snapshot.revision).toBe(
      "2",
    );
    expect(cloneObserver.getCurrentResult().data?.clones).toEqual([]);
    expect(unexpected).not.toHaveBeenCalled();
  });
  it("never refetches local Git observations for unrelated body or private draft events", async () => {
    const { client, transport, cache, change } = fixture();
    await client.wake();
    const observer = new QueryObserver(cache, {
      queryKey: collaborationKeys.localLinks("registered-a", 0),
      queryFn: ({ signal }) => client.localLinks("registered-a", signal),
      staleTime: Infinity,
      retry: false,
    });
    stops.push(observer.subscribe(() => {}));
    await flush();
    await change("detail:subject:body");
    await flush();
    expect(transport.localLinks).toHaveBeenCalledTimes(1);
  });
  it("rejects pending native navigation after a same-view account reset", async () => {
    const { client, transport, change } = fixture();
    await client.wake();
    const pending = deferred<LocalNavigationReceipt>();
    transport.validateLocalNavigation.mockImplementationOnce(
      () => pending.promise,
    );
    const navigation = client.validateLocalNavigation({
      local_repository_id: "registered-a",
      link_id: "link-a",
      generation: "1",
      direction: "git",
    });
    const rejected = expect(navigation).rejects.toBeInstanceOf(
      StaleAuthorizationError,
    );
    await change("account", true);
    pending.resolve(receipt);
    await rejected;
  });
  it("keeps a foreground lease through link edits and retires it with pending navigation on account reset", async () => {
    const { client, transport, change, unexpected } = fixture();
    const activity = vi.fn(async () => ({ generation: "1", active: true }));
    const acquire = vi.fn(async () => ({
      lease_id: "foreground-native-lease",
      owner_generation: "1",
      expires_in_seconds: 45,
      renew_after_seconds: 15,
    }));
    const release = vi.fn(async () => {});
    Object.assign(transport, {
      demandActivity: activity,
      listenDemandActivity: async () => () => {},
      acquireDemand: acquire,
      releaseDemand: release,
    });
    await client.wake();
    const handle = client.forAccount(account).retainDemand({
      kind: "detail",
      repository_id: null,
      subject_id: "subject-a",
      facet: "body",
    });
    await flush();
    expect(acquire).toHaveBeenCalledExactlyOnceWith({
      account_id: account.id,
      authorization_epoch: account.authorization_epoch,
      owner_generation: "1",
      target: {
        kind: "detail",
        repository_id: null,
        subject_id: "subject-a",
        facet: "body",
      },
    });
    await change("local_transport_bindings");
    await flush();
    expect(acquire).toHaveBeenCalledTimes(1);
    expect(release).not.toHaveBeenCalled();
    expect(activity).toHaveBeenCalledTimes(1);

    const pending = deferred<LocalNavigationReceipt>();
    transport.validateLocalNavigation.mockImplementationOnce(
      () => pending.promise,
    );
    const navigation = client.validateLocalNavigation({
      local_repository_id: "registered-a",
      link_id: "link-a",
      generation: "1",
      direction: "git",
    });
    const rejected = expect(navigation).rejects.toBeInstanceOf(
      StaleAuthorizationError,
    );
    await change("account", true);
    pending.resolve(receipt);
    await rejected;
    await flush();
    expect(release).toHaveBeenCalledExactlyOnceWith({
      lease_id: "foreground-native-lease",
    });
    expect(acquire).toHaveBeenCalledTimes(1);
    expect(unexpected).not.toHaveBeenCalled();
    handle.release();
  });
});

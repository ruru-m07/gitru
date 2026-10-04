import {
  type AccountSnapshot,
  type CapabilitySnapshot,
  type CapabilityTarget,
  type ChangePage,
  type CollaborationChange,
  type ConfirmLocalLinkPreview,
  type ContextCapabilityRequest,
  type ContextualCapabilitySnapshot,
  type DemandTarget,
  type DetailQuery,
  type DetailSnapshot,
  type DiscoverNotificationSubjectRequest,
  type GithubCliDiscovery,
  type HydrateDetailRequest,
  type ItemPage,
  type ItemQuery,
  type ItemSnapshot,
  type LocalCloneRequest,
  type LocalCloneSnapshot,
  type LocalDraft,
  type LocalLinkInspection,
  type LocalLinkVersion,
  type LocalLinkWriteReceipt,
  type LocalNavigationReceipt,
  type LocalNavigationRequest,
  type LocalTransportBinding,
  type NotificationSubjectQuery,
  type NotificationSubjectSnapshot,
  type RefreshReceipt,
  type RefreshRequest,
  type RemoteAccount,
  type RepositorySnapshot,
  type ResourceLocator,
  type ResourceResolution,
  type TransportBindingRequest,
} from "@gitru/commands";
import type { QueryClient } from "@tanstack/react-query";
import {
  AuthorizationFence,
  StaleAuthorizationError,
} from "./authorization-fence";
import { installCapabilityDeadlines } from "./capability-deadlines";
import {
  type DemandAccount,
  DemandCoordinator,
  type DemandTransport,
} from "./demand-coordinator";
import {
  type NavigationInput,
  type NavigationResource,
  type NavigationScope,
  NavigationWorkingSet,
} from "./navigation-working-set";
import { compareRevisions, RevisionBridge } from "./revision-bridge";

export interface CollaborationTransport extends DemandTransport {
  localLinks(localRepositoryId: string): Promise<LocalLinkInspection>;
  confirmLocalLink(
    request: ConfirmLocalLinkPreview,
  ): Promise<LocalLinkWriteReceipt>;
  removeLocalLink(version: LocalLinkVersion): Promise<string>;
  saveTransportBinding(
    request: TransportBindingRequest,
  ): Promise<LocalTransportBinding>;
  removeTransportBinding(
    version: LocalLinkVersion,
    bindingsGeneration: string,
  ): Promise<string>;
  localClones(request: LocalCloneRequest): Promise<LocalCloneSnapshot>;
  validateLocalNavigation(
    request: LocalNavigationRequest,
  ): Promise<LocalNavigationReceipt>;
  listenLocalChanges(onWake: () => void): Promise<() => void>;
  accounts(): Promise<AccountSnapshot>;
  connectGithub(token: string): Promise<RemoteAccount>;
  connectGitlab(token: string): Promise<RemoteAccount>;
  discoverGithubCli(): Promise<GithubCliDiscovery>;
  connectGithubCli(candidateId: string): Promise<RemoteAccount>;
  disconnect(accountId: string): Promise<string>;
  repositories(accountId: string): Promise<RepositorySnapshot>;
  selectRepository(
    accountId: string,
    repositoryId: string,
    selected: boolean,
  ): Promise<string>;
  items(query: ItemQuery): Promise<ItemPage>;
  item(accountId: string, itemId: string): Promise<ItemSnapshot>;
  refresh(request: RefreshRequest): Promise<RefreshReceipt>;
  changesSince(afterRevision: string): Promise<ChangePage>;
  saveDraft(draft: LocalDraft): Promise<LocalDraft>;
  draft(accountId: string, subjectId: string): Promise<LocalDraft | null>;
  capabilities(accountId: string): Promise<CapabilitySnapshot>;
  contextualCapabilities(
    request: ContextCapabilityRequest,
  ): Promise<ContextualCapabilitySnapshot>;
  resolveResource(
    accountId: string,
    locator: ResourceLocator,
  ): Promise<ResourceResolution>;
  detail(query: DetailQuery): Promise<DetailSnapshot>;
  hydrateDetail(request: HydrateDetailRequest): Promise<RefreshReceipt>;
  notificationSubject(
    query: NotificationSubjectQuery,
  ): Promise<NotificationSubjectSnapshot>;
  discoverNotificationSubject(
    request: DiscoverNotificationSubjectRequest,
  ): Promise<RefreshReceipt>;
  listen(onWake: () => void): Promise<() => void>;
}

export const collaborationKeys = {
  all: ["collaboration"] as const,
  localLinks: (localRepositoryId: string, version: number) =>
    ["collaboration", "local-links", localRepositoryId, version] as const,
  localClones: (
    account: RemoteAccount,
    instanceId: string,
    repositoryId: string,
  ) =>
    [
      ...collaborationKeys.account(account.id),
      account.authorization_epoch,
      "local-clones",
      account.actor_id,
      instanceId,
      repositoryId,
    ] as const,
  accounts: (version: number) =>
    ["collaboration", "accounts", version] as const,
  githubCli: ["collaboration", "github-cli-accounts"] as const,
  account: (accountId: string) =>
    ["collaboration", "account", accountId] as const,
  repositories: (account: RemoteAccount) =>
    [
      ...collaborationKeys.account(account.id),
      account.authorization_epoch,
      "repositories",
    ] as const,
  items: (account: RemoteAccount, query: ItemQuery) =>
    [
      ...collaborationKeys.account(account.id),
      account.authorization_epoch,
      "items",
      query,
    ] as const,
  item: (account: RemoteAccount, itemId: string) =>
    [
      ...collaborationKeys.account(account.id),
      account.authorization_epoch,
      "item",
      itemId,
    ] as const,
  draft: (account: RemoteAccount, subjectId: string) =>
    [
      ...collaborationKeys.account(account.id),
      account.authorization_epoch,
      "draft",
      subjectId,
    ] as const,
  capabilities: (account: RemoteAccount) =>
    [
      ...collaborationKeys.account(account.id),
      account.authorization_epoch,
      "capabilities",
    ] as const,
  contextualCapabilities: (account: RemoteAccount, target: CapabilityTarget) =>
    [...collaborationKeys.capabilities(account), "context", target] as const,
  resource: (account: RemoteAccount, locator: ResourceLocator) =>
    [
      ...collaborationKeys.account(account.id),
      account.authorization_epoch,
      "resource",
      locator,
    ] as const,
  detail: (account: RemoteAccount, query: DetailQuery) =>
    [
      ...collaborationKeys.account(account.id),
      account.authorization_epoch,
      "detail",
      query,
    ] as const,
  notificationSubject: (account: RemoteAccount, notificationId: string) =>
    [
      ...collaborationKeys.account(account.id),
      account.authorization_epoch,
      "notification-subject",
      account.actor_id,
      notificationId,
    ] as const,
};

const ACCOUNTS_SCOPE = "$accounts";
const LOCAL_LINKS_SCOPE = "$local-links";

/** Account-bound local reads and explicit background sync intents. No provider HTTP. */
export class CollaborationClient {
  private readonly fence = new AuthorizationFence();
  private authorizationView: string | null = null;
  private authorizationRevision = "0";
  private version = 0;
  private readonly listeners = new Set<() => void>();
  private readonly changeListeners = new Set<
    (change: CollaborationChange) => void
  >();
  private bridge: RevisionBridge<CollaborationChange> | null = null;
  private queryClient: QueryClient | null = null;
  private readonly demands: DemandCoordinator;
  private navigation: NavigationWorkingSet | null = null;

  constructor(readonly transport: CollaborationTransport) {
    this.demands = new DemandCoordinator(transport);
  }

  /** Ephemeral view interest; it never reads or hydrates provider data itself. */
  retainDemand(account: DemandAccount, target: DemandTarget) {
    return this.demands.retain(account, target);
  }

  /** Bounded speculative reads over the same local cache and authorization fences. */
  navigationScope(input: NavigationInput): NavigationScope {
    return (
      this.navigation?.scope(input) ?? {
        enter() {},
        leave() {},
        visit() {},
        dispose() {},
      }
    );
  }

  subscribe = (listener: () => void) => {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  };
  getVersion = () => this.version;
  subscribeChanges = (listener: (change: CollaborationChange) => void) => {
    this.changeListeners.add(listener);
    return () => {
      this.changeListeners.delete(listener);
    };
  };

  async accounts(signal?: AbortSignal): Promise<AccountSnapshot> {
    const snapshot = await this.fence.read(
      ACCOUNTS_SCOPE,
      () => this.transport.accounts(),
      signal,
    );
    this.acceptSnapshot(snapshot);
    return snapshot;
  }

  async localLinks(localRepositoryId: string, signal?: AbortSignal) {
    const inspection = await this.fence.read(
      LOCAL_LINKS_SCOPE,
      () => this.transport.localLinks(localRepositoryId),
      signal,
    );
    this.acceptSnapshot(inspection.snapshot);
    return inspection;
  }
  async confirmLocalLink(request: ConfirmLocalLinkPreview) {
    const receipt = await this.fence.read(LOCAL_LINKS_SCOPE, () =>
      this.transport.confirmLocalLink({ ...request }),
    );
    this.acceptSnapshot(receipt);
    await this.invalidateLocalLinks();
    return receipt;
  }
  async removeLocalLink(version: LocalLinkVersion) {
    const revision = await this.fence.read(LOCAL_LINKS_SCOPE, () =>
      this.transport.removeLocalLink({ ...version }),
    );
    await this.invalidateLocalLinks();
    return revision;
  }
  async saveTransportBinding(request: TransportBindingRequest) {
    const binding = await this.fence.read(LOCAL_LINKS_SCOPE, () =>
      this.transport.saveTransportBinding({ ...request }),
    );
    await this.invalidateLocalLinks();
    return binding;
  }
  async removeTransportBinding(
    version: LocalLinkVersion,
    bindingsGeneration: string,
  ) {
    const revision = await this.fence.read(LOCAL_LINKS_SCOPE, () =>
      this.transport.removeTransportBinding({ ...version }, bindingsGeneration),
    );
    await this.invalidateLocalLinks();
    return revision;
  }
  validateLocalNavigation(
    request: LocalNavigationRequest,
    signal?: AbortSignal,
  ) {
    return this.fence.read(
      LOCAL_LINKS_SCOPE,
      () => this.transport.validateLocalNavigation({ ...request }),
      signal,
    );
  }
  async invalidateLocalLinks() {
    this.fence.invalidate(LOCAL_LINKS_SCOPE);
    if (!this.queryClient) return;
    const affected = {
      predicate: (query: { queryKey: readonly unknown[] }) =>
        query.queryKey[0] === "collaboration" &&
        (query.queryKey[1] === "local-links" ||
          query.queryKey[4] === "local-clones"),
    };
    await this.queryClient.cancelQueries(affected);
    await this.queryClient.invalidateQueries(affected);
  }

  forAccount(accountInput: RemoteAccount) {
    const account = { ...accountInput };
    const read = async <
      T extends { authorization_view: string; revision: string },
    >(
      load: () => Promise<T>,
      signal?: AbortSignal,
    ): Promise<T> => {
      const snapshot = await this.fence.read(account.id, load, signal);
      this.acceptSnapshot(snapshot);
      return snapshot;
    };
    const cloneAccount = {
      id: account.id,
      authorization_epoch: account.authorization_epoch,
    };
    return {
      retainDemand: (target: DemandTarget) =>
        this.retainDemand(account, target),
      notificationSubject: async (
        notificationId: string,
        signal?: AbortSignal,
      ) => {
        const snapshot = await this.fence.read(
          cloneAccount.id,
          () =>
            this.transport.notificationSubject({
              account_id: cloneAccount.id,
              authorization_epoch: cloneAccount.authorization_epoch,
              notification_id: notificationId,
            }),
          signal,
        );
        if (snapshot.authorization_epoch !== account.authorization_epoch)
          throw new StaleAuthorizationError();
        this.acceptSnapshot(snapshot);
        return snapshot;
      },
      discoverNotificationSubject: (
        notificationId: string,
        selectorGeneration: string,
        signal?: AbortSignal,
      ) =>
        this.fence.read(
          cloneAccount.id,
          () =>
            this.transport.discoverNotificationSubject({
              account_id: cloneAccount.id,
              authorization_epoch: cloneAccount.authorization_epoch,
              notification_id: notificationId,
              selector_generation: selectorGeneration,
            }),
          signal,
        ),
      localClones: (
        instanceId: string,
        repositoryId: string,
        signal?: AbortSignal,
      ) =>
        this.fence.read(
          cloneAccount.id,
          () =>
            this.transport.localClones({
              account_id: cloneAccount.id,
              authorization_epoch: cloneAccount.authorization_epoch,
              instance_id: instanceId,
              repository_id: repositoryId,
            }),
          signal,
        ),
      repositories: (signal?: AbortSignal) =>
        read(() => this.transport.repositories(account.id), signal),
      items: (query: Omit<ItemQuery, "account_id">, signal?: AbortSignal) =>
        read(
          () => this.transport.items({ ...query, account_id: account.id }),
          signal,
        ),
      item: (itemId: string, signal?: AbortSignal) =>
        read(() => this.transport.item(account.id, itemId), signal),
      capabilities: (signal?: AbortSignal) =>
        read(() => this.transport.capabilities(account.id), signal),
      contextualCapabilities: (
        target: CapabilityTarget,
        signal?: AbortSignal,
      ) =>
        read(
          () =>
            this.transport.contextualCapabilities({
              account_id: account.id,
              authorization_epoch: account.authorization_epoch,
              target,
            }),
          signal,
        ),
      resolveResource: (locator: ResourceLocator, signal?: AbortSignal) =>
        read(() => this.transport.resolveResource(account.id, locator), signal),
      detail: (query: Omit<DetailQuery, "account_id">, signal?: AbortSignal) =>
        read(
          () => this.transport.detail({ ...query, account_id: account.id }),
          signal,
        ),
      hydrateDetail: (
        request: Omit<
          HydrateDetailRequest,
          "account_id" | "authorization_epoch"
        >,
      ) =>
        this.fence.read(account.id, () =>
          this.transport.hydrateDetail({
            ...request,
            account_id: account.id,
            authorization_epoch: account.authorization_epoch,
          }),
        ),
      refresh: (request: Omit<RefreshRequest, "account_id">) =>
        this.transport.refresh({ ...request, account_id: account.id }),
      selectRepository: (repositoryId: string, selected: boolean) =>
        this.transport.selectRepository(account.id, repositoryId, selected),
      draft: (subjectId: string, signal?: AbortSignal) =>
        this.fence.read(
          account.id,
          () => this.transport.draft(account.id, subjectId),
          signal,
        ),
      saveDraft: (draft: Omit<LocalDraft, "account_id">) =>
        this.fence.read(account.id, () =>
          this.transport.saveDraft({ ...draft, account_id: account.id }),
        ),
    };
  }

  async connectGithub(token: string): Promise<RemoteAccount> {
    const account = await this.transport.connectGithub(token);
    this.resetLocalView();
    await this.bridge?.wake();
    return account;
  }

  async connectGitlab(token: string): Promise<RemoteAccount> {
    const account = await this.transport.connectGitlab(token);
    this.resetLocalView();
    await this.bridge?.wake();
    return account;
  }

  /** Returns ephemeral account metadata only. Credentials never cross this boundary. */
  discoverGithubCli(signal?: AbortSignal): Promise<GithubCliDiscovery> {
    return this.fence.read(
      "$github-cli",
      () => this.transport.discoverGithubCli(),
      signal,
    );
  }

  async connectGithubCli(candidateId: string): Promise<RemoteAccount> {
    const account = await this.transport.connectGithubCli(candidateId);
    this.resetLocalView();
    await this.bridge?.wake();
    return account;
  }

  async disconnect(accountId: string): Promise<void> {
    // Clear rendered content before waiting on credential deletion/native work.
    this.clearAccount(accountId);
    try {
      await this.transport.disconnect(accountId);
    } finally {
      this.resetLocalView();
      await this.bridge?.wake();
    }
  }

  installBridge(queryClient: QueryClient): () => void {
    if (this.bridge) return () => {};
    this.queryClient = queryClient;
    const resourceTarget = (
      resource: NavigationResource,
    ): CapabilityTarget => ({
      kind: "resource",
      instance_id: resource.instanceId,
      repository_id: null,
      resource_id: resource.subjectId,
      resource_kind: resource.kind,
    });
    const bodyQuery = (resource: NavigationResource) => ({
      subject_id: resource.subjectId,
      facet: "body" as const,
      cursor: null,
      limit: 50,
    });
    const navigation = new NavigationWorkingSet(queryClient, {
      keys: (resource) => ({
        context: collaborationKeys.contextualCapabilities(
          resource.account,
          resourceTarget(resource),
        ),
        item: collaborationKeys.item(resource.account, resource.subjectId),
        body: collaborationKeys.detail(resource.account, {
          ...bodyQuery(resource),
          account_id: resource.account.id,
        }),
      }),
      read: (resource, projection, signal) => {
        const scoped = this.forAccount(resource.account);
        if (projection === "context")
          return scoped.contextualCapabilities(
            resourceTarget(resource),
            signal,
          );
        if (projection === "item")
          return scoped.item(resource.subjectId, signal);
        return scoped.detail(bodyQuery(resource), signal);
      },
      retain: (resource) =>
        this.retainDemand(resource.account, {
          kind: "detail",
          repository_id: null,
          subject_id: resource.subjectId,
          facet: "body",
        }),
      observeActivity: (listener) => this.demands.observeActivity(listener),
    });
    this.navigation = navigation;
    const stopDeadlines = installCapabilityDeadlines(queryClient);
    let disposed = false;
    let stopLocalChanges: (() => void) | undefined;
    void this.transport
      .listenLocalChanges(() => {
        if (!disposed) void this.invalidateLocalLinks();
      })
      .then((remove) => {
        if (disposed) remove();
        else stopLocalChanges = remove;
      })
      .catch(() => {});
    const bridge = new RevisionBridge<CollaborationChange>(
      {
        listen: this.transport.listen,
        catchUp: async (afterRevision) => {
          const page = await this.transport.changesSince(afterRevision ?? "0");
          return {
            changes: page.changes,
            fromExclusive: afterRevision,
            toInclusive: page.revision,
            authorizationView: page.authorization_view,
            reset: page.reset_required,
            hasMore: page.has_more,
          };
        },
      },
      async (batch) => {
        const authorizationChanged =
          this.authorizationView !== null &&
          batch.authorizationView !== this.authorizationView;
        if (batch.reset || authorizationChanged) this.resetLocalView();
        this.authorizationView = batch.authorizationView ?? null;
        this.authorizationRevision = batch.toInclusive;
        if (
          batch.changes.some(
            (change) =>
              change.reset ||
              change.scope === "account" ||
              change.scope === "repositories" ||
              change.scope === "local_transport_bindings" ||
              change.scope.startsWith("local_link:"),
          )
        )
          await this.invalidateLocalLinks();
        for (const change of batch.changes) {
          if (change.reset) this.clearAccount(change.account_id);
          else if (change.scope !== "drafts")
            navigation.invalidate(change.account_id);
          for (const listener of this.changeListeners) listener(change);
          // TanStack preserves an initial fetch with no cached data during
          // invalidation. Cancel affected provider reads first so a late snapshot
          // cannot erase the change and become fresh with staleTime: Infinity.
          const affectedQueries = {
            queryKey: collaborationKeys.account(change.account_id),
            predicate: (query: { queryKey: readonly unknown[] }) =>
              projectionAffected(query.queryKey, change.scope),
          };
          // Authored writes have their own generation/authorization fences.
          if (change.scope !== "drafts")
            await queryClient.cancelQueries(affectedQueries);
          void queryClient.invalidateQueries(affectedQueries);
        }
        if (
          batch.reset ||
          authorizationChanged ||
          batch.changes.some(
            (change) => change.reset || change.scope === "account",
          )
        ) {
          void queryClient.invalidateQueries({
            queryKey: ["collaboration", "accounts"],
          });
        }
        this.demands.ready();
      },
    );
    this.bridge = bridge;
    this.demands.attach();
    void bridge.start();
    return () => {
      disposed = true;
      stopLocalChanges?.();
      stopDeadlines();
      navigation.stop();
      if (this.navigation === navigation) this.navigation = null;
      this.demands.stop();
      bridge.stop();
      if (this.bridge === bridge) this.bridge = null;
      this.queryClient = null;
    };
  }

  wake = () => this.bridge?.wake() ?? Promise.resolve();

  private acceptSnapshot(snapshot: {
    authorization_view: string;
    revision: string;
  }) {
    if (this.authorizationView === null) {
      this.authorizationView = snapshot.authorization_view;
      this.authorizationRevision = snapshot.revision;
    } else if (this.authorizationView !== snapshot.authorization_view) {
      // Repair through the durable stream, never accept an obsolete private view.
      if (compareRevisions(snapshot.revision, this.authorizationRevision) >= 0)
        void this.wake();
      throw new StaleAuthorizationError();
    }
  }

  private clearAccount(accountId: string) {
    this.navigation?.clear(accountId);
    this.demands.clear(accountId);
    this.fence.invalidate(LOCAL_LINKS_SCOPE);
    void this.queryClient?.cancelQueries({
      queryKey: ["collaboration", "local-links"],
    });
    this.queryClient?.removeQueries({
      queryKey: ["collaboration", "local-links"],
    });
    this.fence.invalidate(accountId);
    void this.queryClient?.cancelQueries({
      queryKey: collaborationKeys.account(accountId),
    });
    this.queryClient?.removeQueries({
      queryKey: collaborationKeys.account(accountId),
    });
    this.publish();
  }

  private resetLocalView() {
    this.navigation?.clear();
    this.demands.clear();
    this.fence.invalidate();
    this.authorizationView = null;
    void this.queryClient?.cancelQueries({ queryKey: collaborationKeys.all });
    this.queryClient?.removeQueries({ queryKey: collaborationKeys.all });
    this.publish();
  }

  private publish() {
    this.version += 1;
    for (const listener of this.listeners) listener();
  }
}

function projectionAffected(key: readonly unknown[], scope: string) {
  const projection = key[4];
  if (projection === "capabilities" || projection === "resource")
    return scope !== "drafts";
  if (scope === "drafts") return projection === "draft";
  if (projection === "notification-subject")
    return (
      scope === "provider:rest" ||
      scope === "notifications" ||
      scope.startsWith("notification_subject:") ||
      scope === "repositories" ||
      scope.startsWith("repo:") ||
      scope.startsWith("detail:")
    );
  if (projection === "detail") {
    const query = key[5] as DetailQuery;
    return (
      scope === "repositories" ||
      scope === "notifications" ||
      scope.startsWith("notification_subject:") ||
      scope.startsWith("repo:") ||
      scope === `detail:${query.subject_id}:${query.facet}`
    );
  }
  if (scope === "repositories")
    return (
      projection === "repositories" ||
      projection === "items" ||
      projection === "item"
    );
  if (projection === "item") return true;
  if (projection !== "items") return false;
  const query = key[5] as ItemQuery;
  if (scope === "notifications") return query.kind === "notification";
  const kind = scope.endsWith(":pull_request")
    ? "pull_request"
    : scope.endsWith(":issue")
      ? "issue"
      : null;
  if (query.kind !== kind) return false;
  if (query.repository_id === null) return true;
  return scope === `repo:${query.repository_id}:${query.kind}`;
}

/** Errors from tokens/provider payloads are never echoed into rendered UI. */
export function collaborationErrorMessage(error: unknown): string {
  if (error instanceof StaleAuthorizationError)
    return "Account access changed. Reload this view.";
  const code =
    typeof error === "object" && error !== null && "code" in error
      ? error.code
      : undefined;
  switch (code) {
    case "auth_required":
      return "Your account needs to be reconnected.";
    case "credential_store_unavailable":
      return "Your system credential store is unavailable. Unlock it and try again.";
    case "permission_denied":
      return "This account does not have access to the requested data.";
    case "rate_limited":
      return "The provider asked Gitru to wait. Sync will resume automatically.";
    case "network":
      return "Could not reach the provider. Saved data is still available.";
    case "unsupported":
      return "This operation is unavailable for the connected account.";
    case "not_ready":
      return "Gitru is preparing your saved collaboration data. Try again shortly.";
    case "invalid_input":
      return "Check the supplied account or repository details and try again.";
    case "storage":
      return "Saved collaboration data could not be read. Try again.";
    case "stale_view":
      return "This saved view changed. Reload it before continuing.";
    default:
      return "The operation could not be completed. Try again.";
  }
}

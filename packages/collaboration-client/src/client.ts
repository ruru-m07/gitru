import {
  type AccountSnapshot,
  type CapabilitySnapshot,
  type CapabilityTarget,
  type ChangePage,
  type CollaborationChange,
  type CommandRecoveryActionRequest,
  type CommandRecoveryContext,
  type CommandRecoveryDetail,
  type CommandRecoveryQuery,
  type CommandRecoveryReceipt,
  type CommandRecoveryReplaceRequest,
  type CommandRecoverySnapshot,
  type CommentDraftPage,
  type CommentDraftQuery,
  type CommentDraftSnapshot,
  type CommentSendContext,
  type CommentSubmissionReceipt,
  type ConfirmLocalLinkPreview,
  type ContextCapabilityRequest,
  type ContextualCapabilitySnapshot,
  type CreatedCommentPage,
  type CreatedCommentQuery,
  type DemandTarget,
  type DetailQuery,
  type DetailSnapshot,
  type DiscoverNotificationSubjectRequest,
  type DraftPage,
  type DraftQuery,
  type ExecutePullCheckoutRequest,
  type GithubCliDiscovery,
  type HydrateDetailRequest,
  type InboxPage,
  type InboxQuery,
  type IssueDraftContext,
  type IssueDraftKey,
  type IssueDraftPage,
  type IssueDraftQuery,
  type IssueDraftSnapshot,
  type IssueSubmissionReceipt,
  type ItemPage,
  type ItemQuery,
  type ItemSnapshot,
  type LoadLocalPullFileRequest,
  type LocalCloneRequest,
  type LocalCloneSnapshot,
  type LocalDraft,
  type LocalInboxWriteReceipt,
  type LocalLinkInspection,
  type LocalLinkVersion,
  type LocalLinkWriteReceipt,
  type LocalNavigationReceipt,
  type LocalNavigationRequest,
  type LocalTransportBinding,
  type NotificationSubjectQuery,
  type NotificationSubjectSnapshot,
  type OpenLocalPullCommitReceipt,
  type OpenLocalPullCommitRequest,
  type PullCheckoutPlan,
  type PullCheckoutPlanRequest,
  type CollaborationPullCheckoutReceipt as PullCheckoutReceipt,
  type PullCommitQuery,
  type PullCommitSnapshot,
  type PullFileArtifactSnapshot,
  type PullFileDiffRequest,
  type PullFileQuery,
  type PullFileSnapshot,
  type RefreshReceipt,
  type RefreshRequest,
  type RemoteAccount,
  type RepositorySnapshot,
  type ResourceLocator,
  type ResourceResolution,
  type SaveCommentDraftRequest,
  type SaveIssueDraftRequest,
  type SendCommentRequest,
  type SetLocalInboxStateRequest,
  type SubmitIssueRequest,
  type SyncDiagnosticsExportReceipt,
  type SyncDiagnosticsSnapshot,
  type TextEditContext,
  type TextEditReceipt,
  type TextEditRequest,
  type TextEditSnapshot,
  type TransportBindingRequest,
  type WorkflowStateContext,
  type WorkflowStateReceipt,
  type WorkflowStateRequest,
  type WorkflowStateSnapshot,
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
import { compareRevisions, RevisionBridge } from "./revision-bridge";

export interface CollaborationTransport extends DemandTransport {
  commandRecoveryList(
    query: CommandRecoveryQuery,
  ): Promise<CommandRecoverySnapshot>;
  commandRecoveryDetail(
    accountId: string,
    commandId: string,
  ): Promise<CommandRecoveryDetail>;
  commandRecoveryAction(
    request: CommandRecoveryActionRequest,
  ): Promise<CommandRecoveryReceipt>;
  commandRecoveryReplace(
    request: CommandRecoveryReplaceRequest,
  ): Promise<CommandRecoveryReceipt>;
  commandRecoveryExport(context: CommandRecoveryContext): Promise<boolean>;
  textEditSnapshot(
    accountId: string,
    subjectId: string,
  ): Promise<TextEditSnapshot>;
  submitTextEdit(request: TextEditRequest): Promise<TextEditReceipt>;
  workflowStateSnapshot(
    accountId: string,
    subjectId: string,
  ): Promise<WorkflowStateSnapshot>;
  submitWorkflowState(
    request: WorkflowStateRequest,
  ): Promise<WorkflowStateReceipt>;
  commentDraft(
    accountId: string,
    subjectId: string,
  ): Promise<CommentDraftSnapshot>;
  commentDrafts(query: CommentDraftQuery): Promise<CommentDraftPage>;
  saveCommentDraft(
    request: SaveCommentDraftRequest,
  ): Promise<CommentDraftSnapshot>;
  sendComment(request: SendCommentRequest): Promise<CommentSubmissionReceipt>;
  createdComments(query: CreatedCommentQuery): Promise<CreatedCommentPage>;
  issueDraft(key: IssueDraftKey): Promise<IssueDraftSnapshot>;
  issueDrafts(query: IssueDraftQuery): Promise<IssueDraftPage>;
  saveIssueDraft(request: SaveIssueDraftRequest): Promise<IssueDraftSnapshot>;
  submitIssue(request: SubmitIssueRequest): Promise<IssueSubmissionReceipt>;
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
  listenRuntimeReset(onReset: () => void): Promise<() => void>;
  accounts(): Promise<AccountSnapshot>;
  diagnostics(): Promise<SyncDiagnosticsSnapshot>;
  exportDiagnostics(): Promise<SyncDiagnosticsExportReceipt>;
  connectGithub(token: string): Promise<RemoteAccount>;
  connectGitlab(token: string): Promise<RemoteAccount>;
  connectBitbucketCloud(token: string): Promise<RemoteAccount>;
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
  inbox(query: InboxQuery): Promise<InboxPage>;
  setLocalInboxState(
    request: SetLocalInboxStateRequest,
  ): Promise<LocalInboxWriteReceipt>;
  item(accountId: string, itemId: string): Promise<ItemSnapshot>;
  refresh(request: RefreshRequest): Promise<RefreshReceipt>;
  changesSince(afterRevision: string): Promise<ChangePage>;
  saveDraft(draft: LocalDraft): Promise<LocalDraft>;
  draft(accountId: string, subjectId: string): Promise<LocalDraft | null>;
  drafts(query: DraftQuery): Promise<DraftPage>;
  exportDraft(
    accountId: string,
    subjectId: string,
    generation: string,
  ): Promise<boolean>;
  capabilities(accountId: string): Promise<CapabilitySnapshot>;
  contextualCapabilities(
    request: ContextCapabilityRequest,
  ): Promise<ContextualCapabilitySnapshot>;
  resolveResource(
    accountId: string,
    locator: ResourceLocator,
  ): Promise<ResourceResolution>;
  detail(query: DetailQuery): Promise<DetailSnapshot>;
  pullCommits(query: PullCommitQuery): Promise<PullCommitSnapshot>;
  pullFiles(query: PullFileQuery): Promise<PullFileSnapshot>;
  pullFileArtifact(
    request: PullFileDiffRequest,
  ): Promise<PullFileArtifactSnapshot>;
  hydratePullFile(request: PullFileDiffRequest): Promise<RefreshReceipt>;
  loadLocalPullFile(
    request: LoadLocalPullFileRequest,
  ): Promise<PullFileArtifactSnapshot>;
  hydrateDetail(request: HydrateDetailRequest): Promise<RefreshReceipt>;
  notificationSubject(
    query: NotificationSubjectQuery,
  ): Promise<NotificationSubjectSnapshot>;
  discoverNotificationSubject(
    request: DiscoverNotificationSubjectRequest,
  ): Promise<RefreshReceipt>;
  planPullCheckout(request: PullCheckoutPlanRequest): Promise<PullCheckoutPlan>;
  executePullCheckout(
    request: ExecutePullCheckoutRequest,
  ): Promise<PullCheckoutReceipt>;
  openLocalPullCommit(
    request: OpenLocalPullCommitRequest,
  ): Promise<OpenLocalPullCommitReceipt>;
  listen(onWake: () => void): Promise<() => void>;
}

export const collaborationKeys = {
  commandRecovery: (account: RemoteAccount, query: CommandRecoveryQuery) =>
    [
      ...collaborationKeys.account(account.id),
      account.authorization_epoch,
      "command-recovery",
      query,
    ] as const,
  commandRecoveryDetail: (account: RemoteAccount, commandId: string) =>
    [
      ...collaborationKeys.account(account.id),
      account.authorization_epoch,
      "command-recovery-detail",
      commandId,
    ] as const,
  textEdit: (account: RemoteAccount, subjectId: string) =>
    [
      ...collaborationKeys.account(account.id),
      account.authorization_epoch,
      "text-edit",
      subjectId,
    ] as const,
  workflowState: (account: RemoteAccount, subjectId: string) =>
    [
      ...collaborationKeys.account(account.id),
      account.authorization_epoch,
      "workflow-state",
      subjectId,
    ] as const,
  commentDraft: (account: RemoteAccount, subjectId: string) =>
    [
      ...collaborationKeys.account(account.id),
      "local",
      "comment-draft",
      subjectId,
    ] as const,
  commentDrafts: (account: RemoteAccount, query: CommentDraftQuery) =>
    [
      ...collaborationKeys.account(account.id),
      "local",
      "comment-drafts",
      query,
    ] as const,
  createdComments: (account: RemoteAccount, query: CreatedCommentQuery) =>
    [
      ...collaborationKeys.account(account.id),
      account.authorization_epoch,
      "created-comments",
      query,
    ] as const,
  issueDraft: (account: RemoteAccount, key: IssueDraftKey) =>
    [
      ...collaborationKeys.account(account.id),
      "local",
      "issue-draft",
      key.draft_id,
      key.repository_id,
    ] as const,
  issueDrafts: (account: RemoteAccount, query: IssueDraftQuery) =>
    [
      ...collaborationKeys.account(account.id),
      "local",
      "issue-drafts",
      query,
    ] as const,
  all: ["collaboration"] as const,
  localLinks: (localRepositoryId: string, version: number) =>
    ["collaboration", "local-links", localRepositoryId, version] as const,
  localClones: (
    account: RemoteAccount,
    instanceId: string,
    repositoryId: string,
    sourceRepositoryProviderId: string | null = null,
  ) =>
    [
      ...collaborationKeys.account(account.id),
      account.authorization_epoch,
      "local-clones",
      account.actor_id,
      instanceId,
      repositoryId,
      sourceRepositoryProviderId,
    ] as const,
  accounts: (version: number) =>
    ["collaboration", "accounts", version] as const,
  diagnostics: (version: number) =>
    ["collaboration", "diagnostics", version] as const,
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
      "local",
      "draft",
      subjectId,
    ] as const,
  drafts: (account: RemoteAccount, query: DraftQuery) =>
    [
      ...collaborationKeys.account(account.id),
      "local",
      "drafts",
      query,
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
  pullCommits: (account: RemoteAccount, query: PullCommitQuery) =>
    [
      ...collaborationKeys.account(account.id),
      account.authorization_epoch,
      "pull-commits",
      query,
    ] as const,
  pullFiles: (account: RemoteAccount, query: PullFileQuery) =>
    [
      ...collaborationKeys.account(account.id),
      account.authorization_epoch,
      "pull-files",
      query,
    ] as const,
  pullFileArtifact: (account: RemoteAccount, request: PullFileDiffRequest) =>
    [
      ...collaborationKeys.account(account.id),
      account.authorization_epoch,
      "pull-file-artifact",
      request,
    ] as const,
  notificationSubject: (account: RemoteAccount, notificationId: string) =>
    [
      ...collaborationKeys.account(account.id),
      account.authorization_epoch,
      "notification-subject",
      account.actor_id,
      notificationId,
    ] as const,
  inbox: (account: RemoteAccount, query: InboxQuery) =>
    [
      ...collaborationKeys.account(account.id),
      account.authorization_epoch,
      "inbox",
      query,
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

  constructor(readonly transport: CollaborationTransport) {
    this.demands = new DemandCoordinator(transport);
  }

  /** Ephemeral view interest; it never reads or hydrates provider data itself. */
  retainDemand(account: DemandAccount, target: DemandTarget) {
    return this.demands.retain(account, target);
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

  /** Native cache/scheduler observation only; this never admits provider work. */
  diagnostics(): Promise<SyncDiagnosticsSnapshot> {
    return this.transport.diagnostics();
  }

  exportDiagnostics(): Promise<SyncDiagnosticsExportReceipt> {
    return this.transport.exportDiagnostics();
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
  async invalidateLocalLinks(isCurrent: () => boolean = () => true) {
    this.fence.invalidate(LOCAL_LINKS_SCOPE);
    if (!this.queryClient) return;
    const affected = {
      predicate: (query: { queryKey: readonly unknown[] }) =>
        query.queryKey[0] === "collaboration" &&
        (query.queryKey[1] === "local-links" ||
          query.queryKey[4] === "local-clones"),
    };
    await this.queryClient.cancelQueries(affected);
    if (!isCurrent()) return;
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
    const reviewedContext = (context: CommandRecoveryContext) => {
      if (
        context.account_id !== account.id ||
        context.expected_epoch !== account.authorization_epoch
      )
        throw new StaleAuthorizationError();
      return { ...context };
    };
    const reviewedTextEditContext = (context: TextEditContext) => {
      if (
        context.account_id !== account.id ||
        context.authorization_epoch !== account.authorization_epoch
      )
        throw new StaleAuthorizationError();
      return { ...context };
    };
    const reviewedWorkflowStateContext = (context: WorkflowStateContext) => {
      if (
        context.account_id !== account.id ||
        context.authorization_epoch !== account.authorization_epoch
      )
        throw new StaleAuthorizationError();
      return { ...context };
    };
    const reviewedCommentSendContext = (context: CommentSendContext) => {
      if (
        context.account_id !== account.id ||
        context.authorization_epoch !== account.authorization_epoch
      )
        throw new StaleAuthorizationError();
      return { ...context };
    };
    const reviewedIssueDraftContext = (context: IssueDraftContext) => {
      if (
        context.account_id !== account.id ||
        context.authorization_epoch !== account.authorization_epoch
      )
        throw new StaleAuthorizationError();
      return { ...context };
    };
    return {
      commandRecoveryList: (
        query: Omit<CommandRecoveryQuery, "account_id">,
        signal?: AbortSignal,
      ) =>
        read(
          () =>
            this.transport.commandRecoveryList({
              ...query,
              account_id: account.id,
            }),
          signal,
        ),
      commandRecoveryDetail: async (
        commandId: string,
        signal?: AbortSignal,
      ) => {
        const detail = await this.fence.read(
          account.id,
          () => this.transport.commandRecoveryDetail(account.id, commandId),
          signal,
        );
        if (
          detail.context.account_id !== account.id ||
          detail.command.account_id !== account.id ||
          detail.command.command_id !== commandId ||
          detail.context.command_id !== commandId
        )
          throw new StaleAuthorizationError();
        this.acceptSnapshot({
          revision: detail.revision,
          authorization_view: detail.context.authorization_view,
        });
        return detail;
      },
      commandRecoveryAction: (request: CommandRecoveryActionRequest) => {
        const context = reviewedContext(request.context);
        return this.fence.read(account.id, () =>
          this.transport.commandRecoveryAction({ ...request, context }),
        );
      },
      commandRecoveryReplace: (request: CommandRecoveryReplaceRequest) => {
        const context = reviewedContext(request.context);
        const fields = request.fields.map((field) => ({ ...field }));
        return this.fence.read(account.id, () =>
          this.transport.commandRecoveryReplace({
            ...request,
            context,
            fields,
          }),
        );
      },
      commandRecoveryExport: (context: CommandRecoveryContext) => {
        const reviewed = reviewedContext(context);
        return this.fence.read(account.id, () =>
          this.transport.commandRecoveryExport(reviewed),
        );
      },
      textEditSnapshot: async (subjectId: string, signal?: AbortSignal) => {
        const snapshot = await this.fence.read(
          account.id,
          () => this.transport.textEditSnapshot(account.id, subjectId),
          signal,
        );
        if (
          snapshot.context !== null &&
          (snapshot.context.account_id !== account.id ||
            snapshot.context.subject_id !== subjectId ||
            snapshot.context.authorization_epoch !==
              account.authorization_epoch ||
            snapshot.context.authorization_view !== snapshot.authorization_view)
        )
          throw new StaleAuthorizationError();
        this.acceptSnapshot(snapshot);
        return snapshot;
      },
      submitTextEdit: async (request: TextEditRequest) => {
        const context = reviewedTextEditContext(request.context);
        const receipt = await this.fence.read(account.id, () =>
          this.transport.submitTextEdit({ ...request, context }),
        );
        if (
          receipt.account_id !== account.id ||
          receipt.command_id !== request.command_id
        )
          throw new StaleAuthorizationError();
        return receipt;
      },
      workflowStateSnapshot: async (
        subjectId: string,
        signal?: AbortSignal,
      ) => {
        const snapshot = await this.fence.read(
          account.id,
          () => this.transport.workflowStateSnapshot(account.id, subjectId),
          signal,
        );
        if (
          snapshot.context !== null &&
          (snapshot.context.account_id !== account.id ||
            snapshot.context.subject_id !== subjectId ||
            snapshot.context.authorization_epoch !==
              account.authorization_epoch ||
            snapshot.context.authorization_view !== snapshot.authorization_view)
        )
          throw new StaleAuthorizationError();
        this.acceptSnapshot(snapshot);
        return snapshot;
      },
      submitWorkflowState: async (request: WorkflowStateRequest) => {
        const context = reviewedWorkflowStateContext(request.context);
        const receipt = await this.fence.read(account.id, () =>
          this.transport.submitWorkflowState({ ...request, context }),
        );
        if (
          receipt.account_id !== account.id ||
          receipt.command_id !== request.command_id
        )
          throw new StaleAuthorizationError();
        return receipt;
      },
      commentDraft: async (subjectId: string, signal?: AbortSignal) => {
        const snapshot = await this.fence.read(
          account.id,
          () => this.transport.commentDraft(account.id, subjectId),
          signal,
        );
        if (
          snapshot.account_id !== account.id ||
          snapshot.subject_id !== subjectId ||
          (snapshot.context !== null &&
            (snapshot.context.account_id !== account.id ||
              snapshot.context.subject_id !== subjectId ||
              snapshot.context.authorization_epoch !==
                account.authorization_epoch ||
              snapshot.context.authorization_view !==
                snapshot.authorization_view))
        )
          throw new StaleAuthorizationError();
        this.acceptSnapshot(snapshot);
        return snapshot;
      },
      commentDrafts: async (
        query: Omit<CommentDraftQuery, "account_id">,
        signal?: AbortSignal,
      ) => {
        const page = await this.fence.read(
          account.id,
          () =>
            this.transport.commentDrafts({
              ...query,
              account_id: account.id,
            }),
          signal,
        );
        if (page.account_id !== account.id) throw new StaleAuthorizationError();
        this.acceptSnapshot(page);
        return page;
      },
      saveCommentDraft: async (
        request: Omit<
          SaveCommentDraftRequest,
          "account_id" | "authorization_epoch"
        >,
      ) => {
        const snapshot = await this.fence.read(account.id, () =>
          this.transport.saveCommentDraft({
            ...request,
            account_id: account.id,
            authorization_epoch: account.authorization_epoch,
          }),
        );
        if (
          snapshot.account_id !== account.id ||
          snapshot.subject_id !== request.subject_id ||
          (snapshot.context !== null &&
            (snapshot.context.account_id !== account.id ||
              snapshot.context.subject_id !== request.subject_id ||
              snapshot.context.authorization_epoch !==
                account.authorization_epoch ||
              snapshot.context.authorization_view !==
                snapshot.authorization_view))
        )
          throw new StaleAuthorizationError();
        this.acceptSnapshot(snapshot);
        return snapshot;
      },
      sendComment: async (request: SendCommentRequest) => {
        const context = reviewedCommentSendContext(request.context);
        const receipt = await this.fence.read(account.id, () =>
          this.transport.sendComment({ ...request, context }),
        );
        if (
          receipt.account_id !== account.id ||
          receipt.command_id !== request.command_id
        )
          throw new StaleAuthorizationError();
        return receipt;
      },
      createdComments: async (
        query: Omit<CreatedCommentQuery, "account_id">,
        signal?: AbortSignal,
      ) => {
        const page = await this.fence.read(
          account.id,
          () =>
            this.transport.createdComments({
              ...query,
              account_id: account.id,
            }),
          signal,
        );
        if (
          page.account_id !== account.id ||
          page.subject_id !== query.subject_id
        )
          throw new StaleAuthorizationError();
        this.acceptSnapshot(page);
        return page;
      },
      issueDraft: async (
        key: Omit<IssueDraftKey, "account_id">,
        signal?: AbortSignal,
      ) => {
        const fullKey = { ...key, account_id: account.id };
        const snapshot = await this.fence.read(
          account.id,
          () => this.transport.issueDraft(fullKey),
          signal,
        );
        if (
          snapshot.account_id !== account.id ||
          snapshot.draft_id !== key.draft_id ||
          snapshot.repository_id !== key.repository_id ||
          (snapshot.context !== null &&
            (snapshot.context.account_id !== account.id ||
              snapshot.context.repository_id !== key.repository_id ||
              snapshot.context.authorization_epoch !==
                account.authorization_epoch ||
              snapshot.context.authorization_view !==
                snapshot.authorization_view))
        )
          throw new StaleAuthorizationError();
        this.acceptSnapshot(snapshot);
        return snapshot;
      },
      issueDrafts: async (
        query: Omit<IssueDraftQuery, "account_id">,
        signal?: AbortSignal,
      ) => {
        const page = await this.fence.read(
          account.id,
          () =>
            this.transport.issueDrafts({ ...query, account_id: account.id }),
          signal,
        );
        if (page.account_id !== account.id) throw new StaleAuthorizationError();
        this.acceptSnapshot(page);
        return page;
      },
      saveIssueDraft: async (
        request: Omit<
          SaveIssueDraftRequest,
          "account_id" | "authorization_epoch"
        >,
      ) => {
        const snapshot = await this.fence.read(account.id, () =>
          this.transport.saveIssueDraft({
            ...request,
            account_id: account.id,
            authorization_epoch: account.authorization_epoch,
          }),
        );
        if (
          snapshot.account_id !== account.id ||
          snapshot.draft_id !== request.draft_id ||
          snapshot.repository_id !== request.repository_id ||
          (snapshot.context !== null &&
            (snapshot.context.account_id !== account.id ||
              snapshot.context.repository_id !== request.repository_id ||
              snapshot.context.authorization_epoch !==
                account.authorization_epoch ||
              snapshot.context.authorization_view !==
                snapshot.authorization_view))
        )
          throw new StaleAuthorizationError();
        this.acceptSnapshot(snapshot);
        return snapshot;
      },
      submitIssue: async (request: SubmitIssueRequest) => {
        const context = reviewedIssueDraftContext(request.context);
        const receipt = await this.fence.read(account.id, () =>
          this.transport.submitIssue({ ...request, context }),
        );
        if (
          receipt.account_id !== account.id ||
          receipt.command_id !== request.command_id
        )
          throw new StaleAuthorizationError();
        return receipt;
      },
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
        sourceRepositoryProviderId: string | null = null,
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
              source_repository_provider_id: sourceRepositoryProviderId,
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
      inbox: (query: Omit<InboxQuery, "account_id">, signal?: AbortSignal) =>
        read(
          () => this.transport.inbox({ ...query, account_id: account.id }),
          signal,
        ),
      setLocalInboxState: (
        request: Omit<
          SetLocalInboxStateRequest,
          "account_id" | "authorization_epoch"
        >,
      ) =>
        this.fence.read(account.id, () =>
          this.transport.setLocalInboxState({
            ...request,
            account_id: account.id,
            authorization_epoch: account.authorization_epoch,
          }),
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
      pullCommits: (
        query: Omit<PullCommitQuery, "account_id">,
        signal?: AbortSignal,
      ) =>
        read(
          () =>
            this.transport.pullCommits({ ...query, account_id: account.id }),
          signal,
        ),
      pullFiles: (
        query: Omit<PullFileQuery, "account_id">,
        signal?: AbortSignal,
      ) =>
        read(
          () => this.transport.pullFiles({ ...query, account_id: account.id }),
          signal,
        ),
      pullFileArtifact: (
        request: Omit<
          PullFileDiffRequest,
          "account_id" | "authorization_epoch"
        >,
        signal?: AbortSignal,
      ) =>
        read(
          () =>
            this.transport.pullFileArtifact({
              ...request,
              account_id: account.id,
              authorization_epoch: account.authorization_epoch,
            }),
          signal,
        ),
      hydratePullFile: (
        request: Omit<
          PullFileDiffRequest,
          "account_id" | "authorization_epoch"
        >,
      ) =>
        this.fence.read(account.id, () =>
          this.transport.hydratePullFile({
            ...request,
            account_id: account.id,
            authorization_epoch: account.authorization_epoch,
          }),
        ),
      loadLocalPullFile: (
        request: Omit<
          LoadLocalPullFileRequest,
          "account_id" | "authorization_epoch"
        >,
      ) =>
        read(() =>
          this.transport.loadLocalPullFile({
            ...request,
            account_id: account.id,
            authorization_epoch: account.authorization_epoch,
          }),
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
      planPullCheckout: (
        request: Omit<
          PullCheckoutPlanRequest,
          "account_id" | "authorization_epoch"
        >,
      ) => {
        const {
          instance_id,
          subject_id,
          local_repository_id,
          link_id,
          link_generation,
          local_branch,
        } = request;
        return this.fence.read(cloneAccount.id, () =>
          this.transport.planPullCheckout({
            account_id: cloneAccount.id,
            authorization_epoch: cloneAccount.authorization_epoch,
            instance_id,
            subject_id,
            local_repository_id,
            link_id,
            link_generation,
            ...(local_branch === undefined ? {} : { local_branch }),
          }),
        );
      },
      // Native execution owns the final caller/epoch/link admission gates. A
      // returned receipt is authoritative for the local Git mutation, even if
      // the client fence invalidates while the IPC request is in flight.
      executePullCheckout: (planId: string) =>
        this.transport.executePullCheckout({ plan_id: planId }),
      openLocalPullCommit: (
        request: Omit<
          OpenLocalPullCommitRequest,
          "account_id" | "authorization_epoch"
        >,
      ) =>
        this.fence.read(cloneAccount.id, () =>
          this.transport.openLocalPullCommit({
            ...request,
            account_id: cloneAccount.id,
            authorization_epoch: cloneAccount.authorization_epoch,
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
      drafts: (query: Omit<DraftQuery, "account_id">, signal?: AbortSignal) =>
        this.fence.read(
          account.id,
          () => this.transport.drafts({ ...query, account_id: account.id }),
          signal,
        ),
      exportDraft: (subjectId: string, generation: string) =>
        this.fence.read(account.id, () =>
          this.transport.exportDraft(account.id, subjectId, generation),
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

  async connectBitbucketCloud(token: string): Promise<RemoteAccount> {
    const account = await this.transport.connectBitbucketCloud(token);
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
    const stopDeadlines = installCapabilityDeadlines(queryClient);
    let disposed = false;
    let stopLocalChanges: (() => void) | undefined;
    let stopRuntimeReset: (() => void) | undefined;
    void this.transport
      .listenRuntimeReset(() => {
        if (disposed) return;
        // Recovery replaces native runtime ownership independently of provider
        // revisions. Fence in-flight snapshots immediately, including a cancel
        // that reopens the same database revision under a fresh native owner.
        this.resetLocalView();
        void this.bridge?.restart();
      })
      .then((remove) => {
        if (disposed) remove();
        else stopRuntimeReset = remove;
      })
      .catch(() => {});
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
      async (batch, isCurrent) => {
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
          await this.invalidateLocalLinks(isCurrent);
        if (!isCurrent()) return;
        for (const change of batch.changes) {
          if (change.reset) this.clearAccount(change.account_id);
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
          if (
            change.scope !== "drafts" &&
            !change.scope.startsWith("comment_draft:") &&
            !change.scope.startsWith("issue_draft:")
          )
            await queryClient.cancelQueries(affectedQueries);
          if (!isCurrent()) return;
          if (currentHeadContextChanged(change.scope)) {
            // Repository membership and Body observations can replace or omit
            // the exact base/head/source range. Reset matching commit and check
            // projections before their active refetch so React cannot keep
            // rendering a superseded generation or prior-head green state.
            // resetQueries clears cached data synchronously, then returns the
            // active refetch. Do not hold the revision bridge on provider-local
            // IPC; later change batches must remain able to fence that refetch.
            void queryClient.resetQueries({
              queryKey: collaborationKeys.account(change.account_id),
              predicate: (query: { queryKey: readonly unknown[] }) =>
                currentHeadProjectionAffected(query.queryKey, change.scope) ||
                ((query.queryKey[4] === "pull-commits" ||
                  query.queryKey[4] === "pull-files" ||
                  query.queryKey[4] === "pull-file-artifact") &&
                  projectionAffected(query.queryKey, change.scope)),
            });
          }
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
      stopRuntimeReset?.();
      stopDeadlines();
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
    this.demands.clear(accountId);
    this.fence.invalidate(LOCAL_LINKS_SCOPE);
    void this.queryClient?.cancelQueries({
      queryKey: ["collaboration", "local-links"],
    });
    this.queryClient?.removeQueries({
      queryKey: ["collaboration", "local-links"],
    });
    this.fence.invalidate(accountId);
    this.redactIssueDraftAuthority(collaborationKeys.account(accountId));
    this.redactCommentDraftAuthority(collaborationKeys.account(accountId));
    this.refreshAuthoredDrafts(collaborationKeys.account(accountId));
    this.queryClient?.removeQueries({
      queryKey: collaborationKeys.account(accountId),
      predicate: (query) => !isAuthoredDraft(query.queryKey),
    });
    this.publish();
  }

  private resetLocalView() {
    this.demands.clear();
    this.fence.invalidate();
    this.authorizationView = null;
    this.redactIssueDraftAuthority(collaborationKeys.all);
    this.redactCommentDraftAuthority(collaborationKeys.all);
    // Authored text belongs to the local actor partition, independent of a
    // provider grant. Keep an open editor intact while clearing remote data.
    this.refreshAuthoredDrafts(collaborationKeys.all);
    this.queryClient?.removeQueries({
      queryKey: collaborationKeys.all,
      predicate: (query) => !isAuthoredDraft(query.queryKey),
    });
    this.publish();
  }

  private redactIssueDraftAuthority(queryKey: readonly unknown[]) {
    const cache = this.queryClient;
    if (!cache) return;
    for (const query of cache.getQueryCache().findAll({ queryKey })) {
      if (query.queryKey[4] !== "issue-draft") continue;
      cache.setQueryData<IssueDraftSnapshot>(query.queryKey, (snapshot) => {
        if (!snapshot) return snapshot;
        return {
          ...snapshot,
          context: null,
          availability: "unavailable",
          reason: "account_unavailable",
          published: null,
        };
      });
    }
  }

  private redactCommentDraftAuthority(queryKey: readonly unknown[]) {
    const cache = this.queryClient;
    if (!cache) return;
    for (const query of cache.getQueryCache().findAll({ queryKey })) {
      if (query.queryKey[4] !== "comment-draft") continue;
      cache.setQueryData<CommentDraftSnapshot>(query.queryKey, (snapshot) => {
        if (!snapshot) return snapshot;
        return {
          ...snapshot,
          context: null,
          availability: "unavailable",
          reason: "account_unavailable",
        };
      });
    }
  }

  private refreshAuthoredDrafts(queryKey: readonly unknown[]) {
    const cache = this.queryClient;
    if (!cache) return;
    // Cancellation retains committed query data, so an open editor survives.
    // Re-read authored projections after a stream reset; a lost draft hint must
    // not leave their otherwise-infinite local cache stale.
    void cache.cancelQueries({ queryKey });
    void cache.invalidateQueries({
      queryKey,
      predicate: (query) => isAuthoredDraft(query.queryKey),
    });
  }

  private publish() {
    this.version += 1;
    for (const listener of this.listeners) listener();
  }
}

function isAuthoredDraft(key: readonly unknown[]) {
  return (
    key[1] === "account" &&
    key[3] === "local" &&
    (key[4] === "draft" ||
      key[4] === "drafts" ||
      key[4] === "comment-draft" ||
      key[4] === "comment-drafts" ||
      key[4] === "issue-draft" ||
      key[4] === "issue-drafts")
  );
}

function projectionAffected(key: readonly unknown[], scope: string) {
  const projection = key[4];
  if (
    projection === "command-recovery" ||
    projection === "command-recovery-detail"
  )
    return scope !== "drafts";
  if (scope.startsWith("effective:")) {
    const subject = scope.slice("effective:".length);
    if (projection === "text-edit") return key[5] === subject;
    if (projection === "workflow-state") return key[5] === subject;
    if (projection === "detail") {
      const query = key[5] as DetailQuery;
      return query.subject_id === subject && query.facet === "body";
    }
    // Item effects alter membership, counts and search in every saved list for
    // this account. Identity, capability and head-bound facets remain provider data.
    return (
      projection === "item" || projection === "items" || projection === "inbox"
    );
  }
  if (projection === "comment-draft") {
    const subject = key[5];
    return scope === "commands" || scope === `comment_draft:${subject}`;
  }
  if (projection === "comment-drafts")
    return scope.startsWith("comment_draft:");
  if (projection === "created-comments") {
    const query = key[5] as CreatedCommentQuery;
    return scope === `created_comments:${query.subject_id}`;
  }
  if (projection === "issue-draft") {
    const draftId = key[5];
    return scope === "commands" || scope === `issue_draft:${draftId}`;
  }
  if (projection === "issue-drafts")
    return scope === "commands" || scope.startsWith("issue_draft:");
  if (projection === "text-edit") {
    const subject = key[5];
    return (
      scope === "repositories" ||
      scope.startsWith("repo:") ||
      scope === `detail:${subject}:body`
    );
  }
  if (projection === "workflow-state") {
    const subject = key[5];
    return (
      scope === "commands" ||
      scope === "repositories" ||
      scope.startsWith("repo:") ||
      scope === `effective:${subject}` ||
      scope === `detail:${subject}:body`
    );
  }
  if (projection === "capabilities" || projection === "resource")
    return scope !== "drafts";
  if (scope === "drafts")
    return projection === "draft" || projection === "drafts";
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
  if (projection === "pull-commits") {
    const query = key[5] as PullCommitQuery;
    return (
      scope === "repositories" ||
      scope.startsWith("repo:") ||
      scope === `detail:${query.subject_id}:body` ||
      scope === `detail:${query.subject_id}:commits`
    );
  }
  if (projection === "pull-files" || projection === "pull-file-artifact") {
    const request = key[5] as PullFileQuery | PullFileDiffRequest;
    return (
      scope === "repositories" ||
      scope.startsWith("repo:") ||
      scope === `detail:${request.subject_id}:body` ||
      scope === `detail:${request.subject_id}:files`
    );
  }
  if (projection === "inbox")
    return scope === "notifications" || scope.startsWith("local_inbox:");
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

function currentHeadContextChanged(scope: string) {
  return (
    scope === "repositories" ||
    scope.startsWith("repo:") ||
    (scope.startsWith("detail:") && scope.endsWith(":body"))
  );
}

function currentHeadProjectionAffected(key: readonly unknown[], scope: string) {
  if (key[4] === "pull-commits") return projectionAffected(key, scope);
  if (key[4] !== "detail") return false;
  const query = key[5] as DetailQuery;
  return (
    query.facet === "checks" &&
    (scope === "repositories" ||
      scope.startsWith("repo:") ||
      scope === `detail:${query.subject_id}:body`)
  );
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
    case "local_state_changed":
      return "The local repository changed during the operation. Inspect it before retrying.";
    default:
      return "The operation could not be completed. Try again.";
  }
}

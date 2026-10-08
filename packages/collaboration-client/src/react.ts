import type {
  CapabilityTarget,
  CommandRecoveryQuery,
  CommentDraftPage,
  CommentDraftQuery,
  CommentDraftSnapshot,
  CreatedCommentQuery,
  DemandTarget,
  DetailQuery,
  DraftQuery,
  InboxQuery,
  IssueDraftKey,
  IssueDraftPage,
  IssueDraftQuery,
  IssueDraftSnapshot,
  ItemQuery,
  PullCommitQuery,
  PullDraftKey,
  PullDraftPage,
  PullDraftQuery,
  PullDraftSnapshot,
  PullFileDiffRequest,
  PullFileQuery,
  RemoteAccount,
  ResourceLocator,
  TextEditSnapshot,
  WorkflowStateSnapshot,
} from "@gitru/commands";
import { queryOptions, useQuery } from "@tanstack/react-query";
import { useEffect, useState, useSyncExternalStore } from "react";
import { collaboration, collaborationKeys } from "./index";

const localQueryPolicy = {
  networkMode: "always" as const,
  refetchInterval: false as const,
  refetchOnWindowFocus: false as const,
  refetchOnReconnect: false as const,
  staleTime: Infinity,
  gcTime: 5 * 60_000,
  retry: false as const,
  meta: { collaboration: true },
};

export function providerInboxActionsQueryOptions(
  account: RemoteAccount,
  subjectId: string,
) {
  return queryOptions({
    ...localQueryPolicy,
    queryKey: collaborationKeys.providerInboxActions(account, subjectId),
    queryFn: ({ signal }) =>
      collaboration.forAccount(account).providerInboxActions(subjectId, signal),
  });
}

export function commandRecoveryQueryOptions(
  account: RemoteAccount,
  query: Omit<CommandRecoveryQuery, "account_id">,
) {
  return queryOptions({
    ...localQueryPolicy,
    queryKey: collaborationKeys.commandRecovery(account, {
      ...query,
      account_id: account.id,
    }),
    queryFn: ({ signal }) =>
      collaboration.forAccount(account).commandRecoveryList(query, signal),
  });
}

export function commandRecoveryDetailQueryOptions(
  account: RemoteAccount,
  commandId: string,
) {
  return queryOptions({
    ...localQueryPolicy,
    queryKey: collaborationKeys.commandRecoveryDetail(account, commandId),
    queryFn: ({ signal }) =>
      collaboration
        .forAccount(account)
        .commandRecoveryDetail(commandId, signal),
  });
}

/** Cache-only edit base. Provider reads and delivery remain native-owned. */
export function textEditQueryOptions(
  account: RemoteAccount,
  subjectId: string,
) {
  return queryOptions<TextEditSnapshot>({
    ...localQueryPolicy,
    queryKey: collaborationKeys.textEdit(account, subjectId),
    queryFn: ({ signal }) =>
      collaboration.forAccount(account).textEditSnapshot(subjectId, signal),
  });
}

/** Local merge receipt/status only; provider preview is explicitly requested. */
export function guardedMergeQueryOptions(
  account: RemoteAccount,
  subjectId: string,
) {
  return queryOptions({
    ...localQueryPolicy,
    queryKey: collaborationKeys.guardedMerge(account, subjectId),
    queryFn: ({ signal }) =>
      collaboration.forAccount(account).guardedMergeSnapshot(subjectId, signal),
  });
}

/** Cache-only workflow state. Provider reads and delivery remain native-owned. */
export function workflowStateQueryOptions(
  account: RemoteAccount,
  subjectId: string,
) {
  return queryOptions<WorkflowStateSnapshot>({
    ...localQueryPolicy,
    queryKey: collaborationKeys.workflowState(account, subjectId),
    queryFn: ({ signal }) =>
      collaboration
        .forAccount(account)
        .workflowStateSnapshot(subjectId, signal),
  });
}

/** Dedicated local authored comment draft; opening it never starts provider I/O. */
export function commentDraftQueryOptions(
  account: RemoteAccount,
  subjectId: string,
) {
  return queryOptions<CommentDraftSnapshot>({
    ...localQueryPolicy,
    queryKey: collaborationKeys.commentDraft(account, subjectId),
    queryFn: ({ signal }) =>
      collaboration.forAccount(account).commentDraft(subjectId, signal),
  });
}

/** Local recovery index for dedicated comment drafts, including missing subjects. */
export function commentDraftsQueryOptions(
  account: RemoteAccount,
  query: Omit<CommentDraftQuery, "account_id">,
) {
  return queryOptions<CommentDraftPage>({
    ...localQueryPolicy,
    queryKey: collaborationKeys.commentDrafts(account, {
      ...query,
      account_id: account.id,
    }),
    queryFn: ({ signal }) =>
      collaboration.forAccount(account).commentDrafts(query, signal),
  });
}

/** Validated local creation receipts, separate from provider comment coverage. */
export function createdCommentsQueryOptions(
  account: RemoteAccount,
  query: Omit<CreatedCommentQuery, "account_id">,
) {
  return queryOptions({
    ...localQueryPolicy,
    queryKey: collaborationKeys.createdComments(account, {
      ...query,
      account_id: account.id,
    }),
    queryFn: ({ signal }) =>
      collaboration.forAccount(account).createdComments(query, signal),
  });
}

/** Local authored issue draft plus cached native submission authority. */
export function issueDraftQueryOptions(
  account: RemoteAccount,
  key: Omit<IssueDraftKey, "account_id">,
) {
  return queryOptions<IssueDraftSnapshot>({
    ...localQueryPolicy,
    queryKey: collaborationKeys.issueDraft(account, {
      ...key,
      account_id: account.id,
    }),
    queryFn: ({ signal }) =>
      collaboration.forAccount(account).issueDraft(key, signal),
  });
}

/** Local recovery index for authored issue drafts, including missing repositories. */
export function issueDraftsQueryOptions(
  account: RemoteAccount,
  query: Omit<IssueDraftQuery, "account_id">,
) {
  return queryOptions<IssueDraftPage>({
    ...localQueryPolicy,
    queryKey: collaborationKeys.issueDrafts(account, {
      ...query,
      account_id: account.id,
    }),
    queryFn: ({ signal }) =>
      collaboration.forAccount(account).issueDrafts(query, signal),
  });
}

/** Cache-only authored PR draft; preview grants are never cached. */
export function pullDraftQueryOptions(
  account: RemoteAccount,
  key: Omit<PullDraftKey, "account_id">,
) {
  return queryOptions<PullDraftSnapshot>({
    ...localQueryPolicy,
    queryKey: collaborationKeys.pullDraft(account, {
      ...key,
      account_id: account.id,
    }),
    queryFn: ({ signal }) =>
      collaboration.forAccount(account).pullDraft(key, signal),
  });
}
export function pullDraftsQueryOptions(
  account: RemoteAccount,
  query: Omit<PullDraftQuery, "account_id">,
) {
  return queryOptions<PullDraftPage>({
    ...localQueryPolicy,
    queryKey: collaborationKeys.pullDrafts(account, {
      ...query,
      account_id: account.id,
    }),
    queryFn: ({ signal }) =>
      collaboration.forAccount(account).pullDrafts(query, signal),
  });
}

/** Changes only when local authorization/runtime authority is retired. */
export function useCollaborationAuthorityVersion() {
  return useSyncExternalStore(
    collaboration.subscribe,
    collaboration.getAuthorityVersion,
    collaboration.getAuthorityVersion,
  );
}

export function useCollaborationVersion() {
  return useSyncExternalStore(
    collaboration.subscribe,
    collaboration.getVersion,
    collaboration.getVersion,
  );
}

/** View liveness only. Native owns due times, provider retries and all HTTP. */
export function useVisibleDemand({
  account,
  target,
  enabled,
}: {
  account: RemoteAccount;
  target: DemandTarget;
  enabled: boolean;
}): unknown | null {
  const version = useCollaborationVersion();
  const { id, actor_id, authorization_epoch, provider, host, state } = account;
  const { kind, repository_id, subject_id, facet } = target;
  const identity = JSON.stringify([
    version,
    id,
    actor_id,
    authorization_epoch,
    provider,
    host,
    state,
    kind,
    repository_id,
    subject_id,
    facet,
  ]);
  const [failure, setFailure] = useState<{
    identity: string;
    error: unknown;
  } | null>(null);
  useEffect(() => {
    if (!enabled) return;
    const handle = collaboration.retainDemand(
      { id, actor_id, authorization_epoch, provider, host, state },
      { kind, repository_id, subject_id, facet },
    );
    let current = true;
    const unsubscribe = handle.subscribe((error) => {
      if (current) setFailure(error === null ? null : { identity, error });
    });
    return () => {
      current = false;
      unsubscribe();
      handle.release();
    };
  }, [
    enabled,
    id,
    actor_id,
    authorization_epoch,
    provider,
    host,
    state,
    kind,
    repository_id,
    subject_id,
    facet,
    identity,
  ]);
  return enabled && failure?.identity === identity ? failure.error : null;
}

export function accountsQueryOptions(version = collaboration.getVersion()) {
  return queryOptions({
    ...localQueryPolicy,
    queryKey: collaborationKeys.accounts(version),
    queryFn: ({ signal }) => collaboration.accounts(signal),
    // Account/actor metadata may keep a private editor mounted during a grant
    // refresh. Provider projections never use placeholder data.
    placeholderData: (previous) => previous,
  });
}

export function useCollaborationAccounts() {
  const version = useCollaborationVersion();
  return useQuery(accountsQueryOptions(version));
}

export function diagnosticsQueryOptions(version = collaboration.getVersion()) {
  return queryOptions({
    ...localQueryPolicy,
    queryKey: collaborationKeys.diagnostics(version),
    queryFn: () => collaboration.diagnostics(),
    // Only process-local queue ages/cooldowns change between revision events.
    // This interval remains a SQLite/scheduler observation and never syncs.
    refetchInterval: 10_000,
    refetchOnWindowFocus: "always" as const,
  });
}

export function useCollaborationDiagnostics(enabled = true) {
  const version = useCollaborationVersion();
  return useQuery({ ...diagnosticsQueryOptions(version), enabled });
}

/** Discover only while the account dialog is mounted in the trusted main window. */
export function useGithubCliAccounts(enabled: boolean) {
  return useQuery({
    ...localQueryPolicy,
    queryKey: collaborationKeys.githubCli,
    queryFn: ({ signal }) => collaboration.discoverGithubCli(signal),
    enabled,
    staleTime: 0,
    // Native candidate ids expire; never retain a closed dialog's discovery list.
    gcTime: 0,
  });
}

export function repositoriesQueryOptions(account: RemoteAccount) {
  return queryOptions({
    ...localQueryPolicy,
    queryKey: collaborationKeys.repositories(account),
    queryFn: ({ signal }) =>
      collaboration.forAccount(account).repositories(signal),
  });
}

export function useCollaborationRepositories(
  account: RemoteAccount,
  enabled = true,
) {
  useCollaborationVersion();
  return useQuery({ ...repositoriesQueryOptions(account), enabled });
}

export function itemsQueryOptions(
  account: RemoteAccount,
  query: Omit<ItemQuery, "account_id">,
) {
  const fullQuery = { ...query, account_id: account.id };
  return queryOptions({
    ...localQueryPolicy,
    queryKey: collaborationKeys.items(account, fullQuery),
    queryFn: ({ signal }) =>
      collaboration.forAccount(account).items(query, signal),
  });
}

export function useCollaborationItems(
  account: RemoteAccount,
  query: Omit<ItemQuery, "account_id">,
  enabled = true,
) {
  useCollaborationVersion();
  return useQuery({ ...itemsQueryOptions(account, query), enabled });
}

function inboxRefreshInterval(nextLocalChangeAt: string | null | undefined) {
  if (!nextLocalChangeAt) return false;
  const deadline = Date.parse(nextLocalChangeAt);
  if (!Number.isFinite(deadline)) return false;
  return Math.max(250, Math.min(deadline - Date.now(), 60_000));
}

/** SQLite-only inbox projection; bounded polling repairs snooze expiry/clock resume. */
export function inboxQueryOptions(
  account: RemoteAccount,
  query: Omit<InboxQuery, "account_id">,
) {
  const fullQuery = { ...query, account_id: account.id };
  return queryOptions({
    ...localQueryPolicy,
    queryKey: collaborationKeys.inbox(account, fullQuery),
    queryFn: ({ signal }) =>
      collaboration.forAccount(account).inbox(query, signal),
    refetchInterval: (current) =>
      inboxRefreshInterval(current.state.data?.next_local_change_at),
    refetchOnWindowFocus: "always" as const,
  });
}

export function useCollaborationInbox(
  account: RemoteAccount,
  query: Omit<InboxQuery, "account_id">,
  enabled = true,
) {
  useCollaborationVersion();
  return useQuery({ ...inboxQueryOptions(account, query), enabled });
}

export function itemQueryOptions(account: RemoteAccount, itemId: string) {
  return queryOptions({
    ...localQueryPolicy,
    queryKey: collaborationKeys.item(account, itemId),
    queryFn: ({ signal }) =>
      collaboration.forAccount(account).item(itemId, signal),
  });
}

export function useCollaborationItem(
  account: RemoteAccount,
  itemId: string,
  enabled = true,
) {
  useCollaborationVersion();
  return useQuery({ ...itemQueryOptions(account, itemId), enabled });
}

export function contextualCapabilitiesQueryOptions(
  account: RemoteAccount,
  target: CapabilityTarget,
) {
  return queryOptions({
    ...localQueryPolicy,
    queryKey: collaborationKeys.contextualCapabilities(account, target),
    queryFn: ({ signal }) =>
      collaboration.forAccount(account).contextualCapabilities(target, signal),
  });
}

export function useContextualCapabilities(
  account: RemoteAccount,
  target: CapabilityTarget,
) {
  useCollaborationVersion();
  return useQuery(contextualCapabilitiesQueryOptions(account, target));
}

export function draftQueryOptions(account: RemoteAccount, subjectId: string) {
  return queryOptions({
    ...localQueryPolicy,
    queryKey: collaborationKeys.draft(account, subjectId),
    queryFn: ({ signal }) =>
      collaboration.forAccount(account).draft(subjectId, signal),
  });
}

export function draftsQueryOptions(
  account: RemoteAccount,
  query: Omit<DraftQuery, "account_id">,
) {
  const fullQuery = { ...query, account_id: account.id };
  return queryOptions({
    ...localQueryPolicy,
    queryKey: collaborationKeys.drafts(account, fullQuery),
    queryFn: ({ signal }) =>
      collaboration.forAccount(account).drafts(query, signal),
  });
}

export function capabilitiesQueryOptions(account: RemoteAccount) {
  return queryOptions({
    ...localQueryPolicy,
    queryKey: collaborationKeys.capabilities(account),
    queryFn: ({ signal }) =>
      collaboration.forAccount(account).capabilities(signal),
  });
}

export function useCollaborationCapabilities(
  account: RemoteAccount,
  enabled = true,
) {
  useCollaborationVersion();
  return useQuery({ ...capabilitiesQueryOptions(account), enabled });
}

export function resourceQueryOptions(
  account: RemoteAccount,
  locator: ResourceLocator,
) {
  return queryOptions({
    ...localQueryPolicy,
    queryKey: collaborationKeys.resource(account, locator),
    queryFn: ({ signal }) =>
      collaboration.forAccount(account).resolveResource(locator, signal),
  });
}

/**
 * Cache-only query identity/policy for a local projection assembled from one
 * or more detail pages. Callers request hydration separately.
 */
export function detailProjectionQueryOptions<TData>(
  account: RemoteAccount,
  query: Omit<DetailQuery, "account_id">,
  load: (signal: AbortSignal) => Promise<TData>,
  context: readonly unknown[] = [],
) {
  const key = collaborationKeys.detail(account, {
    ...query,
    account_id: account.id,
  });
  return queryOptions({
    ...localQueryPolicy,
    // Optional trusted local context partitions retained query data without
    // changing the native DetailQuery or the bridge's fixed key positions.
    queryKey: [...key, ...context] as const,
    queryFn: ({ signal }) => load(signal),
  });
}

/** Cache-only; callers request hydration separately when the view needs it. */
export function detailQueryOptions(
  account: RemoteAccount,
  query: Omit<DetailQuery, "account_id">,
  context: readonly unknown[] = [],
) {
  return detailProjectionQueryOptions(
    account,
    query,
    (signal) => collaboration.forAccount(account).detail(query, signal),
    context,
  );
}

/** Local read only. Visible demand and manual hydration stay separate. */
export function useCollaborationDetail(
  account: RemoteAccount,
  query: Omit<DetailQuery, "account_id">,
  enabled = true,
) {
  useCollaborationVersion();
  return useQuery({ ...detailQueryOptions(account, query), enabled });
}

/** Ordered cache-only pull commit page; visible demand drives native sync. */
export function pullCommitsQueryOptions(
  account: RemoteAccount,
  query: Omit<PullCommitQuery, "account_id">,
) {
  const fullQuery = { ...query, account_id: account.id };
  return queryOptions({
    ...localQueryPolicy,
    queryKey: collaborationKeys.pullCommits(account, fullQuery),
    queryFn: ({ signal }) =>
      collaboration.forAccount(account).pullCommits(query, signal),
  });
}

export function usePullCommits(
  account: RemoteAccount,
  query: Omit<PullCommitQuery, "account_id">,
  enabled = true,
) {
  useCollaborationVersion();
  return useQuery({ ...pullCommitsQueryOptions(account, query), enabled });
}

/** Ordered cache-only pull file page; visible demand drives native sync. */
export function pullFilesQueryOptions(
  account: RemoteAccount,
  query: Omit<PullFileQuery, "account_id">,
) {
  const fullQuery = { ...query, account_id: account.id };
  return queryOptions({
    ...localQueryPolicy,
    queryKey: collaborationKeys.pullFiles(account, fullQuery),
    queryFn: ({ signal }) =>
      collaboration.forAccount(account).pullFiles(query, signal),
  });
}

export function usePullFiles(
  account: RemoteAccount,
  query: Omit<PullFileQuery, "account_id">,
  enabled = true,
) {
  useCollaborationVersion();
  return useQuery({ ...pullFilesQueryOptions(account, query), enabled });
}

/** Exact selected artifact read from SQLite only. Hydration is explicit. */
export function pullFileArtifactQueryOptions(
  account: RemoteAccount,
  request: Omit<PullFileDiffRequest, "account_id" | "authorization_epoch">,
) {
  const fullRequest = {
    ...request,
    account_id: account.id,
    authorization_epoch: account.authorization_epoch,
  };
  return queryOptions({
    ...localQueryPolicy,
    queryKey: collaborationKeys.pullFileArtifact(account, fullRequest),
    queryFn: ({ signal }) =>
      collaboration.forAccount(account).pullFileArtifact(request, signal),
    // SQLite is the durable cache. Release inactive multi-megabyte patch text
    // immediately so navigation retains only the selected artifact in JS.
    gcTime: 0,
  });
}

export function usePullFileArtifact(
  account: RemoteAccount,
  request: Omit<PullFileDiffRequest, "account_id" | "authorization_epoch">,
  enabled = true,
) {
  useCollaborationVersion();
  return useQuery({
    ...pullFileArtifactQueryOptions(account, request),
    enabled,
  });
}

/** Local Git/SQLite only; no provider request or automatic link selection. */
export function localLinksQueryOptions(
  localRepositoryId: string,
  version = collaboration.getVersion(),
) {
  return queryOptions({
    ...localQueryPolicy,
    queryKey: collaborationKeys.localLinks(localRepositoryId, version),
    queryFn: ({ signal }) =>
      collaboration.localLinks(localRepositoryId, signal),
    staleTime: 0,
    gcTime: 0,
  });
}
export function useLocalRepositoryLinks(localRepositoryId: string) {
  const version = useCollaborationVersion();
  return useQuery(localLinksQueryOptions(localRepositoryId, version));
}
export function localClonesQueryOptions(
  account: RemoteAccount,
  instanceId: string,
  repositoryId: string,
  sourceRepositoryProviderId: string | null = null,
) {
  return queryOptions({
    ...localQueryPolicy,
    queryKey: collaborationKeys.localClones(
      account,
      instanceId,
      repositoryId,
      sourceRepositoryProviderId,
    ),
    queryFn: ({ signal }) =>
      collaboration
        .forAccount(account)
        .localClones(
          instanceId,
          repositoryId,
          sourceRepositoryProviderId,
          signal,
        ),
    staleTime: 0,
    gcTime: 0,
  });
}
export function useLocalClones(
  account: RemoteAccount,
  instanceId: string,
  repositoryId: string,
  sourceRepositoryProviderId: string | null = null,
) {
  useCollaborationVersion();
  return useQuery(
    localClonesQueryOptions(
      account,
      instanceId,
      repositoryId,
      sourceRepositoryProviderId,
    ),
  );
}

/** Local identity/provenance only. Discovery is an explicit, separate intent. */
export function notificationSubjectQueryOptions(
  account: RemoteAccount,
  notificationId: string,
) {
  // Query options may outlive a caller's mutable account object.
  const captured = { ...account };
  return queryOptions({
    ...localQueryPolicy,
    queryKey: collaborationKeys.notificationSubject(captured, notificationId),
    queryFn: ({ signal }) =>
      collaboration
        .forAccount(captured)
        .notificationSubject(notificationId, signal),
  });
}

export function useNotificationSubject(
  account: RemoteAccount,
  notificationId: string,
  enabled = true,
) {
  useCollaborationVersion();
  return useQuery({
    ...notificationSubjectQueryOptions(account, notificationId),
    enabled,
  });
}

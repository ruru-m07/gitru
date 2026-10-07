import type {
  CapabilityTarget,
  DemandTarget,
  DetailQuery,
  DraftQuery,
  ItemQuery,
  PullCommitQuery,
  RemoteAccount,
  ResourceLocator,
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

/** Cache-only; callers request hydration separately when the view needs it. */
export function detailQueryOptions(
  account: RemoteAccount,
  query: Omit<DetailQuery, "account_id">,
) {
  return queryOptions({
    ...localQueryPolicy,
    queryKey: collaborationKeys.detail(account, {
      ...query,
      account_id: account.id,
    }),
    queryFn: ({ signal }) =>
      collaboration.forAccount(account).detail(query, signal),
  });
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

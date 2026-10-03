import type {
  CapabilityTarget,
  DetailQuery,
  ItemQuery,
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

export function capabilitiesQueryOptions(account: RemoteAccount) {
  return queryOptions({
    ...localQueryPolicy,
    queryKey: collaborationKeys.capabilities(account),
    queryFn: ({ signal }) =>
      collaboration.forAccount(account).capabilities(signal),
  });
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

/** Local read only. Selected views submit hydration through the separate hook. */
export function useCollaborationDetail(
  account: RemoteAccount,
  query: Omit<DetailQuery, "account_id">,
  enabled = true,
) {
  useCollaborationVersion();
  return useQuery({ ...detailQueryOptions(account, query), enabled });
}

/** Synchronize selected-view intent with the native engine, never a query fetch. */
export function useSelectedDetailHydration({
  account,
  subjectId,
  parentHeadOid,
  eligible,
}: {
  account: RemoteAccount;
  subjectId: string;
  parentHeadOid: string | null;
  eligible: boolean;
}) {
  const identity = JSON.stringify([
    account.id,
    account.actor_id,
    account.authorization_epoch,
    subjectId,
    parentHeadOid,
  ]);
  const [failure, setFailure] = useState<{
    identity: string;
    error: unknown;
  } | null>(null);
  useEffect(() => {
    const lease = collaboration.forAccount(account).retainDetailSelection({
      subject_id: subjectId,
      facet: "body",
    });
    let current = true;
    if (eligible)
      void lease.hydrate(parentHeadOid ?? "").catch((error: unknown) => {
        if (current) setFailure({ identity, error });
      });
    return () => {
      current = false;
      lease.release();
    };
  }, [account, subjectId, parentHeadOid, eligible, identity]);
  return eligible && failure?.identity === identity ? failure.error : null;
}

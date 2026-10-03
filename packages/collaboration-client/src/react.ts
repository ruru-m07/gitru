import type { ItemQuery, RemoteAccount } from "@gitru/commands";
import { queryOptions, useQuery } from "@tanstack/react-query";
import { useSyncExternalStore } from "react";
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

export function useCollaborationRepositories(account: RemoteAccount) {
  useCollaborationVersion();
  return useQuery(repositoriesQueryOptions(account));
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
) {
  useCollaborationVersion();
  return useQuery(itemsQueryOptions(account, query));
}

export function itemQueryOptions(account: RemoteAccount, itemId: string) {
  return queryOptions({
    ...localQueryPolicy,
    queryKey: collaborationKeys.item(account, itemId),
    queryFn: ({ signal }) =>
      collaboration.forAccount(account).item(itemId, signal),
  });
}

export function useCollaborationItem(account: RemoteAccount, itemId: string) {
  useCollaborationVersion();
  return useQuery(itemQueryOptions(account, itemId));
}

export function draftQueryOptions(account: RemoteAccount, subjectId: string) {
  return queryOptions({
    ...localQueryPolicy,
    queryKey: collaborationKeys.draft(account, subjectId),
    queryFn: ({ signal }) =>
      collaboration.forAccount(account).draft(subjectId, signal),
  });
}

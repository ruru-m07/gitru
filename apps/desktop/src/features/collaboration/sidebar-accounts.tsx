import { type RemoteAccount } from "@gitru/collaboration-client";
import {
  contextualCapabilitiesQueryOptions,
  inboxQueryOptions,
  useCollaborationAccounts,
  useCollaborationVersion,
} from "@gitru/collaboration-client/react";
import { Avatar, AvatarFallback } from "@gitru/ui/components/avatar";
import { Button } from "@gitru/ui/components/button";
import { ScrollArea } from "@gitru/ui/components/scroll-area";
import { useQueries } from "@tanstack/react-query";
import { Plus, UserRound } from "lucide-react";
import { AccountSettingsButton } from "./account-manager";
import {
  accountCapabilityTarget,
  canReadSaved,
  facetPolicy,
  inboxPresentation,
} from "./capability-policy";

export function useSavedInboxBadge() {
  const accounts = useCollaborationAccounts();
  useCollaborationVersion();
  const connected =
    accounts.data?.accounts.filter((account) => account.state === "active") ??
    [];
  const contexts = useQueries({
    queries: connected.map((account) =>
      contextualCapabilitiesQueryOptions(account, accountCapabilityTarget),
    ),
  });
  const inboxes = connected.flatMap((account, index) => {
    const snapshot = contexts[index]?.data;
    const state = snapshot
      ? inboxPresentation(snapshot.inbox_semantics).badgeState
      : null;
    return snapshot && canReadSaved(facetPolicy(snapshot, "inbox")) && state
      ? [{ account, state }]
      : [];
  });
  const pages = useQueries({
    queries: inboxes.map(({ account, state }) =>
      inboxQueryOptions(account, {
        remote_state: state,
        local_state: "inbox",
        search: null,
        cursor: null,
        limit: 100,
      }),
    ),
  });
  const count = pages.reduce(
    (sum, page) => sum + (page.data?.entries.length ?? 0),
    0,
  );
  return count
    ? count > 99
      ? "99+"
      : `${count}${pages.some((page) => page.data && (page.data.next_cursor || page.data.coverage.state !== "complete" || page.data.coverage.remote_has_more)) ? "+" : ""}`
    : undefined;
}

export function SidebarAccounts() {
  const accounts = useCollaborationAccounts();
  const connected =
    accounts.data?.accounts.filter(
      (account) => account.state !== "disconnected",
    ) ?? [];
  return (
    <ScrollArea className="w-full max-h-[calc(100vh-12rem)]">
      <div className="flex flex-col items-center gap-1">
        {connected.map((account) => (
          <AccountSettingsButton
            key={account.id}
            trigger={
              <Button
                aria-label={`Manage ${account.login} account`}
                variant="ghost"
                size="icon"
                className="size-8 p-0"
              >
                <AccountAvatar account={account} />
              </Button>
            }
          />
        ))}
        <AccountSettingsButton
          trigger={
            <Button
              variant="outline"
              size="icon"
              className="size-8 p-0"
              aria-label="Manage connected accounts"
            >
              <Plus className="size-3.5 opacity-60" aria-hidden="true" />
            </Button>
          }
        />
      </div>
    </ScrollArea>
  );
}

export function AccountAvatar({ account }: { account?: RemoteAccount }) {
  return (
    <Avatar className="rounded-md size-7">
      <AvatarFallback className="rounded-md text-xs">
        {account ? (
          account.login.slice(0, 2).toUpperCase()
        ) : (
          <UserRound className="size-4" aria-hidden="true" />
        )}
      </AvatarFallback>
    </Avatar>
  );
}

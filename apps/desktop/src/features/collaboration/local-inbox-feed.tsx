import {
  type ContextFacetCapability,
  collaboration,
  collaborationErrorMessage,
  collaborationKeys,
  type InboxEntry,
  type LocalInboxFilter,
  type RemoteAccount,
  type RemoteRepository,
} from "@gitru/collaboration-client";
import { useCollaborationInbox } from "@gitru/collaboration-client/react";
import { Badge } from "@gitru/ui/components/badge";
import { Button } from "@gitru/ui/components/button";
import { useQueryClient } from "@tanstack/react-query";
import {
  Bell,
  Bookmark,
  BookmarkCheck,
  Check,
  ChevronLeft,
  ChevronRight,
  Clock3,
  Undo2,
} from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { CapabilityBoundary } from "./capability-boundary";
import { canReadSaved, canSynchronize } from "./capability-policy";
import { NotificationSubjectView } from "./notification-subject-view";
import { CollaborationStatePanel } from "./state-panel";
import { SyncIndicator } from "./sync-indicator";

type Props = {
  account: RemoteAccount;
  instanceId: string | null;
  policy: ContextFacetCapability | undefined;
  contextPending: boolean;
  contextError: string | undefined;
  recheck: () => void;
  repositories: RemoteRepository[];
  remoteState: string | null;
  localState: LocalInboxFilter;
  search: string;
  refresh: () => Promise<void>;
  refreshing: boolean;
};

export function LocalInboxFeed({
  account,
  instanceId,
  policy,
  contextPending,
  contextError,
  recheck,
  repositories,
  remoteState,
  localState,
  search,
  refresh,
  refreshing,
}: Props) {
  const cache = useQueryClient();
  const [cursors, setCursors] = useState<Array<string | null>>([null]);
  const cursor = cursors[cursors.length - 1] ?? null;
  const query = useCollaborationInbox(
    account,
    {
      remote_state: remoteState,
      local_state: localState,
      search: search || null,
      cursor,
      limit: 50,
    },
    canReadSaved(policy),
  );
  const [selectedItem, setSelectedItem] = useState<string | null>(null);
  const [mutating, setMutating] = useState<string | null>(null);
  const [mutationError, setMutationError] = useState<string | null>(null);
  const page = canReadSaved(policy) ? query.data : undefined;
  const selected = page?.entries.find(
    (entry) => entry.item.id === selectedItem,
  );

  useEffect(() => {
    setSelectedItem(null);
    setCursors([null]);
  }, [account.id, account.authorization_epoch]);

  const invalidateInbox = useCallback(
    () =>
      cache.invalidateQueries({
        queryKey: collaborationKeys.account(account.id),
        predicate: (candidate) => candidate.queryKey[4] === "inbox",
      }),
    [account.id, cache],
  );

  useEffect(
    () =>
      collaboration.subscribeChanges((change) => {
        if (
          change.account_id === account.id &&
          (change.scope === "notifications" ||
            change.scope === "account" ||
            change.scope.startsWith("local_inbox:") ||
            change.reset)
        ) {
          setSelectedItem(null);
          setCursors([null]);
        }
      }),
    [account.id],
  );

  useEffect(() => {
    const deadline = page?.next_local_change_at
      ? Date.parse(page.next_local_change_at)
      : Number.NaN;
    if (!Number.isFinite(deadline)) return;
    let timer: ReturnType<typeof setTimeout> | undefined;
    let disposed = false;
    const check = () => {
      if (disposed) return;
      const remaining = deadline - Date.now();
      if (remaining <= 0) {
        setSelectedItem(null);
        setCursors([null]);
        void invalidateInbox();
        return;
      }
      timer = setTimeout(check, Math.min(remaining, 60_000));
    };
    check();
    return () => {
      disposed = true;
      if (timer) clearTimeout(timer);
    };
  }, [invalidateInbox, page?.next_local_change_at]);

  async function update(
    entry: InboxEntry,
    operation:
      | { kind: "bookmark"; value: boolean }
      | {
          kind: "disposition";
          value: "inbox" | "done";
          snoozedUntil: string | null;
        },
  ) {
    setMutating(entry.item.id);
    setMutationError(null);
    try {
      await collaboration.forAccount(account).setLocalInboxState({
        notification_id: entry.item.id,
        expected_activity_updated_at: entry.item.updated_at,
        mutation: operation.kind,
        disposition: operation.kind === "disposition" ? operation.value : null,
        bookmarked: operation.kind === "bookmark" ? operation.value : null,
        snoozed_until:
          operation.kind === "disposition" ? operation.snoozedUntil : null,
        expected_generation: entry.local.generation,
      });
      setSelectedItem(null);
      setCursors([null]);
      await invalidateInbox();
    } catch (error) {
      setMutationError(collaborationErrorMessage(error));
      setSelectedItem(null);
      setCursors([null]);
      await invalidateInbox();
    } finally {
      setMutating(null);
    }
  }

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="flex shrink-0 flex-wrap items-center justify-between gap-2 border-b px-5 py-2">
        <span className="text-xs text-muted-foreground">
          {page
            ? `${page.entries.length} saved items${search ? " matching your search" : " on this page"}`
            : "Saved activity"}
        </span>
        {page ? (
          <SyncIndicator
            state={
              policy?.synchronize.reason === "temporarily_unavailable"
                ? policy.sync.state
                : page.sync.state === "rate_limited" && canSynchronize(policy)
                  ? "idle"
                  : page.sync.state
            }
            validatedAt={page.coverage.validated_at}
            partial={page.coverage.state === "partial"}
          />
        ) : null}
      </div>
      {page?.sync.error ? (
        <p
          role="alert"
          className="border-b px-5 py-2 text-xs text-destructive-foreground"
        >
          {collaborationErrorMessage(page.sync.error)}
        </p>
      ) : null}
      {mutationError ? (
        <p
          role="alert"
          className="border-b px-5 py-2 text-xs text-destructive-foreground"
        >
          {mutationError}
        </p>
      ) : null}
      <div
        className={`grid min-h-0 flex-1 ${selectedItem ? "md:grid-cols-2" : "grid-cols-1"}`}
      >
        <div
          className={`min-w-0 overflow-y-auto ${selectedItem ? "hidden md:block" : ""}`}
        >
          {!canReadSaved(policy) ? (
            <CapabilityBoundary
              policy={policy}
              pending={contextPending}
              error={contextError}
              recheck={recheck}
              busy={refreshing}
            >
              {null}
            </CapabilityBoundary>
          ) : query.isPending ? (
            <CollaborationStatePanel title="Loading saved activity">
              This view reads the data saved on your device.
            </CollaborationStatePanel>
          ) : query.isError ? (
            <CollaborationStatePanel
              title="Could not load this view"
              action="Reload saved items"
              onAction={() => {
                if (cursor !== null) setCursors([null]);
                else void query.refetch();
              }}
            >
              {collaborationErrorMessage(query.error)}
            </CollaborationStatePanel>
          ) : page && !page.entries.length ? (
            <CollaborationStatePanel
              title={
                page.coverage.state === "missing"
                  ? "Not synced yet"
                  : page.coverage.state === "partial" || search
                    ? "No saved matches"
                    : "Nothing here yet"
              }
              offline={page.sync.state === "offline"}
              action={canSynchronize(policy) ? "Refresh activity" : undefined}
              onAction={() => void refresh()}
              busy={refreshing}
            >
              {page.coverage.state === "missing"
                ? "Refresh to bring recent activity onto this device."
                : search
                  ? "Search covers saved items. Try another search or filter."
                  : "There are no saved items matching the selected filters."}
            </CollaborationStatePanel>
          ) : (
            <div>
              {page?.entries.map((entry) => (
                <div
                  key={entry.item.id}
                  className="flex items-center border-b border-border"
                >
                  <Button
                    variant="ghost"
                    className="h-auto min-w-0 flex-1 justify-start gap-3 rounded-none px-5 py-3 text-left whitespace-normal"
                    aria-pressed={selectedItem === entry.item.id}
                    onClick={() => setSelectedItem(entry.item.id)}
                  >
                    <Bell
                      className={`size-4 shrink-0 ${entry.item.unread || entry.item.state === "pending" ? "text-success-foreground" : "text-muted-foreground"}`}
                      aria-hidden="true"
                    />
                    <div className="min-w-0 flex-1">
                      <p className="line-clamp-2 break-words text-sm font-medium">
                        {entry.item.title}
                      </p>
                      <div className="mt-1 flex min-w-0 flex-wrap items-center gap-1.5 text-xs text-muted-foreground">
                        <span className="truncate max-w-60">
                          {repositories.find(
                            (repository) =>
                              repository.id === entry.item.repository_id,
                          )?.full_name ??
                            entry.item.reason ??
                            "Provider activity"}
                        </span>
                        <Badge variant="outline" size="sm">
                          {entry.item.unread === null
                            ? `Provider ${entry.item.state}`
                            : entry.item.unread
                              ? "Provider unread"
                              : "Provider read"}
                        </Badge>
                        <Badge variant="outline" size="sm">
                          Local {entry.local.effective_disposition}
                        </Badge>
                        {entry.local.superseded_by_activity ? (
                          <Badge variant="outline" size="sm">
                            New activity
                          </Badge>
                        ) : null}
                      </div>
                    </div>
                  </Button>
                  <div className="flex shrink-0 items-center gap-0.5 pr-2">
                    <Button
                      type="button"
                      variant="ghost"
                      size="icon-sm"
                      disabled={mutating === entry.item.id}
                      aria-label={
                        entry.local.bookmarked
                          ? `Remove bookmark from ${entry.item.title}`
                          : `Bookmark ${entry.item.title}`
                      }
                      title={
                        entry.local.bookmarked ? "Remove bookmark" : "Bookmark"
                      }
                      onClick={() =>
                        void update(entry, {
                          kind: "bookmark",
                          value: !entry.local.bookmarked,
                        })
                      }
                    >
                      {entry.local.bookmarked ? (
                        <BookmarkCheck aria-hidden="true" />
                      ) : (
                        <Bookmark aria-hidden="true" />
                      )}
                    </Button>
                    <Button
                      type="button"
                      variant="ghost"
                      size="icon-sm"
                      disabled={mutating === entry.item.id}
                      aria-label={`Snooze ${entry.item.title} for one hour`}
                      title="Snooze for one hour"
                      onClick={() =>
                        void update(entry, {
                          kind: "disposition",
                          value: "inbox",
                          snoozedUntil: new Date(
                            Date.now() + 60 * 60_000,
                          ).toISOString(),
                        })
                      }
                    >
                      <Clock3 aria-hidden="true" />
                    </Button>
                    <Button
                      type="button"
                      variant="ghost"
                      size="icon-sm"
                      disabled={mutating === entry.item.id}
                      aria-label={
                        entry.local.effective_disposition !== "inbox"
                          ? `Move ${entry.item.title} to local inbox`
                          : `Mark ${entry.item.title} locally done`
                      }
                      title={
                        entry.local.effective_disposition !== "inbox"
                          ? "Move to local inbox"
                          : "Mark locally done"
                      }
                      onClick={() =>
                        void update(entry, {
                          kind: "disposition",
                          value:
                            entry.local.effective_disposition !== "inbox"
                              ? "inbox"
                              : "done",
                          snoozedUntil: null,
                        })
                      }
                    >
                      {entry.local.effective_disposition !== "inbox" ? (
                        <Undo2 aria-hidden="true" />
                      ) : (
                        <Check aria-hidden="true" />
                      )}
                    </Button>
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>
        {selectedItem && selected && instanceId ? (
          <NotificationSubjectView
            key={selectedItem}
            account={account}
            notificationId={selectedItem}
            instanceId={instanceId}
            localState={selected.local}
            close={() => setSelectedItem(null)}
          />
        ) : null}
      </div>
      {page &&
      (page.next_cursor ||
        cursors.length > 1 ||
        page.coverage.remote_has_more) ? (
        <footer className="flex shrink-0 flex-wrap items-center justify-between gap-2 border-t px-5 py-2">
          <div className="flex gap-2">
            <Button
              size="sm"
              variant="ghost"
              disabled={cursors.length <= 1}
              onClick={() => {
                setCursors((values) => values.slice(0, -1));
                setSelectedItem(null);
              }}
            >
              <ChevronLeft aria-hidden="true" /> Previous
            </Button>
            <Button
              size="sm"
              variant="ghost"
              disabled={!page.next_cursor}
              onClick={() => {
                if (page.next_cursor)
                  setCursors((values) => [...values, page.next_cursor]);
                setSelectedItem(null);
              }}
            >
              Next saved page <ChevronRight aria-hidden="true" />
            </Button>
          </div>
          {page.coverage.remote_has_more ? (
            <Button
              size="sm"
              variant="outline"
              disabled={refreshing || !canSynchronize(policy)}
              onClick={() => void refresh()}
            >
              Sync more activity
            </Button>
          ) : null}
        </footer>
      ) : null}
    </div>
  );
}

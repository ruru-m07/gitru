import {
  collaboration,
  collaborationErrorMessage,
  type LocalInboxState,
  type NotificationSubjectSnapshot,
  type RemoteAccount,
} from "@gitru/collaboration-client";
import {
  draftQueryOptions,
  useCollaborationItem,
  useContextualCapabilities,
  useNotificationSubject,
} from "@gitru/collaboration-client/react";
import { Badge } from "@gitru/ui/components/badge";
import { Button } from "@gitru/ui/components/button";
import { useQuery } from "@tanstack/react-query";
import { ArrowLeft } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { CapabilityBoundary } from "./capability-boundary";
import {
  canReadSaved,
  facetPolicy,
  providerInboxState,
  resourceCapabilityTarget,
} from "./capability-policy";
import { SavedDraftEditor } from "./private-draft";
import { ProviderLink } from "./provider-link";
import { SavedItemDetail } from "./saved-item-detail";

type SubjectBinding = {
  id: string;
  kind: "pull_request" | "issue";
  instanceId: string;
};

const explanations: Record<NotificationSubjectSnapshot["state"], string> = {
  resolved:
    "This notification opens its saved subject. Opening it does not mark the notification as read.",
  not_cached: "This notification’s subject is not saved on this device yet.",
  unsupported:
    "This notification does not have a supported pull request or issue subject.",
  ambiguous:
    "The saved identities for this subject conflict. Open the provider to view it.",
  unavailable:
    "This notification is no longer available in the current saved view.",
  identity_unverified:
    "The provider response could not verify this subject’s repository identity.",
};

function subjectBinding(
  snapshot: NotificationSubjectSnapshot | undefined,
  account: RemoteAccount,
  instanceId: string,
): SubjectBinding | null {
  const resource = snapshot?.state === "resolved" ? snapshot.subject : null;
  return snapshot?.authorization_epoch === account.authorization_epoch &&
    resource?.account_id === account.id &&
    resource.instance_id === instanceId &&
    (resource.kind === "pull_request" || resource.kind === "issue")
    ? { id: resource.id, kind: resource.kind, instanceId: resource.instance_id }
    : null;
}

/** A thread remains a thread. Only a current native resolver can name its subject. */
export function NotificationSubjectView({
  account,
  notificationId,
  instanceId,
  localState,
  close,
}: {
  account: RemoteAccount;
  notificationId: string;
  instanceId: string;
  localState?: LocalInboxState;
  close: () => void;
}) {
  const context = useContextualCapabilities(
    account,
    resourceCapabilityTarget(instanceId, notificationId, "notification"),
  );
  const policy = facetPolicy(context.data, "inbox");
  const readable = canReadSaved(policy);
  const resolution = useNotificationSubject(account, notificationId, readable);
  const snapshot =
    readable && !resolution.isError ? resolution.data : undefined;
  const notification = useCollaborationItem(
    account,
    notificationId,
    readable && snapshot?.state !== "unavailable",
  );
  const resolved = readable
    ? subjectBinding(snapshot, account, instanceId)
    : null;
  const bindingMismatch = snapshot?.state === "resolved" && resolved === null;
  // Keep only a verified binding to the authored editor, never old provider text.
  // Parent actor/thread keys replace this state on navigation. Grant refreshes
  // can hide content without replacing the private editor's inspected generation.
  const owner = JSON.stringify([
    account.id,
    account.actor_id,
    notificationId,
    instanceId,
  ]);
  const [retained, setRetained] = useState<{
    owner: string;
    subject: SubjectBinding;
  } | null>(() => (resolved ? { owner, subject: resolved } : null));
  if (
    resolved &&
    (retained?.owner !== owner ||
      retained.subject.id !== resolved.id ||
      retained.subject.kind !== resolved.kind ||
      retained.subject.instanceId !== resolved.instanceId)
  )
    setRetained({ owner, subject: resolved });
  const subject =
    resolved ?? (retained?.owner === owner ? retained.subject : null);
  const original =
    readable &&
    snapshot &&
    !bindingMismatch &&
    snapshot.state !== "unavailable" &&
    !notification.isError
      ? notification.data?.item
      : null;
  const isTodo =
    original?.native_inbox?.source === "todo" || account.provider === "gitlab";
  const fallbackUrl =
    isTodo && snapshot?.state === "unsupported"
      ? (original?.web_url ?? snapshot.fallback_web_url)
      : snapshot?.fallback_web_url;
  return (
    <section
      className="min-w-0 overflow-y-auto border-l p-5"
      aria-label={isTodo ? "To-do subject" : "Notification subject"}
    >
      <Button variant="ghost" size="sm" onClick={close} className="mb-4">
        <ArrowLeft aria-hidden="true" /> Back to list
      </Button>
      {original?.kind === "notification" ? (
        <div
          className="mb-4 space-y-2"
          aria-label={isTodo ? "Original to-do" : "Original notification"}
        >
          <p className="break-words text-sm">{original.title}</p>
          <div className="flex flex-wrap items-center gap-2 text-xs text-muted-foreground">
            {original.reason ? <span>Reason: {original.reason}</span> : null}
            <Badge variant="outline" size="sm">
              {providerInboxState(original)}
            </Badge>
            {localState ? (
              <Badge variant="outline" size="sm">
                Local {localState.effective_disposition}
              </Badge>
            ) : null}
            {localState?.bookmarked ? (
              <Badge variant="outline" size="sm">
                Bookmarked locally
              </Badge>
            ) : null}
            {localState?.superseded_by_activity ? (
              <Badge variant="outline" size="sm">
                New activity restored this item to the local inbox
              </Badge>
            ) : null}
          </div>
        </div>
      ) : null}
      {!readable ? (
        <CapabilityBoundary
          policy={policy}
          pending={context.isPending}
          error={
            context.isError
              ? collaborationErrorMessage(context.error)
              : undefined
          }
        >
          {null}
        </CapabilityBoundary>
      ) : resolution.isError ? (
        <p role="alert">{collaborationErrorMessage(resolution.error)}</p>
      ) : bindingMismatch ? (
        <p role="alert">
          This notification’s saved subject does not match the current account
          or installation.
        </p>
      ) : snapshot ? (
        <div className="space-y-3 text-sm">
          <p>
            {isTodo
              ? snapshot.state === "resolved"
                ? "This to-do opens its saved subject. Opening it does not complete the to-do on GitLab."
                : explanations[snapshot.state].replace(/notification/g, "to-do")
              : explanations[snapshot.state]}
          </p>
          {isTodo ? (
            <p className="text-muted-foreground">
              Complete to-dos in GitLab. Local dismissal and snoozing only
              change your Gitru inbox.
            </p>
          ) : null}
          {snapshot.reason === "not_found" ? (
            <p>
              The subject was not found or is inaccessible. This does not
              establish that it was deleted.
            </p>
          ) : null}
          {snapshot.reason === "attempts_exhausted" ? (
            <p>
              The bounded loading attempts have stopped. Try again explicitly
              when ready.
            </p>
          ) : null}
          {snapshot.state !== "unavailable" && fallbackUrl ? (
            <ProviderLink url={fallbackUrl} />
          ) : null}
          {!resolved ? (
            <DiscoveryAction
              key={JSON.stringify([
                account.id,
                account.actor_id,
                account.authorization_epoch,
                notificationId,
                snapshot.selector_generation,
              ])}
              account={account}
              notificationId={notificationId}
              snapshot={snapshot}
              onAccepted={async () => {
                await collaboration.wake();
                await resolution.refetch();
              }}
            />
          ) : null}
        </div>
      ) : (
        <p role="status" className="text-sm text-muted-foreground">
          Reading the saved {isTodo ? "to-do" : "notification"} subject…
        </p>
      )}
      {subject ? (
        <SavedItemDetail
          key={`${owner}:${subject.kind}:${subject.id}`}
          account={account}
          itemId={subject.id}
          kind={subject.kind}
          instanceId={subject.instanceId}
          providerEnabled={resolved !== null}
        />
      ) : null}
      <NotificationDraft
        key={owner}
        account={account}
        notificationId={notificationId}
      />
    </section>
  );
}

function DiscoveryAction({
  account,
  notificationId,
  snapshot,
  onAccepted,
}: {
  account: RemoteAccount;
  notificationId: string;
  snapshot: NotificationSubjectSnapshot;
  onAccepted: () => Promise<void>;
}) {
  const [busy, setBusy] = useState(false);
  const [failure, setFailure] = useState<unknown | null>(null);
  const [accepted, setAccepted] = useState(false);
  const alive = useRef(false);
  useEffect(() => {
    alive.current = true;
    return () => {
      alive.current = false;
    };
  }, []);
  const allowed =
    snapshot.discovery.support === "supported" &&
    snapshot.discovery.admission &&
    snapshot.selector_generation !== null;
  const retryAt = snapshot.discovery.retry_at
    ? new Date(snapshot.discovery.retry_at)
    : null;
  const retryLabel =
    retryAt && !Number.isNaN(retryAt.getTime())
      ? retryAt.toLocaleString()
      : null;
  const showAccepted =
    accepted &&
    !failure &&
    !snapshot.discovery.sync.error &&
    snapshot.discovery.support === "supported" &&
    snapshot.state !== "unavailable" &&
    (snapshot.reason === null || snapshot.reason === "not_cached");
  async function discover() {
    if (!allowed || busy || snapshot.selector_generation === null) return;
    setBusy(true);
    setFailure(null);
    setAccepted(false);
    try {
      await collaboration
        .forAccount(account)
        .discoverNotificationSubject(
          notificationId,
          snapshot.selector_generation,
        );
      if (alive.current) {
        setAccepted(true);
        await onAccepted();
      }
    } catch (error) {
      if (alive.current) {
        setAccepted(false);
        setFailure(error);
      }
    } finally {
      if (alive.current) setBusy(false);
    }
  }
  return (
    <div className="space-y-2">
      {snapshot.discovery.paused ? (
        <p className="text-xs text-muted-foreground">
          Loading is paused
          {retryLabel
            ? ` until ${retryLabel}`
            : " while the provider is unavailable"}
          . An accepted request waits until the provider allows another request.
        </p>
      ) : null}
      {showAccepted ? (
        <p role="status" className="text-xs text-muted-foreground">
          Loading request accepted. The subject is not loaded yet.
        </p>
      ) : null}
      {snapshot.discovery.sync.error ? (
        <p role="alert" className="text-xs">
          {collaborationErrorMessage(snapshot.discovery.sync.error)}
        </p>
      ) : null}
      {failure ? (
        <p role="alert" className="text-xs">
          {collaborationErrorMessage(failure)}
        </p>
      ) : null}
      {snapshot.discovery.support === "supported" ? (
        <Button
          size="sm"
          variant="outline"
          disabled={busy || !allowed}
          onClick={() => {
            void discover();
          }}
        >
          {busy
            ? "Requesting subject…"
            : snapshot.discovery.attempts
              ? "Try loading subject again"
              : "Load notification subject"}
        </Button>
      ) : null}
    </div>
  );
}

function NotificationDraft({
  account,
  notificationId,
}: {
  account: RemoteAccount;
  notificationId: string;
}) {
  const query = useQuery(draftQueryOptions(account, notificationId));
  const [hasSaved, setHasSaved] = useState(query.data != null);
  if (query.data != null && !hasSaved) setHasSaved(true);
  return hasSaved ? (
    <details className="mt-4 text-xs text-muted-foreground">
      <summary>Saved draft for this notification</summary>
      <p className="mt-2">
        This thread draft is separate from the subject’s private draft.
      </p>
      <SavedDraftEditor
        account={account}
        subjectId={notificationId}
        label="Private notification draft"
      />
    </details>
  ) : null;
}

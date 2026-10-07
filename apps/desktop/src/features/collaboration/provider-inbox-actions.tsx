import {
  collaboration,
  collaborationErrorMessage,
  type ProviderInboxAction,
  type ProviderInboxActionReason,
  type QueueProviderInboxActionRequest,
  type RemoteAccount,
} from "@gitru/collaboration-client";
import { providerInboxActionsQueryOptions } from "@gitru/collaboration-client/react";
import { Button } from "@gitru/ui/components/button";
import { useQuery } from "@tanstack/react-query";
import { useState } from "react";

const reasons: Record<ProviderInboxActionReason, string> = {
  not_implemented: "This provider action is not available in Gitru yet.",
  provider_semantics: "This provider does not support this action.",
  missing_source_evidence:
    "Refresh this item before choosing a provider action.",
  already_applied: "The saved provider state already has this value.",
  authentication_required: "Reconnect this account to use provider actions.",
  unsupported_credential: "This credential cannot use the provider inbox API.",
  pending_command: "A saved change is already waiting for this item.",
};

/** Mounted only for the selected notification. Availability and admission read
 * native local state; opening this panel never starts a provider request. */
export function ProviderInboxActions({
  account,
  notificationId,
}: {
  account: RemoteAccount;
  notificationId: string;
}) {
  const query = useQuery(
    providerInboxActionsQueryOptions(account, notificationId),
  );
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [saved, setSaved] = useState(false);
  const [retry, setRetry] = useState<QueueProviderInboxActionRequest | null>(
    null,
  );
  const client = collaboration.forAccount(account);
  const provider =
    account.provider === "github"
      ? "GitHub"
      : account.provider === "gitlab"
        ? "GitLab"
        : "provider";
  const snapshot = query.isError ? undefined : query.data;
  const available = snapshot?.actions.some(
    (action) => action.availability === "available",
  );
  async function submit(request: QueueProviderInboxActionRequest) {
    if (busy) return;
    setBusy(true);
    setError(null);
    setSaved(false);
    try {
      await client.queueProviderInboxAction(request);
      setRetry(null);
      setSaved(true);
      await query.refetch();
      void collaboration.wake();
    } catch (failure) {
      // The native writer may have committed before the IPC response was lost.
      // Keep the exact UUID, activity fence and policy when the user retries.
      setRetry(request);
      setError(collaborationErrorMessage(failure));
      void query.refetch();
    } finally {
      setBusy(false);
    }
  }
  function queue(action: ProviderInboxAction) {
    if (!snapshot) return;
    void submit({
      account_id: snapshot.account_id,
      authorization_epoch: snapshot.authorization_epoch,
      authorization_view: snapshot.authorization_view,
      subject_id: snapshot.subject_id,
      expected_activity_version: snapshot.activity_version,
      command_id: crypto.randomUUID(),
      action,
      activity_policy: "best_effort_current_item",
    });
  }
  return (
    <section
      className="space-y-2 rounded-lg border p-3 text-xs"
      aria-label="Provider inbox actions"
      aria-busy={busy}
    >
      <p className="font-medium">On {provider}</p>
      {query.isPending ? <p role="status">Checking saved actions…</p> : null}
      {query.isError ? (
        <div className="space-y-2">
          <p role="alert">{collaborationErrorMessage(query.error)}</p>
          <Button
            type="button"
            size="sm"
            variant="outline"
            disabled={busy}
            onClick={() => {
              void query.refetch();
            }}
          >
            Reload saved actions
          </Button>
        </div>
      ) : null}
      {available ? (
        <p className="text-muted-foreground">
          Saved now and sent when connected. This can also affect activity that
          arrives before delivery.
        </p>
      ) : null}
      {snapshot?.actions.map((descriptor) => (
        <div
          key={descriptor.action}
          className="flex flex-wrap items-center gap-2"
        >
          <Button
            type="button"
            size="sm"
            variant="outline"
            disabled={
              busy || retry !== null || descriptor.availability !== "available"
            }
            onClick={() => queue(descriptor.action)}
          >
            {descriptor.action === "mark_read"
              ? `Mark read on ${provider}`
              : `Mark done on ${provider}`}
          </Button>
          {descriptor.reason ? (
            <span className="text-muted-foreground">
              {reasons[descriptor.reason]}
            </span>
          ) : null}
        </div>
      ))}
      {error ? <p role="alert">{error}</p> : null}
      {retry ? (
        <Button
          type="button"
          size="sm"
          variant="outline"
          disabled={busy}
          onClick={() => {
            void submit(retry);
          }}
        >
          Retry saving this action
        </Button>
      ) : null}
      {saved ? (
        <p role="status">
          Saved locally. Open Saved changes to follow delivery.
        </p>
      ) : null}
    </section>
  );
}

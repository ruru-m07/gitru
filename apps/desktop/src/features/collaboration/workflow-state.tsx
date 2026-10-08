import {
  collaboration,
  collaborationErrorMessage,
  type RemoteAccount,
  type RemoteItemKind,
  type WorkflowStateRequest,
  type WorkflowStateSnapshot,
} from "@gitru/collaboration-client";
import {
  itemQueryOptions,
  workflowStateQueryOptions,
} from "@gitru/collaboration-client/react";
import { Badge } from "@gitru/ui/components/badge";
import { Button } from "@gitru/ui/components/button";
import { Checkbox } from "@gitru/ui/components/checkbox";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useId, useState } from "react";

type AvailableSnapshot = WorkflowStateSnapshot & {
  context: NonNullable<WorkflowStateSnapshot["context"]>;
  current_state: "open" | "closed";
};

type Review = {
  base: AvailableSnapshot;
  desiredState: "open" | "closed";
};

export function ResourceWorkflowState({
  account,
  subjectId,
  kind,
}: {
  account: RemoteAccount;
  subjectId: string;
  kind: Exclude<RemoteItemKind, "notification">;
}) {
  const query = useQuery(workflowStateQueryOptions(account, subjectId));
  const queryClient = useQueryClient();
  const consentId = useId();
  const [review, setReview] = useState<Review | null>(null);
  const [consent, setConsent] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const [retryRequest, setRetryRequest] = useState<WorkflowStateRequest | null>(
    null,
  );
  const [error, setError] = useState<string | null>(null);
  const [status, setStatus] = useState<string | null>(null);

  if (query.isPending) return null;
  if (query.isError && query.data === undefined)
    return (
      <p role="alert" className="mt-4 text-xs text-destructive-foreground">
        {collaborationErrorMessage(query.error)}
      </p>
    );
  if (!query.data) return null;

  const snapshot = query.data;
  const available = isAvailable(snapshot) ? snapshot : null;
  const baseChanged =
    review !== null &&
    (snapshot.context?.review_token !== review.base.context.review_token ||
      snapshot.context?.authorization_view !==
        review.base.context.authorization_view ||
      snapshot.context?.authorization_epoch !==
        review.base.context.authorization_epoch ||
      snapshot.current_state !== review.base.current_state);
  const targetLabel = kind === "pull_request" ? "pull request" : "issue";
  const actionLabel = review?.desiredState === "closed" ? "close" : "reopen";
  const retryAllowed = retryRequest !== null && !baseChanged;

  function begin(desiredState: "open" | "closed") {
    if (!available || query.isError) return;
    setReview({ base: available, desiredState });
    setConsent(false);
    setRetryRequest(null);
    setError(null);
    setStatus(null);
  }

  function loadCurrent() {
    if (!available) return;
    setReview({
      base: available,
      desiredState: available.current_state === "open" ? "closed" : "open",
    });
    setConsent(false);
    setRetryRequest(null);
    setError(null);
    setStatus(null);
  }

  async function submit(request: WorkflowStateRequest) {
    setSubmitting(true);
    setRetryRequest(request);
    setError(null);
    setStatus(null);
    try {
      const receipt = await collaboration
        .forAccount(account)
        .submitWorkflowState(request);
      setReview(null);
      setRetryRequest(null);
      setConsent(false);
      setStatus(
        receipt.duplicate
          ? "This exact status change was already queued."
          : "Status change saved locally and queued for delivery.",
      );
      await Promise.all([
        queryClient.invalidateQueries({
          queryKey: workflowStateQueryOptions(account, subjectId).queryKey,
        }),
        queryClient.invalidateQueries({
          queryKey: itemQueryOptions(account, subjectId).queryKey,
        }),
      ]);
      void collaboration.wake();
    } catch (failure) {
      setError(collaborationErrorMessage(failure));
      void queryClient.invalidateQueries({
        queryKey: workflowStateQueryOptions(account, subjectId).queryKey,
      });
    } finally {
      setSubmitting(false);
    }
  }

  function queue() {
    if (
      !review ||
      !consent ||
      retryRequest ||
      baseChanged ||
      query.isError ||
      submitting
    )
      return;
    void submit({
      context: { ...review.base.context },
      command_id: crypto.randomUUID(),
      desired_state: review.desiredState,
      accept_best_effort: true,
    });
  }

  return (
    <section
      className="mt-5 space-y-3 border-t pt-4"
      aria-label="Provider status"
    >
      <div className="flex flex-wrap items-center justify-between gap-2">
        <div>
          <h3 className="text-sm font-medium">Status</h3>
          <p className="mt-1 text-xs text-muted-foreground">
            Close or reopen this {targetLabel} through the connected provider.
          </p>
        </div>
        {available ? (
          <Badge variant="outline" size="sm">
            {available.current_state === "open" ? "Open" : "Closed"}
          </Badge>
        ) : null}
      </div>
      {query.isError ? (
        <p role="alert" className="text-xs text-destructive-foreground">
          {collaborationErrorMessage(query.error)} New status changes are
          disabled until the saved context reloads.
        </p>
      ) : null}
      {!review && available ? (
        <Button
          type="button"
          size="sm"
          variant="outline"
          disabled={query.isError}
          onClick={() =>
            begin(available.current_state === "open" ? "closed" : "open")
          }
        >
          {available.current_state === "open" ? "Close" : "Reopen"}{" "}
          {targetLabel}
        </Button>
      ) : null}
      {!available ? (
        <p className="text-xs text-muted-foreground">
          {unavailableMessage(snapshot)}
        </p>
      ) : null}
      {review ? (
        <div className="space-y-3 rounded-md border p-3">
          <p className="text-sm">
            Queue a request to {actionLabel} this {targetLabel}?
          </p>
          <div className="flex items-start gap-2 text-xs text-muted-foreground">
            <Checkbox
              aria-labelledby={consentId}
              checked={consent}
              disabled={submitting || retryRequest !== null}
              onCheckedChange={(checked) => setConsent(checked === true)}
            />
            <span id={consentId}>
              I understand GitHub applies this as a best-effort update. A
              provider change after Gitru checks the item may win, or this
              change may overwrite it.
            </span>
          </div>
          {baseChanged ? (
            <div className="space-y-2 text-xs text-muted-foreground">
              <p>
                This item changed after you started. Load its latest saved
                status before queuing another request.
              </p>
              {available && !query.isError ? (
                <Button
                  type="button"
                  size="sm"
                  variant="outline"
                  onClick={loadCurrent}
                >
                  Load latest saved status
                </Button>
              ) : null}
            </div>
          ) : null}
          {error ? (
            <div className="space-y-2">
              <p role="alert" className="text-xs text-destructive-foreground">
                {error} Your exact request identity is preserved.
              </p>
              {retryAllowed ? (
                <Button
                  type="button"
                  size="sm"
                  variant="outline"
                  disabled={submitting}
                  onClick={() => void submit(retryRequest)}
                >
                  {submitting ? "Retrying…" : "Retry exact status change"}
                </Button>
              ) : null}
            </div>
          ) : null}
          <div className="flex flex-wrap justify-end gap-2">
            <Button
              type="button"
              size="sm"
              variant="ghost"
              disabled={submitting}
              onClick={() => {
                setReview(null);
                setConsent(false);
                setRetryRequest(null);
                setError(null);
              }}
            >
              Cancel
            </Button>
            <Button
              type="button"
              size="sm"
              disabled={
                submitting ||
                retryRequest !== null ||
                baseChanged ||
                query.isError ||
                !consent
              }
              onClick={queue}
            >
              {submitting ? "Saving locally…" : `Save and queue ${actionLabel}`}
            </Button>
          </div>
        </div>
      ) : null}
      {status ? (
        <p role="status" className="text-xs text-muted-foreground">
          {status}
        </p>
      ) : null}
    </section>
  );
}

function isAvailable(
  snapshot: WorkflowStateSnapshot,
): snapshot is AvailableSnapshot {
  return (
    snapshot.availability === "available" &&
    snapshot.context !== null &&
    (snapshot.current_state === "open" || snapshot.current_state === "closed")
  );
}

function unavailableMessage(snapshot: WorkflowStateSnapshot) {
  switch (snapshot.reason) {
    case "unsupported_provider":
      return "Changing status is not available for this provider yet.";
    case "account_unavailable":
      return "Reconnect this account before changing provider status.";
    case "missing_target":
      return "Sync this item before changing its provider status.";
    case "unknown_state":
      return "The saved provider status is unknown. Sync this item before changing it.";
    case "merged_pull_request":
      return "Merged pull requests cannot be reopened.";
    case "pending_intent":
      return "A status change is already tracked in Saved changes.";
    default:
      return "Changing status is unavailable for this saved item.";
  }
}

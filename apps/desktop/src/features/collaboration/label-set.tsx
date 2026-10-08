import {
  collaboration,
  collaborationErrorMessage,
  type LabelIdentity,
  type LabelSetRequest,
  type LabelSetSnapshot,
  type RemoteAccount,
} from "@gitru/collaboration-client";
import {
  itemQueryOptions,
  labelSetQueryOptions,
} from "@gitru/collaboration-client/react";
import { Badge } from "@gitru/ui/components/badge";
import { Button } from "@gitru/ui/components/button";
import { Checkbox } from "@gitru/ui/components/checkbox";
import { useQueryClient } from "@tanstack/react-query";
import { useId, useState } from "react";

type AvailableSnapshot = LabelSetSnapshot & {
  context: NonNullable<LabelSetSnapshot["context"]>;
};

type Review = {
  base: AvailableSnapshot;
  selectedIds: readonly string[];
};

export function ResourceLabelSet({
  account,
  subjectId,
  snapshot,
  pending,
  queryError,
}: {
  account: RemoteAccount;
  subjectId: string;
  snapshot: LabelSetSnapshot | undefined;
  pending: boolean;
  queryError: unknown | null;
}) {
  const queryClient = useQueryClient();
  const consentId = useId();
  const [review, setReview] = useState<Review | null>(null);
  const [consent, setConsent] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const [retryRequest, setRetryRequest] = useState<LabelSetRequest | null>(
    null,
  );
  const [error, setError] = useState<string | null>(null);
  const [status, setStatus] = useState<string | null>(null);

  if (pending) return null;
  if (!snapshot && queryError)
    return (
      <p role="alert" className="mt-4 text-xs text-destructive-foreground">
        {collaborationErrorMessage(queryError)}
      </p>
    );
  if (!snapshot) return null;

  const available = isAvailable(snapshot) ? snapshot : null;
  const choices = review ? choicesFor(review.base) : [];
  const baseChanged =
    review !== null &&
    (snapshot.context?.review_token !== review.base.context.review_token ||
      snapshot.context?.authorization_view !==
        review.base.context.authorization_view ||
      snapshot.context?.authorization_epoch !==
        review.base.context.authorization_epoch ||
      labelFingerprint(snapshot.canonical_labels) !==
        labelFingerprint(review.base.canonical_labels));
  const delta = review ? deltaFor(review) : null;
  const tooMany = delta !== null && delta.touched > 32;
  const retryAllowed =
    retryRequest !== null &&
    retryRequest.context.account_id === account.id &&
    retryRequest.context.subject_id === subjectId &&
    retryRequest.context.authorization_epoch === account.authorization_epoch;

  function begin() {
    if (!available || queryError) return;
    setReview({
      base: available,
      selectedIds: available.canonical_labels.map((label) => label.provider_id),
    });
    setConsent(false);
    setRetryRequest(null);
    setError(null);
    setStatus(null);
  }

  function loadCurrent() {
    if (!available) return;
    setReview({
      base: available,
      selectedIds: available.canonical_labels.map((label) => label.provider_id),
    });
    setConsent(false);
    setRetryRequest(null);
    setError(null);
    setStatus(null);
  }

  function toggle(providerId: string, checked: boolean) {
    setReview((current) => {
      if (!current || retryRequest) return current;
      const selected = new Set(current.selectedIds);
      if (checked) selected.add(providerId);
      else selected.delete(providerId);
      return { ...current, selectedIds: [...selected].sort(compareIds) };
    });
    setConsent(false);
    setError(null);
  }

  async function submit(request: LabelSetRequest) {
    setSubmitting(true);
    setRetryRequest(request);
    setError(null);
    setStatus(null);
    try {
      const receipt = await collaboration
        .forAccount(account)
        .submitLabelSet(request);
      setReview(null);
      setRetryRequest(null);
      setConsent(false);
      setStatus(
        receipt.duplicate
          ? "This exact label change was already queued."
          : "Label change saved locally and queued for delivery.",
      );
      await Promise.all([
        queryClient.invalidateQueries({
          queryKey: labelSetQueryOptions(account, subjectId).queryKey,
        }),
        queryClient.invalidateQueries({
          queryKey: itemQueryOptions(account, subjectId).queryKey,
        }),
      ]);
      void collaboration.wake();
    } catch (failure) {
      setError(collaborationErrorMessage(failure));
      void queryClient.invalidateQueries({
        queryKey: labelSetQueryOptions(account, subjectId).queryKey,
      });
    } finally {
      setSubmitting(false);
    }
  }

  function queue() {
    if (
      !review ||
      !delta ||
      delta.touched === 0 ||
      tooMany ||
      !consent ||
      retryRequest ||
      baseChanged ||
      queryError ||
      submitting
    )
      return;
    void submit({
      context: { ...review.base.context },
      command_id: crypto.randomUUID(),
      add_labels: delta.add.map((label) => ({ ...label })),
      remove_labels: delta.remove.map((label) => ({ ...label })),
      accept_best_effort: true,
    });
  }

  return (
    <section
      className="mt-5 space-y-3 border-t pt-4"
      aria-label="Provider labels"
    >
      <div className="flex flex-wrap items-start justify-between gap-2">
        <div>
          <h3 className="text-sm font-medium">Labels</h3>
          <p className="mt-1 text-xs text-muted-foreground">
            Choose from labels saved on this device.
          </p>
        </div>
        {snapshot.pending_intent ? (
          <Badge variant="secondary" size="sm">
            Queued
          </Badge>
        ) : null}
      </div>
      <p className="text-xs text-muted-foreground">
        Only labels already seen in this repository are shown.
        {snapshot.catalog_truncated
          ? " Some saved choices are hidden by the local display limit."
          : ""}
      </p>
      {queryError ? (
        <p role="alert" className="text-xs text-destructive-foreground">
          {collaborationErrorMessage(queryError)} New label changes are disabled
          until the saved context reloads.
        </p>
      ) : null}
      {!review && available ? (
        <Button
          type="button"
          size="sm"
          variant="outline"
          disabled={queryError !== null}
          onClick={begin}
        >
          Edit labels
        </Button>
      ) : null}
      {!available ? (
        <p className="text-xs text-muted-foreground">
          {unavailableMessage(snapshot)}
        </p>
      ) : null}
      {review ? (
        <div className="space-y-3 rounded-md border p-3">
          <fieldset
            className="space-y-2"
            disabled={submitting || !!retryRequest}
          >
            <legend className="text-sm font-medium">Saved label choices</legend>
            {choices.length ? (
              <div className="grid gap-2 sm:grid-cols-2">
                {choices.map((label) => {
                  const labelId = `${consentId}-${label.provider_id}`;
                  return (
                    <div
                      key={label.provider_id}
                      className="flex min-w-0 items-start gap-2 rounded-md border px-2 py-1.5 text-sm"
                    >
                      <Checkbox
                        aria-labelledby={labelId}
                        disabled={submitting || retryRequest !== null}
                        checked={review.selectedIds.includes(label.provider_id)}
                        onCheckedChange={(checked) =>
                          toggle(label.provider_id, checked === true)
                        }
                      />
                      <span id={labelId} className="min-w-0 break-words">
                        {label.name}
                      </span>
                    </div>
                  );
                })}
              </div>
            ) : (
              <p className="text-xs text-muted-foreground">
                No saved labels are available.
              </p>
            )}
          </fieldset>
          {delta?.touched ? (
            <p className="text-xs text-muted-foreground">
              {delta.add.length} to add · {delta.remove.length} to remove
            </p>
          ) : (
            <p className="text-xs text-muted-foreground">
              No changes selected.
            </p>
          )}
          {tooMany ? (
            <p role="alert" className="text-xs text-destructive-foreground">
              One request can change at most 32 labels.
            </p>
          ) : null}
          <div className="flex items-start gap-2 text-xs text-muted-foreground">
            <Checkbox
              aria-labelledby={`${consentId}-consent`}
              checked={consent}
              disabled={submitting || retryRequest !== null || !delta?.touched}
              onCheckedChange={(checked) => setConsent(checked === true)}
            />
            <span id={`${consentId}-consent`}>
              I understand GitHub applies label changes by name without a
              compare-and-swap token. A simultaneous rename or name reuse can
              change the outcome, and an interrupted multi-step change may be
              only partly applied.
            </span>
          </div>
          {baseChanged ? (
            <div className="space-y-2 text-xs text-muted-foreground">
              <p>
                Labels changed after you started. Load the latest saved labels
                before queuing another request.
              </p>
              {available && !queryError && retryRequest === null ? (
                <Button
                  type="button"
                  size="sm"
                  variant="outline"
                  onClick={loadCurrent}
                >
                  Load latest saved labels
                </Button>
              ) : null}
            </div>
          ) : null}
          {error ? (
            <div className="space-y-2">
              <p role="alert" className="text-xs text-destructive-foreground">
                {error} You can retry this change safely.
              </p>
              {retryAllowed ? (
                <Button
                  type="button"
                  size="sm"
                  variant="outline"
                  disabled={submitting}
                  onClick={() => void submit(retryRequest)}
                >
                  {submitting ? "Retrying…" : "Retry exact label change"}
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
                queryError !== null ||
                !consent ||
                !delta?.touched ||
                tooMany
              }
              onClick={queue}
            >
              {submitting ? "Saving locally…" : "Save and queue label changes"}
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
  snapshot: LabelSetSnapshot,
): snapshot is AvailableSnapshot {
  return snapshot.availability === "available" && snapshot.context !== null;
}

function choicesFor(snapshot: AvailableSnapshot) {
  const choices = new Map<string, LabelIdentity>();
  for (const label of [
    ...snapshot.canonical_labels,
    ...snapshot.available_labels,
  ])
    if (!choices.has(label.provider_id)) choices.set(label.provider_id, label);
  return [...choices.values()].sort(compareLabels);
}

function deltaFor(review: Review) {
  const selected = new Set(review.selectedIds);
  const canonical = new Set(
    review.base.canonical_labels.map((label) => label.provider_id),
  );
  const choices = choicesFor(review.base);
  const add = choices
    .filter(
      (label) =>
        selected.has(label.provider_id) && !canonical.has(label.provider_id),
    )
    .sort(compareLabels);
  const remove = review.base.canonical_labels
    .filter((label) => !selected.has(label.provider_id))
    .sort(compareLabels);
  return { add, remove, touched: add.length + remove.length };
}

function compareIds(left: string, right: string) {
  const a = BigInt(left);
  const b = BigInt(right);
  return a < b ? -1 : a > b ? 1 : 0;
}

function compareLabels(left: LabelIdentity, right: LabelIdentity) {
  return compareIds(left.provider_id, right.provider_id);
}

function labelFingerprint(labels: readonly LabelIdentity[]) {
  return labels
    .map(
      (label) =>
        `${label.provider_id}\u0000${label.name}\u0000${label.color ?? ""}`,
    )
    .join("\u0001");
}

function unavailableMessage(snapshot: LabelSetSnapshot) {
  switch (snapshot.reason) {
    case "unsupported_provider":
      return "Changing labels is not available for this provider yet.";
    case "account_unavailable":
      return "Reconnect this account before changing labels.";
    case "missing_labels":
      return "Sync this item before changing its labels.";
    case "oversized_labels":
      return "This item has more labels than the local editing limit.";
    case "pending_intent":
      return "A label change is already tracked in Saved changes.";
    default:
      return "Changing labels is unavailable for this saved item.";
  }
}

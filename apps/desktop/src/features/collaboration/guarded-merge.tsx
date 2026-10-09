import {
  collaboration,
  collaborationErrorMessage,
  type GuardedMergePreview,
  type GuardedMergeRequest,
  type GuardedMergeSnapshot,
  type RemoteAccount,
} from "@gitru/collaboration-client";
import {
  guardedMergeQueryOptions,
  itemQueryOptions,
} from "@gitru/collaboration-client/react";
import { Button } from "@gitru/ui/components/button";
import { Checkbox } from "@gitru/ui/components/checkbox";
import { Label } from "@gitru/ui/components/label";
import { Radio, RadioGroup } from "@gitru/ui/components/radio-group";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useId, useState } from "react";

type Props = {
  account: RemoteAccount;
  subjectId: string;
  headOid: string | null;
};
type Reason = NonNullable<GuardedMergePreview["reason"]>;
type Method = GuardedMergeRequest["method"];
const methods: Record<Method, string> = {
  merge: "Merge commit",
  squash: "Squash and merge",
  rebase: "Rebase and merge",
};

export function GuardedPullMerge(props: Props) {
  return (
    <MergeSession
      key={JSON.stringify([
        props.account.id,
        props.account.actor_id,
        props.account.authorization_epoch,
        props.subjectId,
        props.headOid,
      ])}
      {...props}
    />
  );
}
function MergeSession({ account, subjectId, headOid }: Props) {
  const query = useQuery(guardedMergeQueryOptions(account, subjectId));
  const cache = useQueryClient();
  const consentId = useId();
  const [preview, setPreview] = useState<GuardedMergePreview | null>(null);
  const [method, setMethod] = useState<Method | null>(null);
  const [consent, setConsent] = useState(false);
  const [expired, setExpired] = useState(false);
  const [busy, setBusy] = useState(false);
  const [retry, setRetry] = useState<GuardedMergeRequest | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  // This timer only disables the presentation; native monotonic grants authorize.
  useEffect(() => {
    if (!preview?.context) return;
    const timer = setTimeout(
      () => setExpired(true),
      preview.expires_in_seconds * 1000,
    );
    return () => clearTimeout(timer);
  }, [preview]);
  const current =
    account.state === "active" && !query.isError && query.data !== undefined;
  const reviewCurrent =
    current &&
    preview?.context != null &&
    query.data?.authorization_view === preview.authorization_view &&
    preview.context.authorization_epoch === account.authorization_epoch &&
    preview.expected_head === headOid;
  const ready = reviewCurrent && query.data?.reason === null && !expired;
  const retryAllowed =
    current &&
    retry !== null &&
    retry.context.authorization_view === query.data?.authorization_view &&
    retry.context.expected_head === headOid &&
    (query.data?.reason === null || query.data?.reason === "pending_command");
  async function checkOnline() {
    if (!current || busy || query.data?.reason !== null) return;
    setBusy(true);
    setError(null);
    setNotice(null);
    setPreview(null);
    setRetry(null);
    setConsent(false);
    setMethod(null);
    setExpired(false);
    try {
      setPreview(
        await collaboration.forAccount(account).previewGuardedMerge(subjectId),
      );
    } catch (failure) {
      setError(collaborationErrorMessage(failure));
    } finally {
      setBusy(false);
    }
  }
  async function submit(request: GuardedMergeRequest) {
    setBusy(true);
    setRetry(request);
    setError(null);
    try {
      const receipt = await collaboration
        .forAccount(account)
        .submitGuardedMerge(request);
      setRetry(null);
      setPreview(null);
      setConsent(false);
      setNotice(
        receipt.duplicate
          ? "The exact merge request is already recorded locally."
          : "Merge request recorded locally. Awaiting GitHub confirmation.",
      );
      await Promise.all([
        cache.invalidateQueries({
          queryKey: guardedMergeQueryOptions(account, subjectId).queryKey,
        }),
        cache.invalidateQueries({
          queryKey: itemQueryOptions(account, subjectId).queryKey,
        }),
      ]);
      void collaboration.wake();
    } catch (failure) {
      setError(collaborationErrorMessage(failure));
    } finally {
      setBusy(false);
    }
  }
  function merge() {
    if (
      !ready ||
      !preview?.context ||
      method === null ||
      !preview.methods.includes(method) ||
      !consent ||
      retry !== null ||
      busy
    )
      return;
    void submit({
      context: { ...preview.context },
      command_id: crypto.randomUUID(),
      method,
      confirm_inspected_head: true,
    });
  }
  if (query.isPending) return null;
  return (
    <section
      aria-label="Merge pull request"
      className="mt-5 space-y-3 border-t pt-4"
    >
      <h3 className="text-sm font-medium">Merge pull request</h3>
      <p className="text-xs text-muted-foreground">
        GitHub.com direct merge requires a fresh online check and your exact
        head confirmation. Stacked merges, merge queues and auto-merge are
        unavailable.
      </p>
      {current && query.data.latest ? (
        <p role="status" className="text-xs">
          {statusMessage(query.data)}
        </p>
      ) : null}
      {query.isError ? (
        <p role="alert" className="text-xs text-destructive-foreground">
          {collaborationErrorMessage(query.error)}
        </p>
      ) : null}
      {current && query.data.reason !== null ? (
        <p className="text-xs text-muted-foreground">
          {reasonMessage(query.data.reason)}
        </p>
      ) : null}
      <Button
        type="button"
        size="sm"
        variant="outline"
        disabled={!current || query.data?.reason !== null || busy}
        onClick={() => void checkOnline()}
      >
        {busy && !retry ? "Checking…" : "Check merge online"}
      </Button>
      {preview &&
      current &&
      query.data.authorization_view === preview.authorization_view ? (
        <div className="space-y-3 rounded-md border p-3">
          {preview.reason ? (
            <p className="text-xs">{reasonMessage(preview.reason)}</p>
          ) : (
            <>
              <p className="text-xs">
                GitHub currently reports push permission and clean mergeability.
                The provider enforces its rules again when merging.
              </p>
              <p className="break-all text-xs">
                Inspected head: <code>{preview.expected_head}</code>
              </p>
              <p className="text-xs text-muted-foreground">
                Review the saved checks and reviews below. Their coverage can be
                partial or stale; they do not authorize a merge.
              </p>
              <fieldset
                disabled={busy || retry !== null || !ready}
                className="space-y-2"
              >
                <legend className="text-xs font-medium">Merge method</legend>
                <RadioGroup
                  aria-label="Merge method"
                  value={method}
                  disabled={busy || retry !== null || !ready}
                  onValueChange={(value) => {
                    if (preview.methods.includes(value as Method))
                      setMethod(value as Method);
                  }}
                >
                  {preview.methods.map((value) => (
                    <Label
                      key={value}
                      className="flex items-center gap-2 text-xs"
                    >
                      <Radio value={value} aria-label={methods[value]} />
                      {methods[value]}
                    </Label>
                  ))}
                </RadioGroup>
              </fieldset>
              <div className="flex items-start gap-2 text-xs">
                <Checkbox
                  aria-labelledby={consentId}
                  checked={consent}
                  disabled={busy || retry !== null || !ready}
                  onCheckedChange={(value) => setConsent(value === true)}
                />
                <span id={consentId}>
                  I inspected this head and want to merge it now using the
                  selected method.
                </span>
              </div>
              {!reviewCurrent ? (
                <p role="status" className="text-xs">
                  The account or saved head changed. Check online again.
                </p>
              ) : expired ? (
                <p role="status" className="text-xs">
                  This online preview expired. Check online again.
                </p>
              ) : null}
              <Button
                type="button"
                size="sm"
                disabled={
                  !ready || !method || !consent || busy || retry !== null
                }
                onClick={merge}
              >
                Merge inspected head
              </Button>
            </>
          )}
        </div>
      ) : null}
      {error && current ? (
        <div className="space-y-2">
          <p role="alert" className="text-xs text-destructive-foreground">
            {error}
          </p>
          {retryAllowed ? (
            <Button
              type="button"
              size="sm"
              variant="outline"
              disabled={busy}
              onClick={() => retry && void submit(retry)}
            >
              Recover exact local receipt
            </Button>
          ) : null}
        </div>
      ) : null}
      {notice && current ? (
        <p role="status" className="text-xs">
          {notice}
        </p>
      ) : null}
      <p className="text-xs text-muted-foreground">
        A lost or accepted response is reconciled without sending another merge.
        Local recording never means the pull request has merged. Commands shows
        pending or uncertain outcomes.
      </p>
    </section>
  );
}
function statusMessage(s: GuardedMergeSnapshot) {
  switch (s.latest?.state) {
    case "confirmed":
      return "GitHub confirmed the recorded head is merged.";
    case "accepted":
      return "GitHub accepted the request; merge confirmation is pending.";
    case "outcome_unknown":
      return "Merge outcome is uncertain; Gitru will only check its result.";
    case "queued":
    case "sending":
      return "Online merge request is pending.";
    case "conflict":
      return "Merge stopped because the inspected context or online consent changed.";
    case "rejected":
      return "GitHub declined this merge request.";
    default:
      return "This merge request needs attention in Commands.";
  }
}
function reasonMessage(reason: Reason) {
  const messages: Record<Reason, string> = {
    unsupported_provider:
      "Direct merge is available only for connected GitHub.com accounts.",
    account_unavailable: "Reconnect this account before merging.",
    missing_context:
      "Sync this pull request’s body and head before checking merge.",
    pending_command: "Wait for or resolve pending commands before merging.",
    head_changed:
      "GitHub reports a different head. Sync and inspect it before merging.",
    not_open: "This pull request is no longer open.",
    draft: "Publish the draft on GitHub before merging.",
    permission_unavailable: "Current push permission could not be verified.",
    mergeability_unavailable:
      "GitHub did not report clean mergeability. Resolve checks, reviews or conflicts on GitHub, then check again.",
    methods_unavailable: "The selected merge method is no longer available.",
    automatic_merge_unsupported:
      "Existing or unknown auto-merge settings are unsupported here.",
    consent_expired: "This online preview expired. Check online again.",
    provider_conflict:
      "GitHub rejected the expected head. Sync and inspect it again.",
    provider_rejected: "GitHub declined the merge request.",
  };
  return messages[reason];
}

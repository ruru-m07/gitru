import {
  type CommentDraftSnapshot,
  type CommentSendReason,
  type CreatedCommentReceipt,
  collaboration,
  collaborationErrorMessage,
  collaborationKeys,
  type RemoteAccount,
  type SendCommentRequest,
} from "@gitru/collaboration-client";
import {
  commentDraftQueryOptions,
  createdCommentsQueryOptions,
} from "@gitru/collaboration-client/react";
import { Button } from "@gitru/ui/components/button";
import { Checkbox } from "@gitru/ui/components/checkbox";
import {
  Field,
  FieldDescription,
  FieldLabel,
} from "@gitru/ui/components/field";
import { Textarea } from "@gitru/ui/components/textarea";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { type FormEvent, useId, useState } from "react";
import { ProviderLink } from "./provider-link";

const MAX_COMMENT_BYTES = 16 * 1024;
const RECEIPT_LIMIT = 25;
const CURSOR_LIMIT = 100;

const reasons: Record<CommentSendReason, string> = {
  unsupported_provider:
    "Sending comments from Gitru is not available for this provider. Your comment draft stays on this device.",
  account_unavailable:
    "Reconnect this account before queuing the saved comment.",
  missing_target:
    "The provider target is unavailable. Your comment draft stays on this device.",
  empty_draft: "Write and save a comment before sending.",
  already_submitted:
    "This saved comment version was already submitted. Edit and save a new version to comment again.",
  pending_submission:
    "A submitted comment is still being tracked. Review Saved changes before sending another.",
};

export function CommentComposer({
  account,
  subjectId,
}: {
  account: RemoteAccount;
  subjectId: string;
}) {
  const query = useQuery(commentDraftQueryOptions(account, subjectId));
  if (query.isPending) {
    return (
      <p role="status" className="text-xs text-muted-foreground">
        Loading comment draft…
      </p>
    );
  }
  if (query.isError && query.data === undefined) {
    return (
      <p role="alert" className="text-xs text-destructive-foreground">
        {collaborationErrorMessage(query.error)}
      </p>
    );
  }
  if (!query.data) return null;
  return (
    <section className="space-y-4 border-t pt-4" aria-label="Comment composer">
      <div>
        <h3 className="text-sm font-medium">Comment draft</h3>
        <p className="mt-1 text-xs text-muted-foreground">
          Separate from your Private note. Saving never sends it to the
          provider.
        </p>
      </div>
      {query.isError ? (
        <p role="alert" className="text-xs text-destructive-foreground">
          {collaborationErrorMessage(query.error)}
        </p>
      ) : null}
      <CommentDraftForm
        account={account}
        subjectId={subjectId}
        initialSnapshot={query.data}
        currentSnapshot={query.data}
        authorityUnavailable={query.isError}
      />
      <CreatedCommentHistory account={account} subjectId={subjectId} />
    </section>
  );
}

function CommentDraftForm({
  account,
  subjectId,
  initialSnapshot,
  currentSnapshot,
  authorityUnavailable,
}: {
  account: RemoteAccount;
  subjectId: string;
  initialSnapshot: CommentDraftSnapshot;
  currentSnapshot: CommentDraftSnapshot;
  authorityUnavailable: boolean;
}) {
  const bodyId = useId();
  const consentId = useId();
  const queryClient = useQueryClient();
  const [base, setBase] = useState(initialSnapshot);
  const [body, setBody] = useState(initialSnapshot.body);
  const [savedBody, setSavedBody] = useState(initialSnapshot.body);
  const [consent, setConsent] = useState(false);
  const [saving, setSaving] = useState(false);
  const [sending, setSending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [status, setStatus] = useState<string | null>(null);
  const [retryRequest, setRetryRequest] = useState<SendCommentRequest | null>(
    null,
  );
  const [admittedGeneration, setAdmittedGeneration] = useState<string | null>(
    null,
  );
  const [previousBody, setPreviousBody] = useState<string | null>(null);
  const bodyError = validateBody(body);
  const dirty = body !== savedBody;
  const baseChanged =
    currentSnapshot.generation !== base.generation ||
    currentSnapshot.authorization_view !== base.authorization_view;
  const retryObserved =
    retryRequest !== null &&
    currentSnapshot.submission?.command_id === retryRequest.command_id;
  const activeRetry = retryObserved ? null : retryRequest;
  const retryAllowed =
    activeRetry !== null &&
    !baseChanged &&
    !authorityUnavailable &&
    currentSnapshot.reason !== "account_unavailable";
  const canSend =
    !saving &&
    !sending &&
    !authorityUnavailable &&
    !dirty &&
    !baseChanged &&
    body.length > 0 &&
    base.generation !== "0" &&
    currentSnapshot.availability === "available" &&
    currentSnapshot.context !== null &&
    currentSnapshot.submission === null &&
    consent &&
    activeRetry === null &&
    admittedGeneration !== base.generation &&
    bodyError === null;

  function acceptSnapshot(snapshot: CommentDraftSnapshot) {
    queryClient.setQueryData(
      commentDraftQueryOptions(account, subjectId).queryKey,
      snapshot,
    );
    setBase(snapshot);
    setBody(snapshot.body);
    setSavedBody(snapshot.body);
    setConsent(false);
    setAdmittedGeneration((generation) =>
      generation === snapshot.generation ? generation : null,
    );
  }

  async function save(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (
      saving ||
      sending ||
      authorityUnavailable ||
      activeRetry ||
      baseChanged ||
      !dirty ||
      bodyError
    )
      return;
    setSaving(true);
    setError(null);
    setStatus(null);
    try {
      const snapshot = await collaboration
        .forAccount(account)
        .saveCommentDraft({
          subject_id: subjectId,
          authorization_view: base.authorization_view,
          expected_generation: base.generation,
          body,
        });
      acceptSnapshot(snapshot);
      setStatus("Comment draft saved on this device.");
    } catch (failure) {
      setError(collaborationErrorMessage(failure));
      void queryClient.invalidateQueries({
        queryKey: commentDraftQueryOptions(account, subjectId).queryKey,
      });
    } finally {
      setSaving(false);
    }
  }

  async function send(request: SendCommentRequest) {
    setSending(true);
    setError(null);
    setStatus(null);
    setRetryRequest(request);
    try {
      const receipt = await collaboration
        .forAccount(account)
        .sendComment(request);
      setRetryRequest(null);
      setAdmittedGeneration(request.draft_generation);
      setStatus(
        receipt.duplicate
          ? "This exact saved comment was already queued."
          : "Comment saved locally and queued for background delivery.",
      );
      void queryClient.invalidateQueries({
        queryKey: commentDraftQueryOptions(account, subjectId).queryKey,
      });
      void queryClient.invalidateQueries({
        queryKey: [
          ...collaborationKeys.account(account.id),
          account.authorization_epoch,
          "created-comments",
        ],
      });
      void collaboration.wake();
    } catch (failure) {
      setError(collaborationErrorMessage(failure));
      void queryClient.invalidateQueries({
        queryKey: commentDraftQueryOptions(account, subjectId).queryKey,
      });
    } finally {
      setSending(false);
    }
  }

  function queue() {
    if (!canSend || !currentSnapshot.context) return;
    void send({
      context: { ...currentSnapshot.context },
      draft_generation: base.generation,
      command_id: crypto.randomUUID(),
      accept_background_delivery: true,
    });
  }

  function loadCurrent() {
    if (dirty) setPreviousBody(body);
    acceptSnapshot(currentSnapshot);
    setRetryRequest(null);
    setError(null);
    setStatus(null);
  }

  return (
    <form className="space-y-3" onSubmit={save}>
      <Field name="comment-draft" invalid={bodyError !== null}>
        <FieldLabel htmlFor={bodyId}>Comment</FieldLabel>
        <Textarea
          id={bodyId}
          name="comment-draft"
          value={body}
          disabled={saving || sending || activeRetry !== null}
          aria-invalid={bodyError !== null}
          onChange={(event) => setBody(event.currentTarget.value)}
          rows={5}
          placeholder="Write a comment…"
        />
        <FieldDescription>
          {utf8Size(body).toLocaleString()}/{MAX_COMMENT_BYTES.toLocaleString()}{" "}
          bytes
        </FieldDescription>
        {bodyError ? (
          <p role="alert" className="text-xs text-destructive-foreground">
            {bodyError}
          </p>
        ) : null}
      </Field>
      {baseChanged ? (
        <div className="space-y-2 text-xs text-muted-foreground">
          <p>
            This comment draft or account context changed. Your text is still
            here; load the latest saved draft before continuing.
          </p>
          {!authorityUnavailable ? (
            <Button
              type="button"
              size="sm"
              variant="outline"
              onClick={loadCurrent}
            >
              Load latest comment draft
            </Button>
          ) : null}
        </div>
      ) : null}
      {previousBody !== null ? (
        <details className="text-xs text-muted-foreground">
          <summary className="cursor-pointer">
            Your previous comment text
          </summary>
          <p className="mt-2 whitespace-pre-wrap break-words">
            {previousBody || "Empty comment"}
          </p>
        </details>
      ) : null}
      <div className="flex items-start gap-2 text-xs text-muted-foreground">
        <Checkbox
          aria-labelledby={consentId}
          checked={consent}
          disabled={saving || sending || activeRetry !== null}
          onCheckedChange={(checked) => setConsent(checked === true)}
        />
        <span id={consentId}>
          I understand this exact saved comment may be delivered in the
          background after I reconnect.
        </span>
      </div>
      {currentSnapshot.reason ? (
        <p className="text-xs text-muted-foreground">
          {reasons[currentSnapshot.reason]}
        </p>
      ) : null}
      {currentSnapshot.submission ? (
        <p className="text-xs text-muted-foreground">
          {currentSnapshot.submission.quarantined
            ? "This submitted comment was restored and needs review in Saved changes."
            : "This submitted comment is tracked in Saved changes."}
        </p>
      ) : null}
      {error ? (
        <div className="space-y-2">
          <p role="alert" className="text-xs text-destructive-foreground">
            {error} Your comment text
            {activeRetry ? " and request identity are" : " is"} preserved.
          </p>
          {retryAllowed ? (
            <Button
              type="button"
              size="sm"
              variant="outline"
              disabled={sending}
              onClick={() => void send(activeRetry)}
            >
              {sending ? "Retrying…" : "Retry exact comment"}
            </Button>
          ) : null}
        </div>
      ) : null}
      {retryObserved || status ? (
        <p role="status" className="text-xs text-muted-foreground">
          {retryObserved
            ? "This exact saved comment is tracked in Saved changes."
            : status}
        </p>
      ) : null}
      <div className="flex flex-wrap justify-end gap-2">
        <Button
          type="submit"
          size="sm"
          variant="outline"
          disabled={
            saving ||
            sending ||
            authorityUnavailable ||
            activeRetry !== null ||
            baseChanged ||
            !dirty ||
            bodyError !== null
          }
        >
          {saving ? "Saving…" : "Save comment draft"}
        </Button>
        <Button type="button" size="sm" disabled={!canSend} onClick={queue}>
          {sending ? "Queuing…" : "Queue saved comment"}
        </Button>
      </div>
    </form>
  );
}

function CreatedCommentHistory({
  account,
  subjectId,
}: {
  account: RemoteAccount;
  subjectId: string;
}) {
  const [open, setOpen] = useState(false);
  const [cursors, setCursors] = useState<Array<string | null>>([null]);
  const cursor = cursors.at(-1) ?? null;
  const query = useQuery({
    ...createdCommentsQueryOptions(account, {
      subject_id: subjectId,
      cursor,
      limit: RECEIPT_LIMIT,
    }),
    enabled: open,
  });
  const next = query.data?.next_cursor ?? null;
  const repeated = next !== null && cursors.includes(next);
  const capped = cursors.length >= CURSOR_LIMIT;
  return (
    <details
      open={open}
      onToggle={(event) => setOpen(event.currentTarget.open)}
      className="border-t pt-3"
    >
      <summary className="cursor-pointer text-sm font-medium">
        Submitted from Gitru
      </summary>
      <div className="mt-3 space-y-3">
        <p className="text-xs text-muted-foreground">
          Validated local creation receipts only. This is not the full provider
          conversation.
        </p>
        {query.isPending || query.isFetching ? (
          <p role="status" className="text-xs text-muted-foreground">
            Loading submitted comments…
          </p>
        ) : query.isError ? (
          <p role="alert" className="text-xs text-destructive-foreground">
            {collaborationErrorMessage(query.error)}
          </p>
        ) : query.data?.comments.length ? (
          <ul className="space-y-4" aria-label="Comments submitted from Gitru">
            {query.data.comments.map((comment) => (
              <CreatedCommentRow key={comment.command_id} comment={comment} />
            ))}
          </ul>
        ) : (
          <p className="text-xs text-muted-foreground">
            No confirmed comment submissions are saved for this item.
          </p>
        )}
        <nav
          className="flex flex-wrap items-center gap-2"
          aria-label="Submitted comment pages"
        >
          <Button
            type="button"
            size="sm"
            variant="outline"
            disabled={cursors.length === 1 || query.isFetching}
            onClick={() => setCursors((current) => current.slice(0, -1))}
          >
            Previous submissions
          </Button>
          <Button
            type="button"
            size="sm"
            variant="outline"
            disabled={!next || repeated || capped || query.isFetching}
            onClick={() => {
              if (next) setCursors((current) => [...current, next]);
            }}
          >
            Next submissions
          </Button>
        </nav>
        {repeated || capped ? (
          <p className="text-xs text-muted-foreground">
            Restart this local history to browse from the beginning.
          </p>
        ) : null}
      </div>
    </details>
  );
}

function CreatedCommentRow({ comment }: { comment: CreatedCommentReceipt }) {
  return (
    <li className="space-y-2 break-words text-sm">
      <p className="font-medium">{comment.author}</p>
      <p className="text-xs text-muted-foreground">
        Created{" "}
        <time dateTime={comment.created_at}>
          {new Date(comment.created_at).toLocaleString()}
        </time>
      </p>
      <p className="whitespace-pre-wrap">{comment.body}</p>
      <ProviderLink url={comment.url} />
    </li>
  );
}

function validateBody(body: string) {
  return utf8Size(body) <= MAX_COMMENT_BYTES
    ? null
    : `Keep the comment under ${MAX_COMMENT_BYTES.toLocaleString()} UTF-8 bytes.`;
}

function utf8Size(value: string) {
  return new TextEncoder().encode(value).byteLength;
}

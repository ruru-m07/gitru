import {
  collaboration,
  collaborationErrorMessage,
  type RemoteAccount,
  type TextEditRequest,
  type TextEditSnapshot,
} from "@gitru/collaboration-client";
import {
  itemQueryOptions,
  textEditQueryOptions,
} from "@gitru/collaboration-client/react";
import { Button } from "@gitru/ui/components/button";
import { Checkbox } from "@gitru/ui/components/checkbox";
import {
  Field,
  FieldDescription,
  FieldLabel,
} from "@gitru/ui/components/field";
import { Input } from "@gitru/ui/components/input";
import { Textarea } from "@gitru/ui/components/textarea";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { type FormEvent, useId, useState } from "react";

const MAX_TITLE_BYTES = 1024;
const MAX_TITLE_CHARACTERS = 256;
const MAX_BODY_BYTES = 16 * 1024;

type AvailableSnapshot = TextEditSnapshot & {
  context: NonNullable<TextEditSnapshot["context"]>;
  title: string;
};

export function ResourceTextEditor({
  account,
  subjectId,
}: {
  account: RemoteAccount;
  subjectId: string;
}) {
  const query = useQuery(textEditQueryOptions(account, subjectId));
  const [editingBase, setEditingBase] = useState<AvailableSnapshot | null>(
    null,
  );
  const [status, setStatus] = useState<string | null>(null);

  if (query.isPending) return null;
  if (query.isError && query.data === undefined)
    return (
      <p role="alert" className="mt-4 text-xs text-destructive-foreground">
        {collaborationErrorMessage(query.error)}
      </p>
    );

  const snapshot = query.data;
  const available = isAvailable(snapshot) ? snapshot : null;
  return (
    <section
      className="mt-5 space-y-3 border-t pt-4"
      aria-label="Provider editing"
    >
      <div className="flex flex-wrap items-center justify-between gap-2">
        <div>
          <h3 className="text-sm font-medium">Title and description</h3>
          <p className="mt-1 text-xs text-muted-foreground">
            Changes are saved locally first and delivered in the background.
          </p>
        </div>
        {!editingBase && available ? (
          <Button
            type="button"
            size="sm"
            variant="outline"
            onClick={() => {
              setStatus(null);
              setEditingBase(available);
            }}
          >
            Edit
          </Button>
        ) : null}
      </div>
      {query.isError ? (
        <p role="alert" className="text-xs text-destructive-foreground">
          {collaborationErrorMessage(query.error)}
        </p>
      ) : null}
      {editingBase && snapshot ? (
        <TextEditForm
          account={account}
          subjectId={subjectId}
          initialSnapshot={editingBase}
          currentSnapshot={snapshot}
          onCancel={() => setEditingBase(null)}
          onQueued={(message) => {
            setStatus(message);
            setEditingBase(null);
          }}
        />
      ) : !available && snapshot ? (
        <p className="text-xs text-muted-foreground">
          {unavailableMessage(snapshot)}
        </p>
      ) : null}
      {status ? (
        <p role="status" className="text-xs text-muted-foreground">
          {status}
        </p>
      ) : null}
    </section>
  );
}

function TextEditForm({
  account,
  subjectId,
  initialSnapshot,
  currentSnapshot,
  onCancel,
  onQueued,
}: {
  account: RemoteAccount;
  subjectId: string;
  initialSnapshot: AvailableSnapshot;
  currentSnapshot: TextEditSnapshot;
  onCancel: () => void;
  onQueued: (message: string) => void;
}) {
  const titleId = useId();
  const bodyId = useId();
  const consentId = useId();
  const [base, setBase] = useState(initialSnapshot);
  const [title, setTitle] = useState(initialSnapshot.title);
  const [body, setBody] = useState(initialSnapshot.body ?? "");
  const [consent, setConsent] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const [retryRequest, setRetryRequest] = useState<TextEditRequest | null>(
    null,
  );
  const [error, setError] = useState<string | null>(null);
  const [previousText, setPreviousText] = useState<{
    title: string;
    body: string;
  } | null>(null);
  const queryClient = useQueryClient();
  const currentAvailable = isAvailable(currentSnapshot)
    ? currentSnapshot
    : null;
  const baseChanged =
    currentSnapshot.context?.review_token !== base.context.review_token ||
    currentSnapshot.context?.authorization_view !==
      base.context.authorization_view ||
    currentSnapshot.context?.authorization_epoch !==
      base.context.authorization_epoch;
  const titleError = validateTitle(title);
  const bodyError = validateBody(body);
  const changedTitle = title !== base.title;
  const changedBody = body !== (base.body ?? "");
  const hasChanges = changedTitle || changedBody;
  const lockedAttempt = retryRequest !== null;

  async function submit(request: TextEditRequest) {
    setSubmitting(true);
    setError(null);
    setRetryRequest(request);
    try {
      const receipt = await collaboration
        .forAccount(account)
        .submitTextEdit(request);
      onQueued(
        receipt.duplicate
          ? "This exact change was already queued."
          : "Changes saved locally and queued for delivery.",
      );
      await Promise.all([
        queryClient.invalidateQueries({
          queryKey: textEditQueryOptions(account, subjectId).queryKey,
        }),
        queryClient.invalidateQueries({
          queryKey: itemQueryOptions(account, subjectId).queryKey,
        }),
      ]);
    } catch (failure) {
      setError(collaborationErrorMessage(failure));
      // Repair a missed native hint without replacing entered text or request identity.
      void queryClient.invalidateQueries({
        queryKey: textEditQueryOptions(account, subjectId).queryKey,
      });
    } finally {
      setSubmitting(false);
    }
  }

  function save(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (
      submitting ||
      retryRequest ||
      baseChanged ||
      !hasChanges ||
      !consent ||
      titleError ||
      bodyError
    )
      return;
    void submit({
      context: { ...base.context },
      command_id: crypto.randomUUID(),
      accept_best_effort: true,
      title: changedTitle ? title : null,
      body: changedBody ? body : null,
    });
  }

  function loadCurrent() {
    if (!currentAvailable) return;
    if (hasChanges) setPreviousText({ title, body });
    setBase(currentAvailable);
    setTitle(currentAvailable.title);
    setBody(currentAvailable.body ?? "");
    setConsent(false);
    setRetryRequest(null);
    setError(null);
  }

  return (
    <form className="space-y-4" onSubmit={save}>
      <Field name="provider-title" invalid={titleError !== null}>
        <FieldLabel htmlFor={titleId}>Title</FieldLabel>
        <Input
          id={titleId}
          name="provider-title"
          value={title}
          disabled={submitting || lockedAttempt}
          aria-invalid={titleError !== null}
          onChange={(event) => setTitle(event.currentTarget.value)}
        />
        <FieldDescription>
          {characterCount(title)}/{MAX_TITLE_CHARACTERS} characters
        </FieldDescription>
        {titleError ? (
          <p role="alert" className="text-xs text-destructive-foreground">
            {titleError}
          </p>
        ) : null}
      </Field>
      <Field name="provider-body" invalid={bodyError !== null}>
        <FieldLabel htmlFor={bodyId}>Description</FieldLabel>
        <Textarea
          id={bodyId}
          name="provider-body"
          value={body}
          disabled={submitting || lockedAttempt}
          aria-invalid={bodyError !== null}
          onChange={(event) => setBody(event.currentTarget.value)}
          rows={7}
        />
        <FieldDescription>
          {utf8Size(body).toLocaleString()}/{MAX_BODY_BYTES.toLocaleString()}{" "}
          bytes
        </FieldDescription>
        {bodyError ? (
          <p role="alert" className="text-xs text-destructive-foreground">
            {bodyError}
          </p>
        ) : null}
      </Field>
      <div className="flex items-start gap-2 text-xs text-muted-foreground">
        <Checkbox
          aria-labelledby={consentId}
          checked={consent}
          disabled={submitting || lockedAttempt}
          onCheckedChange={(checked) => setConsent(checked === true)}
        />
        <span id={consentId}>
          I understand GitHub applies this as a best-effort update, so a
          simultaneous edit may overwrite my change, or my change may overwrite
          theirs.
        </span>
      </div>
      {baseChanged ? (
        <div className="space-y-2 text-xs text-muted-foreground">
          <p>
            This item changed after you started editing. Your text is preserved;
            load the latest saved version before submitting.
          </p>
          {currentAvailable ? (
            <Button
              type="button"
              size="sm"
              variant="outline"
              onClick={loadCurrent}
            >
              Load latest saved version
            </Button>
          ) : null}
        </div>
      ) : null}
      {previousText ? (
        <details className="text-xs text-muted-foreground">
          <summary className="cursor-pointer">Your previous edit</summary>
          <p className="mt-2 font-medium text-foreground">
            {previousText.title}
          </p>
          <p className="mt-1 whitespace-pre-wrap break-words">
            {previousText.body || "No description"}
          </p>
        </details>
      ) : null}
      {error ? (
        <div className="space-y-2">
          <p role="alert" className="text-xs text-destructive-foreground">
            {error} Your text and request identity are preserved.
          </p>
          {retryRequest && !baseChanged ? (
            <Button
              type="button"
              size="sm"
              variant="outline"
              disabled={submitting}
              onClick={() => void submit(retryRequest)}
            >
              {submitting ? "Retrying…" : "Retry exact change"}
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
          onClick={onCancel}
        >
          Cancel
        </Button>
        <Button
          type="submit"
          size="sm"
          disabled={
            submitting ||
            lockedAttempt ||
            baseChanged ||
            !hasChanges ||
            !consent ||
            titleError !== null ||
            bodyError !== null
          }
        >
          {submitting ? "Saving locally…" : "Save and queue"}
        </Button>
      </div>
    </form>
  );
}

function isAvailable(
  snapshot: TextEditSnapshot | undefined,
): snapshot is AvailableSnapshot {
  return (
    snapshot?.availability === "available" &&
    snapshot.context !== null &&
    snapshot.title !== null
  );
}

function unavailableMessage(snapshot: TextEditSnapshot) {
  switch (snapshot.reason) {
    case "unsupported_provider":
      return "Editing is not available for this provider yet.";
    case "account_unavailable":
      return "Reconnect this account before editing provider content.";
    case "missing_base":
      return "Sync this item before editing its title or description.";
    case "oversized_text":
      return "This title or description is too large to edit in Gitru.";
    case "pending_intent":
      return "A title or description change is already queued.";
    default:
      return "Editing is unavailable for this saved item.";
  }
}

function characterCount(value: string) {
  return [...value].length;
}

function utf8Size(value: string) {
  return new TextEncoder().encode(value).byteLength;
}

function validateTitle(value: string): string | null {
  if (!value.trim()) return "Enter a title.";
  if (value.trim() !== value) return "Remove leading or trailing whitespace.";
  if (/\p{Cc}/u.test(value)) return "Remove control characters from the title.";
  if (characterCount(value) > MAX_TITLE_CHARACTERS)
    return `Keep the title to ${MAX_TITLE_CHARACTERS} characters or fewer.`;
  if (utf8Size(value) > MAX_TITLE_BYTES)
    return `Keep the title to ${MAX_TITLE_BYTES.toLocaleString()} bytes or fewer.`;
  return null;
}

function validateBody(value: string): string | null {
  if (value.includes("\0"))
    return "Remove null characters from the description.";
  if (utf8Size(value) > MAX_BODY_BYTES)
    return `Keep the description to ${MAX_BODY_BYTES.toLocaleString()} bytes or fewer.`;
  return null;
}

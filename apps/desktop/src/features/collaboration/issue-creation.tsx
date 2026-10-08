import {
  collaboration,
  collaborationErrorMessage,
  type IssueDraftKey,
  type IssueDraftReason,
  type IssueDraftSnapshot,
  type RemoteAccount,
  type RemoteRepository,
  type SubmitIssueRequest,
} from "@gitru/collaboration-client";
import {
  issueDraftQueryOptions,
  issueDraftsQueryOptions,
} from "@gitru/collaboration-client/react";
import { Button } from "@gitru/ui/components/button";
import { Checkbox } from "@gitru/ui/components/checkbox";
import {
  Dialog,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogPanel,
  DialogPopup,
  DialogTitle,
  DialogTrigger,
} from "@gitru/ui/components/dialog";
import {
  Field,
  FieldDescription,
  FieldLabel,
} from "@gitru/ui/components/field";
import { Input } from "@gitru/ui/components/input";
import { Textarea } from "@gitru/ui/components/textarea";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { CirclePlus } from "lucide-react";
import { type FormEvent, useId, useState } from "react";
import { ProviderLink } from "./provider-link";

const MAX_TITLE_BYTES = 1024;
const MAX_TITLE_CHARACTERS = 256;
const MAX_BODY_BYTES = 16 * 1024;

type LocalIssueDraftKey = Omit<IssueDraftKey, "account_id">;

export function NewIssueDialog({
  account,
  repository,
  onOpenCreated,
}: {
  account: RemoteAccount;
  repository: RemoteRepository;
  onOpenCreated: (subjectId: string) => void;
}) {
  const [session, setSession] = useState<{
    draftId: string;
    open: boolean;
  } | null>(null);
  if (!session)
    return (
      <Button
        type="button"
        size="sm"
        onClick={() => setSession({ draftId: crypto.randomUUID(), open: true })}
      >
        <CirclePlus aria-hidden="true" />
        New issue
      </Button>
    );
  return (
    <NewIssueSession
      key={session.draftId}
      account={account}
      repository={repository}
      draftId={session.draftId}
      open={session.open}
      onOpenChange={(open) =>
        setSession((current) => current && { ...current, open })
      }
      onStartAnother={() =>
        setSession({ draftId: crypto.randomUUID(), open: true })
      }
      onOpenCreated={(subjectId) => {
        setSession((current) => current && { ...current, open: false });
        onOpenCreated(subjectId);
      }}
    />
  );
}

function NewIssueSession({
  account,
  repository,
  draftId,
  open,
  onOpenChange,
  onStartAnother,
  onOpenCreated,
}: {
  account: RemoteAccount;
  repository: RemoteRepository;
  draftId: string;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onStartAnother: () => void;
  onOpenCreated: (subjectId: string) => void;
}) {
  const editor = useIssueDraftEditor(account, {
    draft_id: draftId,
    repository_id: repository.id,
  });
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogTrigger render={<Button type="button" size="sm" />}>
        <CirclePlus aria-hidden="true" />
        New issue
      </DialogTrigger>
      <DialogPopup className="sm:max-w-2xl">
        <DialogHeader>
          <DialogTitle>New issue</DialogTitle>
          <DialogDescription>
            Draft an issue for {repository.full_name}. Saving stays on this
            device; submission is a separate explicit step.
          </DialogDescription>
        </DialogHeader>
        <DialogPanel className="max-h-[65dvh] overflow-y-auto">
          <IssueDraftForm editor={editor} onOpenCreated={onOpenCreated} />
        </DialogPanel>
        {editor.base?.generation !== "0" &&
        !editor.dirty &&
        (editor.current?.submission === null ||
          editor.current?.published !== null) ? (
          <DialogFooter>
            <Button type="button" variant="outline" onClick={onStartAnother}>
              Start another draft
            </Button>
          </DialogFooter>
        ) : null}
      </DialogPopup>
    </Dialog>
  );
}

export function RecoveredIssueDraft({
  account,
  draftId,
  repositoryId,
  onOpenCreated,
}: {
  account: RemoteAccount;
  draftId: string;
  repositoryId: string;
  onOpenCreated?: (subjectId: string) => void;
}) {
  const editor = useIssueDraftEditor(account, {
    draft_id: draftId,
    repository_id: repositoryId,
  });
  return <IssueDraftForm editor={editor} onOpenCreated={onOpenCreated} />;
}

function useIssueDraftEditor(account: RemoteAccount, key: LocalIssueDraftKey) {
  const queryClient = useQueryClient();
  const queryOptions = issueDraftQueryOptions(account, key);
  const query = useQuery(queryOptions);
  const [base, setBase] = useState<IssueDraftSnapshot | null>(null);
  const [title, setTitle] = useState("");
  const [body, setBody] = useState("");
  const [savedTitle, setSavedTitle] = useState("");
  const [savedBody, setSavedBody] = useState("");
  const [previous, setPrevious] = useState<{
    title: string;
    body: string;
  } | null>(null);
  const [consent, setConsent] = useState(false);
  const [saving, setSaving] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [status, setStatus] = useState<string | null>(null);
  const [retryRequest, setRetryRequest] = useState<SubmitIssueRequest | null>(
    null,
  );
  if (query.data && base === null) {
    setBase(query.data);
    setTitle(query.data.title);
    setBody(query.data.body);
    setSavedTitle(query.data.title);
    setSavedBody(query.data.body);
  }
  const current = query.data;
  const dirty = title !== savedTitle || body !== savedBody;
  const baseChanged =
    base !== null &&
    current !== undefined &&
    (current.generation !== base.generation ||
      current.authorization_view !== base.authorization_view ||
      current.repository_id !== base.repository_id);
  const retryObserved =
    retryRequest !== null &&
    current?.submission?.command_id === retryRequest.command_id;
  // A pending snapshot can be the durable side of a locally lost admission
  // receipt. Keep the exact UUID retry available; native duplicate admission
  // decides whether the command was already committed.
  const activeRetry = retryRequest;
  const titleError = validateTitle(title);
  const bodyError = validateBody(body);
  const retryAllowed =
    activeRetry !== null &&
    !query.isError &&
    current?.reason !== "account_unavailable" &&
    activeRetry.context.account_id === account.id &&
    activeRetry.context.repository_id === key.repository_id &&
    activeRetry.context.authorization_epoch === account.authorization_epoch &&
    activeRetry.draft_id === key.draft_id;
  const canSave =
    base !== null &&
    !saving &&
    !submitting &&
    !query.isError &&
    activeRetry === null &&
    !baseChanged &&
    current?.published === null &&
    dirty &&
    titleError === null &&
    bodyError === null;
  const canSubmit =
    base !== null &&
    current !== undefined &&
    !saving &&
    !submitting &&
    !query.isError &&
    activeRetry === null &&
    !baseChanged &&
    !dirty &&
    base.generation !== "0" &&
    current.availability === "available" &&
    current.context !== null &&
    current.submission === null &&
    current.published === null &&
    consent &&
    titleError === null &&
    bodyError === null;

  function accept(snapshot: IssueDraftSnapshot) {
    queryClient.setQueryData(queryOptions.queryKey, snapshot);
    setBase(snapshot);
    setTitle(snapshot.title);
    setBody(snapshot.body);
    setSavedTitle(snapshot.title);
    setSavedBody(snapshot.body);
    setConsent(false);
  }

  async function save(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!canSave || !base) return;
    setSaving(true);
    setError(null);
    setStatus(null);
    try {
      const snapshot = await collaboration.forAccount(account).saveIssueDraft({
        draft_id: key.draft_id,
        repository_id: key.repository_id,
        authorization_view: base.authorization_view,
        expected_generation: base.generation,
        title,
        body,
      });
      accept(snapshot);
      setStatus("Issue draft saved on this device.");
      void queryClient.invalidateQueries({
        queryKey: issueDraftsQueryOptions(account, {
          cursor: null,
          limit: 50,
        }).queryKey.slice(0, -1),
      });
    } catch (failure) {
      setError(collaborationErrorMessage(failure));
      void queryClient.invalidateQueries({ queryKey: queryOptions.queryKey });
    } finally {
      setSaving(false);
    }
  }

  async function submit(request: SubmitIssueRequest) {
    setSubmitting(true);
    setRetryRequest(request);
    setError(null);
    setStatus(null);
    try {
      const receipt = await collaboration
        .forAccount(account)
        .submitIssue(request);
      setRetryRequest(null);
      setConsent(false);
      setStatus(
        receipt.duplicate
          ? "This exact issue draft was already queued."
          : "Issue saved locally and queued for background submission.",
      );
      void queryClient.invalidateQueries({ queryKey: queryOptions.queryKey });
      void collaboration.wake();
    } catch (failure) {
      setError(collaborationErrorMessage(failure));
      void queryClient.invalidateQueries({ queryKey: queryOptions.queryKey });
    } finally {
      setSubmitting(false);
    }
  }

  function queue() {
    if (!canSubmit || !current?.context || !base) return;
    void submit({
      context: { ...current.context },
      draft_id: key.draft_id,
      draft_generation: base.generation,
      command_id: crypto.randomUUID(),
      accept_background_delivery: true,
    });
  }

  function loadCurrent() {
    if (!current) return;
    if (dirty) setPrevious({ title, body });
    accept(current);
    setRetryRequest(null);
    setError(null);
    setStatus(null);
  }

  return {
    account,
    key,
    query,
    base,
    current,
    title,
    body,
    savedTitle,
    savedBody,
    previous,
    consent,
    saving,
    submitting,
    error,
    status,
    activeRetry,
    retryObserved,
    retryAllowed,
    dirty,
    baseChanged,
    titleError,
    bodyError,
    canSave,
    canSubmit,
    setTitle,
    setBody,
    setConsent,
    save,
    submit,
    queue,
    loadCurrent,
  };
}

type IssueDraftEditor = ReturnType<typeof useIssueDraftEditor>;

function IssueDraftForm({
  editor,
  onOpenCreated,
}: {
  editor: IssueDraftEditor;
  onOpenCreated?: (subjectId: string) => void;
}) {
  const titleId = useId();
  const bodyId = useId();
  const consentId = useId();
  const {
    query,
    base,
    current,
    title,
    body,
    previous,
    consent,
    saving,
    submitting,
    error,
    status,
    activeRetry,
    retryObserved,
    retryAllowed,
    dirty,
    baseChanged,
    titleError,
    bodyError,
    canSave,
    canSubmit,
  } = editor;
  if (query.isPending || (!base && !query.isError))
    return (
      <p role="status" className="text-sm text-muted-foreground">
        Loading local issue draft…
      </p>
    );
  if (query.isError && !base)
    return (
      <p role="alert" className="text-sm text-destructive-foreground">
        {collaborationErrorMessage(query.error)}
      </p>
    );
  if (!base || !current) return null;
  return (
    <form className="space-y-4" onSubmit={editor.save}>
      {query.isError ? (
        <p role="alert" className="text-xs text-destructive-foreground">
          {collaborationErrorMessage(query.error)} Saving and new submissions
          are disabled until this local draft reloads.
        </p>
      ) : null}
      <Field name="issue-title" invalid={titleError !== null}>
        <FieldLabel htmlFor={titleId}>Title</FieldLabel>
        <Input
          id={titleId}
          type="text"
          value={title}
          disabled={saving || submitting || activeRetry !== null}
          aria-invalid={titleError !== null}
          placeholder="Issue title"
          onChange={(event) => editor.setTitle(event.currentTarget.value)}
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
      <Field name="issue-body" invalid={bodyError !== null}>
        <FieldLabel htmlFor={bodyId}>Description</FieldLabel>
        <Textarea
          id={bodyId}
          value={body}
          disabled={saving || submitting || activeRetry !== null}
          aria-invalid={bodyError !== null}
          rows={8}
          placeholder="Describe the issue…"
          onChange={(event) => editor.setBody(event.currentTarget.value)}
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
      {baseChanged ? (
        <div className="space-y-2 text-xs text-muted-foreground">
          <p>
            This saved draft or account context changed. Your text is still
            here; load the latest local draft before continuing.
          </p>
          {!query.isError && activeRetry === null ? (
            <Button
              type="button"
              size="sm"
              variant="outline"
              onClick={editor.loadCurrent}
            >
              Load latest issue draft
            </Button>
          ) : null}
        </div>
      ) : null}
      {previous ? (
        <details className="text-xs text-muted-foreground">
          <summary className="cursor-pointer">Your previous draft text</summary>
          <p className="mt-2 font-medium">{previous.title || "Untitled"}</p>
          <p className="mt-1 whitespace-pre-wrap break-words">
            {previous.body || "Empty description"}
          </p>
        </details>
      ) : null}
      <div className="flex items-start gap-2 text-xs text-muted-foreground">
        <Checkbox
          aria-labelledby={consentId}
          checked={consent}
          disabled={saving || submitting || activeRetry !== null}
          onCheckedChange={(checked) => editor.setConsent(checked === true)}
        />
        <span id={consentId}>
          I understand this exact saved issue may be submitted in the background
          after I reconnect. Gitru will not automatically repeat an uncertain
          creation request.
        </span>
      </div>
      {current.reason ? (
        <p className="text-xs text-muted-foreground">
          {reasonMessage(current.reason)}
        </p>
      ) : null}
      {current.submission ? (
        <p className="text-xs text-muted-foreground">
          {current.submission.quarantined
            ? "This submitted issue draft was restored and needs review in Saved changes."
            : "This issue submission is tracked in Saved changes."}
        </p>
      ) : null}
      {current.published ? (
        <div className="space-y-2 rounded-md border p-3">
          <p role="status" className="text-sm font-medium">
            Issue #{current.published.number} was confirmed by GitHub.
          </p>
          <div className="flex flex-wrap gap-2">
            {onOpenCreated ? (
              <Button
                type="button"
                size="sm"
                onClick={() =>
                  onOpenCreated(current.published?.subject_id ?? "")
                }
              >
                Open created issue
              </Button>
            ) : null}
            <ProviderLink url={current.published.url} />
          </div>
        </div>
      ) : null}
      {error ? (
        <div className="space-y-2">
          <p role="alert" className="text-xs text-destructive-foreground">
            {error} Your draft
            {activeRetry ? " and exact request identity are" : " is"} preserved.
          </p>
          {retryAllowed && activeRetry ? (
            <Button
              type="button"
              size="sm"
              variant="outline"
              disabled={submitting}
              onClick={() => void editor.submit(activeRetry)}
            >
              {submitting ? "Retrying…" : "Retry exact issue submission"}
            </Button>
          ) : null}
        </div>
      ) : null}
      {retryObserved || status ? (
        <p role="status" className="text-xs text-muted-foreground">
          {retryObserved
            ? "This exact issue submission is tracked in Saved changes."
            : status}
        </p>
      ) : null}
      <div className="flex flex-wrap justify-end gap-2">
        <Button type="submit" size="sm" variant="outline" disabled={!canSave}>
          {saving ? "Saving…" : dirty ? "Save issue draft" : "Draft saved"}
        </Button>
        <Button
          type="button"
          size="sm"
          disabled={!canSubmit}
          onClick={editor.queue}
        >
          {submitting ? "Queuing…" : "Queue issue submission"}
        </Button>
      </div>
      <p className="text-xs text-muted-foreground">
        Labels, assignees, milestones and issue types are not available in this
        first creation slice.
      </p>
    </form>
  );
}

function reasonMessage(reason: IssueDraftReason) {
  switch (reason) {
    case "unsupported_provider":
      return "Issue submission is not available for this provider yet. Your draft stays on this device.";
    case "account_unavailable":
      return "Reconnect this account before queuing the saved issue.";
    case "missing_repository":
      return "The saved repository is unavailable. Your issue draft stays on this device.";
    case "empty_title":
      return "Add and save a title before submitting the issue.";
    case "pending_submission":
      return "An issue submission is already tracked. Review Saved changes before submitting another generation.";
    case "already_submitted":
      return "This saved draft generation was already submitted. Its delivery status remains in Saved changes.";
  }
}

function utf8Size(value: string) {
  return new TextEncoder().encode(value).length;
}

function characterCount(value: string) {
  return Array.from(value).length;
}

function validateTitle(value: string) {
  if (!value.trim()) return "Enter an issue title.";
  if (value !== value.trim())
    return "Remove leading or trailing whitespace from the title.";
  if (Array.from(value).some((character) => /\p{Cc}/u.test(character)))
    return "Remove control characters from the title.";
  if (characterCount(value) > MAX_TITLE_CHARACTERS)
    return `Keep the title to ${MAX_TITLE_CHARACTERS} characters or fewer.`;
  if (utf8Size(value) > MAX_TITLE_BYTES)
    return `Keep the title to ${MAX_TITLE_BYTES.toLocaleString()} bytes or fewer.`;
  return null;
}

function validateBody(value: string) {
  if (value.includes("\0"))
    return "Remove null characters from the description.";
  if (utf8Size(value) > MAX_BODY_BYTES)
    return `Keep the description to ${MAX_BODY_BYTES.toLocaleString()} bytes or fewer.`;
  return null;
}

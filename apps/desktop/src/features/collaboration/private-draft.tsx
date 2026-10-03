import {
  collaboration,
  collaborationErrorMessage,
  type RemoteAccount,
  type RemoteItem,
} from "@gitru/collaboration-client";
import { draftQueryOptions } from "@gitru/collaboration-client/react";
import { Button } from "@gitru/ui/components/button";
import { Field, FieldLabel } from "@gitru/ui/components/field";
import { Textarea } from "@gitru/ui/components/textarea";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { type FormEvent, useId, useState } from "react";

export function PrivateDraft({
  account,
  item,
}: {
  account: RemoteAccount;
  item: RemoteItem;
}) {
  return <SavedDraftEditor account={account} subjectId={item.id} />;
}

export function SavedDraftEditor({
  account,
  subjectId,
}: {
  account: RemoteAccount;
  subjectId: string;
}) {
  const query = useQuery(draftQueryOptions(account, subjectId));
  if (query.isPending) return null;
  if (query.isError && query.data === undefined)
    return (
      <p role="alert" className="mt-6 text-xs text-destructive-foreground">
        {collaborationErrorMessage(query.error)}
      </p>
    );
  return (
    <div>
      {query.isError ? (
        <p role="alert" className="mt-6 text-xs text-destructive-foreground">
          {collaborationErrorMessage(query.error)}
        </p>
      ) : null}
      <DraftForm
        account={account}
        itemId={subjectId}
        initialBody={query.data?.body ?? ""}
        generation={query.data?.generation ?? "0"}
      />
    </div>
  );
}

function DraftForm({
  account,
  itemId,
  initialBody,
  generation,
}: {
  account: RemoteAccount;
  itemId: string;
  initialBody: string;
  generation: string;
}) {
  const draftId = useId();
  const [body, setBody] = useState(initialBody);
  const [savedBody, setSavedBody] = useState(initialBody);
  const [draftGeneration, setDraftGeneration] = useState(generation);
  const [previousBody, setPreviousBody] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [exporting, setExporting] = useState(false);
  const [actionStatus, setActionStatus] = useState<string | null>(null);
  const queryClient = useQueryClient();
  async function save(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setSaving(true);
    setError(null);
    setActionStatus(null);
    try {
      const draft = await collaboration
        .forAccount(account)
        .saveDraft({ subject_id: itemId, body, generation: draftGeneration });
      queryClient.setQueryData(
        draftQueryOptions(account, itemId).queryKey,
        draft,
      );
      setDraftGeneration(draft.generation);
      setSavedBody(draft.body);
    } catch (failure) {
      setError(collaborationErrorMessage(failure));
      // Repair a missed notification without replacing the user's text.
      void queryClient.invalidateQueries({
        queryKey: draftQueryOptions(account, itemId).queryKey,
      });
    } finally {
      setSaving(false);
    }
  }
  async function copy() {
    setError(null);
    setActionStatus(null);
    try {
      await navigator.clipboard.writeText(body);
      setActionStatus("Draft text copied.");
    } catch {
      setError(
        "Could not copy the draft. Select its text to copy it manually.",
      );
    }
  }
  async function exportSaved() {
    setExporting(true);
    setError(null);
    setActionStatus(null);
    try {
      const exported = await collaboration
        .forAccount(account)
        .exportDraft(itemId, draftGeneration);
      setActionStatus(exported ? "Saved draft exported." : "Export cancelled.");
    } catch (failure) {
      setError(collaborationErrorMessage(failure));
      void queryClient.invalidateQueries({
        queryKey: draftQueryOptions(account, itemId).queryKey,
      });
    } finally {
      setExporting(false);
    }
  }
  return (
    <form className="mt-6 space-y-3 border-t pt-4" onSubmit={save}>
      <Field name="private-draft">
        <FieldLabel htmlFor={draftId}>Private draft</FieldLabel>
        <Textarea
          id={draftId}
          name="private-draft"
          value={body}
          disabled={saving}
          onChange={(event) => setBody(event.currentTarget.value)}
          maxLength={100_000}
          rows={4}
          placeholder="Keep a draft here for later…"
        />
      </Field>
      <div className="flex flex-wrap items-center justify-between gap-2">
        <p className="text-xs text-muted-foreground">
          Saved on this device. Visible only to you.
        </p>
        <Button
          type="submit"
          size="sm"
          variant="outline"
          disabled={
            saving || body === savedBody || generation !== draftGeneration
          }
        >
          {saving ? "Saving…" : "Save draft"}
        </Button>
      </div>
      <div className="flex flex-wrap items-center gap-2">
        <Button
          type="button"
          size="sm"
          variant="ghost"
          disabled={!body}
          onClick={() => {
            void copy();
          }}
        >
          Copy draft text
        </Button>
        <Button
          type="button"
          size="sm"
          variant="ghost"
          disabled={
            saving ||
            exporting ||
            draftGeneration === "0" ||
            body !== savedBody ||
            generation !== draftGeneration
          }
          onClick={() => {
            void exportSaved();
          }}
        >
          {exporting ? "Exporting…" : "Export saved draft"}
        </Button>
        {body !== savedBody ? (
          <span className="text-xs text-muted-foreground">
            Save changes before exporting.
          </span>
        ) : null}
      </div>
      {generation !== draftGeneration ? (
        <div className="space-y-2 text-xs text-muted-foreground">
          <p>
            This draft changed in another tab. Your text is still here. Reload
            the saved draft before saving again.
          </p>
          <Button
            type="button"
            size="sm"
            variant="outline"
            onClick={() => {
              setPreviousBody(body);
              setBody(initialBody);
              setSavedBody(initialBody);
              setDraftGeneration(generation);
              setError(null);
            }}
          >
            Reload saved draft
          </Button>
        </div>
      ) : null}
      {previousBody !== null ? (
        <details className="text-xs text-muted-foreground">
          <summary className="cursor-pointer">Your previous draft text</summary>
          <Textarea
            aria-label="Previous draft text"
            className="mt-2"
            value={previousBody}
            readOnly
            rows={4}
          />
        </details>
      ) : null}
      {error ? (
        <p role="alert" className="text-xs text-destructive-foreground">
          {error}
        </p>
      ) : null}
      {actionStatus ? (
        <p role="status" className="text-xs text-muted-foreground">
          {actionStatus}
        </p>
      ) : null}
    </form>
  );
}

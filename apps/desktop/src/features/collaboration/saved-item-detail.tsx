import {
  collaboration,
  collaborationErrorMessage,
  type RemoteAccount,
  type RemoteItemKind,
} from "@gitru/collaboration-client";
import {
  draftQueryOptions,
  useCollaborationDetail,
  useCollaborationItem,
  useContextualCapabilities,
  useVisibleDemand,
} from "@gitru/collaboration-client/react";
import type { LocalDraft } from "@gitru/commands";
import { Button } from "@gitru/ui/components/button";
import { Field, FieldLabel } from "@gitru/ui/components/field";
import { Textarea } from "@gitru/ui/components/textarea";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { ArrowLeft } from "lucide-react";
import { type FormEvent, useId, useState } from "react";
import { CapabilityBoundary, ReadOnlyCapability } from "./capability-boundary";
import {
  canMaintainDemand,
  canReadSaved,
  facetPolicy,
  feedFacet,
  resourceCapabilityTarget,
} from "./capability-policy";
import { ResourceCapabilityPanels } from "./resource-capability-panels";
import { SelectedResourceHeader } from "./resource-metadata";

export function SavedItemDetail({
  account,
  itemId,
  close,
  kind,
  instanceId,
  providerEnabled = true,
}: {
  account: RemoteAccount;
  itemId: string;
  kind: RemoteItemKind;
  instanceId: string;
  close?: () => void;
  providerEnabled?: boolean;
}) {
  const context = useContextualCapabilities(
    account,
    resourceCapabilityTarget(instanceId, itemId, kind),
  );
  const policy = facetPolicy(context.data, feedFacet[kind]);
  const query = useCollaborationItem(
    account,
    itemId,
    providerEnabled && canReadSaved(policy),
  );
  const item = query.data?.item;
  const bodyPolicy = facetPolicy(
    context.data,
    kind === "pull_request" ? "pull_details" : "issue_details",
  );
  const body = useCollaborationDetail(
    account,
    { subject_id: itemId, facet: "body", cursor: null, limit: 50 },
    providerEnabled &&
      kind !== "notification" &&
      canReadSaved(policy) &&
      canReadSaved(bodyPolicy),
  );
  const bodyData =
    providerEnabled &&
    canReadSaved(bodyPolicy) &&
    body.data?.evidence.availability !== "unavailable"
      ? body.data
      : undefined;
  const foregroundDemandError = useVisibleDemand({
    account,
    target: {
      kind: "detail",
      repository_id: null,
      subject_id: itemId,
      facet: "body",
    },
    enabled:
      account.state === "active" &&
      providerEnabled &&
      kind !== "notification" &&
      !!item &&
      canReadSaved(policy) &&
      canMaintainDemand(bodyPolicy) &&
      body.data?.evidence.availability !== "unavailable",
  });
  const bodyAccessDenied =
    kind !== "notification" &&
    (bodyPolicy?.saved_read.state === "unavailable" ||
      body.data?.evidence.availability === "unavailable");
  return (
    <article
      className="min-w-0 overflow-y-auto border-l p-5"
      aria-label="Saved item detail"
    >
      {close ? (
        <Button variant="ghost" size="sm" onClick={close} className="mb-4">
          <ArrowLeft aria-hidden="true" />
          Back to list
        </Button>
      ) : null}
      {!providerEnabled ? (
        <p className="text-sm text-muted-foreground">
          Provider content is unavailable in the current notification view. Your
          private draft remains on this device.
        </p>
      ) : !canReadSaved(policy) ? (
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
      ) : bodyAccessDenied ? (
        <CapabilityBoundary policy={bodyPolicy}>{null}</CapabilityBoundary>
      ) : query.isPending ? (
        <p role="status" className="text-sm text-muted-foreground">
          Loading saved detail…
        </p>
      ) : query.isError ? (
        <p role="alert" className="text-sm text-destructive-foreground">
          {collaborationErrorMessage(query.error)}
        </p>
      ) : item ? (
        <>
          <SelectedResourceHeader
            item={item}
            metadata={bodyData?.metadata ?? null}
          />
          {kind === "notification" || !canReadSaved(bodyPolicy) ? (
            <div className="mt-4 whitespace-pre-wrap break-words text-sm leading-relaxed">
              {item.body ??
                (item.body_omitted
                  ? "This description is not saved on this device. Open the provider to read it."
                  : "This saved item has no description.")}
            </div>
          ) : null}
          <ReadOnlyCapability policy={policy} />
        </>
      ) : (
        <p className="text-sm text-muted-foreground">
          This item is no longer available in your saved view.
        </p>
      )}
      {foregroundDemandError ? (
        <p role="alert" className="mt-3 text-xs text-destructive-foreground">
          {collaborationErrorMessage(foregroundDemandError)}
        </p>
      ) : null}
      <ResourceCapabilityPanels
        account={account}
        subjectId={itemId}
        kind={kind}
        snapshot={providerEnabled ? context.data : undefined}
      />
      <PrivateDraft account={account} subjectId={itemId} />
    </article>
  );
}

export function PrivateDraft({
  account,
  subjectId,
  label = "Private draft",
}: {
  account: RemoteAccount;
  subjectId: string;
  label?: string;
}) {
  const query = useQuery(draftQueryOptions(account, subjectId));
  // This is authored editor initialization, scoped by the parent account and
  // subject keys. A provider authorization reset may reread local drafts, but
  // cannot discard an already mounted editor's text during that reread.
  const [loaded, setLoaded] = useState<{ draft: LocalDraft | null } | null>(
    () => (query.data === undefined ? null : { draft: query.data }),
  );
  if (query.data !== undefined && query.data !== loaded?.draft) {
    setLoaded({ draft: query.data });
  }
  return (
    <>
      {query.isError ? (
        <p role="alert" className="mt-6 text-xs text-destructive-foreground">
          {collaborationErrorMessage(query.error)}
        </p>
      ) : null}
      {loaded ? (
        <DraftForm
          account={account}
          itemId={subjectId}
          initialBody={loaded.draft?.body ?? ""}
          generation={loaded.draft?.generation ?? "0"}
          label={label}
        />
      ) : null}
    </>
  );
}

function DraftForm({
  account,
  itemId,
  initialBody,
  generation,
  label,
}: {
  account: RemoteAccount;
  itemId: string;
  initialBody: string;
  generation: string;
  label: string;
}) {
  const draftId = useId();
  const [body, setBody] = useState(initialBody);
  const [savedBody, setSavedBody] = useState(initialBody);
  const [draftGeneration, setDraftGeneration] = useState(generation);
  const [previousBody, setPreviousBody] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const queryClient = useQueryClient();
  async function save(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setSaving(true);
    setError(null);
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
    } finally {
      setSaving(false);
    }
  }
  return (
    <form className="mt-6 space-y-3 border-t pt-4" onSubmit={save}>
      <Field name="private-draft">
        <FieldLabel htmlFor={draftId}>{label}</FieldLabel>
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
      {generation !== draftGeneration ? (
        <div className="space-y-2 text-xs text-muted-foreground">
          <p>
            This draft changed in another tab. Your text is still here. Reload
            the saved draft before saving again.
          </p>
          <Button
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
    </form>
  );
}

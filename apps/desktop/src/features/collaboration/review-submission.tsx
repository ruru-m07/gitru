import {
  collaboration,
  collaborationErrorMessage,
  type RemoteAccount,
  type ReviewDraftAnchorSelection,
  type ReviewDraftSnapshot,
  type ReviewSubmissionEvent,
  type ReviewSubmissionReason,
  type SubmitReviewRequest,
} from "@gitru/collaboration-client";
import {
  reviewDraftQueryOptions,
  reviewDraftsQueryOptions,
  submittedReviewsQueryOptions,
  useCollaborationAuthorityVersion,
} from "@gitru/collaboration-client/react";
import { Button } from "@gitru/ui/components/button";
import { Checkbox } from "@gitru/ui/components/checkbox";
import {
  Dialog,
  DialogDescription,
  DialogHeader,
  DialogPanel,
  DialogPopup,
  DialogTitle,
} from "@gitru/ui/components/dialog";
import { Field, FieldLabel } from "@gitru/ui/components/field";
import {
  Select,
  SelectItem,
  SelectPopup,
  SelectTrigger,
  SelectValue,
} from "@gitru/ui/components/select";
import { Textarea } from "@gitru/ui/components/textarea";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import {
  createContext,
  type FormEvent,
  type ReactNode,
  useContext,
  useId,
  useState,
} from "react";
import { ProviderLink } from "./provider-link";

type Props = { account: RemoteAccount; subjectId: string };
export type InlineReviewSelection = {
  anchor: ReviewDraftAnchorSelection;
  /** Display only. Native save derives the trusted path from the file key. */
  location: string;
};
type IncomingSelection = InlineReviewSelection & {
  id: string;
  authorityVersion: number;
  authorizationEpoch: string;
};
type Authoring = {
  open: () => void;
  add: (selection: InlineReviewSelection) => void;
};
const AuthoringContext = createContext<Authoring | null>(null);

/** The session lives outside the dialog portal, preserving unsaved text on close. */
export function ReviewAuthoringProvider({
  account,
  subjectId,
  children,
}: Props & { children: ReactNode }) {
  const [session, setSession] = useState<{
    open: boolean;
    selection: IncomingSelection | null;
  } | null>(null);
  const authoring: Authoring = {
    open: () =>
      setSession((previous) => ({
        open: true,
        selection: previous?.selection ?? null,
      })),
    add: (selection) =>
      setSession({
        open: true,
        selection: {
          ...selection,
          id: crypto.randomUUID(),
          authorityVersion: collaboration.getAuthorityVersion(),
          authorizationEpoch: account.authorization_epoch,
        },
      }),
  };
  return (
    <AuthoringContext.Provider value={authoring}>
      {children}
      {session ? (
        <ReviewDraftSession
          account={account}
          subjectId={subjectId}
          open={session.open}
          selection={session.selection}
          onOpenChange={(open) =>
            setSession((previous) => previous && { ...previous, open })
          }
        />
      ) : null}
    </AuthoringContext.Provider>
  );
}

export function ReviewComposerTrigger() {
  const authoring = useContext(AuthoringContext);
  if (!authoring) return null;
  return (
    <Button type="button" size="sm" variant="outline" onClick={authoring.open}>
      Write a review
    </Button>
  );
}
export function useInlineReviewAuthoring() {
  return useContext(AuthoringContext);
}

function ReviewDraftSession({
  open,
  onOpenChange,
  selection,
  ...props
}: Props & {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  selection: IncomingSelection | null;
}) {
  const editor = useReviewDraftEditor(props, selection);
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogPopup className="sm:max-w-2xl">
        <DialogHeader>
          <DialogTitle>Review pull request</DialogTitle>
          <DialogDescription>
            Save your review on this device, then explicitly queue it for
            GitHub.
          </DialogDescription>
        </DialogHeader>
        <DialogPanel className="max-h-[65dvh] overflow-y-auto">
          <ReviewDraftForm editor={editor} />
          <SubmittedReviewHistory {...props} />
        </DialogPanel>
      </DialogPopup>
    </Dialog>
  );
}

export function RecoveredReviewDraft(props: Props) {
  const editor = useReviewDraftEditor(props, null);
  return <ReviewDraftForm editor={editor} />;
}

type Comment = {
  comment_id: string;
  body: string;
  anchor: ReviewDraftAnchorSelection | null;
  location: string | null;
};
type Values = {
  event: ReviewSubmissionEvent;
  body: string;
  comments: Comment[];
};
const empty: Values = { event: "comment", body: "", comments: [] };
function valuesFrom(snapshot: ReviewDraftSnapshot): Values {
  return {
    event: snapshot.event,
    body: snapshot.body,
    comments: snapshot.comments.map((comment) => {
      const anchor =
        comment.anchor?.provider === "github" ? comment.anchor.anchor : null;
      return {
        comment_id: comment.comment_id,
        body: comment.body,
        anchor: anchor ? selectionFrom(anchor) : null,
        location: anchor
          ? `${anchor.path}, ${anchor.side} line ${anchor.line}`
          : null,
      };
    }),
  };
}
function selectionFrom(
  anchor: ReviewDraftAnchorSelection,
): ReviewDraftAnchorSelection {
  return {
    file_facet_revision: anchor.file_facet_revision,
    context: anchor.context,
    file_key: anchor.file_key,
    start_line: anchor.start_line,
    line: anchor.line,
    start_side: anchor.start_side,
    side: anchor.side,
  };
}
function authored(values: Values) {
  return JSON.stringify([
    values.event,
    values.body,
    values.comments.map(({ comment_id, body, anchor }) => [
      comment_id,
      body,
      anchor,
    ]),
  ]);
}
function redact(values: Values): Values {
  return {
    ...values,
    comments: values.comments.map((comment) => ({
      ...comment,
      anchor: null,
      location: null,
    })),
  };
}
function validate(values: Values) {
  const size = (value: string) => new TextEncoder().encode(value).length;
  if (values.comments.length > 25)
    return "A review can contain up to 25 inline comments.";
  if (
    [values.body, ...values.comments.map((comment) => comment.body)].some(
      (body) => size(body) > 16_384 || body.includes("\0"),
    )
  )
    return "Keep each review message under 16 KiB and remove null characters.";
  if (
    size(values.body) +
      values.comments.reduce((sum, comment) => sum + size(comment.body), 0) >
    128 * 1024
  )
    return "This review is too large. Shorten or remove some inline comments.";
  if (values.comments.some((comment) => !comment.body.trim()))
    return "Write a message for each inline comment.";
  if (values.comments.some((comment) => !comment.anchor))
    return "Some comments need a fresh diff selection. Keep their text, then remove and add them from the current provider diff.";
  return null;
}

function useReviewDraftEditor(
  { account, subjectId }: Props,
  selection: IncomingSelection | null,
) {
  const options = reviewDraftQueryOptions(account, subjectId);
  const query = useQuery(options);
  const cache = useQueryClient();
  const authorityVersion = useCollaborationAuthorityVersion();
  const authorityIdentity = `${authorityVersion}:${account.authorization_epoch}:${account.state}`;
  const [seenAuthority, setSeenAuthority] = useState(authorityIdentity);
  const [seenError, setSeenError] = useState(false);
  const [seenSelection, setSeenSelection] = useState<string | null>(null);
  const [base, setBase] = useState<ReviewDraftSnapshot | null>(null);
  const [values, setValues] = useState<Values>(empty);
  const [previous, setPrevious] = useState<Values | null>(null);
  const [consent, setConsent] = useState<{
    background: boolean;
    race: boolean;
    context: string | null;
    authority: number;
  }>({
    background: false,
    race: false,
    context: null,
    authority: authorityVersion,
  });
  const [busy, setBusy] = useState(false);
  const [retry, setRetry] = useState<{
    request: SubmitReviewRequest;
    authority: number;
  } | null>(null);
  const [admittedGeneration, setAdmittedGeneration] = useState<string | null>(
    null,
  );
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const id = useId();
  const current = query.data;
  // Render-time retirement prevents one frame of provider authority after reset.
  if (seenAuthority !== authorityIdentity || query.isError !== seenError) {
    setSeenAuthority(authorityIdentity);
    setSeenError(query.isError);
    if (seenAuthority !== authorityIdentity || query.isError) {
      setValues(redact(values));
      setPrevious((value) => value && redact(value));
      setBase(
        (value) =>
          value && {
            ...value,
            context: null,
            comments: value.comments.map((comment) => ({
              ...comment,
              anchor: null,
            })),
          },
      );
      setConsent({
        background: false,
        race: false,
        context: null,
        authority: authorityVersion,
      });
    }
  }
  if (current && !base) {
    const initial =
      query.isError ||
      account.state !== "active" ||
      (current.context !== null &&
        current.context.authorization_epoch !== account.authorization_epoch)
        ? {
            ...current,
            context: null,
            comments: current.comments.map((comment) => ({
              ...comment,
              anchor: null,
            })),
          }
        : current;
    setBase(initial);
    setValues(valuesFrom(initial));
  }
  if (base && selection && seenSelection !== selection.id) {
    setSeenSelection(selection.id);
    if (
      selection.authorityVersion === authorityVersion &&
      selection.authorizationEpoch === account.authorization_epoch &&
      account.state === "active" &&
      !query.isError
    ) {
      setValues((value) => ({
        ...value,
        comments: [
          ...value.comments,
          {
            comment_id: selection.id,
            body: "",
            anchor: selection.anchor,
            location: selection.location,
          },
        ],
      }));
      setConsent({
        background: false,
        race: false,
        context: null,
        authority: authorityVersion,
      });
    }
  }
  const dirty =
    base !== null && authored(values) !== authored(valuesFrom(base));
  const baseChanged =
    base !== null &&
    current !== undefined &&
    (base.generation !== current.generation ||
      base.authorization_view !== current.authorization_view);
  const contextCurrent =
    current?.context !== null &&
    current?.context !== undefined &&
    account.state === "active" &&
    current.context.authorization_epoch === account.authorization_epoch &&
    !query.isError;
  const contextKey =
    contextCurrent && current?.context ? JSON.stringify(current.context) : null;
  const consentCurrent =
    consent.context === contextKey && consent.authority === authorityVersion;
  const validation = validate(values);
  const available =
    base !== null && current !== undefined && !query.isError && !baseChanged;
  const retryObserved =
    retry !== null &&
    current?.submission?.command_id === retry.request.command_id;
  const activeRetry = retryObserved ? null : retry;
  const editable = !busy && activeRetry === null;
  const canSave = available && editable && dirty && !validation;
  const canSubmit =
    available &&
    editable &&
    !dirty &&
    contextCurrent &&
    current.context !== null &&
    current.availability === "available" &&
    current.submission === null &&
    current.generation !== "0" &&
    admittedGeneration !== current.generation &&
    account.state === "active" &&
    validation === null &&
    (values.event === "approve" || values.body.trim().length > 0) &&
    consentCurrent &&
    consent.background &&
    consent.race;
  const retryAllowed =
    !busy &&
    !query.isError &&
    account.state === "active" &&
    activeRetry !== null &&
    activeRetry.authority === authorityVersion &&
    current !== undefined &&
    activeRetry.request.context.authorization_epoch ===
      account.authorization_epoch &&
    activeRetry.request.context.authorization_view ===
      current.authorization_view;

  function edit(change: Partial<Values>) {
    setValues((value) => ({ ...value, ...change }));
    setConsent({
      background: false,
      race: false,
      context: null,
      authority: authorityVersion,
    });
    setNotice(null);
  }
  function accept(snapshot: ReviewDraftSnapshot) {
    cache.setQueryData(options.queryKey, snapshot);
    setBase(snapshot);
    setValues(valuesFrom(snapshot));
    setConsent({
      background: false,
      race: false,
      context: null,
      authority: authorityVersion,
    });
  }
  async function save(event: FormEvent) {
    event.preventDefault();
    if (!canSave || !base) return;
    setBusy(true);
    setError(null);
    setNotice(null);
    try {
      accept(
        await collaboration.forAccount(account).saveReviewDraft({
          key: { account_id: account.id, subject_id: subjectId },
          authorization_view: base.authorization_view,
          expected_generation: base.generation,
          event: values.event,
          body: values.body,
          comments: values.comments.flatMap((comment) =>
            comment.anchor
              ? [
                  {
                    comment_id: comment.comment_id,
                    body: comment.body,
                    anchor: comment.anchor,
                  },
                ]
              : [],
          ),
        }),
      );
      setNotice("Review draft saved on this device.");
      void cache.invalidateQueries({
        queryKey: reviewDraftsQueryOptions(account, {
          cursor: null,
          limit: 50,
        }).queryKey.slice(0, -1),
      });
    } catch (failure) {
      setError(collaborationErrorMessage(failure));
      void cache.invalidateQueries({ queryKey: options.queryKey });
    } finally {
      setBusy(false);
    }
  }
  async function submit(request: SubmitReviewRequest) {
    setBusy(true);
    setError(null);
    setNotice(null);
    setRetry({ request, authority: authorityVersion });
    try {
      const receipt = await collaboration
        .forAccount(account)
        .submitReview(request);
      setRetry(null);
      setAdmittedGeneration(request.draft_generation);
      setConsent({
        background: false,
        race: false,
        context: null,
        authority: authorityVersion,
      });
      setNotice(
        receipt.duplicate
          ? "This exact review request is already recorded locally."
          : "Review queued. Awaiting provider confirmation.",
      );
      void collaboration.wake();
    } catch (failure) {
      if (
        typeof failure === "object" &&
        failure !== null &&
        "code" in failure &&
        failure.code === "invalid_input"
      ) {
        setRetry(null);
        setConsent({
          background: false,
          race: false,
          context: null,
          authority: authorityVersion,
        });
        setError(
          "Check the selected diff lines and try shorter review messages before submitting again. Your draft is still saved.",
        );
      } else setError(collaborationErrorMessage(failure));
    } finally {
      setBusy(false);
      void cache.invalidateQueries({ queryKey: options.queryKey });
    }
  }
  function queue() {
    if (!canSubmit || !current?.context) return;
    void submit({
      context: current.context,
      draft_generation: current.generation,
      command_id: crypto.randomUUID(),
      accept_background_delivery: true,
      accept_best_effort_race: true,
    });
  }
  return {
    account,
    contextCurrent,
    query,
    current,
    base,
    values,
    previous,
    id,
    busy,
    editable,
    dirty,
    baseChanged,
    validation,
    canSave,
    canSubmit,
    activeRetry,
    retryAllowed,
    error,
    notice,
    background: consentCurrent && consent.background,
    race: consentCurrent && consent.race,
    setConsent: (key: "background" | "race", checked: boolean) =>
      setConsent((value) => ({
        ...(consentCurrent ? value : { background: false, race: false }),
        [key]: checked,
        context: contextKey,
        authority: authorityVersion,
      })),
    edit,
    save,
    queue,
    retry: () => {
      if (retryAllowed && activeRetry) void submit(activeRetry.request);
    },
    loadLatest: () => {
      if (current && !query.isError) {
        if (dirty) setPrevious(values);
        accept(current);
        setRetry(null);
      }
    },
  };
}

const reasons: Record<ReviewSubmissionReason, string> = {
  unsupported_provider:
    "Submitting reviews from Gitru is currently available for GitHub.com. This draft stays local.",
  account_unavailable: "Reconnect this account before submitting a review.",
  missing_target:
    "The pull request is unavailable. Your authored review stays here.",
  stale_context:
    "The pull request changed. Reload the saved draft and select current diff lines before submitting.",
  missing_provider_diff:
    "Load the selected diff from the provider before adding an inline comment.",
  invalid_anchor:
    "A selected diff line is no longer current. Your comment text is preserved.",
  empty_required_body: "Comment and request-changes reviews need a summary.",
  pending_submission:
    "A review is still being tracked. Check Saved changes before submitting another.",
  already_submitted:
    "This draft version has been submitted. Save a new version to write another review.",
};
function ReviewDraftForm({
  editor: e,
}: {
  editor: ReturnType<typeof useReviewDraftEditor>;
}) {
  if (!e.base)
    return (
      <p role={e.query.isError ? "alert" : "status"}>
        {e.query.isError
          ? collaborationErrorMessage(e.query.error)
          : "Loading saved review draft…"}
      </p>
    );
  return (
    <form className="space-y-4" aria-label="Review draft" onSubmit={e.save}>
      <Field>
        <FieldLabel htmlFor={`${e.id}-event`}>Review decision</FieldLabel>
        <Select
          value={e.values.event}
          onValueChange={(event) => {
            if (event) e.edit({ event: event as ReviewSubmissionEvent });
          }}
        >
          <SelectTrigger id={`${e.id}-event`} disabled={!e.editable}>
            <SelectValue />
          </SelectTrigger>
          <SelectPopup>
            <SelectItem value="comment">Comment</SelectItem>
            <SelectItem value="approve">Approve</SelectItem>
            <SelectItem value="request_changes">Request changes</SelectItem>
          </SelectPopup>
        </Select>
      </Field>
      <Field>
        <FieldLabel htmlFor={`${e.id}-body`}>Review summary</FieldLabel>
        <Textarea
          id={`${e.id}-body`}
          value={e.values.body}
          disabled={!e.editable}
          rows={5}
          onChange={(event) => e.edit({ body: event.target.value })}
        />
      </Field>
      <p className="text-xs text-muted-foreground">
        Add inline comments from a saved provider diff in Files. Saving a draft
        does not send it.
      </p>
      {e.values.comments.map((comment, index) => (
        <fieldset
          key={comment.comment_id}
          className="space-y-2 rounded-md border p-3"
        >
          <legend className="px-1 text-sm">Inline comment {index + 1}</legend>
          <p className="break-all text-xs text-muted-foreground">
            {comment.location ??
              "Diff selection unavailable; authored text preserved."}
          </p>
          <Field>
            <FieldLabel htmlFor={`${e.id}-${comment.comment_id}`}>
              Comment {index + 1}
            </FieldLabel>
            <Textarea
              id={`${e.id}-${comment.comment_id}`}
              value={comment.body}
              disabled={!e.editable}
              onChange={(event) =>
                e.edit({
                  comments: e.values.comments.map((value) =>
                    value.comment_id === comment.comment_id
                      ? { ...value, body: event.target.value }
                      : value,
                  ),
                })
              }
            />
          </Field>
          <Button
            type="button"
            size="sm"
            variant="ghost"
            disabled={!e.editable}
            onClick={() =>
              e.edit({
                comments: e.values.comments.filter(
                  (value) => value.comment_id !== comment.comment_id,
                ),
              })
            }
          >
            Remove comment {index + 1}
          </Button>
        </fieldset>
      ))}
      {e.baseChanged ? (
        <div role="status" className="space-y-2 text-sm">
          <p>The saved draft changed. Your edits are still here.</p>
          <Button
            type="button"
            variant="outline"
            disabled={e.busy || e.query.isError}
            onClick={e.loadLatest}
          >
            Load latest saved review
          </Button>
        </div>
      ) : null}
      {e.previous ? (
        <details>
          <summary>Previous unsaved review text</summary>
          <pre className="whitespace-pre-wrap break-words text-xs">
            {[
              e.previous.body,
              ...e.previous.comments.map((comment) => comment.body),
            ].join("\n\n")}
          </pre>
        </details>
      ) : null}
      {e.validation ? (
        <p role="status" className="text-xs text-muted-foreground">
          {e.validation}
        </p>
      ) : null}
      <Button type="submit" disabled={!e.canSave}>
        {e.busy ? "Working…" : "Save review draft"}
      </Button>
      {e.current?.reason ? (
        <p role="status" className="text-xs text-muted-foreground">
          {reasons[e.current.reason]}
        </p>
      ) : null}
      {e.current?.context && e.contextCurrent ? (
        <p className="break-all text-xs">
          Inspected commit: {e.current.context.review_context.head_oid}
        </p>
      ) : null}
      <div className="space-y-2 border-t pt-3">
        <label className="flex items-start gap-2 text-sm">
          <Checkbox
            aria-labelledby={`${e.id}-background`}
            checked={e.background}
            disabled={!e.editable}
            onCheckedChange={(checked) =>
              e.setConsent("background", checked === true)
            }
          />
          <span id={`${e.id}-background`}>
            Queue this saved review for delivery when GitHub is available.
          </span>
        </label>
        <label className="flex items-start gap-2 text-sm">
          <Checkbox
            aria-labelledby={`${e.id}-race`}
            checked={e.race}
            disabled={!e.editable}
            onCheckedChange={(checked) =>
              e.setConsent("race", checked === true)
            }
          />
          <span id={`${e.id}-race`}>
            I understand a force-push can race the final check. This review
            applies to the inspected commit.
          </span>
        </label>
        <Button type="button" disabled={!e.canSubmit} onClick={e.queue}>
          Submit saved review
        </Button>
      </div>
      {e.activeRetry ? (
        <div className="space-y-2 text-sm">
          <p>
            The local receipt was not received. Recover this same request before
            starting another.
          </p>
          <Button
            type="button"
            variant="outline"
            disabled={!e.retryAllowed}
            onClick={e.retry}
          >
            Recover review receipt
          </Button>
        </div>
      ) : null}
      {e.current?.submission ? (
        <p role="status" className="text-sm">
          {submissionLabel(e.current.submission.state)}
        </p>
      ) : null}
      {e.notice ? (
        <p role="status" className="text-sm">
          {e.notice}
        </p>
      ) : null}
      {e.error || e.query.isError ? (
        <p role="alert" className="text-sm text-destructive-foreground">
          {e.error ?? collaborationErrorMessage(e.query.error)}
        </p>
      ) : null}
    </form>
  );
}
function submissionLabel(state: string) {
  if (state === "confirmed")
    return "The provider confirmed this review at its recorded commit.";
  if (state === "accepted")
    return "GitHub accepted this review. Exact receipt verification is still pending.";
  if (state === "outcome_unknown")
    return "The review outcome is unknown. Gitru will not submit it again automatically; inspect Saved changes.";
  if (state === "conflict")
    return "The pull request changed before submission. Review Saved changes.";
  if (state === "rejected")
    return "The provider rejected this review. Review Saved changes.";
  if (state === "cancelled") return "This review request was cancelled.";
  return "This review is queued or being checked. Awaiting provider confirmation.";
}
function SubmittedReviewHistory({ account, subjectId }: Props) {
  const [cursors, setCursors] = useState<Array<string | null>>([null]);
  const [position, setPosition] = useState(0);
  const authority = useCollaborationAuthorityVersion();
  const [seen, setSeen] = useState(authority);
  if (seen !== authority) {
    setSeen(authority);
    setCursors([null]);
    setPosition(0);
  }
  const cursor = cursors[position];
  const query = useQuery({
    ...submittedReviewsQueryOptions(account, {
      subject_id: subjectId,
      cursor,
      limit: 25,
    }),
    enabled: account.state === "active",
  });
  if (account.state !== "active") return null;
  const next = query.data?.next_cursor;
  const canNext =
    !query.isError &&
    !query.isFetching &&
    next != null &&
    !cursors.includes(next) &&
    position < 99;
  return (
    <section
      className="mt-4 space-y-2 border-t pt-3"
      aria-label="Submitted from Gitru"
    >
      <h3 className="text-sm font-medium">Submitted from Gitru</h3>
      <p className="text-xs text-muted-foreground">
        Exact submission receipts. This history does not describe all provider
        reviews or approval of the latest commit.
      </p>
      {query.isError ? (
        <p role="alert">{collaborationErrorMessage(query.error)}</p>
      ) : (
        query.data?.reviews.map((review) => (
          <article
            key={review.command_id}
            className="space-y-1 rounded-md border p-3 text-xs"
          >
            <p>
              {review.confirmed
                ? "Confirmed"
                : "Accepted; verification pending"}{" "}
              · {review.event.replace(/_/g, " ")}
            </p>
            <p className="break-all">
              Reviewed commit: {review.reviewed_commit_oid}
            </p>
            <p className="whitespace-pre-wrap break-words">{review.body}</p>
            <ProviderLink url={review.url} />
          </article>
        ))
      )}
      <div className="flex gap-2">
        <Button
          type="button"
          size="sm"
          variant="ghost"
          disabled={position === 0 || query.isFetching}
          onClick={() => setPosition((value) => value - 1)}
        >
          Previous receipts
        </Button>
        <Button
          type="button"
          size="sm"
          variant="ghost"
          disabled={!canNext}
          onClick={() => {
            if (canNext && next) {
              setCursors([...cursors.slice(0, position + 1), next]);
              setPosition(position + 1);
            }
          }}
        >
          Next receipts
        </Button>
      </div>
    </section>
  );
}

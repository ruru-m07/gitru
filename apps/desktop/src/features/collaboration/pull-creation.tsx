import {
  collaboration,
  collaborationErrorMessage,
  type LocalLinkInspection,
  type PullCreationPreview,
  type PullCreationReason,
  type PullDraftSnapshot,
  type PullDraftValues,
  type RemoteAccount,
  type RemoteRepository,
  type SubmitPullRequest,
} from "@gitru/collaboration-client";
import {
  localLinksQueryOptions,
  pullDraftQueryOptions,
  pullDraftsQueryOptions,
  useCollaborationAuthorityVersion,
  useCollaborationVersion,
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
  DialogTrigger,
} from "@gitru/ui/components/dialog";
import { Field, FieldLabel } from "@gitru/ui/components/field";
import { Input } from "@gitru/ui/components/input";
import {
  Select,
  SelectItem,
  SelectPopup,
  SelectTrigger,
  SelectValue,
} from "@gitru/ui/components/select";
import { Textarea } from "@gitru/ui/components/textarea";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { CirclePlus } from "lucide-react";
import { type FormEvent, useEffect, useId, useState } from "react";
import { useAppStore } from "@/store/use-app-store";
import { type LocalLinkRouteTarget } from "./local-link-navigation";
import { ProviderLink } from "./provider-link";

const emptyValues: PullDraftValues = {
  title: "",
  body: "",
  source_branch: "",
  base_branch: "",
  local_repository_id: "",
  link_id: "",
  link_generation: "",
  is_draft: true,
};
type Props = {
  account: RemoteAccount;
  repository: RemoteRepository;
  target?: LocalLinkRouteTarget;
  onOpenCreated: (subjectId: string) => void;
};

export function NewPullDialog(props: Props) {
  const [session, setSession] = useState<{ id: string; open: boolean } | null>(
    null,
  );
  if (!session)
    return (
      <Button
        type="button"
        size="sm"
        onClick={() => setSession({ id: crypto.randomUUID(), open: true })}
      >
        <CirclePlus aria-hidden="true" />
        New pull request
      </Button>
    );
  return (
    <NewPullSession
      key={session.id}
      {...props}
      draftId={session.id}
      open={session.open}
      onOpenChange={(open) =>
        setSession((value) => value && { ...value, open })
      }
      onStartAnother={() => setSession({ id: crypto.randomUUID(), open: true })}
    />
  );
}
function NewPullSession({
  account,
  repository,
  target,
  onOpenCreated,
  draftId,
  open,
  onOpenChange,
  onStartAnother,
}: Props & {
  draftId: string;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onStartAnother: () => void;
}) {
  const editor = usePullDraftEditor({
    account,
    draftId,
    repositoryId: repository.id,
    initialTarget: target,
  });
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogTrigger render={<Button type="button" size="sm" />}>
        <CirclePlus aria-hidden="true" />
        New pull request
      </DialogTrigger>
      <DialogPopup className="sm:max-w-2xl">
        <DialogHeader>
          <DialogTitle>New pull request</DialogTitle>
          <DialogDescription>
            Create a pull request in {repository.full_name} from two published
            branches in this repository. Save locally, then check the branches
            online before creating.
          </DialogDescription>
        </DialogHeader>
        <DialogPanel className="max-h-[65dvh] overflow-y-auto">
          <PullDraftForm
            editor={editor}
            onOpenCreated={(id) => {
              onOpenChange(false);
              onOpenCreated(id);
            }}
          />
        </DialogPanel>
        {editor.current?.published && !editor.dirty ? (
          <Button type="button" variant="outline" onClick={onStartAnother}>
            Start another pull request draft
          </Button>
        ) : null}
      </DialogPopup>
    </Dialog>
  );
}

export function RecoveredPullDraft(props: {
  account: RemoteAccount;
  draftId: string;
  repositoryId: string;
  onOpenCreated?: (subjectId: string) => void;
}) {
  const editor = usePullDraftEditor(props);
  return <PullDraftForm editor={editor} onOpenCreated={props.onOpenCreated} />;
}

function usePullDraftEditor({
  account,
  draftId,
  repositoryId,
  initialTarget,
}: {
  account: RemoteAccount;
  draftId: string;
  repositoryId: string;
  initialTarget?: LocalLinkRouteTarget;
}) {
  const key = {
    account_id: account.id,
    draft_id: draftId,
    repository_id: repositoryId,
  };
  const options = pullDraftQueryOptions(account, key);
  const query = useQuery(options);
  const cache = useQueryClient();
  const authorityVersion = useCollaborationAuthorityVersion();
  const version = useCollaborationVersion();
  const [base, setBase] = useState<PullDraftSnapshot | null>(null);
  const [values, setValues] = useState<PullDraftValues>(emptyValues);
  const [previous, setPrevious] = useState<PullDraftValues | null>(null);
  const [preview, setPreview] = useState<{
    result: PullCreationPreview;
    authorityVersion: number;
    deadline: number;
  } | null>(null);
  const [expired, setExpired] = useState(false);
  const [consent, setConsent] = useState(false);
  const [busy, setBusy] = useState(false);
  const [retry, setRetry] = useState<{
    request: SubmitPullRequest;
    authorityVersion: number;
  } | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const id = useId();
  const linksQuery = useQuery({
    ...localLinksQueryOptions(values.local_repository_id, version),
    enabled: values.local_repository_id.length > 0,
  });
  const linkCurrent =
    !linksQuery.isError &&
    linksQuery.data?.observation_error === null &&
    linksQuery.data.snapshot.links.some(
      (link) =>
        link.id === values.link_id &&
        link.generation === values.link_generation &&
        link.state === "linked" &&
        link.account_id === account.id &&
        link.actor_id === account.actor_id &&
        link.repository_id === repositoryId,
    );

  if (query.data && base === null) {
    setBase(query.data);
    setValues(
      query.data.generation === "0" && initialTarget
        ? {
            ...query.data.values,
            local_repository_id: initialTarget.local_repository_id,
            link_id: initialTarget.link_id,
            link_generation: initialTarget.generation,
          }
        : query.data.values,
    );
  }
  // Presentation only: the native process owns and validates the monotonic grant.
  useEffect(() => {
    if (!preview) return;
    const timer = setTimeout(
      () => setExpired(true),
      Math.max(0, preview.deadline - performance.now()),
    );
    return () => clearTimeout(timer);
  }, [preview]);
  const current = query.data;
  const dirty = base !== null && !sameValues(values, base.values);
  const baseChanged =
    base !== null &&
    current !== undefined &&
    (current.generation !== base.generation ||
      current.authorization_view !== base.authorization_view);
  const validation = validateValues(values);
  const available =
    current !== undefined && base !== null && !query.isError && !baseChanged;
  const editable = !busy && retry === null;
  const canSave =
    available &&
    editable &&
    dirty &&
    current.published === null &&
    validation === null;
  const canPreview =
    available &&
    editable &&
    !dirty &&
    current.can_preview &&
    linkCurrent &&
    account.state === "active";
  const previewCurrent =
    available &&
    !dirty &&
    preview !== null &&
    preview.authorityVersion === authorityVersion &&
    preview.result.authorization_view === current.authorization_view &&
    preview.result.context?.draft_generation === current.generation &&
    preview.result.context.authorization_epoch ===
      account.authorization_epoch &&
    sameValues(values, preview.result.values);
  const canSubmit =
    canPreview &&
    previewCurrent &&
    !expired &&
    consent &&
    preview.result.context !== null;
  const retryAllowed =
    !busy &&
    !query.isError &&
    retry !== null &&
    retry.authorityVersion === authorityVersion &&
    current !== undefined &&
    account.state === "active" &&
    retry.request.context.authorization_epoch === account.authorization_epoch &&
    retry.request.context.authorization_view === current.authorization_view;

  function edit(patch: Partial<PullDraftValues>) {
    setValues((value) => ({ ...value, ...patch }));
    setPreview(null);
    setConsent(false);
    setExpired(false);
    setNotice(null);
  }
  function accept(snapshot: PullDraftSnapshot) {
    cache.setQueryData(options.queryKey, snapshot);
    setBase(snapshot);
    setValues(snapshot.values);
    setPreview(null);
    setConsent(false);
    setExpired(false);
  }
  async function save(event: FormEvent) {
    event.preventDefault();
    if (!canSave || !base) return;
    setBusy(true);
    setError(null);
    setNotice(null);
    try {
      accept(
        await collaboration.forAccount(account).savePullDraft({
          key,
          authorization_view: base.authorization_view,
          expected_generation: base.generation,
          values,
        }),
      );
      setNotice("Pull request draft saved on this device.");
      void cache.invalidateQueries({
        queryKey: pullDraftsQueryOptions(account, {
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
  async function checkOnline() {
    if (!canPreview || !current) return;
    setBusy(true);
    setPreview(null);
    setConsent(false);
    setExpired(false);
    setError(null);
    setNotice(null);
    const generation = collaboration.getAuthorityVersion();
    const started = performance.now();
    try {
      const result = await collaboration
        .forAccount(account)
        .previewPullCreation({
          key,
          draft_generation: current.generation,
          authorization_view: current.authorization_view,
        });
      setPreview({
        result,
        authorityVersion: generation,
        deadline: started + result.expires_in_seconds * 1000,
      });
    } catch (failure) {
      setError(collaborationErrorMessage(failure));
    } finally {
      setBusy(false);
    }
  }
  async function submit(request: SubmitPullRequest) {
    setBusy(true);
    setRetry({ request, authorityVersion });
    setError(null);
    setNotice(null);
    try {
      const receipt = await collaboration
        .forAccount(account)
        .submitPull(request);
      setRetry(null);
      setPreview(null);
      setConsent(false);
      setNotice(
        receipt.duplicate
          ? "This exact creation request is already recorded locally."
          : "Creation request recorded. Awaiting GitHub confirmation.",
      );
      void collaboration.wake();
    } catch (failure) {
      setError(collaborationErrorMessage(failure));
    } finally {
      setBusy(false);
      void cache.invalidateQueries({ queryKey: options.queryKey });
    }
  }
  function create() {
    if (
      !canSubmit ||
      !preview.result.context ||
      performance.now() >= preview.deadline
    )
      return;
    void submit({
      context: preview.result.context,
      command_id: crypto.randomUUID(),
      policy: "best_effort_current_branches",
      confirm_current_branches: true,
    });
  }
  return {
    account,
    repositoryId,
    query,
    base,
    values,
    previous,
    preview,
    expired,
    consent,
    busy,
    retry,
    error,
    notice,
    id,
    linksQuery,
    current,
    dirty,
    baseChanged,
    editable,
    canSave,
    canPreview,
    previewCurrent,
    canSubmit,
    retryAllowed,
    validation,
    authorityVersion,
    edit,
    accept,
    save,
    checkOnline,
    submit,
    create,
    setPrevious,
    setError,
    setConsent,
  };
}
type PullDraftEditor = ReturnType<typeof usePullDraftEditor>;
function PullDraftForm({
  editor,
  onOpenCreated,
}: {
  editor: PullDraftEditor;
  onOpenCreated?: (subjectId: string) => void;
}) {
  const {
    account,
    repositoryId,
    query,
    base,
    values,
    previous,
    preview,
    expired,
    consent,
    busy,
    retry,
    error,
    notice,
    id,
    linksQuery,
    current,
    dirty,
    baseChanged,
    editable,
    canSave,
    canPreview,
    previewCurrent,
    canSubmit,
    retryAllowed,
    validation,
    authorityVersion,
    edit,
    accept,
    save,
    checkOnline,
    submit,
    create,
    setPrevious,
    setError,
    setConsent,
  } = editor;
  if (query.isPending || (!base && !query.isError))
    return (
      <p role="status" className="text-sm text-muted-foreground">
        Loading local pull request draft…
      </p>
    );
  if (!base || !current)
    return <p role="alert">{collaborationErrorMessage(query.error)}</p>;
  const published =
    account.state === "active" && !query.isError ? current.published : null;
  return (
    <form className="space-y-4" onSubmit={save}>
      {query.isError ? (
        <p role="alert" className="text-xs text-destructive-foreground">
          {collaborationErrorMessage(query.error)} Reload the local draft before
          saving or creating.
        </p>
      ) : null}
      <Field>
        <FieldLabel htmlFor={`${id}-title`}>Title</FieldLabel>
        <Input
          id={`${id}-title`}
          value={values.title}
          disabled={!editable}
          onChange={(event) => edit({ title: event.currentTarget.value })}
        />
      </Field>
      <Field>
        <FieldLabel htmlFor={`${id}-body`}>Description</FieldLabel>
        <Textarea
          id={`${id}-body`}
          rows={6}
          value={values.body}
          disabled={!editable}
          onChange={(event) => edit({ body: event.currentTarget.value })}
        />
      </Field>
      <div className="grid gap-3 sm:grid-cols-2">
        <Field>
          <FieldLabel htmlFor={`${id}-source`}>Source branch</FieldLabel>
          <Input
            id={`${id}-source`}
            value={values.source_branch}
            disabled={!editable}
            onChange={(event) =>
              edit({ source_branch: event.currentTarget.value })
            }
            placeholder="feature/my-change"
          />
        </Field>
        <Field>
          <FieldLabel htmlFor={`${id}-base`}>Base branch</FieldLabel>
          <Input
            id={`${id}-base`}
            value={values.base_branch}
            disabled={!editable}
            onChange={(event) =>
              edit({ base_branch: event.currentTarget.value })
            }
            placeholder="main"
          />
        </Field>
      </div>
      <PullCloneChoice
        inspection={linksQuery.data}
        observationError={
          linksQuery.isError
            ? collaborationErrorMessage(linksQuery.error)
            : null
        }
        pending={linksQuery.isPending}
        account={account}
        repositoryId={repositoryId}
        values={values}
        disabled={!editable}
        onChange={edit}
      />
      <label className="flex items-center gap-2 text-sm">
        <Checkbox
          aria-label="Create as draft"
          checked={values.is_draft}
          disabled={!editable}
          onCheckedChange={(checked) => edit({ is_draft: checked === true })}
        />
        Create as draft
      </label>
      {validation ? (
        <p role="alert" className="text-xs text-destructive-foreground">
          {validation}
        </p>
      ) : null}
      {baseChanged ? (
        <div className="space-y-2 text-xs">
          <p>
            The saved draft or account changed. Your edits are still here. Load
            the latest local draft to continue.
          </p>
          <Button
            type="button"
            size="sm"
            variant="outline"
            disabled={!editable || query.isError}
            onClick={() => {
              if (dirty) setPrevious(values);
              accept(current);
              setError(null);
            }}
          >
            Load latest pull request draft
          </Button>
        </div>
      ) : null}
      {previous ? (
        <details className="text-xs">
          <summary>Your previous draft text</summary>
          <p className="mt-2 whitespace-pre-wrap break-words">
            {previous.title}\n{previous.body}
          </p>
          <p>
            {previous.source_branch} → {previous.base_branch}
          </p>
        </details>
      ) : null}
      <div className="flex flex-wrap gap-2">
        <Button type="submit" size="sm" variant="outline" disabled={!canSave}>
          Save pull request draft
        </Button>
        <Button
          type="button"
          size="sm"
          variant="outline"
          disabled={!canPreview}
          onClick={() => void checkOnline()}
        >
          {busy && !retry ? "Checking…" : "Check branches online"}
        </Button>
      </div>
      {current.reason ? (
        <p className="text-xs text-muted-foreground">
          {reasonMessage(current.reason)}
        </p>
      ) : null}
      {preview &&
      preview.authorityVersion === authorityVersion &&
      preview.result.authorization_view === current.authorization_view ? (
        <div className="space-y-3 rounded-md border p-3 text-xs">
          {preview.result.reason ? (
            <p>{reasonMessage(preview.result.reason)}</p>
          ) : (
            <>
              <p>
                GitHub currently reports push permission. The published source
                matches the checked local branch.
              </p>
              <p className="break-all">
                Source: <code>{preview.result.observed_source_oid}</code>
              </p>
              <p className="break-all">
                Base: <code>{preview.result.observed_base_oid}</code>
              </p>
              <label className="flex items-start gap-2">
                <Checkbox
                  aria-labelledby={`${id}-consent`}
                  checked={consent}
                  disabled={!previewCurrent || expired || !editable}
                  onCheckedChange={(checked) => setConsent(checked === true)}
                />
                <span id={`${id}-consent`}>
                  I want to create this pull request against the branches’
                  current tips. Branches can move after this check.
                </span>
              </label>
              {expired ? (
                <p role="status">
                  This branch check expired. Check online again.
                </p>
              ) : !previewCurrent ? (
                <p role="status">The draft changed. Check online again.</p>
              ) : null}
              <Button
                type="button"
                size="sm"
                disabled={!canSubmit}
                onClick={create}
              >
                Create pull request
              </Button>
            </>
          )}
        </div>
      ) : null}
      {current.submission ? (
        <p role="status" className="text-xs">
          {current.submission.quarantined
            ? "This restored submission needs review in Saved changes."
            : current.submission.state === "unknown"
              ? "The creation outcome is uncertain. Inspect Saved changes before taking further action; Gitru will not automatically send it again."
              : "This submission is tracked in Saved changes."}
        </p>
      ) : null}
      {published ? (
        <div className="space-y-2 rounded-md border p-3">
          <p role="status" className="text-sm font-medium">
            Pull request #{published.number} was confirmed by GitHub.
          </p>
          {published.branches_changed ? (
            <div className="space-y-1 text-xs">
              <p>
                The branches moved after inspection. The created pull request is
                preserved.
              </p>
              <p className="break-all">
                Source inspected {published.inspected_source_oid}; created{" "}
                {published.observed_source_oid}.
              </p>
              <p className="break-all">
                Base inspected {published.inspected_base_oid}; created{" "}
                {published.observed_base_oid}.
              </p>
            </div>
          ) : null}
          <div className="flex flex-wrap gap-2">
            {onOpenCreated ? (
              <Button
                type="button"
                size="sm"
                onClick={() => onOpenCreated(published.subject_id)}
              >
                Open created pull request
              </Button>
            ) : null}
            <ProviderLink url={published.url} />
          </div>
        </div>
      ) : null}
      {error ? (
        <div className="space-y-2">
          <p role="alert" className="text-xs text-destructive-foreground">
            {error} Your draft
            {retry ? " and exact request identity are" : " is"} preserved.
          </p>
          {retry ? (
            <Button
              type="button"
              size="sm"
              variant="outline"
              disabled={!retryAllowed}
              onClick={() => retry && void submit(retry.request)}
            >
              Recover exact local receipt
            </Button>
          ) : null}
        </div>
      ) : null}
      {notice ? (
        <p role="status" className="text-xs">
          {notice}
        </p>
      ) : null}
    </form>
  );
}

function PullCloneChoice({
  inspection,
  observationError,
  pending,
  account,
  repositoryId,
  values,
  disabled,
  onChange,
}: {
  inspection: LocalLinkInspection | undefined;
  observationError: string | null;
  pending: boolean;
  account: RemoteAccount;
  repositoryId: string;
  values: PullDraftValues;
  disabled: boolean;
  onChange: (values: Partial<PullDraftValues>) => void;
}) {
  const repositories = useAppStore((state) => state.repositories);
  const choices = repositories.map((repo) => ({
    label: repo.name,
    value: repo.id,
  }));
  return (
    <div className="space-y-2">
      <Select
        items={choices}
        value={values.local_repository_id || null}
        disabled={disabled}
        onValueChange={(value) =>
          onChange({
            local_repository_id: value ?? "",
            link_id: "",
            link_generation: "",
          })
        }
      >
        <SelectTrigger aria-label="Local source repository" className="w-full">
          <SelectValue>
            {(value) =>
              choices.find((choice) => choice.value === value)?.label ??
              "Choose the local source repository"
            }
          </SelectValue>
        </SelectTrigger>
        <SelectPopup>
          {choices.map((choice) => (
            <SelectItem key={choice.value} value={choice.value}>
              {choice.label}
            </SelectItem>
          ))}
        </SelectPopup>
      </Select>
      {!repositories.length ? (
        <p className="text-xs text-muted-foreground">
          Open a local clone and link it to this provider repository in Linked
          collaboration.
        </p>
      ) : null}
      {values.local_repository_id ? (
        <PullLinkChoice
          inspection={inspection}
          observationError={observationError}
          pending={pending}
          key={values.local_repository_id}
          account={account}
          repositoryId={repositoryId}
          values={values}
          disabled={disabled}
          onChange={onChange}
        />
      ) : null}
    </div>
  );
}
function PullLinkChoice({
  inspection,
  observationError,
  pending,
  account,
  repositoryId,
  values,
  disabled,
  onChange,
}: {
  inspection: LocalLinkInspection | undefined;
  observationError: string | null;
  pending: boolean;
  account: RemoteAccount;
  repositoryId: string;
  values: PullDraftValues;
  disabled: boolean;
  onChange: (values: Partial<PullDraftValues>) => void;
}) {
  const links =
    inspection?.snapshot.links.filter(
      (link) =>
        link.state === "linked" &&
        link.account_id === account.id &&
        link.actor_id === account.actor_id &&
        link.repository_id === repositoryId,
    ) ?? [];
  const choice = links.find(
    (link) =>
      link.id === values.link_id && link.generation === values.link_generation,
  );
  return (
    <div className="space-y-2">
      <Select
        items={links.map((link) => ({
          label: `${link.endpoint.remote_name} · ${link.endpoint.direction}`,
          value: link.id,
        }))}
        value={choice?.id ?? null}
        disabled={disabled || observationError !== null || pending}
        onValueChange={(id) => {
          const link = links.find((candidate) => candidate.id === id);
          if (link)
            onChange({ link_id: link.id, link_generation: link.generation });
        }}
      >
        <SelectTrigger aria-label="Source repository link" className="w-full">
          <SelectValue>
            {() =>
              choice
                ? `${choice.endpoint.remote_name} · ${choice.endpoint.direction}`
                : "Choose a confirmed repository link"
            }
          </SelectValue>
        </SelectTrigger>
        <SelectPopup>
          {links.map((link) => (
            <SelectItem key={link.id} value={link.id}>
              {link.endpoint.remote_name} · {link.endpoint.direction}
            </SelectItem>
          ))}
        </SelectPopup>
      </Select>
      {observationError ? (
        <p role="alert" className="text-xs">
          {observationError}
        </p>
      ) : !pending && !choice ? (
        <p className="text-xs text-muted-foreground">
          Choose a current confirmed link to this account and repository. Manage
          links in Linked collaboration.
        </p>
      ) : null}
    </div>
  );
}
function sameValues(a: PullDraftValues, b: PullDraftValues) {
  return (Object.keys(emptyValues) as Array<keyof PullDraftValues>).every(
    (field) => a[field] === b[field],
  );
}
function validateValues(values: PullDraftValues) {
  const size = (text: string) => new TextEncoder().encode(text).length;
  if (
    Array.from(values.title).length > 256 ||
    size(values.title) > 1024 ||
    /[\u0000-\u001f\u007f]/.test(values.title)
  )
    return "Use a single-line title of at most 256 characters and 1,024 bytes.";
  if (size(values.body) > 16384 || values.body.includes("\0"))
    return "The description must be at most 16,384 bytes with no null characters.";
  if (size(values.source_branch) > 1024 || size(values.base_branch) > 1024)
    return "Branch names must be at most 1,024 bytes.";
  return null;
}
function reasonMessage(reason: PullCreationReason): string {
  const messages: Record<PullCreationReason, string> = {
    unsupported_provider:
      "Pull request creation currently supports GitHub.com. This draft remains local.",
    account_unavailable:
      "Reconnect this account before checking branches. Your draft remains editable.",
    missing_repository:
      "This provider repository is unavailable. Your draft is preserved.",
    incomplete_draft:
      "Save a title, distinct branch names, local clone and confirmed repository link before checking online.",
    pending_submission:
      "A submission for this draft is already tracked in Saved changes.",
    already_submitted: "This draft has a confirmed created pull request.",
    local_mapping_changed:
      "The local repository link changed. Choose a current link and save before checking again.",
    local_head_changed:
      "The local branch changed. Check the source branch and save before trying again.",
    unpublished_source:
      "Publish this source branch from Local Git before checking again.",
    same_branch: "Choose different source and base branches.",
    permission_unavailable:
      "GitHub did not confirm permission to create this pull request.",
    grant_expired:
      "The branch check expired. Check online again before creating.",
  };
  return messages[reason];
}

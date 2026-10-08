import {
  collaboration,
  collaborationErrorMessage,
  type IssueMetadataKind,
  type IssueMetadataOption,
  type IssueMetadataOutcome,
  type IssueMetadataReference,
  type IssueMetadataSelection,
  type RemoteAccount,
} from "@gitru/collaboration-client";
import {
  issueMetadataOptionsQueryOptions,
  useCollaborationAuthorityVersion,
  useVisibleDemand,
} from "@gitru/collaboration-client/react";
import { Button } from "@gitru/ui/components/button";
import {
  Collapsible,
  CollapsiblePanel,
  CollapsibleTrigger,
} from "@gitru/ui/components/collapsible";
import { Input } from "@gitru/ui/components/input";
import { useQuery } from "@tanstack/react-query";
import { useId, useState } from "react";

const families = ["labels", "assignees", "milestones"] as const;
const names = {
  labels: "Labels",
  assignees: "Assignees",
  milestones: "Milestone",
};

export function emptyIssueMetadata(): IssueMetadataSelection {
  return { labels: [], assignees: [], milestone: null };
}

export function hasIssueMetadata(value: IssueMetadataSelection) {
  return (
    value.labels.length > 0 ||
    value.assignees.length > 0 ||
    value.milestone !== null
  );
}

export function issueMetadataSummary(value: IssueMetadataSelection) {
  return [
    ...value.labels.map((label) => label.name),
    ...value.assignees.map((assignee) => `@${assignee.login}`),
    ...(value.milestone ? [value.milestone.title] : []),
  ].join(" · ");
}

function references(
  value: IssueMetadataSelection,
  kind: IssueMetadataKind,
): IssueMetadataReference[] {
  if (kind === "labels")
    return value.labels.map((item) => ({ kind: "label", value: item }));
  if (kind === "assignees")
    return value.assignees.map((item) => ({ kind: "assignee", value: item }));
  return value.milestone ? [{ kind: "milestone", value: value.milestone }] : [];
}

function referenceName(reference: IssueMetadataReference) {
  switch (reference.kind) {
    case "label":
      return reference.value.name;
    case "assignee":
      return `@${reference.value.login}`;
    case "milestone":
      return reference.value.title;
  }
}

function select(
  value: IssueMetadataSelection,
  reference: IssueMetadataReference,
  remove: boolean,
): IssueMetadataSelection {
  switch (reference.kind) {
    case "label":
      return {
        ...value,
        labels: remove
          ? value.labels.filter(
              (item) => item.provider_id !== reference.value.provider_id,
            )
          : [...value.labels, reference.value],
      };
    case "assignee":
      return {
        ...value,
        assignees: remove
          ? value.assignees.filter(
              (item) => item.provider_id !== reference.value.provider_id,
            )
          : [...value.assignees, reference.value],
      };
    case "milestone":
      return { ...value, milestone: remove ? null : reference.value };
  }
}

/** Authored names survive catalog eviction, account retirement and offline use. */
export function IssueMetadataEditor({
  account,
  repositoryId,
  value,
  disabled,
  visible,
  onChange,
}: {
  account: RemoteAccount;
  repositoryId: string;
  value: IssueMetadataSelection;
  disabled: boolean;
  visible: boolean;
  onChange: (value: IssueMetadataSelection) => void;
}) {
  const authority = useCollaborationAuthorityVersion();
  const canBrowse =
    visible &&
    account.state === "active" &&
    account.provider === "github" &&
    account.host === "github.com";
  return (
    <section
      aria-label="Issue metadata"
      className="space-y-3 rounded-md border p-3"
    >
      <p className="text-xs text-muted-foreground">
        Saved selections stay with your draft. GitHub checks their availability
        again before submission.
      </p>
      {families.map((kind) => {
        const selected = references(value, kind);
        return (
          <div key={kind} className="space-y-2">
            <h3 className="text-sm font-medium">{names[kind]}</h3>
            {selected.length ? (
              <ul
                className="flex flex-wrap gap-2"
                aria-label={`Selected ${names[kind].toLowerCase()}`}
              >
                {selected.map((reference) => (
                  <li key={reference.value.provider_id}>
                    <Button
                      type="button"
                      size="sm"
                      variant="secondary"
                      disabled={disabled}
                      aria-label={`Remove ${referenceName(reference)}`}
                      onClick={() => onChange(select(value, reference, true))}
                    >
                      {referenceName(reference)} ×
                    </Button>
                  </li>
                ))}
              </ul>
            ) : (
              <p className="text-xs text-muted-foreground">None selected</p>
            )}
            {canBrowse ? (
              <MetadataChooser
                key={`${authority}:${account.id}:${account.actor_id}:${account.authorization_epoch}:${repositoryId}:${kind}`}
                account={account}
                repositoryId={repositoryId}
                kind={kind}
                selected={selected}
                disabled={disabled}
                onSelect={(reference) =>
                  onChange(select(value, reference, false))
                }
              />
            ) : (
              <p className="text-xs text-muted-foreground">
                Reconnect to browse saved options. You can still remove a
                selection.
              </p>
            )}
          </div>
        );
      })}
    </section>
  );
}

function MetadataChooser(props: {
  account: RemoteAccount;
  repositoryId: string;
  kind: IssueMetadataKind;
  selected: IssueMetadataReference[];
  disabled: boolean;
  onSelect: (reference: IssueMetadataReference) => void;
}) {
  const [open, setOpen] = useState(false);
  return (
    <Collapsible open={open} onOpenChange={setOpen}>
      <CollapsibleTrigger
        render={<Button type="button" size="sm" variant="outline" />}
      >
        Choose {names[props.kind].toLowerCase()}
      </CollapsibleTrigger>
      <CollapsiblePanel className="motion-reduce:transition-none">
        {open ? <OpenedMetadataChooser {...props} /> : null}
      </CollapsiblePanel>
    </Collapsible>
  );
}

function OpenedMetadataChooser({
  account,
  repositoryId,
  kind,
  selected,
  disabled,
  onSelect,
}: Parameters<typeof MetadataChooser>[0]) {
  const [search, setSearch] = useState("");
  const [cursor, setCursor] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [failure, setFailure] = useState<string | null>(null);
  const searchId = useId();
  // Only one bounded local page remains observed; changing the query retires it.
  const query = useQuery(
    issueMetadataOptionsQueryOptions(account, {
      repository_id: repositoryId,
      kind,
      search,
      cursor,
      limit: 50,
    }),
  );
  const demandError = useVisibleDemand({
    account,
    target: {
      kind:
        kind === "labels"
          ? "repository_labels"
          : kind === "assignees"
            ? "repository_assignees"
            : "repository_milestones",
      repository_id: repositoryId,
      subject_id: null,
      facet: null,
    },
    enabled: account.state === "active",
  });
  async function refresh() {
    setBusy(true);
    setFailure(null);
    setCursor(null);
    try {
      await collaboration
        .forAccount(account)
        .refreshIssueMetadata({ repository_id: repositoryId, kind });
      if (cursor === null) void query.refetch();
    } catch (error) {
      setFailure(collaborationErrorMessage(error));
    } finally {
      setBusy(false);
    }
  }
  const page = query.isError ? undefined : query.data;
  function canSelect(option: IssueMetadataOption) {
    if (
      disabled ||
      option.availability === "unavailable" ||
      selected.some(
        (item) => item.value.provider_id === option.reference.value.provider_id,
      )
    )
      return false;
    return (
      kind === "milestones" || selected.length < (kind === "labels" ? 32 : 10)
    );
  }
  return (
    <div className="mt-2 space-y-2 rounded-md border p-3">
      <label htmlFor={searchId} className="text-xs">
        Search saved {names[kind].toLowerCase()}
      </label>
      <Input
        id={searchId}
        value={search}
        maxLength={128}
        onChange={(event) => {
          setSearch(event.currentTarget.value);
          setCursor(null);
        }}
      />
      <div className="flex flex-wrap gap-2">
        <Button
          type="button"
          size="sm"
          variant="outline"
          disabled={busy}
          onClick={() => void refresh()}
        >
          {busy
            ? "Requesting refresh…"
            : `Refresh ${names[kind].toLowerCase()}`}
        </Button>
        {cursor !== null || query.isError ? (
          <Button
            type="button"
            size="sm"
            variant="ghost"
            onClick={() => {
              setCursor(null);
              if (cursor === null) void query.refetch();
            }}
          >
            Restart saved options
          </Button>
        ) : null}
      </div>
      {query.isPending ? (
        <p role="status" className="text-xs">
          Loading saved options…
        </p>
      ) : null}
      {query.isError ? (
        <p role="alert" className="text-xs text-destructive-foreground">
          {collaborationErrorMessage(query.error)}
        </p>
      ) : null}
      {page ? (
        <>
          <p className="text-xs text-muted-foreground">
            {page.coverage.state === "missing"
              ? "These options have not been synced yet."
              : page.coverage.state === "partial"
                ? "Saved options cover part of this repository; a missing option may still exist on GitHub."
                : "Saved repository options."}
            {page.freshness !== "fresh"
              ? " Saved options may be out of date."
              : ""}
            {page.coverage.remote_has_more
              ? " More remote options remain."
              : ""}
            {page.sync.state === "syncing"
              ? " Syncing…"
              : page.sync.state === "offline"
                ? " Offline."
                : page.sync.state === "rate_limited"
                  ? " Waiting for GitHub’s rate limit."
                  : ""}
          </p>
          {!page.options.length && page.coverage.state !== "missing" ? (
            <p className="text-xs">No saved options match this search.</p>
          ) : null}
          <ul
            className="max-h-56 space-y-1 overflow-y-auto"
            aria-label={`Available ${names[kind].toLowerCase()}`}
          >
            {page.options.map((option) => (
              <li key={option.reference.value.provider_id}>
                <Button
                  type="button"
                  size="sm"
                  variant="ghost"
                  className="h-auto w-full justify-start whitespace-normal break-words text-left"
                  disabled={!canSelect(option)}
                  onClick={() => onSelect(option.reference)}
                >
                  {referenceName(option.reference)}
                  {option.availability === "unavailable"
                    ? " · Unavailable"
                    : option.availability === "unknown"
                      ? " · Availability not yet verified"
                      : ""}
                </Button>
              </li>
            ))}
          </ul>
          {page.next_cursor ? (
            <Button
              type="button"
              size="sm"
              variant="outline"
              onClick={() => setCursor(page.next_cursor)}
            >
              Next saved options
            </Button>
          ) : null}
        </>
      ) : null}
      {failure || demandError ? (
        <p role="alert" className="text-xs text-destructive-foreground">
          {failure ?? collaborationErrorMessage(demandError)}
        </p>
      ) : null}
    </div>
  );
}

export function IssueMetadataReceipt({
  outcome,
}: {
  outcome: IssueMetadataOutcome;
}) {
  return (
    <div className="space-y-1 text-xs text-muted-foreground">
      <p>
        {outcome.needs_attention
          ? "The issue was created, but some requested metadata needs attention."
          : "Metadata observed in GitHub’s creation receipt:"}
      </p>
      <ul>
        {outcome.fields
          .filter((field) => field.result !== "not_requested")
          .map((field) => (
            <li key={field.field}>
              {field.field === "labels"
                ? "Labels"
                : field.field === "assignees"
                  ? "Assignees"
                  : "Milestone"}
              :{" "}
              {field.result === "applied"
                ? "matched the saved selection"
                : field.result === "different"
                  ? "differed from the saved selection"
                  : "could not be verified from the receipt"}
              .
            </li>
          ))}
      </ul>
      <p>
        This records the creation response. Review the created issue to make any
        further changes.
      </p>
    </div>
  );
}

import {
  collaboration,
  collaborationErrorMessage,
  type RemoteAccount,
} from "@gitru/collaboration-client";
import {
  commentDraftsQueryOptions,
  draftsQueryOptions,
  issueDraftsV2QueryOptions,
  pullDraftsQueryOptions,
  reviewDraftsQueryOptions,
  useCollaborationVersion,
} from "@gitru/collaboration-client/react";
import { Button } from "@gitru/ui/components/button";
import {
  Select,
  SelectItem,
  SelectPopup,
  SelectTrigger,
  SelectValue,
} from "@gitru/ui/components/select";
import { useQuery } from "@tanstack/react-query";
import { useEffect, useMemo, useState } from "react";
import { CommentComposer } from "./comment-composer";
import { RecoveredIssueDraft } from "./issue-creation";
import { issueMetadataSummary } from "./issue-metadata";
import { SavedDraftEditor } from "./private-draft";
import { RecoveredPullDraft } from "./pull-creation";
import { RecoveredReviewDraft } from "./review-submission";
import { CollaborationStatePanel } from "./state-panel";

export function DraftRecovery({
  accounts,
  onOpenIssue,
  onOpenPull,
}: {
  accounts: RemoteAccount[];
  onOpenPull?: (
    account: RemoteAccount,
    repositoryId: string,
    subjectId: string,
  ) => void;
  onOpenIssue?: (
    account: RemoteAccount,
    repositoryId: string,
    subjectId: string,
  ) => void;
}) {
  const [draftKind, setDraftKind] = useState<
    "private" | "comment" | "issue" | "pull" | "review"
  >("private");
  const [accountId, setAccountId] = useState<string | null>(
    accounts[0]?.id ?? null,
  );
  const accountItems = useMemo(
    () =>
      accounts.map((candidate) => ({
        label: `@${candidate.login} · ${candidate.host}`,
        value: candidate.id,
      })),
    [accounts],
  );
  const account =
    accounts.find((candidate) => candidate.id === accountId) ?? accounts[0];
  return (
    <section
      className="flex min-h-0 flex-1 flex-col"
      aria-label="Draft recovery"
    >
      <div className="space-y-2 border-b px-5 py-3">
        <h2 className="text-sm font-medium">
          {draftKind === "private"
            ? "Saved private drafts"
            : draftKind === "comment"
              ? "Saved comment drafts"
              : draftKind === "issue"
                ? "Saved issue drafts"
                : draftKind === "pull"
                  ? "Saved pull request drafts"
                  : "Saved review drafts"}
        </h2>
        <p className="text-xs text-muted-foreground">
          Recover your text even when an account is disconnected or an item is
          unavailable. Private notes and provider comment drafts stay separate.
          Issue and pull request drafts keep their own local identities until
          the provider confirms creation.
        </p>
        <div className="flex flex-wrap gap-2" aria-label="Draft kind">
          <Button
            type="button"
            size="sm"
            variant={draftKind === "private" ? "secondary" : "outline"}
            aria-pressed={draftKind === "private"}
            onClick={() => setDraftKind("private")}
          >
            Private notes
          </Button>
          <Button
            type="button"
            size="sm"
            variant={draftKind === "comment" ? "secondary" : "outline"}
            aria-pressed={draftKind === "comment"}
            onClick={() => setDraftKind("comment")}
          >
            Comment drafts
          </Button>
          <Button
            type="button"
            size="sm"
            variant={draftKind === "issue" ? "secondary" : "outline"}
            aria-pressed={draftKind === "issue"}
            onClick={() => setDraftKind("issue")}
          >
            Issue drafts
          </Button>
          <Button
            type="button"
            size="sm"
            variant={draftKind === "pull" ? "secondary" : "outline"}
            aria-pressed={draftKind === "pull"}
            onClick={() => setDraftKind("pull")}
          >
            Pull request drafts
          </Button>
          <Button
            type="button"
            size="sm"
            variant={draftKind === "review" ? "secondary" : "outline"}
            aria-pressed={draftKind === "review"}
            onClick={() => setDraftKind("review")}
          >
            Review drafts
          </Button>
        </div>
        {account ? (
          <Select
            key={accounts.map((candidate) => candidate.id).join(":")}
            items={accountItems}
            value={account.id}
            onValueChange={setAccountId}
          >
            <SelectTrigger
              size="sm"
              className="w-full max-w-sm"
              aria-label="Draft account"
            >
              <SelectValue />
            </SelectTrigger>
            <SelectPopup>
              {accounts.map((candidate) => (
                <SelectItem key={candidate.id} value={candidate.id}>
                  @{candidate.login} · {candidate.host}
                </SelectItem>
              ))}
            </SelectPopup>
          </Select>
        ) : null}
        {account ? (
          <p className="text-xs text-muted-foreground">
            {account.state === "disconnected"
              ? "Disconnected account"
              : account.state === "auth_required"
                ? "Account needs reconnection"
                : "Connected account"}{" "}
            · Local drafts remain editable
          </p>
        ) : null}
      </div>
      {account && draftKind === "private" ? (
        <AccountDrafts key={account.id} account={account} />
      ) : account && draftKind === "comment" ? (
        <AccountCommentDrafts key={account.id} account={account} />
      ) : account && draftKind === "review" ? (
        <AccountReviewDrafts key={account.id} account={account} />
      ) : account && draftKind === "pull" ? (
        <AccountPullDrafts
          key={account.id}
          account={account}
          onOpenPull={onOpenPull}
        />
      ) : account ? (
        <AccountIssueDrafts
          key={account.id}
          account={account}
          onOpenIssue={onOpenIssue}
        />
      ) : (
        <CollaborationStatePanel title="No saved accounts">
          Saved drafts from your connected accounts will appear here.
        </CollaborationStatePanel>
      )}
    </section>
  );
}

function AccountIssueDrafts({
  account,
  onOpenIssue,
}: {
  account: RemoteAccount;
  onOpenIssue?: (
    account: RemoteAccount,
    repositoryId: string,
    subjectId: string,
  ) => void;
}) {
  const [cursors, setCursors] = useState<Array<string | null>>([null]);
  const [selected, setSelected] = useState<{
    draftId: string;
    repositoryId: string;
  } | null>(null);
  useEffect(
    () =>
      collaboration.subscribeChanges((change) => {
        if (
          change.account_id === account.id &&
          change.scope.startsWith("issue_draft:")
        ) {
          setCursors((values) => (values.length > 1 ? [null] : values));
        }
      }),
    [account.id],
  );
  const query = useQuery(
    issueDraftsV2QueryOptions(account, {
      cursor: cursors.at(-1) ?? null,
      limit: 50,
    }),
  );
  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div
        className={`grid min-h-0 flex-1 ${selected ? "md:grid-cols-2" : "grid-cols-1"}`}
      >
        <div
          className={`min-w-0 overflow-y-auto ${selected ? "hidden md:block" : ""}`}
        >
          {query.isPending ? (
            <p
              className="px-5 py-4 text-sm text-muted-foreground"
              role="status"
            >
              Loading saved issue drafts…
            </p>
          ) : query.isError ? (
            <div className="space-y-2 p-5">
              <p role="alert" className="text-sm text-destructive-foreground">
                {collaborationErrorMessage(query.error)}
              </p>
              <Button
                size="sm"
                variant="outline"
                onClick={() => void query.refetch()}
              >
                Retry issue drafts
              </Button>
            </div>
          ) : !query.data?.drafts.length ? (
            <p className="px-5 py-4 text-sm text-muted-foreground">
              No saved issue drafts for this account.
            </p>
          ) : (
            query.data.drafts.map(({ draft, metadata }) => (
              <Button
                key={draft.draft_id}
                variant="ghost"
                className="h-auto w-full min-w-0 flex-col items-start gap-1 rounded-none border-b px-5 py-3 text-left whitespace-normal"
                aria-label={`Open issue draft ${draft.title || draft.draft_id}`}
                aria-pressed={selected?.draftId === draft.draft_id}
                onClick={() =>
                  setSelected({
                    draftId: draft.draft_id,
                    repositoryId: draft.repository_id,
                  })
                }
              >
                <span className="line-clamp-2 w-full break-words text-sm font-medium">
                  {draft.title || "Untitled issue"}
                </span>
                <span className="line-clamp-2 w-full break-words text-xs text-muted-foreground">
                  {draft.preview || "Empty description"}
                </span>
                <span className="line-clamp-2 w-full break-words text-xs text-muted-foreground">
                  {issueMetadataSummary(metadata)}
                </span>
                {draft.submission ? (
                  <span className="text-xs text-muted-foreground">
                    Submission tracked in Saved changes
                  </span>
                ) : null}
              </Button>
            ))
          )}
        </div>
        {selected ? (
          <article
            className="min-w-0 overflow-y-auto border-l p-5"
            aria-label="Recovered issue draft"
          >
            <Button size="sm" variant="ghost" onClick={() => setSelected(null)}>
              Back to issue drafts
            </Button>
            <h3 className="mt-4 text-sm font-medium">Recovered issue draft</h3>
            <p className="mt-2 break-all text-xs text-muted-foreground">
              Repository: {selected.repositoryId}
            </p>
            <div className="mt-4">
              <RecoveredIssueDraft
                key={`${selected.draftId}:${selected.repositoryId}`}
                account={account}
                draftId={selected.draftId}
                repositoryId={selected.repositoryId}
                onOpenCreated={
                  onOpenIssue
                    ? (subjectId) =>
                        onOpenIssue(account, selected.repositoryId, subjectId)
                    : undefined
                }
              />
            </div>
          </article>
        ) : null}
      </div>
      {query.data?.next_cursor || cursors.length > 1 ? (
        <footer className="flex shrink-0 gap-2 border-t px-5 py-2">
          <Button
            size="sm"
            variant="ghost"
            disabled={cursors.length <= 1}
            onClick={() => setCursors((values) => values.slice(0, -1))}
          >
            Previous issue drafts
          </Button>
          <Button
            size="sm"
            variant="ghost"
            disabled={!query.data?.next_cursor}
            onClick={() => {
              if (query.data?.next_cursor)
                setCursors((values) => [...values, query.data.next_cursor]);
            }}
          >
            Next issue drafts
          </Button>
        </footer>
      ) : null}
    </div>
  );
}

function AccountPullDrafts({
  account,
  onOpenPull,
}: {
  account: RemoteAccount;
  onOpenPull?: (
    account: RemoteAccount,
    repositoryId: string,
    subjectId: string,
  ) => void;
}) {
  const [cursors, setCursors] = useState<Array<string | null>>([null]);
  const [selected, setSelected] = useState<{
    draftId: string;
    repositoryId: string;
  } | null>(null);
  useEffect(
    () =>
      collaboration.subscribeChanges((change) => {
        if (
          change.account_id === account.id &&
          change.scope.startsWith("pull_draft:")
        ) {
          setCursors((values) => (values.length > 1 ? [null] : values));
        }
      }),
    [account.id],
  );
  const query = useQuery(
    pullDraftsQueryOptions(account, {
      cursor: cursors.at(-1) ?? null,
      limit: 50,
    }),
  );
  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div
        className={`grid min-h-0 flex-1 ${selected ? "md:grid-cols-2" : "grid-cols-1"}`}
      >
        <div
          className={`min-w-0 overflow-y-auto ${selected ? "hidden md:block" : ""}`}
        >
          {query.isPending ? (
            <p
              className="px-5 py-4 text-sm text-muted-foreground"
              role="status"
            >
              Loading saved pull request drafts…
            </p>
          ) : query.isError ? (
            <div className="space-y-2 p-5">
              <p role="alert" className="text-sm text-destructive-foreground">
                {collaborationErrorMessage(query.error)}
              </p>
              <Button
                size="sm"
                variant="outline"
                onClick={() => void query.refetch()}
              >
                Retry pull request drafts
              </Button>
            </div>
          ) : !query.data?.drafts.length ? (
            <p className="px-5 py-4 text-sm text-muted-foreground">
              No saved pull request drafts for this account.
            </p>
          ) : (
            query.data.drafts.map((draft) => (
              <Button
                key={draft.draft_id}
                variant="ghost"
                className="h-auto w-full min-w-0 flex-col items-start gap-1 rounded-none border-b px-5 py-3 text-left whitespace-normal"
                aria-label={`Open pull request draft ${draft.title || draft.draft_id}`}
                aria-pressed={selected?.draftId === draft.draft_id}
                onClick={() =>
                  setSelected({
                    draftId: draft.draft_id,
                    repositoryId: draft.repository_id,
                  })
                }
              >
                <span className="line-clamp-2 w-full break-words text-sm font-medium">
                  {draft.title || "Untitled pull request"}
                </span>
                <span className="line-clamp-2 w-full break-words text-xs text-muted-foreground">
                  {draft.preview || "Empty description"}
                </span>
                {draft.submission ? (
                  <span className="text-xs text-muted-foreground">
                    Submission tracked in Saved changes
                  </span>
                ) : null}
              </Button>
            ))
          )}
        </div>
        {selected ? (
          <article
            className="min-w-0 overflow-y-auto border-l p-5"
            aria-label="Recovered pull request draft"
          >
            <Button size="sm" variant="ghost" onClick={() => setSelected(null)}>
              Back to pull request drafts
            </Button>
            <h3 className="mt-4 text-sm font-medium">
              Recovered pull request draft
            </h3>
            <p className="mt-2 break-all text-xs text-muted-foreground">
              Repository: {selected.repositoryId}
            </p>
            <div className="mt-4">
              <RecoveredPullDraft
                key={`${selected.draftId}:${selected.repositoryId}`}
                account={account}
                draftId={selected.draftId}
                repositoryId={selected.repositoryId}
                onOpenCreated={
                  onOpenPull
                    ? (subjectId) =>
                        onOpenPull(account, selected.repositoryId, subjectId)
                    : undefined
                }
              />
            </div>
          </article>
        ) : null}
      </div>
      {query.data?.next_cursor || cursors.length > 1 ? (
        <footer className="flex shrink-0 gap-2 border-t px-5 py-2">
          <Button
            size="sm"
            variant="ghost"
            disabled={cursors.length <= 1}
            onClick={() => setCursors((values) => values.slice(0, -1))}
          >
            Previous pull request drafts
          </Button>
          <Button
            size="sm"
            variant="ghost"
            disabled={!query.data?.next_cursor}
            onClick={() => {
              if (query.data?.next_cursor)
                setCursors((values) => [...values, query.data.next_cursor]);
            }}
          >
            Next pull request drafts
          </Button>
        </footer>
      ) : null}
    </div>
  );
}

function AccountCommentDrafts({ account }: { account: RemoteAccount }) {
  const [cursors, setCursors] = useState<Array<string | null>>([null]);
  const [subjectId, setSubjectId] = useState<string | null>(null);
  useEffect(
    () =>
      collaboration.subscribeChanges((change) => {
        if (
          change.account_id === account.id &&
          change.scope.startsWith("comment_draft:")
        ) {
          setCursors((values) => (values.length > 1 ? [null] : values));
        }
      }),
    [account.id],
  );
  const query = useQuery(
    commentDraftsQueryOptions(account, {
      cursor: cursors.at(-1) ?? null,
      limit: 50,
    }),
  );
  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div
        className={`grid min-h-0 flex-1 ${subjectId ? "md:grid-cols-2" : "grid-cols-1"}`}
      >
        <div
          className={`min-w-0 overflow-y-auto ${subjectId ? "hidden md:block" : ""}`}
        >
          {query.isPending ? (
            <p
              className="px-5 py-4 text-sm text-muted-foreground"
              role="status"
            >
              Loading saved comment drafts…
            </p>
          ) : query.isError ? (
            <div className="space-y-2 p-5">
              <p role="alert" className="text-sm text-destructive-foreground">
                {collaborationErrorMessage(query.error)}
              </p>
              <Button
                size="sm"
                variant="outline"
                onClick={() => void query.refetch()}
              >
                Retry comment drafts
              </Button>
            </div>
          ) : !query.data?.drafts.length ? (
            <p className="px-5 py-4 text-sm text-muted-foreground">
              No saved comment drafts for this account.
            </p>
          ) : (
            query.data.drafts.map((draft) => (
              <Button
                key={draft.subject_id}
                variant="ghost"
                className="h-auto w-full min-w-0 flex-col items-start gap-1 rounded-none border-b px-5 py-3 text-left whitespace-normal"
                aria-label={`Open comment draft for ${draft.subject_id}`}
                aria-pressed={subjectId === draft.subject_id}
                onClick={() => setSubjectId(draft.subject_id)}
              >
                <span className="w-full break-all text-xs font-medium">
                  {draft.subject_id}
                </span>
                <span className="line-clamp-2 w-full break-words text-sm text-muted-foreground">
                  {draft.preview || "Empty comment draft"}
                </span>
              </Button>
            ))
          )}
        </div>
        {subjectId ? (
          <article
            className="min-w-0 overflow-y-auto border-l p-5"
            aria-label="Recovered comment draft"
          >
            <Button
              size="sm"
              variant="ghost"
              onClick={() => setSubjectId(null)}
            >
              Back to comment drafts
            </Button>
            <h3 className="mt-4 break-all text-sm font-medium">{subjectId}</h3>
            <p className="mt-2 text-xs text-muted-foreground">
              @{account.login} · {account.host}
            </p>
            <CommentComposer
              key={`${account.id}:${subjectId}`}
              account={account}
              subjectId={subjectId}
            />
          </article>
        ) : null}
      </div>
      {query.data?.next_cursor || cursors.length > 1 ? (
        <footer className="flex shrink-0 gap-2 border-t px-5 py-2">
          <Button
            size="sm"
            variant="ghost"
            disabled={cursors.length <= 1}
            onClick={() => setCursors((values) => values.slice(0, -1))}
          >
            Previous comment drafts
          </Button>
          <Button
            size="sm"
            variant="ghost"
            disabled={!query.data?.next_cursor}
            onClick={() => {
              if (query.data?.next_cursor)
                setCursors((values) => [...values, query.data.next_cursor]);
            }}
          >
            Next comment drafts
          </Button>
        </footer>
      ) : null}
    </div>
  );
}

function AccountDrafts({ account }: { account: RemoteAccount }) {
  const [cursors, setCursors] = useState<Array<string | null>>([null]);
  const [subjectId, setSubjectId] = useState<string | null>(null);
  const version = useCollaborationVersion();
  const [observedVersion, setObservedVersion] = useState(version);
  if (observedVersion !== version) {
    setObservedVersion(version);
    if (cursors.length > 1) setCursors([null]);
  }
  useEffect(
    () =>
      collaboration.subscribeChanges((change) => {
        if (change.account_id === account.id && change.scope === "drafts") {
          setCursors((values) => (values.length > 1 ? [null] : values));
        }
      }),
    [account.id],
  );
  const query = useQuery(
    draftsQueryOptions(account, { cursor: cursors.at(-1) ?? null, limit: 50 }),
  );
  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div
        className={`grid min-h-0 flex-1 ${subjectId ? "md:grid-cols-2" : "grid-cols-1"}`}
      >
        <div
          className={`min-w-0 overflow-y-auto ${subjectId ? "hidden md:block" : ""}`}
        >
          {query.isPending ? (
            <p
              className="px-5 py-4 text-sm text-muted-foreground"
              role="status"
            >
              Loading saved drafts…
            </p>
          ) : query.isError ? (
            <div className="space-y-2 p-5">
              <p role="alert" className="text-sm text-destructive-foreground">
                {collaborationErrorMessage(query.error)}
              </p>
              <Button
                size="sm"
                variant="outline"
                onClick={() => {
                  void query.refetch();
                }}
              >
                Retry saved drafts
              </Button>
            </div>
          ) : !query.data?.drafts.length ? (
            <p className="px-5 py-4 text-sm text-muted-foreground">
              No saved drafts for this account.
            </p>
          ) : (
            query.data.drafts.map((draft) => (
              <Button
                key={draft.subject_id}
                variant="ghost"
                className="h-auto w-full min-w-0 flex-col items-start gap-1 rounded-none border-b px-5 py-3 text-left whitespace-normal"
                aria-label={`Open draft for ${draft.subject_id}`}
                aria-pressed={subjectId === draft.subject_id}
                onClick={() => setSubjectId(draft.subject_id)}
              >
                <span className="w-full break-all text-xs font-medium">
                  {draft.subject_id}
                </span>
                <span className="line-clamp-2 w-full break-words text-sm text-muted-foreground">
                  {draft.preview || "Empty draft"}
                </span>
              </Button>
            ))
          )}
        </div>
        {subjectId ? (
          <article
            className="min-w-0 overflow-y-auto border-l p-5"
            aria-label="Recovered private draft"
          >
            <Button
              size="sm"
              variant="ghost"
              onClick={() => setSubjectId(null)}
            >
              Back to drafts
            </Button>
            <h3 className="mt-4 break-all text-sm font-medium">{subjectId}</h3>
            <p className="mt-2 text-xs text-muted-foreground">
              @{account.login} · {account.host}
            </p>
            <SavedDraftEditor
              key={`${account.id}:${subjectId}`}
              account={account}
              subjectId={subjectId}
            />
          </article>
        ) : null}
      </div>
      {query.data?.next_cursor || cursors.length > 1 ? (
        <footer className="flex shrink-0 gap-2 border-t px-5 py-2">
          <Button
            size="sm"
            variant="ghost"
            disabled={cursors.length <= 1}
            onClick={() => {
              setCursors((values) => values.slice(0, -1));
            }}
          >
            Previous drafts
          </Button>
          <Button
            size="sm"
            variant="ghost"
            disabled={!query.data?.next_cursor}
            onClick={() => {
              if (query.data?.next_cursor)
                setCursors((values) => [...values, query.data.next_cursor]);
            }}
          >
            Next drafts
          </Button>
        </footer>
      ) : null}
    </div>
  );
}

function AccountReviewDrafts({ account }: { account: RemoteAccount }) {
  const [cursors, setCursors] = useState<Array<string | null>>([null]);
  const [selected, setSelected] = useState<string | null>(null);
  useEffect(
    () =>
      collaboration.subscribeChanges((change) => {
        if (
          change.account_id === account.id &&
          change.scope.startsWith("review_draft:")
        )
          setCursors((values) => (values.length > 1 ? [null] : values));
      }),
    [account.id],
  );
  const query = useQuery(
    reviewDraftsQueryOptions(account, {
      cursor: cursors.at(-1) ?? null,
      limit: 50,
    }),
  );
  const next = query.data?.next_cursor;
  const canNext =
    !query.isError &&
    !query.isFetching &&
    next != null &&
    !cursors.includes(next) &&
    cursors.length < 100;
  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div
        className={`grid min-h-0 flex-1 ${selected ? "md:grid-cols-2" : "grid-cols-1"}`}
      >
        <div
          className={`min-w-0 overflow-y-auto ${selected ? "hidden md:block" : ""}`}
        >
          {query.isPending ? (
            <p role="status" className="p-5 text-sm">
              Loading saved review drafts…
            </p>
          ) : query.isError ? (
            <p role="alert" className="p-5 text-sm">
              {collaborationErrorMessage(query.error)}
            </p>
          ) : !query.data?.drafts.length ? (
            <p className="p-5 text-sm text-muted-foreground">
              No saved review drafts for this account.
            </p>
          ) : (
            query.data.drafts.map((draft) => (
              <Button
                key={draft.subject_id}
                variant="ghost"
                className="h-auto w-full min-w-0 flex-col items-start gap-1 rounded-none border-b px-5 py-3 text-left whitespace-normal"
                aria-label={`Open review draft ${draft.preview || draft.subject_id}`}
                aria-pressed={selected === draft.subject_id}
                onClick={() => setSelected(draft.subject_id)}
              >
                <span className="line-clamp-2 w-full break-words text-sm">
                  {draft.preview || "Review without a summary"}
                </span>
                <span className="text-xs text-muted-foreground">
                  {draft.event.replace(/_/g, " ")} ·{" "}
                  {draft.inline_comment_count} inline comments
                </span>
                {draft.submission ? (
                  <span className="text-xs">
                    Submission tracked in Saved changes
                  </span>
                ) : null}
              </Button>
            ))
          )}
        </div>
        {selected ? (
          <article
            className="min-w-0 overflow-y-auto border-l p-5"
            aria-label="Recovered review draft"
          >
            <Button
              type="button"
              size="sm"
              variant="ghost"
              onClick={() => setSelected(null)}
            >
              Back to review drafts
            </Button>
            <h3 className="my-4 text-sm font-medium">Recovered review draft</h3>
            <RecoveredReviewDraft
              key={selected}
              account={account}
              subjectId={selected}
            />
          </article>
        ) : null}
      </div>
      {next || cursors.length > 1 ? (
        <footer className="flex shrink-0 gap-2 border-t px-5 py-2">
          <Button
            type="button"
            size="sm"
            variant="ghost"
            disabled={cursors.length <= 1 || query.isFetching}
            onClick={() => setCursors((values) => values.slice(0, -1))}
          >
            Previous review drafts
          </Button>
          <Button
            type="button"
            size="sm"
            variant="ghost"
            disabled={!canNext}
            onClick={() => {
              if (canNext && next) setCursors((values) => [...values, next]);
            }}
          >
            Next review drafts
          </Button>
        </footer>
      ) : null}
    </div>
  );
}

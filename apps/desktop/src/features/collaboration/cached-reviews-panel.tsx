import {
  type ContextFacetCapability,
  collaboration,
  collaborationErrorMessage,
  type DetailEntry,
  type DetailSnapshot,
  type RemoteAccount,
} from "@gitru/collaboration-client";
import {
  detailQueryOptions,
  useVisibleDemand,
} from "@gitru/collaboration-client/react";
import { Badge } from "@gitru/ui/components/badge";
import { Button } from "@gitru/ui/components/button";
import {
  Collapsible,
  CollapsiblePanel,
  CollapsibleTrigger,
} from "@gitru/ui/components/collapsible";
import { type UseQueryResult, useQuery } from "@tanstack/react-query";
import { ChevronDown } from "lucide-react";
import { useState } from "react";
import {
  CapabilityBoundary,
  ReadOnlyCapability,
  SynchronizationAvailability,
} from "./capability-boundary";
import {
  canMaintainDemand,
  canReadSaved,
  canSynchronize,
  dispatchCapabilityIntent,
} from "./capability-policy";
import { GitlabReviewNote } from "./gitlab-review-note";
import { ReviewComposerTrigger } from "./review-submission";

export type PullReviewContext = {
  baseOid: string;
  headOid: string;
  baseRepositoryProviderId: string;
  sourceRepositoryProviderId: string;
  metadataFacetRevision: string;
  /** Alias consumed by the exact-head checks panel. */
  facetRevision: string;
};

type Props = {
  account: RemoteAccount;
  subjectId: string;
  authorizationView: string | undefined;
  policy: ContextFacetCapability | undefined;
  bodyContext: PullReviewContext | null;
};

type ReviewFacet = "review_summaries" | "review_threads";
const PAGE_LIMIT = 50;
const CURSOR_LIMIT = 100;

export function CachedReviewsPanel(props: Props) {
  const { account, subjectId, policy, bodyContext } = props;
  const [open, setOpen] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function synchronize(recheck = false) {
    setBusy(true);
    setError(null);
    try {
      await dispatchCapabilityIntent(
        policy,
        recheck ? "recheck_access" : "synchronize",
        async () => {
          const handle = collaboration.forAccount(account);
          await Promise.all([
            handle.hydrateDetail({
              subject_id: subjectId,
              facet: "review_summaries",
            }),
            handle.hydrateDetail({
              subject_id: subjectId,
              facet: "review_threads",
            }),
          ]);
        },
      );
    } catch (failure) {
      setError(collaborationErrorMessage(failure));
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="space-y-2 border-t pt-4" aria-label="Reviews">
      <Collapsible open={open} onOpenChange={setOpen}>
        <div className="flex flex-wrap items-center justify-between gap-2">
          <CollapsibleTrigger
            type="button"
            className="flex items-center gap-2 text-sm font-medium"
          >
            <ChevronDown
              aria-hidden="true"
              className={open ? "size-4 rotate-180" : "size-4"}
            />
            Reviews
          </CollapsibleTrigger>
          <div className="flex items-center gap-2">
            <ReadOnlyCapability policy={policy} />
            <ReviewComposerTrigger />
          </div>
        </div>
        <CollapsiblePanel className="motion-reduce:transition-none">
          {open ? (
            <div className="space-y-3 pt-3">
              <p className="text-xs text-muted-foreground">
                {account.provider === "gitlab"
                  ? "Saved approver observations and discussions, including general and system notes. GitLab does not identify the approved commit in these observations."
                  : "Saved review decisions and inline discussion from the provider. Historical reviews do not approve the current commit."}
              </p>
              <SynchronizationAvailability policy={policy} />
              <CapabilityBoundary
                policy={policy}
                busy={busy}
                recheck={() => {
                  void synchronize(true);
                }}
              >
                {canReadSaved(policy) ? (
                  <>
                    <Button
                      type="button"
                      size="sm"
                      variant="ghost"
                      disabled={busy || !canSynchronize(policy)}
                      onClick={() => {
                        void synchronize();
                      }}
                    >
                      Sync reviews
                    </Button>
                    {bodyContext ? (
                      <OpenedReviews {...props} bodyContext={bodyContext} />
                    ) : (
                      <p
                        role="status"
                        className="text-xs text-muted-foreground"
                      >
                        Reviews need saved pull request branch information
                        before they can be read.
                      </p>
                    )}
                  </>
                ) : null}
              </CapabilityBoundary>
              {error ? (
                <p role="alert" className="text-xs text-destructive-foreground">
                  {error}
                </p>
              ) : null}
            </div>
          ) : null}
        </CollapsiblePanel>
      </Collapsible>
    </section>
  );
}

function matchesScope(
  snapshot: DetailSnapshot,
  facet: ReviewFacet,
  props: Props,
) {
  return (
    snapshot.subject_id === props.subjectId &&
    snapshot.evidence.facet === facet &&
    snapshot.evidence.authorization_epoch ===
      props.account.authorization_epoch &&
    snapshot.authorization_view === props.authorizationView
  );
}

function OpenedReviews(props: Props & { bodyContext: PullReviewContext }) {
  const summaries = useQuery({
    ...detailQueryOptions(props.account, {
      subject_id: props.subjectId,
      facet: "review_summaries",
      cursor: null,
      limit: PAGE_LIMIT,
    }),
    gcTime: 0,
  });
  const threads = useQuery({
    ...detailQueryOptions(props.account, {
      subject_id: props.subjectId,
      facet: "review_threads",
      cursor: null,
      limit: PAGE_LIMIT,
    }),
    gcTime: 0,
  });
  const summariesMatch =
    !summaries.data || matchesScope(summaries.data, "review_summaries", props);
  const threadsMatch =
    !threads.data || matchesScope(threads.data, "review_threads", props);
  const summaryDemandError = useVisibleDemand({
    account: props.account,
    target: {
      kind: "detail",
      repository_id: null,
      subject_id: props.subjectId,
      facet: "review_summaries",
    },
    enabled:
      props.account.state === "active" &&
      canMaintainDemand(props.policy) &&
      summaries.data !== undefined &&
      summariesMatch &&
      summaries.data.evidence.availability !== "unavailable",
  });
  const threadDemandError = useVisibleDemand({
    account: props.account,
    target: {
      kind: "detail",
      repository_id: null,
      subject_id: props.subjectId,
      facet: "review_threads",
    },
    enabled:
      props.account.state === "active" &&
      canMaintainDemand(props.policy) &&
      threads.data !== undefined &&
      threadsMatch &&
      threads.data.evidence.availability !== "unavailable",
  });

  return (
    <div className="space-y-5">
      <ReviewFacetView
        {...props}
        facet="review_summaries"
        label={
          props.account.provider === "gitlab"
            ? "Approval observations"
            : "Review decisions"
        }
        query={summaries}
        scopeMatches={summariesMatch}
      />
      <ReviewFacetView
        {...props}
        facet="review_threads"
        label={
          props.account.provider === "gitlab"
            ? "Discussions"
            : "Inline discussion"
        }
        query={threads}
        scopeMatches={threadsMatch}
      />
      {[summaryDemandError, threadDemandError]
        .filter(Boolean)
        .map((failure) => (
          <p
            key={collaborationErrorMessage(failure)}
            role="alert"
            className="text-xs text-destructive-foreground"
          >
            {collaborationErrorMessage(failure)}
          </p>
        ))}
    </div>
  );
}

type HeadQuery = UseQueryResult<DetailSnapshot, Error>;

function ReviewFacetView({
  facet,
  label,
  query,
  scopeMatches,
  ...props
}: Props & {
  bodyContext: PullReviewContext;
  facet: ReviewFacet;
  label: string;
  query: HeadQuery;
  scopeMatches: boolean;
}) {
  const [restartGeneration, setRestartGeneration] = useState(0);
  const restart = () => {
    setRestartGeneration((generation) => generation + 1);
    void query.refetch();
  };
  const head =
    scopeMatches && query.data?.evidence.availability !== "unavailable"
      ? query.data
      : undefined;
  return (
    <section className="space-y-3" aria-label={label}>
      <h4 className="text-sm font-medium">{label}</h4>
      {query.isPending || query.isFetching ? (
        <p role="status" className="text-xs text-muted-foreground">
          {query.isPending
            ? `Loading saved ${label.toLowerCase()}…`
            : "Refreshing saved reviews…"}
        </p>
      ) : query.isError ? (
        <RestartNotice
          message={collaborationErrorMessage(query.error)}
          restart={restart}
        />
      ) : query.data?.evidence.availability === "unavailable" ? (
        <p className="text-xs text-muted-foreground">
          Saved {label.toLowerCase()} are unavailable in the current authorized
          view.
        </p>
      ) : !scopeMatches ? (
        <RestartNotice
          message="The saved review view changed. Read its first page again."
          restart={restart}
        />
      ) : head ? (
        <ReviewPager
          key={JSON.stringify([
            facet,
            props.account.id,
            props.account.actor_id,
            props.account.authorization_epoch,
            props.subjectId,
            head.authorization_view,
            head.evidence.facet_revision,
            props.bodyContext.metadataFacetRevision,
            props.bodyContext.headOid,
            restartGeneration,
          ])}
          {...props}
          facet={facet}
          label={label}
          head={head}
          restart={restart}
        />
      ) : null}
    </section>
  );
}

function RestartNotice({
  message,
  restart,
}: {
  message: string;
  restart: () => void;
}) {
  return (
    <div className="space-y-2">
      <p role="alert" className="text-xs text-destructive-foreground">
        {message}
      </p>
      <Button type="button" size="sm" variant="outline" onClick={restart}>
        Restart saved reviews
      </Button>
    </div>
  );
}

function ReviewPager({
  facet,
  label,
  head,
  restart,
  ...props
}: Props & {
  bodyContext: PullReviewContext;
  facet: ReviewFacet;
  label: string;
  head: DetailSnapshot;
  restart: () => void;
}) {
  const [pages, setPages] = useState<{
    cursors: Array<string | null>;
    position: number;
  }>({ cursors: [null], position: 0 });
  const discussionName =
    props.account.provider === "gitlab" ? "Discussions" : "Inline discussions";
  const cursor = pages.cursors[pages.position];
  const page = useQuery({
    ...detailQueryOptions(props.account, {
      subject_id: props.subjectId,
      facet,
      cursor,
      limit: PAGE_LIMIT,
    }),
    enabled: cursor !== null,
    gcTime: 0,
  });
  const pending = cursor !== null && (page.isPending || page.isFetching);
  const failed = cursor !== null && page.isError;
  const snapshot = cursor === null ? head : page.data;
  const current =
    snapshot &&
    matchesScope(snapshot, facet, props) &&
    snapshot.evidence.facet_revision === head.evidence.facet_revision &&
    snapshot.authorization_view === head.authorization_view;
  const data =
    current && snapshot.evidence.availability !== "unavailable"
      ? snapshot
      : undefined;
  const next = data?.next_cursor ?? null;
  const capReached = pages.position + 1 >= CURSOR_LIMIT;
  const repeated =
    next !== null &&
    (next === cursor ||
      (pages.cursors.includes(next) &&
        pages.cursors[pages.position + 1] !== next));
  const canNext = !pending && !failed && !!next && !capReached && !repeated;

  return (
    <div className="space-y-3">
      {pending ? (
        <p role="status" className="text-xs text-muted-foreground">
          Loading saved review page…
        </p>
      ) : failed ? (
        <RestartNotice
          message={collaborationErrorMessage(page.error)}
          restart={restart}
        />
      ) : !current ? (
        <RestartNotice
          message="The saved review pages changed. Restart from the first page."
          restart={restart}
        />
      ) : data ? (
        <>
          <ReviewEvidence snapshot={data} />
          {data.evidence.availability === "missing" ? (
            <p className="text-xs text-muted-foreground">
              {facet === "review_summaries"
                ? "Review decisions have not been saved on this device yet."
                : `${discussionName} have not been saved on this device yet.`}
            </p>
          ) : data.entries.length ? (
            <ul
              className="space-y-4"
              aria-label={`Saved ${label.toLowerCase()}`}
            >
              {data.entries.map((entry) =>
                facet === "review_summaries" ? (
                  <ReviewRow
                    key={entry.id}
                    entry={entry}
                    context={props.bodyContext}
                  />
                ) : (
                  <ThreadRow
                    key={entry.id}
                    entry={entry}
                    context={props.bodyContext}
                  />
                ),
              )}
            </ul>
          ) : (
            <p className="text-xs text-muted-foreground">
              {data.evidence.coverage.state === "complete"
                ? facet === "review_summaries"
                  ? "No review decisions were returned in this saved observation."
                  : `No ${discussionName.toLowerCase()} were returned in this saved observation.`
                : facet === "review_summaries"
                  ? "No review decisions are saved in this partial view."
                  : `No ${discussionName.toLowerCase()} are saved in this partial view.`}
            </p>
          )}
        </>
      ) : null}
      <nav
        aria-label={`Saved ${label.toLowerCase()} pages`}
        className="flex flex-wrap items-center gap-2"
      >
        <Button
          type="button"
          size="sm"
          variant="outline"
          disabled={pages.position === 0}
          onClick={() => setPages({ ...pages, position: pages.position - 1 })}
        >
          Previous
        </Button>
        <p className="text-xs text-muted-foreground">
          Saved page {pages.position + 1}
        </p>
        <Button
          type="button"
          size="sm"
          variant="outline"
          disabled={!canNext}
          onClick={() => {
            if (!next || !canNext) return;
            setPages({
              cursors: [...pages.cursors.slice(0, pages.position + 1), next],
              position: pages.position + 1,
            });
          }}
        >
          Next
        </Button>
        <Button type="button" size="sm" variant="outline" onClick={restart}>
          Restart
        </Button>
      </nav>
      {capReached && next ? (
        <p className="text-xs text-muted-foreground">
          This view can browse up to 100 saved pages.
        </p>
      ) : null}
      {repeated ? (
        <p role="alert" className="text-xs text-destructive-foreground">
          The saved continuation did not advance. Restart from the first page.
        </p>
      ) : null}
    </div>
  );
}

function ReviewEvidence({ snapshot }: { snapshot: DetailSnapshot }) {
  return (
    <>
      <div className="flex flex-wrap gap-2">
        {snapshot.evidence.freshness === "stale" ? (
          <Badge variant="outline" size="sm">
            Saved data may be stale
          </Badge>
        ) : null}
        {snapshot.evidence.coverage.state === "partial" ? (
          <Badge variant="outline" size="sm">
            Partial provider history
          </Badge>
        ) : null}
      </div>
      {snapshot.evidence.sync.error ? (
        <p role="alert" className="text-xs text-destructive-foreground">
          {collaborationErrorMessage(snapshot.evidence.sync.error)}
        </p>
      ) : null}
      {snapshot.evidence.sync.next_retry_at ? (
        <p role="status" className="text-xs text-muted-foreground">
          Sync can resume after{" "}
          <time dateTime={snapshot.evidence.sync.next_retry_at}>
            {new Date(snapshot.evidence.sync.next_retry_at).toLocaleString()}
          </time>
          . Saved reviews remain available.
        </p>
      ) : null}
    </>
  );
}

function contextMatches(entry: DetailEntry, context: PullReviewContext) {
  const native = entry.native;
  if (native?.kind !== "review.v1" && native?.kind !== "review_thread.v1")
    return false;
  const observed = native.value.context;
  return (
    observed.base_oid === context.baseOid &&
    observed.head_oid === context.headOid &&
    observed.base_repository_provider_id === context.baseRepositoryProviderId &&
    observed.source_repository_provider_id ===
      context.sourceRepositoryProviderId &&
    observed.metadata_facet_revision === context.metadataFacetRevision
  );
}

function ReviewRow({
  entry,
  context,
}: {
  entry: DetailEntry;
  context: PullReviewContext;
}) {
  if (entry.native?.kind !== "review.v1")
    return <UnsupportedRow label="review decision" />;
  const review = entry.native.value;
  const exactContext = contextMatches(entry, context);
  const commitState = !review.reviewed_commit_oid
    ? "Commit unknown"
    : exactContext && review.reviewed_commit_oid === context.headOid
      ? "Current commit"
      : "Historical commit";
  return (
    <li className="space-y-2 break-words text-sm">
      <div className="flex flex-wrap items-center gap-2">
        <p className="font-medium">
          {review.reviewer?.display_name ??
            review.reviewer?.login ??
            "Reviewer unavailable"}
        </p>
        <Badge variant="outline" size="sm">
          {review.decision.replace(/_/g, " ")}
        </Badge>
        <Badge variant="outline" size="sm">
          {commitState}
        </Badge>
      </div>
      {review.decision === "unknown" ? (
        <p className="text-xs text-muted-foreground">
          Provider state: {review.provider_state}
        </p>
      ) : null}
      {entry.body.state === "known" ? (
        <p className="whitespace-pre-wrap break-words">
          {entry.body.text === ""
            ? "This review has no message."
            : entry.body.text}
        </p>
      ) : entry.body.state === "oversized" ? (
        <p className="text-xs text-muted-foreground">
          This review message exceeds the local text limit.
        </p>
      ) : null}
      {review.submitted_at ? (
        <p className="text-xs text-muted-foreground">
          Submitted{" "}
          <time dateTime={review.submitted_at}>
            {new Date(review.submitted_at).toLocaleString()}
          </time>
        </p>
      ) : null}
    </li>
  );
}

function ThreadRow({
  entry,
  context,
}: {
  entry: DetailEntry;
  context: PullReviewContext;
}) {
  if (entry.native?.kind !== "review_thread.v1")
    return <UnsupportedRow label="review comment" />;
  const thread = entry.native.value;
  const anchorState =
    thread.provider_outdated === true
      ? "Provider marked outdated"
      : thread.native?.provider === "gitlab" && thread.native.value.position
        ? "GitLab diff position"
        : !thread.anchor
          ? "General discussion"
          : !thread.anchor.commit_oid
            ? "Anchor commit unknown"
            : contextMatches(entry, context) &&
                thread.anchor.commit_oid === context.headOid
              ? "Current anchor"
              : "Historical anchor";
  return (
    <li className="space-y-2 break-words text-sm">
      <div className="flex flex-wrap items-center gap-2">
        <p className="font-medium">
          {thread.author?.display_name ??
            thread.author?.login ??
            "Author unavailable"}
        </p>
        <Badge variant="outline" size="sm">
          {anchorState}
        </Badge>
        {thread.provider_resolved !== null ? (
          <Badge variant="outline" size="sm">
            {thread.provider_resolved
              ? "Provider resolved"
              : "Provider unresolved"}
          </Badge>
        ) : null}
      </div>
      {thread.native?.provider === "gitlab" ? (
        <GitlabReviewNote note={thread.native.value} />
      ) : null}
      {thread.anchor ? (
        <p className="text-xs text-muted-foreground">
          {thread.anchor.path}
          {thread.anchor.line ? ` · line ${thread.anchor.line}` : ""}
        </p>
      ) : null}
      <p className="whitespace-pre-wrap break-words">
        {entry.body.state === "known"
          ? entry.body.text === ""
            ? "This comment is empty."
            : entry.body.text
          : entry.body.state === "oversized"
            ? "This comment exceeds the local text limit."
            : "Comment text is not saved."}
      </p>
      <p className="text-xs text-muted-foreground">
        Updated{" "}
        <time dateTime={thread.updated_at}>
          {new Date(thread.updated_at).toLocaleString()}
        </time>
      </p>
    </li>
  );
}

function UnsupportedRow({ label }: { label: string }) {
  return (
    <li className="text-sm">
      <p className="font-medium">Unsupported saved {label}</p>
      <p className="text-xs text-muted-foreground">
        Update Gitru to read this provider representation.
      </p>
    </li>
  );
}

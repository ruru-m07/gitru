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
import { useQuery } from "@tanstack/react-query";
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
import { CommentComposer } from "./comment-composer";

type Props = {
  account: RemoteAccount;
  subjectId: string;
  authorizationView: string | undefined;
  policy: ContextFacetCapability | undefined;
};
const PAGE_LIMIT = 50;
const CURSOR_LIMIT = 100;

/** Disclosure owns only Comments; the Body view and authored editor stay mounted. */
export function ConversationCommentsPanel({
  account,
  subjectId,
  authorizationView,
  policy,
}: Props) {
  const [open, setOpen] = useState(false);
  const [openedOnce, setOpenedOnce] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  async function synchronize(recheck = false) {
    setBusy(true);
    setError(null);
    try {
      await dispatchCapabilityIntent(
        policy,
        recheck ? "recheck_access" : "synchronize",
        () =>
          collaboration
            .forAccount(account)
            .hydrateDetail({ subject_id: subjectId, facet: "comments" }),
      );
    } catch (failure) {
      setError(collaborationErrorMessage(failure));
    } finally {
      setBusy(false);
    }
  }
  return (
    <section className="space-y-2 border-t pt-4" aria-label="Comments">
      <Collapsible
        open={open}
        onOpenChange={(nextOpen) => {
          setOpen(nextOpen);
          if (nextOpen) setOpenedOnce(true);
        }}
      >
        <div className="flex flex-wrap items-center justify-between gap-2">
          <CollapsibleTrigger
            type="button"
            className="flex items-center gap-2 text-sm font-medium"
          >
            <ChevronDown
              aria-hidden="true"
              className={open ? "size-4 rotate-180" : "size-4"}
            />
            Comments
          </CollapsibleTrigger>
          <ReadOnlyCapability policy={policy} />
        </div>
        <CollapsiblePanel
          keepMounted={openedOnce}
          className="motion-reduce:transition-none"
        >
          {open ? (
            <div className="space-y-3 pt-3">
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
                    {account.provider === "bitbucket_cloud" ? (
                      <p className="text-xs text-muted-foreground">
                        Top-level conversation comments are saved here. Inline
                        comments, replies, and pending comments are not
                        included.
                      </p>
                    ) : null}
                    {account.provider === "gitlab" ? (
                      <p className="text-xs text-muted-foreground">
                        Top-level conversation comments are saved here. System
                        activity, inline discussions, and resolvable notes are
                        not included.
                      </p>
                    ) : null}
                    <Button
                      size="sm"
                      variant="ghost"
                      disabled={busy || !canSynchronize(policy)}
                      onClick={() => {
                        void synchronize();
                      }}
                    >
                      Sync comments
                    </Button>
                    <OpenedComments
                      account={account}
                      subjectId={subjectId}
                      authorizationView={authorizationView}
                      policy={policy}
                    />
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
          {openedOnce ? (
            <div className="space-y-3 pt-3">
              <CommentComposer account={account} subjectId={subjectId} />
            </div>
          ) : null}
        </CollapsiblePanel>
      </Collapsible>
    </section>
  );
}

function matchesScope(snapshot: DetailSnapshot, props: Props) {
  return (
    snapshot.subject_id === props.subjectId &&
    snapshot.evidence.facet === "comments" &&
    snapshot.evidence.authorization_epoch ===
      props.account.authorization_epoch &&
    snapshot.authorization_view === props.authorizationView
  );
}

/** One first-page observer and one demand survive changes to the local page. */
function OpenedComments(props: Props) {
  const { account, subjectId, policy } = props;
  const [restartGeneration, setRestartGeneration] = useState(0);
  const head = useQuery({
    ...detailQueryOptions(account, {
      subject_id: subjectId,
      facet: "comments",
      cursor: null,
      limit: PAGE_LIMIT,
    }),
    gcTime: 0,
  });
  const scopeMatches = !head.data || matchesScope(head.data, props);
  const demandError = useVisibleDemand({
    account,
    target: {
      kind: "detail",
      repository_id: null,
      subject_id: subjectId,
      facet: "comments",
    },
    enabled:
      account.state === "active" &&
      canMaintainDemand(policy) &&
      head.data !== undefined &&
      scopeMatches &&
      head.data?.evidence.availability !== "unavailable",
  });
  const data =
    scopeMatches && head.data?.evidence.availability !== "unavailable"
      ? head.data
      : undefined;
  function restart() {
    // Even a synchronous, unchanged read must discard the old cursor chain.
    setRestartGeneration((generation) => generation + 1);
    void head.refetch();
  }
  return (
    <div className="space-y-3">
      {head.isPending || head.isFetching ? (
        <p role="status" className="text-xs text-muted-foreground">
          {head.isPending
            ? "Loading saved comments…"
            : "Refreshing saved conversation…"}
        </p>
      ) : head.isError ? (
        <>
          <p role="alert" className="text-xs text-destructive-foreground">
            {collaborationErrorMessage(head.error)}
          </p>
          <RestartConversation restart={restart} />
        </>
      ) : head.data?.evidence.availability === "unavailable" ? (
        <p className="text-xs text-muted-foreground">
          Saved comments are unavailable in the current authorized view.
        </p>
      ) : !scopeMatches ? (
        <>
          <p role="alert" className="text-xs text-destructive-foreground">
            The saved conversation changed. Read the current view again.
          </p>
          <RestartConversation restart={restart} />
        </>
      ) : data ? (
        <CommentPager
          key={JSON.stringify([
            account.id,
            account.actor_id,
            account.authorization_epoch,
            subjectId,
            data.authorization_view,
            data.evidence.facet_revision,
            restartGeneration,
          ])}
          {...props}
          head={data}
          restart={restart}
        />
      ) : null}
      {demandError ? (
        <p role="alert" className="text-xs text-destructive-foreground">
          {collaborationErrorMessage(demandError)}
        </p>
      ) : null}
    </div>
  );
}

function RestartConversation({ restart }: { restart: () => void }) {
  return (
    <Button size="sm" variant="outline" onClick={restart}>
      Restart saved conversation
    </Button>
  );
}

function CommentPager({
  head,
  restart,
  ...props
}: Props & { head: DetailSnapshot; restart: () => void }) {
  const [pages, setPages] = useState<{
    cursors: Array<string | null>;
    position: number;
  }>({ cursors: [null], position: 0 });
  const cursor = pages.cursors[pages.position];
  const page = useQuery({
    ...detailQueryOptions(props.account, {
      subject_id: props.subjectId,
      facet: "comments",
      cursor,
      limit: PAGE_LIMIT,
    }),
    enabled: cursor !== null,
    gcTime: 0,
  });
  const pending = cursor !== null && (page.isPending || page.isFetching);
  const error = cursor !== null && page.isError;
  const snapshot = cursor === null ? head : page.data;
  const current =
    snapshot &&
    matchesScope(snapshot, props) &&
    snapshot.evidence.facet_revision === head.evidence.facet_revision &&
    snapshot.authorization_view === head.authorization_view;
  const data =
    current && snapshot.evidence.availability !== "unavailable"
      ? snapshot
      : undefined;
  const next = data?.next_cursor ?? null;
  const capReached = pages.position + 1 >= CURSOR_LIMIT;
  const repeatedCursor =
    next !== null &&
    (next === cursor ||
      (pages.cursors.includes(next) &&
        pages.cursors[pages.position + 1] !== next));
  const canNext = !pending && !error && next && !capReached && !repeatedCursor;
  function advance() {
    if (!canNext || next === null) return;
    setPages({
      cursors: [...pages.cursors.slice(0, pages.position + 1), next],
      position: pages.position + 1,
    });
  }
  return (
    <div className="space-y-3">
      {pending ? (
        <p role="status" className="text-xs text-muted-foreground">
          Loading saved comment page…
        </p>
      ) : error ? (
        <p role="alert" className="text-xs text-destructive-foreground">
          {collaborationErrorMessage(page.error)}
        </p>
      ) : !current ? (
        <p role="alert" className="text-xs text-destructive-foreground">
          The saved conversation changed. Restart to read the current pages.
        </p>
      ) : snapshot.evidence.availability === "unavailable" ? (
        <p className="text-xs text-muted-foreground">
          Saved comments are unavailable in the current authorized view.
        </p>
      ) : data ? (
        <>
          <ConversationEvidence snapshot={data} />
          {data.evidence.availability === "missing" ? (
            <p className="text-xs text-muted-foreground">
              Comments have not been saved on this device yet.
            </p>
          ) : data.entries.length ? (
            <ul className="space-y-4" aria-label="Saved conversation comments">
              {data.entries.map((entry) => (
                <CommentRow key={entry.id} entry={entry} />
              ))}
            </ul>
          ) : (
            <p className="text-xs text-muted-foreground">
              {data.evidence.coverage.state === "complete"
                ? "No conversation comments were returned in the saved observation."
                : "No comments are saved in this partial view."}
            </p>
          )}
        </>
      ) : null}
      <nav
        aria-label="Saved comment pages"
        className="flex flex-wrap items-center gap-2"
      >
        <Button
          size="sm"
          variant="outline"
          disabled={pages.position === 0}
          onClick={() => setPages({ ...pages, position: pages.position - 1 })}
        >
          Previous saved comments
        </Button>
        <p className="text-xs text-muted-foreground">
          Saved page {pages.position + 1}
        </p>
        <Button
          size="sm"
          variant="outline"
          disabled={!canNext}
          onClick={advance}
        >
          Next saved comments
        </Button>
        <RestartConversation restart={restart} />
      </nav>
      {next && capReached ? (
        <p className="text-xs text-muted-foreground">
          This view can browse up to 100 saved pages. Restart the saved
          conversation to browse from the beginning.
        </p>
      ) : null}
      {repeatedCursor ? (
        <p role="alert" className="text-xs text-destructive-foreground">
          The saved continuation did not advance. Restart the saved conversation
          to read the current local view.
        </p>
      ) : null}
    </div>
  );
}

function ConversationEvidence({ snapshot }: { snapshot: DetailSnapshot }) {
  const { evidence } = snapshot;
  return (
    <>
      <div className="flex flex-wrap gap-2">
        {evidence.freshness === "stale" ? (
          <Badge variant="outline" size="sm">
            Saved data may be stale
          </Badge>
        ) : null}
        {evidence.coverage.state === "partial" ? (
          <Badge variant="outline" size="sm">
            Partial conversation history
          </Badge>
        ) : null}
      </div>
      {evidence.sync.error ? (
        <p role="alert" className="text-xs text-destructive-foreground">
          {collaborationErrorMessage(evidence.sync.error)}
        </p>
      ) : null}
      {evidence.sync.next_retry_at ? (
        <p role="status" className="text-xs text-muted-foreground">
          Sync can resume after{" "}
          <ObservedTime at={evidence.sync.next_retry_at} />. Saved comments
          remain available.
        </p>
      ) : null}
    </>
  );
}

function ObservedTime({ at }: { at: string }) {
  return <time dateTime={at}>{new Date(at).toLocaleString()}</time>;
}

function CommentRow({ entry }: { entry: DetailEntry }) {
  const author = entry.field_validations.find(
    (field) => field.field === "author",
  );
  const body = entry.field_validations.find((field) => field.field === "body");
  const state = entry.field_validations.find(
    (field) => field.field === "state",
  );
  const updated = entry.field_validations.find(
    (field) => field.field === "updated_at",
  );
  const deleted = state !== undefined && entry.state === "deleted";
  const observed = entry.field_mask.includes("body")
    ? entry.observed_body_state
    : "not_loaded";
  const retained = observed !== "known" && entry.body.state === "known" && body;
  return (
    <li className="space-y-2 break-words text-sm">
      <p className="font-medium">
        {deleted
          ? "Comment deleted"
          : author
            ? (entry.author ?? "Author unavailable")
            : "Unknown author"}
      </p>
      {updated && entry.updated_at ? (
        <p className="text-xs text-muted-foreground">
          Updated <ObservedTime at={entry.updated_at} />
        </p>
      ) : null}
      {!deleted && (observed === "omitted" || observed === "oversized") ? (
        <Badge variant="outline" size="sm">
          {observed === "oversized"
            ? "Latest comment text exceeds the text limit"
            : "Latest comment text was omitted"}
        </Badge>
      ) : null}
      {!deleted ? (
        <p className="whitespace-pre-wrap">
          {entry.body.state === "known" && body
            ? entry.body.text === ""
              ? "This comment is empty."
              : (entry.body.text ?? "No comment text is saved.")
            : entry.body.state === "oversized"
              ? "This comment exceeds the local text limit; no text is saved."
              : entry.body.state === "omitted"
                ? "The provider omitted this comment; no text is saved."
                : "Comment text has not been saved yet."}
        </p>
      ) : null}
      {!deleted && body ? (
        <p className="text-xs text-muted-foreground">
          {retained
            ? "Saved text retained from an earlier observation · last observed "
            : "Saved text last observed "}
          <ObservedTime at={body.validated_at} />
        </p>
      ) : null}
    </li>
  );
}

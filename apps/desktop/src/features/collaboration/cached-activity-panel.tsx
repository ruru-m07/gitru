import {
  type ActivityEvent,
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

type Props = {
  account: RemoteAccount;
  subjectId: string;
  authorizationView: string | undefined;
  policy: ContextFacetCapability | undefined;
};
const PAGE_LIMIT = 50;
const CURSOR_LIMIT = 100;

/** Activity demand and local pages exist only while this disclosure is open. */
export function CachedActivityPanel({
  account,
  subjectId,
  authorizationView,
  policy,
}: Props) {
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
        () =>
          collaboration
            .forAccount(account)
            .hydrateDetail({ subject_id: subjectId, facet: "activity" }),
      );
    } catch (failure) {
      setError(collaborationErrorMessage(failure));
    } finally {
      setBusy(false);
    }
  }
  return (
    <section className="space-y-2 border-t pt-4" aria-label="Activity">
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
            Activity
          </CollapsibleTrigger>
          <ReadOnlyCapability policy={policy} />
        </div>
        <CollapsiblePanel className="motion-reduce:transition-none">
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
                    <Button
                      size="sm"
                      variant="ghost"
                      disabled={busy || !canSynchronize(policy)}
                      onClick={() => {
                        void synchronize();
                      }}
                    >
                      Sync activity
                    </Button>
                    <OpenedActivity
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
        </CollapsiblePanel>
      </Collapsible>
    </section>
  );
}

function matchesScope(snapshot: DetailSnapshot, props: Props) {
  return (
    snapshot.subject_id === props.subjectId &&
    snapshot.evidence.facet === "activity" &&
    snapshot.evidence.authorization_epoch ===
      props.account.authorization_epoch &&
    snapshot.authorization_view === props.authorizationView
  );
}

/** One first-page observer and one demand survive changes to the local page. */
function OpenedActivity(props: Props) {
  const { account, subjectId, policy } = props;
  const [restartGeneration, setRestartGeneration] = useState(0);
  const head = useQuery({
    ...detailQueryOptions(account, {
      subject_id: subjectId,
      facet: "activity",
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
      facet: "activity",
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
            ? "Loading saved activity…"
            : "Refreshing saved activity timeline…"}
        </p>
      ) : head.isError ? (
        <>
          <p role="alert" className="text-xs text-destructive-foreground">
            {collaborationErrorMessage(head.error)}
          </p>
          <RestartActivity restart={restart} />
        </>
      ) : head.data?.evidence.availability === "unavailable" ? (
        <p className="text-xs text-muted-foreground">
          Saved activity is unavailable in the current authorized view.
        </p>
      ) : !scopeMatches ? (
        <>
          <p role="alert" className="text-xs text-destructive-foreground">
            The saved activity timeline changed. Read the current view again.
          </p>
          <RestartActivity restart={restart} />
        </>
      ) : data ? (
        <ActivityPager
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

function RestartActivity({ restart }: { restart: () => void }) {
  return (
    <Button size="sm" variant="outline" onClick={restart}>
      Restart saved activity timeline
    </Button>
  );
}

function ActivityPager({
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
      facet: "activity",
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
          Loading saved activity page…
        </p>
      ) : error ? (
        <p role="alert" className="text-xs text-destructive-foreground">
          {collaborationErrorMessage(page.error)}
        </p>
      ) : !current ? (
        <p role="alert" className="text-xs text-destructive-foreground">
          The saved activity timeline changed. Restart to read the current
          pages.
        </p>
      ) : snapshot.evidence.availability === "unavailable" ? (
        <p className="text-xs text-muted-foreground">
          Saved activity is unavailable in the current authorized view.
        </p>
      ) : data ? (
        <>
          <ActivityEvidence snapshot={data} />
          {props.account.provider === "gitlab" ? (
            <p className="text-xs text-muted-foreground">
              GitLab activity includes system notes, state changes, and label
              changes. Each sync reads up to 1,000 records; other history may be
              missing.
            </p>
          ) : null}
          {data.evidence.availability === "missing" ? (
            <p className="text-xs text-muted-foreground">
              Activity has not been saved on this device yet.
            </p>
          ) : data.entries.length ? (
            <ul className="space-y-4" aria-label="Saved activity timeline">
              {data.entries.map((entry) => (
                <ActivityRow key={entry.id} entry={entry} />
              ))}
            </ul>
          ) : (
            <p className="text-xs text-muted-foreground">
              {data.evidence.coverage.state === "complete"
                ? "No activity was returned in the saved observation."
                : "No activity is saved in this partial view."}
            </p>
          )}
        </>
      ) : null}
      <nav
        aria-label="Saved activity pages"
        className="flex flex-wrap items-center gap-2"
      >
        <Button
          size="sm"
          variant="outline"
          disabled={pages.position === 0}
          onClick={() => setPages({ ...pages, position: pages.position - 1 })}
        >
          Previous saved activity
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
          Next saved activity
        </Button>
        <RestartActivity restart={restart} />
      </nav>
      {next && capReached ? (
        <p className="text-xs text-muted-foreground">
          This view can browse up to 100 saved pages. Restart the saved activity
          timeline to browse from the beginning.
        </p>
      ) : null}
      {repeatedCursor ? (
        <p role="alert" className="text-xs text-destructive-foreground">
          The saved continuation did not advance. Restart the saved activity
          timeline to read the current local view.
        </p>
      ) : null}
    </div>
  );
}

function ActivityEvidence({ snapshot }: { snapshot: DetailSnapshot }) {
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
            Partial activity history
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
          <ObservedTime at={evidence.sync.next_retry_at} />. Saved activity
          remains available.
        </p>
      ) : null}
    </>
  );
}

function ObservedTime({ at }: { at: string }) {
  return <time dateTime={at}>{new Date(at).toLocaleString()}</time>;
}

function activity(entry: DetailEntry): ActivityEvent | null {
  return entry.native?.kind === "activity.v1" ? entry.native.value : null;
}

const ACTIVITY_LABELS: Record<string, string> = {
  system_note: "System note",
  opened: "Opened",
  closed: "Closed",
  reopened: "Reopened",
  merged: "Merged",
  labeled: "Added label",
  unlabeled: "Removed label",
};

function ActivityRow({ entry }: { entry: DetailEntry }) {
  const value = activity(entry);
  if (!value)
    return (
      <li className="space-y-1 break-words text-sm">
        <p className="font-medium">Unsupported saved activity</p>
        <p className="text-xs text-muted-foreground">
          This cached row uses an activity representation Gitru cannot display.
        </p>
      </li>
    );
  return (
    <li className="space-y-2 break-words text-sm">
      <p className="font-medium">
        {value.supported
          ? (entry.title ?? ACTIVITY_LABELS[value.kind] ?? value.kind)
          : `Unsupported activity · ${value.kind}`}
      </p>
      {value.supported && entry.title ? (
        <p className="text-xs text-muted-foreground">{value.kind}</p>
      ) : null}
      {entry.author ? (
        <p className="text-xs text-muted-foreground">{entry.author}</p>
      ) : null}
      {value.occurred_at ? (
        <p className="text-xs text-muted-foreground">
          <ObservedTime at={value.occurred_at} />
        </p>
      ) : null}
      {!value.supported ? (
        <Badge variant="outline" size="sm">
          Event details are not interpreted
        </Badge>
      ) : null}
      {value.description ? (
        <p className="whitespace-pre-wrap">{value.description}</p>
      ) : null}
      {entry.body.state === "known" &&
      entry.body.text &&
      entry.body.text !== value.description ? (
        <p className="whitespace-pre-wrap">{entry.body.text}</p>
      ) : entry.body.state === "oversized" ? (
        <p className="text-xs text-muted-foreground">
          This activity text exceeds the local text limit.
        </p>
      ) : entry.body.state === "omitted" ? (
        <p className="text-xs text-muted-foreground">
          The provider omitted this activity text.
        </p>
      ) : null}
    </li>
  );
}

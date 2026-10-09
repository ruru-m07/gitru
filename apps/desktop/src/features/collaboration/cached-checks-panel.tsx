import {
  type CheckV1,
  type ContextFacetCapability,
  collaboration,
  collaborationErrorMessage,
  type DetailEntry,
  type DetailSnapshot,
  type RemoteAccount,
} from "@gitru/collaboration-client";
import {
  detailProjectionQueryOptions,
  useVisibleDemand,
} from "@gitru/collaboration-client/react";
import { Badge } from "@gitru/ui/components/badge";
import { Button } from "@gitru/ui/components/button";
import { useQuery } from "@tanstack/react-query";
import { CircleCheck, CircleHelp, CircleX, Clock3 } from "lucide-react";
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
  policy: ContextFacetCapability | undefined;
  bodyContext: { headOid: string; facetRevision: string } | null;
};

type CachedChecksRead = {
  first: DetailSnapshot;
  aggregate: DetailSnapshot;
  hasMoreSaved: boolean;
};

const LOCAL_CHECK_PAGE_SIZE = 50;
const MAX_LOCAL_CHECK_PAGES = 100;

/**
 * Read the complete saved generation for the aggregate while keeping the
 * rendered list bounded to its first local page. Native cursors bind the
 * authorization view and facet revision, so a replacement generation aborts
 * this read instead of mixing pages.
 */
async function readCachedChecks(
  account: RemoteAccount,
  subjectId: string,
  signal: AbortSignal,
): Promise<CachedChecksRead> {
  const reader = collaboration.forAccount(account);
  const first = await reader.detail(
    {
      subject_id: subjectId,
      facet: "checks",
      cursor: null,
      limit: LOCAL_CHECK_PAGE_SIZE,
    },
    signal,
  );
  const entries = [...first.entries];
  const hasMoreSaved = first.next_cursor !== null;
  let last = first;
  let pages = 1;
  while (last.next_cursor !== null) {
    if (pages >= MAX_LOCAL_CHECK_PAGES)
      throw new Error("Saved checks exceed the supported local cache bound");
    last = await reader.detail(
      {
        subject_id: subjectId,
        facet: "checks",
        cursor: last.next_cursor,
        limit: LOCAL_CHECK_PAGE_SIZE,
      },
      signal,
    );
    entries.push(...last.entries);
    pages += 1;
  }
  return {
    first,
    aggregate: { ...last, entries, next_cursor: null },
    hasMoreSaved,
  };
}

export type CachedChecksSummaryState =
  | "missing"
  | "unavailable"
  | "syncing"
  | "stale"
  | "partial"
  | "empty"
  | "pending"
  | "failed"
  | "passed"
  | "unknown";

export type CachedChecksSummary = {
  state: CachedChecksSummaryState;
  authoritative: boolean;
  total: number;
};

function rowState(check: CheckV1): CachedChecksSummaryState {
  if (check.state.kind === "check_run") {
    if (check.state.status !== "completed")
      return ["queued", "in_progress", "pending"].includes(check.state.status)
        ? "pending"
        : "unknown";
    if (
      ["success", "neutral", "skipped"].includes(check.state.conclusion ?? "")
    )
      return "passed";
    if (
      [
        "failure",
        "cancelled",
        "timed_out",
        "action_required",
        "stale",
      ].includes(check.state.conclusion ?? "")
    )
      return "failed";
    return check.state.conclusion === null ? "pending" : "unknown";
  }
  if (check.state.state === "success") return "passed";
  if (["failure", "error"].includes(check.state.state)) return "failed";
  if (["pending", "expected"].includes(check.state.state)) return "pending";
  return "unknown";
}

/** Presentation only. This does not infer branch protection or merge policy. */
export function summarizeCachedChecks(
  snapshot: DetailSnapshot,
  currentHead: string | null,
): CachedChecksSummary {
  const total = snapshot.entries.length;
  const nonAuthoritative = (
    state: CachedChecksSummaryState,
  ): CachedChecksSummary => ({ state, authoritative: false, total });
  if (snapshot.evidence.availability === "unavailable")
    return nonAuthoritative("unavailable");
  if (snapshot.evidence.availability === "missing")
    return nonAuthoritative("missing");
  if (snapshot.evidence.sync.state === "syncing")
    return nonAuthoritative("syncing");
  if (
    !currentHead ||
    snapshot.evidence.freshness !== "fresh" ||
    snapshot.entries.some((entry) => entry.head_oid !== currentHead)
  )
    return nonAuthoritative("stale");
  if (
    snapshot.evidence.availability !== "ready" ||
    snapshot.evidence.coverage.state !== "complete" ||
    snapshot.next_cursor !== null
  )
    return nonAuthoritative("partial");
  if (snapshot.entries.length === 0) return nonAuthoritative("empty");

  let failed = false;
  let pending = false;
  let unknown = false;
  for (const entry of snapshot.entries) {
    if (entry.native?.kind !== "check.v1") {
      unknown = true;
      continue;
    }
    const next = rowState(entry.native.value);
    if (next === "failed") failed = true;
    else if (next === "pending") pending = true;
    else if (next === "unknown") unknown = true;
  }
  const state: CachedChecksSummaryState = failed
    ? "failed"
    : pending
      ? "pending"
      : unknown
        ? "unknown"
        : "passed";
  return {
    state,
    authoritative:
      !pending && !unknown && (state === "passed" || state === "failed"),
    total,
  };
}

const summaryCopy: Record<CachedChecksSummaryState, string> = {
  missing: "Checks have not been saved on this device yet.",
  unavailable: "Saved checks are unavailable in the current authorized view.",
  syncing: "Refreshing saved checks for the current head.",
  stale: "Saved checks are stale or belong to a different pull request head.",
  partial:
    "The saved check set is partial, so its result is not authoritative.",
  empty: "The provider returned no checks for this exact head.",
  pending: "One or more reported checks are still running.",
  failed: "One or more reported checks failed.",
  passed: "All reported checks passed for this exact head.",
  unknown: "One or more checks have an outcome Gitru does not recognize.",
};

function check(entry: DetailEntry): CheckV1 | null {
  return entry.native?.kind === "check.v1" ? entry.native.value : null;
}

function rowLabel(value: CheckV1) {
  if (value.state.kind === "check_run")
    return value.state.conclusion
      ? `${value.state.status} · ${value.state.conclusion}`
      : value.state.status;
  return value.state.state;
}

function StatusIcon({ state }: { state: CachedChecksSummaryState }) {
  const className = "mt-0.5 size-4 shrink-0";
  if (state === "passed")
    return <CircleCheck aria-hidden="true" className={className} />;
  if (state === "failed")
    return <CircleX aria-hidden="true" className={className} />;
  if (state === "pending")
    return <Clock3 aria-hidden="true" className={className} />;
  return <CircleHelp aria-hidden="true" className={className} />;
}

function CheckRow({ entry }: { entry: DetailEntry }) {
  const value = check(entry);
  if (!value)
    return (
      <li className="text-sm">
        <p className="font-medium">Unknown saved check</p>
        <p className="text-xs text-muted-foreground">
          This cached row uses an unsupported check representation.
        </p>
      </li>
    );
  const state = rowState(value);
  return (
    <li className="flex min-w-0 gap-2 text-sm">
      <StatusIcon state={state} />
      <div className="min-w-0 space-y-1">
        <p className="break-words font-medium">{value.name}</p>
        <p className="break-words text-xs text-muted-foreground">
          {rowLabel(value)}
          {value.producer ? ` · ${value.producer}` : ""}
        </p>
        {value.description.state === "known" && value.description.text ? (
          <p className="whitespace-pre-wrap break-words text-xs">
            {value.description.text}
          </p>
        ) : value.description.state === "oversized" ? (
          <p className="text-xs text-muted-foreground">
            The provider description exceeds the local text limit.
          </p>
        ) : null}
      </div>
    </li>
  );
}

export function CachedChecksPanel({
  account,
  subjectId,
  policy,
  bodyContext,
}: Props) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const options = detailProjectionQueryOptions(
    account,
    {
      subject_id: subjectId,
      facet: "checks",
      cursor: null,
      limit: LOCAL_CHECK_PAGE_SIZE,
    },
    (signal) => readCachedChecks(account, subjectId, signal),
    [
      "body-context",
      bodyContext?.facetRevision ?? null,
      bodyContext?.headOid ?? null,
    ],
  );
  const query = useQuery({
    ...options,
    enabled: canReadSaved(policy) && bodyContext !== null,
  });
  const demandError = useVisibleDemand({
    account,
    target: {
      kind: "detail",
      repository_id: null,
      subject_id: subjectId,
      facet: "checks",
    },
    enabled:
      account.state === "active" &&
      canMaintainDemand(policy) &&
      query.data?.first.evidence.availability !== "unavailable",
  });
  const pages = canReadSaved(policy) ? query.data : undefined;
  const data = pages?.first;
  const summary = pages
    ? summarizeCachedChecks(pages.aggregate, bodyContext?.headOid ?? null)
    : null;

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
            .hydrateDetail({ subject_id: subjectId, facet: "checks" }),
      );
    } catch (failure) {
      setError(collaborationErrorMessage(failure));
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="space-y-3 border-t pt-4" aria-label="Checks">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <h3 className="text-sm font-medium">Checks</h3>
        <ReadOnlyCapability policy={policy} />
        <Button
          type="button"
          size="sm"
          variant="ghost"
          disabled={busy || !canSynchronize(policy)}
          onClick={() => {
            void synchronize();
          }}
        >
          Sync checks
        </Button>
      </div>
      <p className="text-xs text-muted-foreground">
        Saved provider observations for the exact pull request head. This does
        not determine required checks or merge eligibility.
      </p>
      <SynchronizationAvailability policy={policy} />
      <CapabilityBoundary
        policy={policy}
        busy={busy}
        recheck={() => {
          void synchronize(true);
        }}
      >
        {!bodyContext ? (
          <p role="status" className="text-xs text-muted-foreground">
            Checks need saved current-head metadata before they can be read.
          </p>
        ) : query.isPending ? (
          <p role="status" className="text-xs text-muted-foreground">
            Loading saved checks…
          </p>
        ) : query.isError ? (
          <p role="alert" className="text-xs text-destructive-foreground">
            {collaborationErrorMessage(query.error)}
          </p>
        ) : data && summary ? (
          <>
            <div className="flex flex-wrap items-center gap-2">
              <Badge
                size="sm"
                variant={
                  summary.state === "passed"
                    ? "success"
                    : summary.state === "failed"
                      ? "error"
                      : summary.state === "pending"
                        ? "warning"
                        : "outline"
                }
              >
                {summary.authoritative ? "Exact head" : "Not authoritative"}
              </Badge>
              <p role="status" className="text-xs text-muted-foreground">
                {summaryCopy[summary.state]}
              </p>
            </div>
            {data.evidence.sync.error ? (
              <p role="alert" className="text-xs text-destructive-foreground">
                {collaborationErrorMessage(data.evidence.sync.error)}
              </p>
            ) : null}
            {data.evidence.sync.next_retry_at ? (
              <p role="status" className="text-xs text-muted-foreground">
                Sync can resume after{" "}
                <time dateTime={data.evidence.sync.next_retry_at}>
                  {new Date(data.evidence.sync.next_retry_at).toLocaleString()}
                </time>
                . Saved checks remain available.
              </p>
            ) : null}
            {data.entries.length ? (
              <ul className="space-y-3" aria-label="Saved checks">
                {data.entries.map((entry) => (
                  <CheckRow key={entry.id} entry={entry} />
                ))}
              </ul>
            ) : null}
            {pages.hasMoreSaved ? (
              <p className="text-xs text-muted-foreground">
                More saved checks are available. This view shows the first 50.
              </p>
            ) : null}
          </>
        ) : null}
      </CapabilityBoundary>
      {demandError ? (
        <p role="alert" className="text-xs text-destructive-foreground">
          {collaborationErrorMessage(demandError)}
        </p>
      ) : null}
      {error ? (
        <p role="alert" className="text-xs text-destructive-foreground">
          {error}
        </p>
      ) : null}
    </section>
  );
}

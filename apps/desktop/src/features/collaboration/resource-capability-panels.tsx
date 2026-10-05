import {
  type ContextualCapabilitySnapshot,
  collaboration,
  collaborationErrorMessage,
  type DetailFacet,
  type RemoteAccount,
  type RemoteItemKind,
  type ResourceFacet,
} from "@gitru/collaboration-client";
import { detailQueryOptions } from "@gitru/collaboration-client/react";
import { Badge } from "@gitru/ui/components/badge";
import { Button } from "@gitru/ui/components/button";
import { useQuery } from "@tanstack/react-query";
import { useState } from "react";
import {
  CapabilityBoundary,
  ReadOnlyCapability,
  SynchronizationAvailability,
} from "./capability-boundary";
import {
  canReadSaved,
  canSynchronize,
  dispatchCapabilityIntent,
  facetPolicy,
} from "./capability-policy";
import { NativeParticipantsPanel } from "./native-participants-panel";
import { NativeTasksPanel } from "./native-tasks-panel";

const facetLabels: Record<DetailFacet, string> = {
  body: "Full description",
  comments: "Comments",
  reviews: "Reviews",
  checks: "Checks",
  participants: "Participants",
  tasks: "Tasks",
};

export function ResourceCapabilityPanels({
  account,
  subjectId,
  kind,
  snapshot,
}: {
  account: RemoteAccount;
  subjectId: string;
  kind: RemoteItemKind;
  snapshot: ContextualCapabilitySnapshot | undefined;
}) {
  if (kind === "notification") return null;
  const facets: Array<{ detail: DetailFacet; capability: ResourceFacet }> = [
    {
      detail: "body",
      capability: kind === "pull_request" ? "pull_details" : "issue_details",
    },
    { detail: "comments", capability: "comments" },
    ...(kind === "pull_request"
      ? [
          { detail: "reviews" as const, capability: "reviews" as const },
          { detail: "checks" as const, capability: "checks" as const },
        ]
      : []),
  ];
  return (
    <div className="mt-6 space-y-4">
      {facets.map((facet) => (
        <ResourceFacetPanel
          key={facet.detail}
          account={account}
          subjectId={subjectId}
          detail={facet.detail}
          capability={facet.capability}
          snapshot={snapshot}
        />
      ))}
      {kind === "pull_request" ? (
        <NativeParticipantsPanel
          key={JSON.stringify([
            account.id,
            account.actor_id,
            account.authorization_epoch,
            subjectId,
          ])}
          account={account}
          subjectId={subjectId}
          policy={facetPolicy(snapshot, "participants")}
        />
      ) : null}
      {kind === "pull_request" ? (
        <NativeTasksPanel
          key={JSON.stringify([
            "tasks",
            account.id,
            account.actor_id,
            account.authorization_epoch,
            subjectId,
          ])}
          account={account}
          subjectId={subjectId}
          policy={facetPolicy(snapshot, "tasks")}
        />
      ) : null}
      {kind === "pull_request" ? (
        <section
          className="space-y-2 border-t pt-4"
          aria-label="Remote actions"
        >
          <p className="text-xs text-muted-foreground">
            Remote actions are not implemented. Save a private draft locally or
            open the provider to make changes.
          </p>
          <Button
            size="sm"
            variant="outline"
            disabled
            aria-label="Merge pull request unavailable"
          >
            Merge unavailable
          </Button>
        </section>
      ) : null}
    </div>
  );
}

function ResourceFacetPanel({
  account,
  subjectId,
  detail,
  capability,
  snapshot,
}: {
  account: RemoteAccount;
  subjectId: string;
  detail: DetailFacet;
  capability: ResourceFacet;
  snapshot: ContextualCapabilitySnapshot | undefined;
}) {
  const policy = facetPolicy(snapshot, capability);
  const query = useQuery({
    ...detailQueryOptions(account, {
      subject_id: subjectId,
      facet: detail,
      cursor: null,
      limit: 50,
    }),
    enabled: canReadSaved(policy),
  });
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
            .hydrateDetail({ subject_id: subjectId, facet: detail }),
      );
    } catch (failure) {
      setError(collaborationErrorMessage(failure));
    } finally {
      setBusy(false);
    }
  }
  const data = canReadSaved(policy) ? query.data : undefined;
  return (
    <section
      className="space-y-2 border-t pt-4"
      aria-label={facetLabels[detail]}
    >
      <div className="flex flex-wrap items-center justify-between gap-2">
        <h3 className="text-sm font-medium">{facetLabels[detail]}</h3>
        <ReadOnlyCapability policy={policy} />
        <Button
          size="sm"
          variant="ghost"
          disabled={busy || !canSynchronize(policy)}
          onClick={() => {
            void synchronize();
          }}
        >
          Sync {facetLabels[detail].toLowerCase()}
        </Button>
      </div>
      <SynchronizationAvailability policy={policy} />
      <CapabilityBoundary
        policy={policy}
        recheck={() => {
          void synchronize(true);
        }}
        busy={busy}
      >
        {query.isPending ? (
          <p role="status" className="text-xs text-muted-foreground">
            Loading saved {facetLabels[detail].toLowerCase()}…
          </p>
        ) : query.isError ? (
          <p role="alert" className="text-xs text-destructive-foreground">
            {collaborationErrorMessage(query.error)}
          </p>
        ) : data ? (
          <>
            <div className="flex flex-wrap gap-2 text-xs text-muted-foreground">
              {data.evidence.freshness === "stale" ? (
                <Badge variant="outline" size="sm">
                  Saved data may be stale
                </Badge>
              ) : null}
              {data.evidence.coverage.state === "partial" ? (
                <Badge variant="outline" size="sm">
                  Partial history
                </Badge>
              ) : null}
              {data.evidence.observed_state === "omitted" ||
              data.evidence.observed_state === "oversized" ? (
                <Badge variant="outline" size="sm">
                  {data.evidence.observed_state === "oversized"
                    ? "Latest description exceeds the text limit"
                    : "Latest provider value was omitted"}
                </Badge>
              ) : null}
            </div>
            {data.evidence.availability === "missing" ? (
              <p className="text-xs text-muted-foreground">
                Not saved on this device yet. Sync this facet to load it.
              </p>
            ) : detail === "body" ? (
              <p className="whitespace-pre-wrap break-words text-sm">
                {data.body.state === "known"
                  ? data.body.text === ""
                    ? "This description is empty."
                    : (data.body.text ?? "This resource has no description.")
                  : data.body.state === "oversized"
                    ? "This description exceeds the local text limit."
                    : data.body.state === "omitted" ||
                        data.evidence.observed_state === "omitted"
                      ? "The provider omitted this description; no text is saved."
                      : "The provider has not returned this description yet."}
              </p>
            ) : data.entries.length ? (
              <ul className="space-y-3">
                {data.entries.map((entry) => (
                  <li key={entry.id} className="space-y-1 text-sm">
                    <p className="font-medium">
                      {entry.title ?? entry.author ?? facetLabels[detail]}
                      {entry.state ? ` · ${entry.state}` : ""}
                    </p>
                    {entry.body.state === "known" && entry.body.text ? (
                      <p className="whitespace-pre-wrap break-words">
                        {entry.body.text}
                      </p>
                    ) : null}
                  </li>
                ))}
              </ul>
            ) : (
              <p className="text-xs text-muted-foreground">
                {data.evidence.coverage.state === "complete"
                  ? `No ${facetLabels[detail].toLowerCase()} were returned by the provider.`
                  : `No ${facetLabels[detail].toLowerCase()} are saved in this partial view.`}
              </p>
            )}
            {data.next_cursor ? (
              <p className="text-xs text-muted-foreground">
                More saved entries are available. This view shows the first 50.
              </p>
            ) : null}
          </>
        ) : null}
      </CapabilityBoundary>
      {error ? (
        <p role="alert" className="text-xs text-destructive-foreground">
          {error}
        </p>
      ) : null}
    </section>
  );
}

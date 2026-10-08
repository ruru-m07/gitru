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
import { CachedActivityPanel } from "./cached-activity-panel";
import { CachedChecksPanel } from "./cached-checks-panel";
import {
  CachedReviewsPanel,
  type PullReviewContext,
} from "./cached-reviews-panel";
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
import { ConversationCommentsPanel } from "./conversation-comments-panel";
import { NativeParticipantsPanel } from "./native-participants-panel";
import { NativeTasksPanel } from "./native-tasks-panel";
import { PullCommitsPanel } from "./pull-commits-panel";
import { PullFilesPanel } from "./pull-files-panel";

const facetLabels: Record<DetailFacet, string> = {
  body: "Full description",
  activity: "Activity",
  comments: "Comments",
  reviews: "Reviews",
  review_summaries: "Review decisions",
  review_threads: "Inline discussion",
  checks: "Checks",
  participants: "Participants",
  tasks: "Tasks",
  commits: "Commits",
  files: "Files",
};

export function ResourceCapabilityPanels({
  account,
  subjectId,
  kind,
  snapshot,
  instanceId,
  repositoryId,
  bodyContext,
}: {
  account: RemoteAccount;
  subjectId: string;
  kind: RemoteItemKind;
  snapshot: ContextualCapabilitySnapshot | undefined;
  instanceId: string;
  repositoryId: string | null;
  bodyContext: PullReviewContext | null;
}) {
  if (kind === "notification") return null;
  const facets: Array<{ detail: DetailFacet; capability: ResourceFacet }> = [
    {
      detail: "body",
      capability: kind === "pull_request" ? "pull_details" : "issue_details",
    },
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
        <CachedReviewsPanel
          key={JSON.stringify([
            "reviews",
            account.id,
            account.actor_id,
            account.authorization_epoch,
            subjectId,
            bodyContext?.metadataFacetRevision ?? null,
            bodyContext?.headOid ?? null,
          ])}
          account={account}
          subjectId={subjectId}
          authorizationView={snapshot?.authorization_view}
          policy={facetPolicy(snapshot, "reviews")}
          bodyContext={bodyContext}
        />
      ) : null}
      {kind === "pull_request" ? (
        <CachedChecksPanel
          key={JSON.stringify([
            "checks",
            account.id,
            account.actor_id,
            account.authorization_epoch,
            subjectId,
            bodyContext?.facetRevision ?? null,
            bodyContext?.headOid ?? null,
          ])}
          account={account}
          subjectId={subjectId}
          policy={facetPolicy(snapshot, "checks")}
          bodyContext={bodyContext}
        />
      ) : null}
      <ConversationCommentsPanel
        key={JSON.stringify([
          "comments",
          account.id,
          account.actor_id,
          account.authorization_epoch,
          subjectId,
        ])}
        account={account}
        subjectId={subjectId}
        authorizationView={snapshot?.authorization_view}
        policy={facetPolicy(snapshot, "comments")}
      />
      <CachedActivityPanel
        key={JSON.stringify([
          "activity",
          account.id,
          account.actor_id,
          account.authorization_epoch,
          subjectId,
        ])}
        account={account}
        subjectId={subjectId}
        authorizationView={snapshot?.authorization_view}
        policy={facetPolicy(snapshot, "activity")}
      />
      {kind === "pull_request" ? (
        <PullFilesPanel
          key={JSON.stringify([
            "files",
            account.id,
            account.actor_id,
            account.authorization_epoch,
            subjectId,
          ])}
          account={account}
          subjectId={subjectId}
          instanceId={instanceId}
          repositoryId={repositoryId}
          policy={facetPolicy(snapshot, "pull_files")}
        />
      ) : null}
      {kind === "pull_request" ? (
        <PullCommitsPanel
          key={JSON.stringify([
            "commits",
            account.id,
            account.actor_id,
            account.authorization_epoch,
            subjectId,
          ])}
          account={account}
          subjectId={subjectId}
          instanceId={instanceId}
          repositoryId={repositoryId}
          policy={facetPolicy(snapshot, "pull_commits")}
        />
      ) : null}
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
      {kind === "pull_request" &&
      (account.provider !== "github" || account.host !== "github.com") ? (
        <section
          className="space-y-2 border-t pt-4"
          aria-label="Remote actions"
        >
          <p className="text-xs text-muted-foreground">
            Direct merging is not available for this provider. Open the provider
            to merge this pull request.
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
            {data.evidence.availability === "missing" &&
            !data.pending_intent?.commands.some((command) =>
              command.fields.includes("body"),
            ) ? (
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

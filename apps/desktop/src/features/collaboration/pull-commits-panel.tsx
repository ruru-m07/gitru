import {
  type ContextFacetCapability,
  collaboration,
  collaborationErrorMessage,
  type LocalCloneRecord,
  type PullCommit,
  type PullCommitCompleteness,
  type PullCommitContext,
  type RemoteAccount,
} from "@gitru/collaboration-client";
import {
  pullCommitsQueryOptions,
  useLocalClones,
  useVisibleDemand,
} from "@gitru/collaboration-client/react";
import { listRepositories } from "@gitru/commands";
import { Badge } from "@gitru/ui/components/badge";
import { Button } from "@gitru/ui/components/button";
import {
  Collapsible,
  CollapsiblePanel,
  CollapsibleTrigger,
} from "@gitru/ui/components/collapsible";
import {
  Dialog,
  DialogClose,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogPanel,
  DialogPopup,
  DialogTitle,
  DialogTrigger,
} from "@gitru/ui/components/dialog";
import { useQuery } from "@tanstack/react-query";
import { useRouter } from "@tanstack/react-router";
import { ChevronDown, GitCommitHorizontal } from "lucide-react";
import { useId, useState } from "react";
import { useAppStore } from "@/store/use-app-store";
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
import {
  localLinkStateLabel,
  useLocalLinkIntent,
} from "./local-repository-links";

const PAGE_LIMIT = 50;
const CURSOR_LIMIT = 20;

type Props = {
  account: RemoteAccount;
  subjectId: string;
  instanceId: string;
  repositoryId: string | null;
  policy: ContextFacetCapability | undefined;
};

export function PullCommitsPanel(props: Props) {
  const { account, subjectId, policy } = props;
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
            .hydrateDetail({ subject_id: subjectId, facet: "commits" }),
      );
    } catch (failure) {
      setError(collaborationErrorMessage(failure));
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="space-y-2 border-t pt-4" aria-label="Commits">
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
            Commits
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
                      type="button"
                      size="sm"
                      variant="ghost"
                      disabled={busy || !canSynchronize(policy)}
                      onClick={() => {
                        void synchronize();
                      }}
                    >
                      Sync commits
                    </Button>
                    <ShownPullCommits {...props} />
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

function ShownPullCommits({
  account,
  subjectId,
  instanceId,
  repositoryId,
  policy,
}: Props) {
  const [pages, setPages] = useState<{
    cursors: Array<string | null>;
    position: number;
  }>({ cursors: [null], position: 0 });
  const cursor = pages.cursors[pages.position];
  const query = useQuery(
    pullCommitsQueryOptions(account, {
      subject_id: subjectId,
      cursor,
      limit: PAGE_LIMIT,
    }),
  );
  const demandError = useVisibleDemand({
    account,
    target: {
      kind: "detail",
      repository_id: null,
      subject_id: subjectId,
      facet: "commits",
    },
    enabled: account.state === "active" && canMaintainDemand(policy),
  });
  const next = query.data?.next_cursor ?? null;
  const capReached = pages.position + 1 >= CURSOR_LIMIT;
  const repeatedCursor =
    next !== null &&
    (next === cursor ||
      (pages.cursors.includes(next) &&
        pages.cursors[pages.position + 1] !== next));
  const canNext =
    !query.isPending &&
    !query.isError &&
    next !== null &&
    !capReached &&
    !repeatedCursor;
  function advance() {
    if (!canNext || next === null) return;
    setPages({
      cursors: [...pages.cursors.slice(0, pages.position + 1), next],
      position: pages.position + 1,
    });
  }

  return (
    <div className="space-y-3">
      <p className="text-xs text-muted-foreground">
        Saved in pull request order from base to head. Opening a commit only
        reads an explicitly linked local clone.
      </p>
      {query.isPending ? (
        <p role="status" className="text-xs text-muted-foreground">
          Loading saved commits…
        </p>
      ) : query.isError ? (
        <p role="alert" className="text-xs text-destructive-foreground">
          {collaborationErrorMessage(query.error)}
        </p>
      ) : query.data ? (
        <>
          <CommitEvidence
            completeness={query.data.completeness}
            freshness={query.data.freshness}
            remoteHasMore={query.data.coverage.remote_has_more}
            syncState={query.data.sync.state}
          />
          {query.data.context ? (
            <SavedCommitContext context={query.data.context} />
          ) : null}
          {query.data.sync.error ? (
            <p role="alert" className="text-xs text-destructive-foreground">
              {collaborationErrorMessage(query.data.sync.error)}
            </p>
          ) : null}
          {query.data.sync.next_retry_at ? (
            <p role="status" className="text-xs text-muted-foreground">
              Sync can resume after{" "}
              <time dateTime={query.data.sync.next_retry_at}>
                {new Date(query.data.sync.next_retry_at).toLocaleString()}
              </time>
              . Saved commits remain available.
            </p>
          ) : null}
          {query.data.commits.length ? (
            <ol className="space-y-2" aria-label="Saved pull request commits">
              {query.data.commits.map((commit) => (
                <CommitRow
                  key={commit.oid}
                  account={account}
                  subjectId={subjectId}
                  instanceId={instanceId}
                  repositoryId={repositoryId}
                  sourceRepositoryProviderId={
                    query.data.context?.source_repository_provider_id ?? null
                  }
                  facetRevision={query.data.facet_revision}
                  commit={commit}
                />
              ))}
            </ol>
          ) : (
            <p className="text-xs text-muted-foreground">
              {emptyMessage(query.data.completeness)}
            </p>
          )}
          <nav
            aria-label="Saved commit pages"
            className="flex flex-wrap items-center gap-2"
          >
            <Button
              type="button"
              size="sm"
              variant="outline"
              disabled={pages.position === 0}
              onClick={() =>
                setPages({ ...pages, position: pages.position - 1 })
              }
            >
              Previous saved commits
            </Button>
            <p className="text-xs text-muted-foreground">
              Saved page {pages.position + 1}
            </p>
            <Button
              type="button"
              size="sm"
              variant="outline"
              disabled={!canNext}
              onClick={advance}
            >
              Next saved commits
            </Button>
          </nav>
          {next && capReached ? (
            <p className="text-xs text-muted-foreground">
              This view can browse up to 20 saved pages. Close and reopen
              Commits to start again.
            </p>
          ) : null}
          {repeatedCursor ? (
            <p role="alert" className="text-xs text-destructive-foreground">
              The saved continuation did not advance. Reopen Commits to read the
              current local view.
            </p>
          ) : null}
        </>
      ) : null}
      {demandError ? (
        <p role="alert" className="text-xs text-destructive-foreground">
          {collaborationErrorMessage(demandError)}
        </p>
      ) : null}
    </div>
  );
}

function SavedCommitContext({ context }: { context: PullCommitContext }) {
  return (
    <section
      aria-label="Saved commit context"
      className="rounded-md border bg-muted/30 p-3"
    >
      <dl className="grid grid-cols-[auto_minmax(0,1fr)] gap-x-4 gap-y-2 text-xs">
        <SavedContextValue
          label="Source repository provider ID"
          value={context.source_repository_provider_id}
        />
        <SavedContextValue
          label="Base OID"
          value={shortOid(context.base_oid)}
          fullValue={context.base_oid}
        />
        <SavedContextValue
          label="Head OID"
          value={shortOid(context.head_oid)}
          fullValue={context.head_oid}
        />
        <SavedContextValue
          label="Metadata facet revision"
          value={context.metadata_facet_revision}
        />
      </dl>
    </section>
  );
}

function SavedContextValue({
  label,
  value,
  fullValue,
}: {
  label: string;
  value: string;
  fullValue?: string;
}) {
  return (
    <div className="contents">
      <dt className="text-muted-foreground">{label}</dt>
      <dd className="min-w-0 break-all">
        <code
          aria-label={fullValue ? `${label}: ${fullValue}` : undefined}
          title={fullValue}
        >
          {value}
        </code>
      </dd>
    </div>
  );
}

function CommitEvidence({
  completeness,
  freshness,
  remoteHasMore,
  syncState,
}: {
  completeness: PullCommitCompleteness;
  freshness: "unknown" | "fresh" | "stale";
  remoteHasMore: boolean;
  syncState:
    | "idle"
    | "syncing"
    | "offline"
    | "rate_limited"
    | "auth_required"
    | "error";
}) {
  return (
    <div className="flex flex-wrap gap-2 text-xs text-muted-foreground">
      {freshness === "stale" ? (
        <Badge variant="outline" size="sm">
          Saved data may be stale
        </Badge>
      ) : null}
      {syncState === "syncing" ? (
        <Badge variant="outline" size="sm">
          Updating
        </Badge>
      ) : null}
      {completeness.state === "partial" ? (
        <Badge variant="outline" size="sm">
          Partial commit list
        </Badge>
      ) : null}
      {completeness.state === "capped" ? (
        <Badge variant="outline" size="sm">
          {completeness.reason === "provider_limit"
            ? "Provider limit reached"
            : "Local limit reached"}
        </Badge>
      ) : null}
      {remoteHasMore ? (
        <Badge variant="outline" size="sm">
          More commits exist remotely
        </Badge>
      ) : null}
    </div>
  );
}

function emptyMessage(completeness: PullCommitCompleteness) {
  switch (completeness.state) {
    case "complete":
      return "The provider returned no commits for this saved range.";
    case "syncing":
      return "Commit synchronization is in progress.";
    case "missing":
      return "Commits have not been saved on this device yet.";
    default:
      return "No commits are available in this saved partial view.";
  }
}

function CommitRow({
  account,
  subjectId,
  instanceId,
  repositoryId,
  sourceRepositoryProviderId,
  facetRevision,
  commit,
}: {
  account: RemoteAccount;
  subjectId: string;
  instanceId: string;
  repositoryId: string | null;
  sourceRepositoryProviderId: string | null;
  facetRevision: string | null;
  commit: PullCommit;
}) {
  return (
    <li className="rounded-md border p-3">
      <div className="flex min-w-0 items-start justify-between gap-3">
        <div className="min-w-0 space-y-1">
          <p className="break-words text-sm font-medium">{commit.summary}</p>
          <p className="flex flex-wrap items-center gap-x-2 gap-y-1 text-xs text-muted-foreground">
            <code>{shortOid(commit.oid)}</code>
            <span>{commit.author.name}</span>
            {commit.committed_at ? (
              <time dateTime={commit.committed_at}>
                {new Date(commit.committed_at).toLocaleString()}
              </time>
            ) : null}
          </p>
        </div>
        {repositoryId && facetRevision ? (
          <OpenCommitDialog
            account={account}
            subjectId={subjectId}
            instanceId={instanceId}
            repositoryId={repositoryId}
            sourceRepositoryProviderId={sourceRepositoryProviderId}
            facetRevision={facetRevision}
            commit={commit}
          />
        ) : null}
      </div>
    </li>
  );
}

function OpenCommitDialog({
  account,
  subjectId,
  instanceId,
  repositoryId,
  sourceRepositoryProviderId,
  facetRevision,
  commit,
}: {
  account: RemoteAccount;
  subjectId: string;
  instanceId: string;
  repositoryId: string;
  sourceRepositoryProviderId: string | null;
  facetRevision: string;
  commit: PullCommit;
}) {
  const [open, setOpen] = useState(false);
  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger
        render={<Button type="button" size="sm" variant="outline" />}
      >
        <GitCommitHorizontal aria-hidden="true" />
        Open locally
      </DialogTrigger>
      <DialogPopup>
        <DialogHeader>
          <DialogTitle>Open saved commit</DialogTitle>
          <DialogDescription>
            Choose an explicitly linked clone that already contains{" "}
            {shortOid(commit.oid)}. Gitru will not fetch or change the clone.
          </DialogDescription>
        </DialogHeader>
        <DialogPanel>
          {open ? (
            <CommitClonePicker
              account={account}
              subjectId={subjectId}
              instanceId={instanceId}
              repositoryId={repositoryId}
              sourceRepositoryProviderId={sourceRepositoryProviderId}
              facetRevision={facetRevision}
              commit={commit}
              onOpened={() => setOpen(false)}
            />
          ) : null}
        </DialogPanel>
        <DialogFooter>
          <DialogClose render={<Button type="button" variant="ghost" />}>
            Cancel
          </DialogClose>
        </DialogFooter>
      </DialogPopup>
    </Dialog>
  );
}

function CommitClonePicker({
  account,
  subjectId,
  instanceId,
  repositoryId,
  sourceRepositoryProviderId,
  facetRevision,
  commit,
  onOpened,
}: {
  account: RemoteAccount;
  subjectId: string;
  instanceId: string;
  repositoryId: string;
  sourceRepositoryProviderId: string | null;
  facetRevision: string;
  commit: PullCommit;
  onOpened: () => void;
}) {
  const query = useLocalClones(
    account,
    instanceId,
    repositoryId,
    sourceRepositoryProviderId,
  );
  const registrations = useAppStore((state) => state.repositories);
  const registeredById = new Map(
    registrations.map((repository) => [repository.id, repository]),
  );
  const pickerId = useId();
  const router = useRouter({ warn: false });
  const begin = useLocalLinkIntent();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function choose(clone: LocalCloneRecord) {
    const current = begin();
    setBusy(true);
    setError(null);
    try {
      const receipt = await collaboration
        .forAccount(account)
        .openLocalPullCommit({
          subject_id: subjectId,
          commit_oid: commit.oid,
          facet_revision: facetRevision,
          local_repository_id: clone.local_repository_id,
          link_id: clone.link_id,
          link_generation: clone.generation,
        });
      if (!current()) return;
      if (
        receipt.local_repository_id !== clone.local_repository_id ||
        receipt.oid !== commit.oid
      )
        throw { code: "stale_view" };
      const repositories = await listRepositories({ refreshStale: false });
      if (!current()) return;
      const repository = repositories.find(
        (candidate) => candidate.id === receipt.local_repository_id,
      );
      if (!repository || !router) throw { code: "not_found" };
      const store = useAppStore.getState();
      store.setRepositories(repositories);
      store.syncActiveTab({
        repositoryId: repository.id,
        routePath: "/app/git",
        title: repository.name,
      });
      store.setGitViewStateForRepo(
        {
          leftPanelView: "history",
          changesTab: "history",
          selectedHistoryCommitHash: receipt.oid,
        },
        repository.path,
      );
      await router.navigate({ to: "/app/git" });
      if (current()) onOpened();
    } catch (failure) {
      if (current()) setError(commitNavigationError(failure));
    } finally {
      if (current()) setBusy(false);
    }
  }

  return (
    <section className="space-y-3" aria-label="Linked clones for saved commit">
      {query.isPending ? (
        <p role="status" className="text-sm text-muted-foreground">
          Inspecting linked local clones…
        </p>
      ) : null}
      {query.isError ? (
        <p role="alert" className="text-sm text-destructive-foreground">
          {collaborationErrorMessage(query.error)}
        </p>
      ) : null}
      {error ? (
        <p role="alert" className="text-sm text-destructive-foreground">
          {error}
        </p>
      ) : null}
      {!query.isError
        ? query.data?.clones.map((clone, index) => {
            const registration = registeredById.get(clone.local_repository_id);
            const descriptionId = `${pickerId}-${index}`;
            return (
              <article
                key={clone.link_id}
                className="space-y-2 rounded-md border p-3"
                aria-label={`Local clone ${clone.local_repository_id}`}
              >
                <p className="break-all text-sm">
                  {clone.local_repository_name ?? "Missing local registration"}
                </p>
                <p
                  id={descriptionId}
                  className="break-all text-xs text-muted-foreground"
                >
                  {registration
                    ? `Local path: ${registration.path}`
                    : `Registration: ${clone.local_repository_id}`}
                </p>
                <p className="text-xs text-muted-foreground">
                  {localLinkStateLabel[clone.state]}
                </p>
                <Button
                  type="button"
                  size="sm"
                  aria-describedby={descriptionId}
                  disabled={busy || clone.state !== "linked"}
                  onClick={() => {
                    void choose(clone);
                  }}
                >
                  Open {shortOid(commit.oid)}
                </Button>
              </article>
            );
          })
        : null}
      {query.data && !query.data.clones.length ? (
        <p className="text-sm text-muted-foreground">
          No saved local clone is linked. Open the local Git repository and
          choose Linked collaboration.
        </p>
      ) : null}
      <Button
        type="button"
        size="sm"
        variant="outline"
        disabled={busy || query.isFetching}
        onClick={() => {
          void query.refetch();
        }}
      >
        Inspect clones again
      </Button>
    </section>
  );
}

function shortOid(oid: string) {
  return oid.slice(0, 12);
}

function commitNavigationError(error: unknown) {
  const code =
    typeof error === "object" && error !== null && "code" in error
      ? error.code
      : undefined;
  if (code === "not_found")
    return "That commit is not available in this clone. Fetch it explicitly, then inspect the clone again.";
  return `${collaborationErrorMessage(error)} Inspect the saved commit and clone again before retrying.`;
}

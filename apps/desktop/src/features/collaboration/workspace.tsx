import {
  type ContextFacetCapability,
  type ContextualCapabilitySnapshot,
  collaboration,
  collaborationErrorMessage,
  type InboxSemantics,
  type RemoteAccount,
  type RemoteItemKind,
  type RemoteRepository,
} from "@gitru/collaboration-client";
import {
  draftQueryOptions,
  useCollaborationAccounts,
  useCollaborationItem,
  useCollaborationItems,
  useCollaborationRepositories,
  useContextualCapabilities,
} from "@gitru/collaboration-client/react";
import type { LocalDraft } from "@gitru/commands";
import { Badge } from "@gitru/ui/components/badge";
import { Button } from "@gitru/ui/components/button";
import { Checkbox } from "@gitru/ui/components/checkbox";
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
import {
  ArrowLeft,
  Bell,
  ChevronLeft,
  ChevronRight,
  CircleDot,
  FolderGit2,
  GitPullRequest,
  RefreshCw,
  Search,
} from "lucide-react";
import {
  type FormEvent,
  useDeferredValue,
  useEffect,
  useId,
  useMemo,
  useState,
} from "react";
import PageLayout from "@/components/page-layout";
import { AccountSettingsButton } from "./account-manager";
import {
  CapabilityBoundary,
  ReadOnlyCapability,
  SynchronizationAvailability,
} from "./capability-boundary";
import {
  accountCapabilityTarget,
  canReadSaved,
  canSynchronize,
  dispatchCapabilityIntent,
  facetPolicy,
  feedFacet,
  inboxPresentation,
  repositoryCapabilityTarget,
  resourceCapabilityTarget,
} from "./capability-policy";
import { ProviderLink } from "./provider-link";
import { ResourceCapabilityPanels } from "./resource-capability-panels";
import { CollaborationStatePanel } from "./state-panel";
import { SyncIndicator } from "./sync-indicator";

const labels: Record<RemoteItemKind, string> = {
  pull_request: "Pull requests",
  issue: "Issues",
  notification: "Inbox",
};
const icons = {
  pull_request: GitPullRequest,
  issue: CircleDot,
  notification: Bell,
};

export function CollaborationWorkspace({ kind }: { kind: RemoteItemKind }) {
  const accounts = useCollaborationAccounts();
  const [accountId, setAccountId] = useState<string | null>(null);
  const connected = useMemo(
    () =>
      accounts.data?.accounts.filter(
        (account) => account.state !== "disconnected",
      ) ?? [],
    [accounts.data],
  );
  const accountItems = useMemo(
    () =>
      connected.map((candidate) => ({
        label: `@${candidate.login}`,
        value: candidate.id,
      })),
    [connected],
  );
  const account =
    connected.find((candidate) => candidate.id === accountId) ??
    connected.find((candidate) => candidate.state === "active") ??
    connected[0];
  return (
    <PageLayout className="min-w-0">
      <header className="flex shrink-0 flex-wrap items-center justify-between gap-3 border-b px-5 py-3">
        {kind === "notification" && account ? (
          <InboxHeading account={account} />
        ) : (
          <h1 className="text-base font-semibold">{labels[kind]}</h1>
        )}
        <div className="flex min-w-0 items-center gap-2">
          {connected.length > 1 && account ? (
            <Select
              items={accountItems}
              value={account.id}
              onValueChange={setAccountId}
            >
              <SelectTrigger
                size="sm"
                className="max-w-48"
                aria-label="Provider account"
              >
                <SelectValue />
              </SelectTrigger>
              <SelectPopup>
                {connected.map((candidate) => (
                  <SelectItem key={candidate.id} value={candidate.id}>
                    @{candidate.login}
                  </SelectItem>
                ))}
              </SelectPopup>
            </Select>
          ) : account ? (
            <span className="truncate text-xs text-muted-foreground">
              @{account.login}
            </span>
          ) : null}
          <AccountSettingsButton />
        </div>
      </header>
      {accounts.isPending ? (
        <CollaborationStatePanel title="Loading saved accounts">
          Your connected accounts will appear here.
        </CollaborationStatePanel>
      ) : accounts.isError ? (
        <CollaborationStatePanel
          title="Could not load accounts"
          action="Try again"
          onAction={() => {
            void accounts.refetch();
          }}
        >
          {collaborationErrorMessage(accounts.error)}
        </CollaborationStatePanel>
      ) : !account ? (
        <CollaborationStatePanel title="Bring your remote work into Gitru">
          Connect a GitHub account using Accounts above, then choose
          repositories to sync. Your saved data will be available offline.
        </CollaborationStatePanel>
      ) : (
        <AccountContextWorkspace
          key={`${account.id}:${account.actor_id}:${kind}`}
          account={account}
          kind={kind}
        />
      )}
    </PageLayout>
  );
}

function InboxHeading({ account }: { account: RemoteAccount }) {
  const context = useContextualCapabilities(account, accountCapabilityTarget);
  return (
    <h1 className="text-base font-semibold">
      {inboxPresentation(context.data?.inbox_semantics ?? "none").title}
    </h1>
  );
}

function AccountContextWorkspace({
  account,
  kind,
}: {
  account: RemoteAccount;
  kind: RemoteItemKind;
}) {
  const context = useContextualCapabilities(account, accountCapabilityTarget);
  const [metadata, setMetadata] = useState<{
    instanceId: string;
    semantics: InboxSemantics;
  } | null>(() =>
    context.data
      ? {
          instanceId: context.data.instance.id,
          semantics: context.data.inbox_semantics,
        }
      : null,
  );
  if (
    context.data &&
    (metadata?.instanceId !== context.data.instance.id ||
      metadata.semantics !== context.data.inbox_semantics)
  ) {
    setMetadata({
      instanceId: context.data.instance.id,
      semantics: context.data.inbox_semantics,
    });
  }
  return (
    <AccountWorkspace
      account={account}
      kind={kind}
      snapshot={context.data}
      instanceId={metadata?.instanceId ?? null}
      semantics={metadata?.semantics ?? null}
      contextError={
        context.isError ? collaborationErrorMessage(context.error) : undefined
      }
    />
  );
}

function AccountWorkspace({
  account,
  kind,
  snapshot,
  instanceId,
  semantics,
  contextError,
}: {
  account: RemoteAccount;
  kind: RemoteItemKind;
  snapshot: ContextualCapabilitySnapshot | undefined;
  instanceId: string | null;
  semantics: InboxSemantics | null;
  contextError: string | undefined;
}) {
  const repositoryPolicy = facetPolicy(snapshot, "repositories");
  const repositories = useCollaborationRepositories(
    account,
    canReadSaved(repositoryPolicy),
  );
  const inbox = inboxPresentation(semantics ?? "none");
  const [observedSemantics, setObservedSemantics] = useState(semantics);
  const [manageRepositories, setManageRepositories] = useState(false);
  const [repositoryId, setRepositoryId] = useState<string | null>(null);
  const [search, setSearch] = useState("");
  const searchValue = useDeferredValue(search.trim());
  const [state, setState] = useState<string | null>(
    kind === "notification" ? inbox.initialState : "open",
  );
  if (semantics !== observedSemantics) {
    setObservedSemantics(semantics);
    if (kind === "notification") setState(inbox.initialState);
  }
  const [refreshing, setRefreshing] = useState(false);
  const [refreshError, setRefreshError] = useState<string | null>(null);
  const selected =
    repositories.data?.repositories.filter(
      (repository) => repository.selected,
    ) ?? [];
  const selectedRepositoryId = selected.some(
    (repository) => repository.id === repositoryId,
  )
    ? repositoryId
    : null;
  const context = useContextualCapabilities(
    account,
    selectedRepositoryId && instanceId
      ? repositoryCapabilityTarget(instanceId, selectedRepositoryId)
      : accountCapabilityTarget,
  );
  const policy = facetPolicy(context.data, feedFacet[kind]);

  async function refresh(recheck = false) {
    setRefreshing(true);
    setRefreshError(null);
    try {
      await dispatchCapabilityIntent(
        policy,
        recheck ? "recheck_access" : "synchronize",
        () =>
          collaboration
            .forAccount(account)
            .refresh({ repository_id: selectedRepositoryId, kind }),
      );
    } catch (error) {
      setRefreshError(collaborationErrorMessage(error));
    } finally {
      setRefreshing(false);
    }
  }

  const filterItems =
    kind === "notification"
      ? inbox.filters
      : [
          { label: "Open", value: "open" },
          { label: "Closed", value: "closed" },
          ...(kind === "pull_request"
            ? [{ label: "Merged", value: "merged" }]
            : []),
          { label: "All states", value: "all" },
        ];

  return (
    <>
      <div className="flex shrink-0 flex-wrap items-center gap-2 border-b px-5 py-3">
        <div className="relative min-w-32 flex-1 max-w-sm">
          <Input
            type="search"
            aria-label={`Search saved ${labels[kind].toLowerCase()}`}
            placeholder="Search saved items…"
            maxLength={256}
            value={search}
            onChange={(event) => setSearch(event.currentTarget.value)}
            className="pl-6"
          />
          <Search
            className="pointer-events-none absolute left-2.5 top-2 size-3.5 text-muted-foreground"
            aria-hidden="true"
          />
        </div>
        <Select
          key={`${kind}:${semantics ?? "unknown"}`}
          items={filterItems}
          value={state ?? "all"}
          onValueChange={(value) => setState(value === "all" ? null : value)}
        >
          <SelectTrigger
            size="sm"
            className="min-w-28 w-28"
            aria-label="Item state"
          >
            <SelectValue />
          </SelectTrigger>
          <SelectPopup>
            {filterItems.map((item) => (
              <SelectItem key={item.value} value={item.value}>
                {item.label}
              </SelectItem>
            ))}
          </SelectPopup>
        </Select>
        {kind !== "notification" && selected.length > 1 ? (
          <Select
            items={[
              { label: "All selected repos", value: "all" },
              ...selected.map((repository) => ({
                label: repository.full_name,
                value: repository.id,
              })),
            ]}
            value={selectedRepositoryId ?? "all"}
            onValueChange={(value) =>
              setRepositoryId(value === "all" ? null : value)
            }
          >
            <SelectTrigger
              size="sm"
              className="max-w-52"
              aria-label="Repository"
            >
              <SelectValue />
            </SelectTrigger>
            <SelectPopup>
              <SelectItem value="all">All selected repos</SelectItem>
              {selected.map((repository) => (
                <SelectItem key={repository.id} value={repository.id}>
                  {repository.full_name}
                </SelectItem>
              ))}
            </SelectPopup>
          </Select>
        ) : null}
        <Button
          variant="ghost"
          size="sm"
          onClick={() => setManageRepositories((value) => !value)}
          aria-expanded={manageRepositories}
        >
          <FolderGit2 aria-hidden="true" />
          Repositories
        </Button>
        <Button
          variant="outline"
          size="sm"
          disabled={refreshing || !canSynchronize(policy)}
          onClick={() => {
            void refresh();
          }}
        >
          <RefreshCw aria-hidden="true" />
          Refresh
        </Button>
        {canReadSaved(policy) && policy?.can_recheck_access ? (
          <Button
            size="sm"
            variant="outline"
            disabled={refreshing}
            onClick={() => {
              void refresh(true);
            }}
          >
            Recheck access
          </Button>
        ) : null}
      </div>
      {refreshError ? (
        <p
          role="alert"
          className="border-b px-5 py-2 text-xs text-destructive-foreground"
        >
          {refreshError}
        </p>
      ) : null}
      {manageRepositories || (kind !== "notification" && !selected.length) ? (
        <RepositoryPicker account={account} policy={repositoryPolicy} />
      ) : null}
      <ReadOnlyCapability policy={policy} />
      <div className="px-5">
        <SynchronizationAvailability policy={policy} />
      </div>
      <ItemFeed
        key={`${kind}:${repositoryId}:${state}:${searchValue}`}
        account={account}
        kind={kind}
        instanceId={instanceId}
        policy={policy}
        contextPending={context.isPending}
        contextError={
          contextError ??
          (context.isError
            ? collaborationErrorMessage(context.error)
            : undefined)
        }
        recheck={() => {
          void refresh(true);
        }}
        repositoryId={selectedRepositoryId}
        repositories={repositories.data?.repositories ?? []}
        state={state}
        search={searchValue}
        refresh={refresh}
        refreshing={refreshing}
      />
    </>
  );
}

function RepositoryPicker({
  account,
  policy,
}: {
  account: RemoteAccount;
  policy: ContextFacetCapability | undefined;
}) {
  const query = useCollaborationRepositories(account, canReadSaved(policy));
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [filter, setFilter] = useState("");
  const matchingRepositories =
    (canReadSaved(policy) ? query.data?.repositories : undefined)?.filter(
      (repository) =>
        repository.full_name.toLowerCase().includes(filter.toLowerCase()),
    ) ?? [];
  const repositories = matchingRepositories.slice(0, 100);

  async function discover() {
    setBusy("discovery");
    setError(null);
    try {
      await dispatchCapabilityIntent(
        policy,
        policy?.can_recheck_access ? "recheck_access" : "synchronize",
        () =>
          collaboration
            .forAccount(account)
            .refresh({ repository_id: null, kind: null }),
      );
    } catch (failure) {
      setError(collaborationErrorMessage(failure));
    } finally {
      setBusy(null);
    }
  }

  async function select(repository: RemoteRepository, checked: boolean) {
    if (!canReadSaved(policy)) return;
    setBusy(repository.id);
    setError(null);
    try {
      await collaboration
        .forAccount(account)
        .selectRepository(repository.id, checked);
      await query.refetch();
    } catch (failure) {
      setError(collaborationErrorMessage(failure));
    } finally {
      setBusy(null);
    }
  }

  return (
    <section
      className="shrink-0 border-b bg-muted/15 px-5 py-3"
      aria-label="Repository sync selection"
    >
      <div className="flex flex-wrap items-center justify-between gap-2">
        <div>
          <h2 className="text-sm font-medium">Repositories to sync</h2>
          <p className="text-xs text-muted-foreground">
            Select the repositories you work on to keep their activity
            available.
          </p>
        </div>
        <Button
          size="sm"
          variant="outline"
          disabled={
            busy !== null ||
            (!canSynchronize(policy) && !policy?.can_recheck_access)
          }
          onClick={() => {
            void discover();
          }}
        >
          <RefreshCw aria-hidden="true" />
          {policy?.can_recheck_access
            ? "Recheck repository access"
            : "Discover repositories"}
        </Button>
      </div>
      {!canReadSaved(policy) ? (
        <CapabilityBoundary
          policy={policy}
          recheck={() => {
            void discover();
          }}
          busy={busy !== null}
        >
          {null}
        </CapabilityBoundary>
      ) : null}
      {canReadSaved(policy) && query.data ? (
        <div className="mt-2">
          <SyncIndicator
            state={query.data.sync.state}
            validatedAt={query.data.coverage.validated_at}
            partial={query.data.coverage.state === "partial"}
          />
        </div>
      ) : null}
      {(query.data?.repositories.length ?? 0) > 5 ? (
        <Input
          type="search"
          className="mt-3 max-w-sm"
          aria-label="Filter repositories"
          placeholder="Filter repositories…"
          value={filter}
          onChange={(event) => setFilter(event.currentTarget.value)}
        />
      ) : null}
      <div className="mt-3 max-h-40 overflow-y-auto grid gap-1 sm:grid-cols-2">
        {repositories.map((repository) => (
          <label
            key={repository.id}
            className="flex min-w-0 cursor-pointer items-center gap-2 rounded-md p-1.5 hover:bg-accent"
          >
            <Checkbox
              checked={repository.selected}
              disabled={busy !== null}
              onCheckedChange={(checked) => {
                void select(repository, checked);
              }}
            />
            <span className="truncate text-xs" title={repository.full_name}>
              {repository.full_name}
            </span>
          </label>
        ))}
      </div>
      {matchingRepositories.length > repositories.length ? (
        <p className="mt-2 text-xs text-muted-foreground">
          Showing the first {repositories.length} of{" "}
          {matchingRepositories.length} matching repositories. Refine your
          search to find another repository.
        </p>
      ) : null}
      {!repositories.length && !query.isPending ? (
        <p className="mt-2 text-xs text-muted-foreground">
          {filter
            ? "No repositories match this filter."
            : "Discover repositories to choose what to sync."}
        </p>
      ) : null}
      {error || query.isError ? (
        <p role="alert" className="mt-2 text-xs text-destructive-foreground">
          {error ?? collaborationErrorMessage(query.error)}
        </p>
      ) : null}
    </section>
  );
}

function ItemFeed({
  account,
  kind,
  repositoryId,
  repositories,
  state,
  search,
  refresh,
  refreshing,
  instanceId,
  policy,
  contextPending,
  contextError,
  recheck,
}: {
  account: RemoteAccount;
  kind: RemoteItemKind;
  instanceId: string | null;
  policy: ContextFacetCapability | undefined;
  contextPending: boolean;
  contextError: string | undefined;
  recheck: () => void;
  repositoryId: string | null;
  repositories: RemoteRepository[];
  state: string | null;
  search: string;
  refresh: () => Promise<void>;
  refreshing: boolean;
}) {
  const [cursors, setCursors] = useState<Array<string | null>>([null]);
  useEffect(
    () =>
      collaboration.subscribeChanges((change) => {
        if (change.account_id !== account.id) return;
        const affectsItems =
          change.scope === "repositories" ||
          (kind === "notification"
            ? change.scope === "notifications"
            : repositoryId === null
              ? change.scope.startsWith("repo:") &&
                change.scope.endsWith(`:${kind}`)
              : change.scope === `repo:${repositoryId}:${kind}`);
        if (affectsItems)
          setCursors((values) => (values.length > 1 ? [null] : values));
      }),
    [account.id, kind, repositoryId],
  );
  const cursor = cursors[cursors.length - 1] ?? null;
  const query = useCollaborationItems(
    account,
    {
      kind,
      repository_id: repositoryId,
      state,
      search: search || null,
      cursor,
      limit: 50,
    },
    canReadSaved(policy),
  );
  const [selectedItem, setSelectedItem] = useState<string | null>(null);
  const Icon = icons[kind];
  const page = canReadSaved(policy) ? query.data : undefined;
  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="flex shrink-0 flex-wrap items-center justify-between gap-2 border-b px-5 py-2">
        <span className="text-xs text-muted-foreground">
          {page
            ? `${page.items.length} saved items${search ? " matching your search" : " on this page"}`
            : "Saved activity"}
        </span>
        {page ? (
          <SyncIndicator
            state={
              policy?.synchronize.reason === "temporarily_unavailable"
                ? policy.sync.state
                : page.sync.state === "rate_limited" && canSynchronize(policy)
                  ? "idle"
                  : page.sync.state
            }
            validatedAt={page.coverage.validated_at}
            partial={page.coverage.state === "partial"}
          />
        ) : null}
      </div>
      {page?.sync.error ? (
        <p
          role="alert"
          className="border-b px-5 py-2 text-xs text-destructive-foreground"
        >
          {collaborationErrorMessage(page.sync.error)}
        </p>
      ) : null}
      <div
        className={`grid min-h-0 flex-1 ${selectedItem ? "md:grid-cols-2" : "grid-cols-1"}`}
      >
        <div
          className={`min-w-0 overflow-y-auto ${selectedItem ? "hidden md:block" : ""}`}
        >
          {!canReadSaved(policy) ? (
            <CapabilityBoundary
              policy={policy}
              pending={contextPending}
              error={contextError}
              recheck={recheck}
              busy={refreshing}
            >
              {null}
            </CapabilityBoundary>
          ) : query.isPending ? (
            <CollaborationStatePanel title="Loading saved activity">
              This view reads the data saved on your device.
            </CollaborationStatePanel>
          ) : query.isError ? (
            <CollaborationStatePanel
              title="Could not load this view"
              action="Reload saved items"
              onAction={() => {
                if (cursor !== null) setCursors([null]);
                else void query.refetch();
              }}
            >
              {collaborationErrorMessage(query.error)}
            </CollaborationStatePanel>
          ) : page && !page.items.length ? (
            <CollaborationStatePanel
              title={
                page.coverage.state === "missing"
                  ? "Not synced yet"
                  : page.coverage.state === "partial" || search
                    ? "No saved matches"
                    : "Nothing here yet"
              }
              offline={page.sync.state === "offline"}
              action={canSynchronize(policy) ? "Refresh activity" : undefined}
              onAction={() => {
                void refresh();
              }}
              busy={refreshing}
            >
              {page.coverage.state === "missing"
                ? "Refresh to bring recent activity onto this device."
                : page.coverage.state === "partial"
                  ? "Some history has not been synced. Refresh to check for more activity."
                  : search
                    ? "Search covers saved items. Try another search or refresh to bring in recent activity."
                    : "There are no saved items matching the selected filters."}
            </CollaborationStatePanel>
          ) : (
            <div>
              {page?.items.map((item) => (
                <Button
                  key={item.id}
                  variant="ghost"
                  className="h-auto w-full justify-start gap-3 rounded-none border-b border-border px-5 py-3 text-left whitespace-normal"
                  aria-pressed={selectedItem === item.id}
                  onClick={() => setSelectedItem(item.id)}
                >
                  <Icon
                    className={`size-4 shrink-0 ${item.state === "open" || item.unread ? "text-success-foreground" : "text-muted-foreground"}`}
                    aria-hidden="true"
                  />
                  <div className="min-w-0 flex-1">
                    <p className="line-clamp-2 break-words text-sm font-medium">
                      {item.title}
                    </p>
                    <div className="mt-1 flex min-w-0 flex-wrap items-center gap-x-2 gap-y-1 text-xs text-muted-foreground">
                      <span className="truncate max-w-60">
                        {repositories.find(
                          (repository) => repository.id === item.repository_id,
                        )?.full_name ??
                          item.reason ??
                          "Provider activity"}
                      </span>
                      {item.number ? <span>#{item.number}</span> : null}
                      {item.author ? <span>@{item.author}</span> : null}
                      <span>{item.state}</span>
                    </div>
                  </div>
                  {item.is_draft ? (
                    <Badge variant="outline" size="sm">
                      Draft
                    </Badge>
                  ) : null}
                </Button>
              ))}
            </div>
          )}
        </div>
        {selectedItem && instanceId ? (
          <ItemDetail
            key={selectedItem}
            account={account}
            itemId={selectedItem}
            kind={kind}
            instanceId={instanceId}
            close={() => setSelectedItem(null)}
          />
        ) : null}
      </div>
      {page &&
      (page.next_cursor ||
        cursors.length > 1 ||
        page.coverage.remote_has_more) ? (
        <footer className="flex shrink-0 flex-wrap items-center justify-between gap-2 border-t px-5 py-2">
          <div className="flex gap-2">
            <Button
              size="sm"
              variant="ghost"
              disabled={cursors.length <= 1}
              onClick={() => {
                setCursors((values) => values.slice(0, -1));
                setSelectedItem(null);
              }}
            >
              <ChevronLeft aria-hidden="true" />
              Previous
            </Button>
            <Button
              size="sm"
              variant="ghost"
              disabled={!page.next_cursor}
              onClick={() => {
                if (page.next_cursor) {
                  setCursors((values) => [...values, page.next_cursor]);
                  setSelectedItem(null);
                }
              }}
            >
              Next saved page
              <ChevronRight aria-hidden="true" />
            </Button>
          </div>
          {page.coverage.remote_has_more ? (
            <Button
              size="sm"
              variant="outline"
              disabled={refreshing || !canSynchronize(policy)}
              onClick={() => {
                void refresh();
              }}
            >
              Sync more activity
            </Button>
          ) : null}
        </footer>
      ) : null}
    </div>
  );
}

function ItemDetail({
  account,
  itemId,
  close,
  kind,
  instanceId,
}: {
  account: RemoteAccount;
  itemId: string;
  kind: RemoteItemKind;
  instanceId: string;
  close: () => void;
}) {
  const context = useContextualCapabilities(
    account,
    resourceCapabilityTarget(instanceId, itemId, kind),
  );
  const policy = facetPolicy(context.data, feedFacet[kind]);
  const query = useCollaborationItem(account, itemId, canReadSaved(policy));
  const item = query.data?.item;
  return (
    <article
      className="min-w-0 overflow-y-auto border-l p-5"
      aria-label="Saved item detail"
    >
      <Button variant="ghost" size="sm" onClick={close} className="mb-4">
        <ArrowLeft aria-hidden="true" />
        Back to list
      </Button>
      {!canReadSaved(policy) ? (
        <CapabilityBoundary
          policy={policy}
          pending={context.isPending}
          error={
            context.isError
              ? collaborationErrorMessage(context.error)
              : undefined
          }
        >
          {null}
        </CapabilityBoundary>
      ) : query.isPending ? (
        <p role="status" className="text-sm text-muted-foreground">
          Loading saved detail…
        </p>
      ) : query.isError ? (
        <p role="alert" className="text-sm text-destructive-foreground">
          {collaborationErrorMessage(query.error)}
        </p>
      ) : item ? (
        <>
          <div className="mb-3 flex flex-wrap gap-2">
            <Badge variant="outline">{item.state}</Badge>
            {item.number ? (
              <span className="text-xs text-muted-foreground">
                #{item.number}
              </span>
            ) : null}
            {item.is_draft ? <Badge variant="outline">Draft</Badge> : null}
          </div>
          <h2 className="break-words text-lg font-semibold leading-snug">
            {item.title}
          </h2>
          <p className="mt-2 text-xs text-muted-foreground">
            {item.author ? `@${item.author} · ` : ""}Updated{" "}
            {formatDate(item.updated_at)}
          </p>
          <div className="my-4">
            <ProviderLink url={item.web_url} />
          </div>
          <div className="whitespace-pre-wrap break-words text-sm leading-relaxed">
            {item.body ??
              (item.body_omitted
                ? "This description is not saved on this device. Open the provider to read it."
                : "This saved item has no description.")}
          </div>
          <ReadOnlyCapability policy={policy} />
        </>
      ) : (
        <p className="text-sm text-muted-foreground">
          This item is no longer available in your saved view.
        </p>
      )}
      <ResourceCapabilityPanels
        account={account}
        subjectId={itemId}
        kind={kind}
        snapshot={context.data}
      />
      <PrivateDraft account={account} subjectId={itemId} />
    </article>
  );
}

function PrivateDraft({
  account,
  subjectId,
}: {
  account: RemoteAccount;
  subjectId: string;
}) {
  const query = useQuery(draftQueryOptions(account, subjectId));
  // This is authored editor initialization, scoped by the parent account and
  // subject keys. A provider authorization reset may reread local drafts, but
  // cannot discard an already mounted editor's text during that reread.
  const [loaded, setLoaded] = useState<{ draft: LocalDraft | null } | null>(
    () => (query.data === undefined ? null : { draft: query.data }),
  );
  if (query.data !== undefined && query.data !== loaded?.draft) {
    setLoaded({ draft: query.data });
  }
  return (
    <>
      {query.isError ? (
        <p role="alert" className="mt-6 text-xs text-destructive-foreground">
          {collaborationErrorMessage(query.error)}
        </p>
      ) : null}
      {loaded ? (
        <DraftForm
          account={account}
          itemId={subjectId}
          initialBody={loaded.draft?.body ?? ""}
          generation={loaded.draft?.generation ?? "0"}
        />
      ) : null}
    </>
  );
}

function DraftForm({
  account,
  itemId,
  initialBody,
  generation,
}: {
  account: RemoteAccount;
  itemId: string;
  initialBody: string;
  generation: string;
}) {
  const draftId = useId();
  const [body, setBody] = useState(initialBody);
  const [savedBody, setSavedBody] = useState(initialBody);
  const [draftGeneration, setDraftGeneration] = useState(generation);
  const [previousBody, setPreviousBody] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const queryClient = useQueryClient();
  async function save(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setSaving(true);
    setError(null);
    try {
      const draft = await collaboration
        .forAccount(account)
        .saveDraft({ subject_id: itemId, body, generation: draftGeneration });
      queryClient.setQueryData(
        draftQueryOptions(account, itemId).queryKey,
        draft,
      );
      setDraftGeneration(draft.generation);
      setSavedBody(draft.body);
    } catch (failure) {
      setError(collaborationErrorMessage(failure));
    } finally {
      setSaving(false);
    }
  }
  return (
    <form className="mt-6 space-y-3 border-t pt-4" onSubmit={save}>
      <Field name="private-draft">
        <FieldLabel htmlFor={draftId}>Private draft</FieldLabel>
        <Textarea
          id={draftId}
          name="private-draft"
          value={body}
          disabled={saving}
          onChange={(event) => setBody(event.currentTarget.value)}
          maxLength={100_000}
          rows={4}
          placeholder="Keep a draft here for later…"
        />
      </Field>
      <div className="flex flex-wrap items-center justify-between gap-2">
        <p className="text-xs text-muted-foreground">
          Saved on this device. Visible only to you.
        </p>
        <Button
          type="submit"
          size="sm"
          variant="outline"
          disabled={
            saving || body === savedBody || generation !== draftGeneration
          }
        >
          {saving ? "Saving…" : "Save draft"}
        </Button>
      </div>
      {generation !== draftGeneration ? (
        <div className="space-y-2 text-xs text-muted-foreground">
          <p>
            This draft changed in another tab. Your text is still here. Reload
            the saved draft before saving again.
          </p>
          <Button
            size="sm"
            variant="outline"
            onClick={() => {
              setPreviousBody(body);
              setBody(initialBody);
              setSavedBody(initialBody);
              setDraftGeneration(generation);
              setError(null);
            }}
          >
            Reload saved draft
          </Button>
        </div>
      ) : null}
      {previousBody !== null ? (
        <details className="text-xs text-muted-foreground">
          <summary className="cursor-pointer">Your previous draft text</summary>
          <Textarea
            aria-label="Previous draft text"
            className="mt-2"
            value={previousBody}
            readOnly
            rows={4}
          />
        </details>
      ) : null}
      {error ? (
        <p role="alert" className="text-xs text-destructive-foreground">
          {error}
        </p>
      ) : null}
    </form>
  );
}

function formatDate(value: string) {
  const date = new Date(value);
  return Number.isNaN(date.getTime())
    ? "date unavailable"
    : date.toLocaleString(undefined, {
        dateStyle: "medium",
        timeStyle: "short",
      });
}

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
  useCollaborationAccounts,
  useCollaborationItems,
  useCollaborationRepositories,
  useContextualCapabilities,
  useVisibleDemand,
} from "@gitru/collaboration-client/react";
import { Badge } from "@gitru/ui/components/badge";
import { Button } from "@gitru/ui/components/button";
import { Checkbox } from "@gitru/ui/components/checkbox";
import { Input } from "@gitru/ui/components/input";
import {
  Select,
  SelectItem,
  SelectPopup,
  SelectTrigger,
  SelectValue,
} from "@gitru/ui/components/select";
import {
  Bell,
  ChevronLeft,
  ChevronRight,
  CircleDot,
  FolderGit2,
  GitPullRequest,
  RefreshCw,
  Search,
} from "lucide-react";
import { useDeferredValue, useEffect, useMemo, useState } from "react";
import PageLayout from "@/components/page-layout";
import { AccountSettingsButton } from "./account-manager";
import {
  CapabilityBoundary,
  ReadOnlyCapability,
  SynchronizationAvailability,
} from "./capability-boundary";
import {
  accountCapabilityTarget,
  canMaintainDemand,
  canReadSaved,
  canSynchronize,
  dispatchCapabilityIntent,
  facetPolicy,
  feedFacet,
  inboxPresentation,
  repositoryCapabilityTarget,
} from "./capability-policy";
import { OpenLocalCloneButton } from "./local-clone-picker";
import type { LocalLinkRouteTarget } from "./local-link-navigation";
import { NotificationSubjectView } from "./notification-subject-view";
import { SavedItemDetail } from "./saved-item-detail";
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

const providerLabels: Record<RemoteAccount["provider"], string> = {
  github: "GitHub",
  gitlab: "GitLab",
  bitbucket_cloud: "Bitbucket Cloud",
  bitbucket_dc: "Bitbucket Data Center",
};
const publicInstances: Partial<Record<RemoteAccount["provider"], string>> = {
  github: "github.com",
  gitlab: "gitlab.com",
  bitbucket_cloud: "bitbucket.org",
};

function accountPickerLabel(account: RemoteAccount): string {
  let instance = "";
  try {
    const address = new URL(
      account.host.includes("://") ? account.host : `https://${account.host}`,
    );
    if (
      address.protocol === "https:" &&
      !address.username &&
      !address.password &&
      !address.search &&
      !address.hash
    ) {
      const authority = `${address.host}${address.pathname.replace(/\/$/, "")}`;
      if (authority !== publicInstances[account.provider])
        instance = ` (${authority})`;
    }
  } catch {
    // Installation metadata is presentation only; malformed values add no label.
  }
  return `${providerLabels[account.provider]}${instance} · @${account.login}`;
}

export function CollaborationWorkspace({
  kind,
  target,
}: {
  kind: RemoteItemKind;
  target?: LocalLinkRouteTarget;
}) {
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
        label: accountPickerLabel(candidate),
        value: candidate.id,
      })),
    [connected],
  );
  const account = target
    ? connected.find(
        (candidate) =>
          candidate.id === target.account_id &&
          candidate.authorization_epoch === target.authorization_epoch &&
          candidate.state === "active",
      )
    : (connected.find((candidate) => candidate.id === accountId) ??
      connected.find((candidate) => candidate.state === "active") ??
      connected[0]);
  return (
    <PageLayout className="min-w-0">
      <header className="flex shrink-0 flex-wrap items-center justify-between gap-3 border-b px-5 py-3">
        {kind === "notification" && account ? (
          <InboxHeading account={account} />
        ) : (
          <h1 className="text-base font-semibold">{labels[kind]}</h1>
        )}
        <div className="flex min-w-0 items-center gap-2">
          {!target && connected.length > 1 && account ? (
            <Select
              items={accountItems}
              value={account.id}
              onValueChange={setAccountId}
            >
              <SelectTrigger
                size="sm"
                className="max-w-48"
                aria-label="Provider account"
                title={accountPickerLabel(account)}
              >
                <SelectValue />
              </SelectTrigger>
              <SelectPopup>
                {accountItems.map((candidate) => (
                  <SelectItem key={candidate.value} value={candidate.value}>
                    {candidate.label}
                  </SelectItem>
                ))}
              </SelectPopup>
            </Select>
          ) : account ? (
            <span
              className="truncate text-xs text-muted-foreground"
              title={accountPickerLabel(account)}
            >
              {accountPickerLabel(account)}
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
      ) : target && !account ? (
        <CollaborationStatePanel title="Linked account unavailable">
          Reconnect and open this link again from Local Git. No other account
          has been selected.
        </CollaborationStatePanel>
      ) : !account ? (
        <CollaborationStatePanel title="Bring your remote work into Gitru">
          Connect a provider account using Accounts above, then choose
          repositories to sync. Your saved data will be available offline.
        </CollaborationStatePanel>
      ) : (
        <AccountContextWorkspace
          key={`${account.id}:${account.actor_id}:${kind}`}
          account={account}
          kind={kind}
          target={target}
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
  target,
}: {
  account: RemoteAccount;
  kind: RemoteItemKind;
  target?: LocalLinkRouteTarget;
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
  if (target && !context.data)
    return (
      <CollaborationStatePanel
        title={
          context.isError
            ? "Linked installation unavailable"
            : "Reading linked installation"
        }
      >
        {context.isError
          ? collaborationErrorMessage(context.error)
          : "Waiting for current saved account access."}
      </CollaborationStatePanel>
    );
  if (target && context.data && context.data.instance.id !== target.instance_id)
    return (
      <CollaborationStatePanel title="Linked installation unavailable">
        Open this link again from Local Git.
      </CollaborationStatePanel>
    );
  return (
    <AccountWorkspace
      account={account}
      kind={kind}
      target={target}
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
  target,
  snapshot,
  instanceId,
  semantics,
  contextError,
}: {
  account: RemoteAccount;
  kind: RemoteItemKind;
  target?: LocalLinkRouteTarget;
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
  const [repositoryId, setRepositoryId] = useState<string | null>(
    target?.repository_id ?? null,
  );
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
  const linkedRepository =
    target && canReadSaved(repositoryPolicy)
      ? repositories.data?.repositories.find(
          (repository) => repository.id === target.repository_id,
        )
      : undefined;
  const selectedRepositoryId = target
    ? target.repository_id
    : selected.some((repository) => repository.id === repositoryId)
      ? repositoryId
      : null;
  const context = useContextualCapabilities(
    account,
    selectedRepositoryId && instanceId
      ? repositoryCapabilityTarget(instanceId, selectedRepositoryId)
      : accountCapabilityTarget,
  );
  const policy = facetPolicy(context.data, feedFacet[kind]);
  useVisibleDemand({
    account,
    target: {
      kind: "repositories",
      repository_id: null,
      subject_id: null,
      facet: null,
    },
    enabled: account.state === "active" && canMaintainDemand(repositoryPolicy),
  });
  useVisibleDemand({
    account,
    target: {
      kind:
        kind === "notification"
          ? "inbox"
          : kind === "pull_request"
            ? "pull_requests"
            : "issues",
      repository_id: kind === "notification" ? null : selectedRepositoryId,
      subject_id: null,
      facet: null,
    },
    enabled: account.state === "active" && canMaintainDemand(policy),
  });

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

  if (target && !repositories.isPending && !linkedRepository)
    return (
      <CollaborationStatePanel title="Linked repository unavailable">
        This exact saved repository is missing or inaccessible. Open its link
        again from Local Git.
      </CollaborationStatePanel>
    );
  return (
    <>
      {target && linkedRepository && !linkedRepository.selected ? (
        <div className="space-y-2 border-b p-5">
          <p className="text-sm">
            {linkedRepository.full_name} is not selected for synchronization.
            This view stays scoped to that repository.
          </p>
          <Button
            size="sm"
            disabled={refreshing || !canReadSaved(repositoryPolicy)}
            onClick={() => {
              if (!canReadSaved(repositoryPolicy)) return;
              setRefreshing(true);
              setRefreshError(null);
              void collaboration
                .forAccount(account)
                .selectRepository(linkedRepository.id, true)
                .then(() => collaboration.wake())
                .catch((failure) =>
                  setRefreshError(collaborationErrorMessage(failure)),
                )
                .finally(() => setRefreshing(false));
            }}
          >
            Select this repository
          </Button>
        </div>
      ) : null}
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
            disabled={!!target}
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
        {!target ? (
          <Button
            variant="ghost"
            size="sm"
            onClick={() => setManageRepositories((value) => !value)}
            aria-expanded={manageRepositories}
          >
            <FolderGit2 aria-hidden="true" />
            Repositories
          </Button>
        ) : null}
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
      {!target &&
      (manageRepositories || (kind !== "notification" && !selected.length)) ? (
        <RepositoryPicker account={account} policy={repositoryPolicy} />
      ) : null}
      {selectedRepositoryId &&
      instanceId &&
      canReadSaved(repositoryPolicy) &&
      (target
        ? !!linkedRepository
        : selected.some(
            (repository) => repository.id === selectedRepositoryId,
          )) ? (
        <div className="px-5 py-2">
          <OpenLocalCloneButton
            account={account}
            instanceId={instanceId}
            repositoryId={selectedRepositoryId}
          />
        </div>
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
              aria-label={repository.full_name}
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
          kind === "notification" ? (
            <NotificationSubjectView
              key={selectedItem}
              account={account}
              notificationId={selectedItem}
              instanceId={instanceId}
              close={() => setSelectedItem(null)}
            />
          ) : (
            <SavedItemDetail
              key={selectedItem}
              account={account}
              itemId={selectedItem}
              kind={kind}
              instanceId={instanceId}
              close={() => setSelectedItem(null)}
            />
          )
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

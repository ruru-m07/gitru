import {
  type CapabilitySnapshot,
  collaboration,
  collaborationErrorMessage,
  type GithubCliAccount,
  type RemoteAccount,
} from "@gitru/collaboration-client";
import {
  useCollaborationAccounts,
  useCollaborationCapabilities,
  useGithubCliAccounts,
} from "@gitru/collaboration-client/react";
import { Badge } from "@gitru/ui/components/badge";
import { Button } from "@gitru/ui/components/button";
import {
  Field,
  FieldDescription,
  FieldLabel,
} from "@gitru/ui/components/field";
import { Input } from "@gitru/ui/components/input";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import {
  ExternalLink,
  Github,
  KeyRound,
  Plus,
  RotateCcw,
  Terminal,
  UserRound,
  X,
} from "lucide-react";
import {
  type ComponentProps,
  cloneElement,
  type FormEvent,
  type MouseEvent,
  type ReactElement,
  useEffect,
  useId,
  useRef,
  useState,
} from "react";
import { BitbucketIcon } from "@/components/svgs/bitbucket";
import { GitlabIcon } from "@/components/svgs/gitlab-icon";
import { openExternalUrlSafely } from "@/lib/open-external-url";
import { requestAccountSettings } from "./account-dialog-events";

export function isTrustedAccountWindow(): boolean {
  try {
    return getCurrentWebview().label === "main";
  } catch {
    return false;
  }
}

export function AccountSettingsButton({
  trigger,
}: {
  trigger?: ReactElement<ComponentProps<typeof Button>>;
}) {
  const [error, setError] = useState(false);
  const errorId = useId();
  const button = trigger ?? (
    <Button variant="outline" size="sm">
      <UserRound aria-hidden="true" />
      Accounts
    </Button>
  );
  return (
    <span className="contents">
      {cloneElement(button, {
        type: "button",
        "aria-describedby": error ? errorId : undefined,
        onClick: (event: MouseEvent<HTMLButtonElement>) => {
          button.props.onClick?.(event);
          if (event.defaultPrevented) return;
          setError(false);
          void requestAccountSettings().catch(() => setError(true));
        },
      })}
      {error ? (
        <span
          id={errorId}
          role="alert"
          className="text-xs text-destructive-foreground"
        >
          Could not open account settings. Try again.
        </span>
      ) : null}
    </span>
  );
}

export function AccountManager() {
  const accounts = useCollaborationAccounts();
  const trusted = isTrustedAccountWindow();
  const [disconnects, setDisconnects] = useState<
    Record<string, { login: string; busy: boolean; error: string | null }>
  >({});

  async function disconnect(account: RemoteAccount) {
    setDisconnects((previous) => ({
      ...previous,
      [account.id]: { login: account.login, busy: true, error: null },
    }));
    try {
      await collaboration.disconnect(account.id);
      setDisconnects((previous) => ({
        ...previous,
        [account.id]: { login: account.login, busy: false, error: null },
      }));
    } catch (failure) {
      setDisconnects((previous) => ({
        ...previous,
        [account.id]: {
          login: account.login,
          busy: false,
          error: collaborationErrorMessage(failure),
        },
      }));
    }
  }
  return (
    <div className="min-w-0 space-y-5">
      {accounts.isPending ? (
        <p className="text-sm text-muted-foreground" role="status">
          Loading saved accounts…
        </p>
      ) : null}
      {accounts.isError ? (
        <p className="text-sm text-destructive-foreground" role="alert">
          {collaborationErrorMessage(accounts.error)}
        </p>
      ) : null}
      <div className="space-y-2">
        {accounts.data?.accounts.map((account) => (
          <AccountRow
            key={account.id}
            account={account}
            trusted={trusted}
            busy={disconnects[account.id]?.busy ?? false}
            retry={Boolean(disconnects[account.id]?.error)}
            onDisconnect={() => {
              void disconnect(account);
            }}
          />
        ))}
      </div>
      {Object.entries(disconnects)
        .filter(([, operation]) => operation.error)
        .map(([accountId, operation]) => (
          <p
            key={accountId}
            role="alert"
            className="text-xs text-destructive-foreground"
          >
            Could not finish disconnecting @{operation.login}. {operation.error}
          </p>
        ))}
      {trusted ? (
        <>
          <ConnectGithubForm />
          <ConnectGitlabForm />
          <ConnectBitbucketCloudForm />
        </>
      ) : (
        <div className="space-y-3 rounded-lg border bg-muted/30 p-4 text-sm text-muted-foreground">
          <p>Connected accounts are shared across tabs.</p>
          <AccountSettingsButton />
        </div>
      )}
      <p className="break-words text-xs leading-relaxed text-muted-foreground">
        Provider accounts work independently of Gitru cloud sign-in.
      </p>
    </div>
  );
}

function AccountRow({
  account,
  trusted,
  busy,
  retry,
  onDisconnect,
}: {
  account: RemoteAccount;
  trusted: boolean;
  busy: boolean;
  retry: boolean;
  onDisconnect: () => void;
}) {
  const capabilities = useCollaborationCapabilities(
    account,
    account.state === "active" && !account.notifications_supported,
  );
  const inboxMessage =
    account.state === "active" && !account.notifications_supported
      ? inboxCapabilityMessage(account, capabilities.data)
      : null;
  return (
    <div className="min-w-0 rounded-lg border p-3">
      <div className="flex min-w-0 items-center gap-2 sm:gap-3">
        <AccountProviderIcon provider={account.provider} />
        <div className="min-w-0 flex-1">
          <p className="truncate text-sm font-medium">
            {account.display_name ?? account.login}{" "}
            <span className="font-normal text-muted-foreground">
              @{account.login}
            </span>
          </p>
          <p className="truncate text-xs text-muted-foreground">
            {accountProviderName(account.provider)} · {account.host}
          </p>
        </div>
        <Badge
          variant={account.state === "active" ? "success" : "warning"}
          size="sm"
        >
          {account.state === "active"
            ? "Connected"
            : account.state === "auth_required"
              ? "Reconnect"
              : "Disconnected"}
        </Badge>
        {trusted && (account.state !== "disconnected" || retry || busy) ? (
          <Button
            variant="ghost"
            size="icon-sm"
            disabled={busy}
            aria-label={
              retry
                ? `Retry credential cleanup for ${account.login}`
                : `Disconnect ${account.login}`
            }
            onClick={onDisconnect}
          >
            {retry ? (
              <RotateCcw aria-hidden="true" />
            ) : (
              <X aria-hidden="true" />
            )}
          </Button>
        ) : null}
      </div>
      {inboxMessage ? (
        <p className="mt-2 text-xs text-muted-foreground">{inboxMessage}</p>
      ) : null}
    </div>
  );
}

function inboxCapabilityMessage(
  account: RemoteAccount,
  snapshot: CapabilitySnapshot | undefined,
): string | null {
  let baseUrl: string;
  try {
    baseUrl = new URL(
      account.host.includes("://") ? account.host : `https://${account.host}`,
    ).href;
  } catch {
    return null;
  }
  if (
    !snapshot ||
    snapshot.account_id !== account.id ||
    snapshot.instance.provider !== account.provider ||
    snapshot.instance.base_url !== baseUrl
  )
    return null;
  const inbox = snapshot.facets.find((facet) => facet.facet === "inbox");
  if (!inbox) return null;
  if (inbox.state === "unsupported")
    return "Inbox isn’t supported by this connection in Gitru.";
  if (
    inbox.state === "unavailable" &&
    inbox.reason === "missing_scope" &&
    snapshot.inbox_semantics !== "none"
  )
    return "Inbox needs additional token permissions. Reconnect with a credential that supports inbox to sync notifications.";
  return null;
}

function accountProviderName(provider: RemoteAccount["provider"]) {
  switch (provider) {
    case "github":
      return "GitHub";
    case "gitlab":
      return "GitLab";
    case "bitbucket_cloud":
      return "Bitbucket Cloud";
    case "bitbucket_dc":
      return "Bitbucket Data Center";
  }
}

function AccountProviderIcon({
  provider,
}: {
  provider: RemoteAccount["provider"];
}) {
  const props = {
    className: "size-5 shrink-0 text-muted-foreground",
    "aria-hidden": true as const,
  };
  switch (provider) {
    case "github":
      return <Github {...props} />;
    case "gitlab":
      return <GitlabIcon {...props} aria-labelledby={undefined} />;
    case "bitbucket_cloud":
    case "bitbucket_dc":
      return <BitbucketIcon {...props} />;
  }
}

export function ConnectGitlabForm() {
  const trusted = isTrustedAccountWindow();
  const tokenId = useId();
  const tokenInput = useRef<HTMLInputElement>(null);
  const connecting = useRef(false);
  const alive = useRef(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [connected, setConnected] = useState<string | null>(null);
  const [browserFailed, setBrowserFailed] = useState(false);
  useEffect(() => {
    alive.current = true;
    return () => {
      alive.current = false;
    };
  }, []);

  async function connect(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (connecting.current || !isTrustedAccountWindow()) return;
    const token = tokenInput.current?.value.trim() ?? "";
    if (!token) return;
    // Only the native credential command receives the transient password value.
    if (tokenInput.current) tokenInput.current.value = "";
    connecting.current = true;
    setBusy(true);
    setError(null);
    setConnected(null);
    try {
      const account = await collaboration.connectGitlab(token);
      if (alive.current) setConnected(account.login);
    } catch (failure) {
      if (alive.current)
        setError(
          typeof failure === "object" &&
            failure !== null &&
            "code" in failure &&
            failure.code === "rate_limited"
            ? "GitLab asked Gitru to wait. Try connecting again later."
            : collaborationErrorMessage(failure),
        );
    } finally {
      connecting.current = false;
      if (alive.current) setBusy(false);
    }
  }

  if (!trusted) return null;
  return (
    <form
      className="min-w-0 space-y-3 rounded-xl border p-4"
      onSubmit={connect}
    >
      <div className="flex items-center gap-2 text-sm font-medium">
        <GitlabIcon
          className="size-4"
          aria-hidden="true"
          aria-labelledby={undefined}
        />
        Connect GitLab.com
      </div>
      <p className="text-xs leading-relaxed text-muted-foreground">
        Connect with a personal access token to browse your repositories, merge
        requests, and issues. GitLab inbox isn’t supported in Gitru yet.
      </p>
      <Field name="gitlab-token">
        <FieldLabel htmlFor={tokenId}>GitLab personal access token</FieldLabel>
        <Input
          id={tokenId}
          ref={tokenInput}
          name="gitlab-token"
          type="password"
          required
          autoComplete="off"
          placeholder="GitLab.com personal access token"
          disabled={busy}
          aria-invalid={Boolean(error)}
          aria-describedby={`${tokenId}-help${error ? ` ${tokenId}-error` : ""}`}
        />
        <FieldDescription id={`${tokenId}-help`}>
          Your token must allow reading your profile and member projects. For a
          legacy token, use read_api and read_user. Gitru checks access before
          connecting, then saves the token in your system credential store.
        </FieldDescription>
      </Field>
      <Button
        type="button"
        variant="link"
        size="xs"
        className="px-0"
        disabled={busy}
        onClick={() => {
          setBrowserFailed(false);
          void openExternalUrlSafely(
            "https://gitlab.com/-/user_settings/personal_access_tokens",
          ).then(
            (opened) => {
              if (alive.current) setBrowserFailed(!opened);
            },
            () => {
              if (alive.current) setBrowserFailed(true);
            },
          );
        }}
      >
        <ExternalLink aria-hidden="true" />
        Create a GitLab token
      </Button>
      <div className="flex flex-wrap items-center justify-between gap-3">
        <span className="flex items-center gap-1.5 text-xs text-muted-foreground">
          <KeyRound className="size-3.5" aria-hidden="true" />
          gitlab.com
        </span>
        <Button type="submit" size="sm" disabled={busy}>
          <Plus aria-hidden="true" />
          {busy ? "Connecting to GitLab…" : "Connect GitLab account"}
        </Button>
      </div>
      {error ? (
        <p
          id={`${tokenId}-error`}
          role="alert"
          className="text-xs text-destructive-foreground"
        >
          {error}
        </p>
      ) : null}
      {browserFailed ? (
        <p role="alert" className="text-xs text-destructive-foreground">
          Could not open your browser. Try again.
        </p>
      ) : null}
      {connected ? (
        <p role="status" className="text-xs text-success-foreground">
          Connected to GitLab as {connected}. Choose repositories to sync.
        </p>
      ) : null}
    </form>
  );
}

export function ConnectBitbucketCloudForm() {
  const trusted = isTrustedAccountWindow();
  const tokenId = useId();
  const tokenInput = useRef<HTMLInputElement>(null);
  const connecting = useRef(false);
  const alive = useRef(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [connected, setConnected] = useState<string | null>(null);
  const [browserFailed, setBrowserFailed] = useState(false);
  useEffect(() => {
    alive.current = true;
    return () => {
      alive.current = false;
    };
  }, []);

  async function connect(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (connecting.current || !isTrustedAccountWindow()) return;
    const token = tokenInput.current?.value.trim() ?? "";
    if (!token) return;
    // Only the native credential command receives the transient password value.
    if (tokenInput.current) tokenInput.current.value = "";
    connecting.current = true;
    setBusy(true);
    setError(null);
    setConnected(null);
    try {
      const account = await collaboration.connectBitbucketCloud(token);
      if (alive.current) setConnected(account.login);
    } catch (failure) {
      if (alive.current)
        setError(
          typeof failure === "object" &&
            failure !== null &&
            "code" in failure &&
            failure.code === "rate_limited"
            ? "Bitbucket asked Gitru to wait. Try connecting again later."
            : collaborationErrorMessage(failure),
        );
    } finally {
      connecting.current = false;
      if (alive.current) setBusy(false);
    }
  }

  if (!trusted) return null;
  return (
    <form
      className="min-w-0 space-y-3 rounded-xl border p-4"
      onSubmit={connect}
    >
      <div className="flex items-center gap-2 text-sm font-medium">
        <BitbucketIcon
          className="size-4"
          aria-hidden="true"
          aria-labelledby={undefined}
        />
        Connect Bitbucket Cloud
      </div>
      <p className="text-xs leading-relaxed text-muted-foreground">
        Connect with an API token to browse your repositories and pull requests.
        Pull requests are read-only in Gitru. Issues and an inbox aren’t
        available for this provider.
      </p>
      <Field name="bitbucket-cloud-token">
        <FieldLabel htmlFor={tokenId}>Bitbucket Cloud API token</FieldLabel>
        <Input
          id={tokenId}
          ref={tokenInput}
          name="bitbucket-cloud-token"
          type="password"
          required
          autoComplete="off"
          placeholder="Bitbucket Cloud API token"
          disabled={busy}
          aria-invalid={Boolean(error)}
          aria-describedby={`${tokenId}-help${error ? ` ${tokenId}-error` : ""}`}
        />
        <FieldDescription id={`${tokenId}-help`}>
          Create an API token with read:user:bitbucket,
          read:workspace:bitbucket, and read:repository:bitbucket permissions.
          Add read:pullrequest:bitbucket to read pull requests. You can connect
          without it to browse repositories. Gitru verifies your account before
          connecting, then saves the token in your system credential store.
        </FieldDescription>
      </Field>
      <Button
        type="button"
        variant="link"
        size="xs"
        className="px-0"
        disabled={busy}
        onClick={() => {
          setBrowserFailed(false);
          void openExternalUrlSafely(
            "https://support.atlassian.com/bitbucket-cloud/docs/create-an-api-token/",
          ).then(
            (opened) => {
              if (alive.current) setBrowserFailed(!opened);
            },
            () => {
              if (alive.current) setBrowserFailed(true);
            },
          );
        }}
      >
        <ExternalLink aria-hidden="true" />
        Create a Bitbucket API token
      </Button>
      <div className="flex flex-wrap items-center justify-between gap-3">
        <span className="flex items-center gap-1.5 text-xs text-muted-foreground">
          <KeyRound className="size-3.5" aria-hidden="true" />
          bitbucket.org
        </span>
        <Button type="submit" size="sm" disabled={busy}>
          <Plus aria-hidden="true" />
          {busy ? "Connecting to Bitbucket…" : "Connect Bitbucket account"}
        </Button>
      </div>
      {error ? (
        <p
          id={`${tokenId}-error`}
          role="alert"
          className="text-xs text-destructive-foreground"
        >
          {error}
        </p>
      ) : null}
      {browserFailed ? (
        <p role="alert" className="text-xs text-destructive-foreground">
          Could not open your browser. Try again.
        </p>
      ) : null}
      {connected ? (
        <p role="status" className="text-xs text-success-foreground">
          Connected to Bitbucket as {connected}. Choose repositories to sync.
        </p>
      ) : null}
    </form>
  );
}

export function ConnectGithubForm() {
  const trusted = isTrustedAccountWindow();
  const cli = useGithubCliAccounts(trusted);
  const tokenId = useId();
  const tokenInput = useRef<HTMLInputElement>(null);
  const connecting = useRef(false);
  const [operation, setOperation] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [connected, setConnected] = useState<string | null>(null);
  const [accountFilter, setAccountFilter] = useState("");
  const busy = operation !== null;
  const filteredAccounts = (cli.data?.accounts ?? []).filter((account) =>
    account.login.toLowerCase().includes(accountFilter.trim().toLowerCase()),
  );
  const visibleAccounts = filteredAccounts.slice(0, 10);

  async function connectAccount(
    operationId: string,
    connect: () => Promise<RemoteAccount>,
  ) {
    if (connecting.current || !isTrustedAccountWindow()) return;
    connecting.current = true;
    setOperation(operationId);
    setError(null);
    setConnected(null);
    try {
      const account = await connect();
      setConnected(account.login);
    } catch (failure) {
      setError(
        operationId !== "pat" &&
          typeof failure === "object" &&
          failure !== null &&
          "code" in failure &&
          failure.code === "stale_view"
          ? "This GitHub CLI account changed or the check expired. Check again and choose the account."
          : collaborationErrorMessage(failure),
      );
    } finally {
      connecting.current = false;
      setOperation(null);
    }
  }

  async function connect(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (connecting.current || !isTrustedAccountWindow()) return;
    const token = tokenInput.current?.value.trim() ?? "";
    if (!token) return;
    // Keep credentials out of React state, TanStack mutations and saved form data.
    if (tokenInput.current) tokenInput.current.value = "";
    await connectAccount("pat", () => collaboration.connectGithub(token));
  }

  if (!trusted) return null;

  return (
    <form
      className="min-w-0 space-y-3 rounded-xl border p-4"
      onSubmit={connect}
    >
      <div className="flex items-center gap-2 text-sm font-medium">
        <Github className="size-4" aria-hidden="true" />
        Connect GitHub
      </div>
      <p className="text-xs leading-relaxed text-muted-foreground">
        Use an account already signed in with GitHub CLI, or enter a personal
        access token. CLI accounts keep their existing permissions.
      </p>
      <div className="min-w-0 space-y-2 rounded-lg bg-muted/30 p-3">
        <div className="flex flex-wrap items-center justify-between gap-2">
          <span className="flex items-center gap-1.5 text-xs font-medium">
            <Terminal className="size-3.5" aria-hidden="true" />
            GitHub CLI accounts
          </span>
          <Button
            type="button"
            variant="ghost"
            size="xs"
            disabled={busy || cli.isFetching}
            onClick={() => {
              setError(null);
              void cli.refetch();
            }}
          >
            <RotateCcw aria-hidden="true" />
            Check again
          </Button>
        </div>
        {cli.isFetching ? (
          <p role="status" className="text-xs text-muted-foreground">
            Checking GitHub CLI accounts…
          </p>
        ) : null}
        {cli.isError ? (
          <p role="alert" className="text-xs text-muted-foreground">
            Could not check GitHub CLI accounts. Check again or enter a token
            below.
          </p>
        ) : null}
        {cli.data ? (
          <GithubCliStatus
            status={cli.data.status}
            hasAccounts={cli.data.accounts.length > 0}
          />
        ) : null}
        {(cli.data?.accounts.length ?? 0) > 0 ? (
          <p className="text-xs leading-relaxed text-muted-foreground">
            Gitru saves the existing credential in your system credential store
            and keeps the active CLI account unchanged.
          </p>
        ) : null}
        {(cli.data?.accounts.length ?? 0) > 10 ? (
          <Input
            type="search"
            aria-label="Filter GitHub CLI accounts"
            placeholder="Filter CLI accounts"
            value={accountFilter}
            onChange={(event) => setAccountFilter(event.target.value)}
          />
        ) : null}
        {visibleAccounts.map((account) => (
          <GithubCliAccountRow
            key={account.id}
            account={account}
            busy={busy || cli.isFetching}
            connecting={operation === account.id}
            onConnect={() => {
              void connectAccount(account.id, () =>
                collaboration.connectGithubCli(account.id),
              );
            }}
          />
        ))}
        {filteredAccounts.length > 10 ? (
          <p className="text-xs text-muted-foreground">
            Showing the first 10 of {filteredAccounts.length} accounts. Filter
            to find another account.
          </p>
        ) : null}
        {accountFilter && filteredAccounts.length === 0 ? (
          <p className="text-xs text-muted-foreground">No matching accounts.</p>
        ) : null}
      </div>
      <Field name="github-token">
        <FieldLabel htmlFor={tokenId}>Personal access token</FieldLabel>
        <Input
          id={tokenId}
          ref={tokenInput}
          name="github-token"
          type="password"
          required
          autoComplete="off"
          placeholder="GitHub personal access token"
          disabled={busy}
          aria-describedby={`${tokenId}-help`}
        />
        <FieldDescription id={`${tokenId}-help`}>
          Your token stays in the system credential store. Fine-grained tokens
          need selected repositories and read access to Pull requests and
          Issues. Inbox uses a classic token with notifications access. Classic
          tokens need repo access for private repositories, which also covers
          inbox.
        </FieldDescription>
      </Field>
      <TokenCreationLinks disabled={busy} />
      <div className="flex items-center justify-between gap-3">
        <span className="flex items-center gap-1.5 text-xs text-muted-foreground">
          <KeyRound className="size-3.5" aria-hidden="true" />
          github.com
        </span>
        <Button type="submit" size="sm" disabled={busy}>
          <Plus aria-hidden="true" />
          {operation === "pat" ? "Connecting…" : "Connect account"}
        </Button>
      </div>
      {error ? (
        <p role="alert" className="text-xs text-destructive-foreground">
          {error}
        </p>
      ) : null}
      {connected ? (
        <p role="status" className="text-xs text-success-foreground">
          Connected as {connected}. Choose repositories to start syncing.
        </p>
      ) : null}
    </form>
  );
}

function GithubCliStatus({
  status,
  hasAccounts,
}: {
  status: "available" | "not_installed" | "unsupported" | "unavailable";
  hasAccounts: boolean;
}) {
  const message =
    status === "not_installed"
      ? "GitHub CLI wasn’t found. Enter a personal access token below."
      : status === "unsupported"
        ? "Update GitHub CLI to check its accounts, or enter a token below."
        : status === "unavailable"
          ? "GitHub CLI accounts are unavailable. Check again or enter a token below."
          : !hasAccounts
            ? "No GitHub CLI accounts found for github.com. Enter a token below."
            : null;
  return message ? (
    <p className="text-xs leading-relaxed text-muted-foreground">{message}</p>
  ) : null;
}

function GithubCliAccountRow({
  account,
  busy,
  connecting,
  onConnect,
}: {
  account: GithubCliAccount;
  busy: boolean;
  connecting: boolean;
  onConnect: () => void;
}) {
  const ready = account.availability === "ready";
  return (
    <div className="flex min-w-0 flex-wrap items-center gap-2 rounded-md border bg-background p-2">
      <div className="min-w-0 basis-full sm:flex-1 sm:basis-0">
        <p className="truncate text-sm font-medium">@{account.login}</p>
        <p className="text-xs text-muted-foreground">
          {ready
            ? account.host
            : account.availability === "auth_required"
              ? "Sign in again with gh auth login"
              : "Account unavailable"}
        </p>
      </div>
      {account.active ? (
        <Badge variant="outline" size="sm">
          Active in gh
        </Badge>
      ) : null}
      <Button
        type="button"
        size="sm"
        variant="outline"
        disabled={busy || !ready}
        onClick={onConnect}
        aria-label={`Use GitHub CLI account ${account.login}`}
      >
        {connecting ? "Connecting…" : "Use account"}
      </Button>
    </div>
  );
}

function TokenCreationLinks({ disabled }: { disabled: boolean }) {
  const [failed, setFailed] = useState(false);
  function open(url: string) {
    setFailed(false);
    void openExternalUrlSafely(url).then(
      (opened) => setFailed(!opened),
      () => setFailed(true),
    );
  }
  return (
    <div className="space-y-1">
      <div className="flex flex-wrap items-center gap-x-3 gap-y-1">
        <Button
          type="button"
          size="xs"
          variant="link"
          className="px-0"
          disabled={disabled}
          onClick={() =>
            open(
              "https://github.com/settings/personal-access-tokens/new?name=Gitru&description=Read%20pull%20requests%20and%20issues&expires_in=30&pull_requests=read&issues=read",
            )
          }
        >
          <ExternalLink aria-hidden="true" />
          Create a fine-grained token
        </Button>
        <Button
          type="button"
          size="xs"
          variant="link"
          className="px-0"
          disabled={disabled}
          onClick={() => open("https://github.com/settings/tokens/new")}
        >
          <ExternalLink aria-hidden="true" />
          Create a classic token
        </Button>
      </div>
      {failed ? (
        <p role="alert" className="text-xs text-destructive-foreground">
          Could not open your browser. Try again.
        </p>
      ) : null}
    </div>
  );
}

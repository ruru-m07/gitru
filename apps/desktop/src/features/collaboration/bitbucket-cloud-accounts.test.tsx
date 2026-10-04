import { collaboration, collaborationKeys } from "@gitru/collaboration-client";
import type {
  AcquireDemandRequest,
  CapabilitySnapshot,
  ContextCapabilityRequest,
  RemoteAccount,
  RepositorySnapshot,
} from "@gitru/commands";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { mockWindows } from "@tauri-apps/api/mocks";
import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import {
  fixtureAccount,
  fixtureContextualCapabilities,
  fixtureRepositories,
} from "../../../tests/fixtures/collaboration";
import { mockForegroundDemand } from "../../../tests/mocks/collaboration-demand";
import {
  mockTauriCommand,
  mockTauriCommandResult,
} from "../../../tests/mocks/tauri";
import { AccountManager, ConnectBitbucketCloudForm } from "./account-manager";
import { CollaborationWorkspace } from "./workspace";

const bitbucket: RemoteAccount = {
  ...fixtureAccount,
  id: "fixture-bitbucket-account",
  provider: "bitbucket_cloud",
  host: "bitbucket.org",
  actor_id: "11111111-1111-4111-8111-111111111111",
  login: "bitbucket-user",
  display_name: "Bitbucket User",
  notifications_supported: false,
};
const capabilities: CapabilitySnapshot = {
  account_id: bitbucket.id,
  instance: {
    id: "bitbucket_cloud:https://bitbucket.org/",
    provider: "bitbucket_cloud",
    base_url: "https://bitbucket.org/",
  },
  inbox_semantics: "none",
  revision: "10",
  authorization_view: "1",
  facets: (
    [
      "repositories",
      "pull_requests",
      "issues",
      "inbox",
      "pull_details",
      "issue_details",
      "comments",
      "reviews",
      "checks",
      "merge",
    ] as const
  ).map((facet): CapabilitySnapshot["facets"][number] => ({
    facet,
    state: facet === "repositories" ? "supported" : "unsupported",
    reason:
      facet === "repositories"
        ? null
        : facet === "inbox" || facet === "issues"
          ? "provider_semantics"
          : "not_implemented",
  })),
};
const repository = {
  ...fixtureRepositories.repositories[0],
  id: "bitbucket:repo:33333333-3333-4333-8333-333333333333",
  account_id: bitbucket.id,
  provider_id: "33333333-3333-4333-8333-333333333333",
  full_name: "workspace/engine",
  web_url: "https://bitbucket.org/workspace/engine",
  selected: false,
};
const savedRepositories: RepositorySnapshot = {
  ...fixtureRepositories,
  repositories: [repository],
  sync: { ...fixtureRepositories.sync, state: "offline" },
};
const caches: QueryClient[] = [];
const stops: Array<() => void> = [];
afterEach(() => {
  for (const stop of stops.splice(0)) stop();
  for (const cache of caches.splice(0)) cache.clear();
});

function mount(component: React.ReactNode, bridge = false) {
  const cache = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  caches.push(cache);
  if (bridge) stops.push(collaboration.installBridge(cache));
  return {
    ...render(
      <QueryClientProvider client={cache}>{component}</QueryClientProvider>,
    ),
    cache,
  };
}

function savedAccounts(accounts: RemoteAccount[]) {
  return { accounts, revision: "10", authorization_view: "1" };
}

function managerMocks() {
  mockTauriCommandResult("collaboration_accounts", savedAccounts([bitbucket]));
  mockTauriCommandResult("collaboration_capabilities", capabilities);
  mockTauriCommandResult("collaboration_discover_github_cli", {
    status: "not_installed",
    accounts: [],
  });
}

function repositoryOnlyReads() {
  const demand = mockForegroundDemand();
  vi.spyOn(collaboration.transport, "listen").mockResolvedValue(() => {});
  vi.spyOn(collaboration.transport, "listenLocalChanges").mockResolvedValue(
    () => {},
  );
  mockTauriCommandResult("collaboration_changes_since", {
    revision: "10",
    authorization_view: "1",
    changes: [],
    has_more: false,
    reset_required: false,
  });
  mockTauriCommandResult("collaboration_accounts", savedAccounts([bitbucket]));
  mockTauriCommand("collaboration_contextual_capabilities", (payload) => {
    const { request } = payload as { request: ContextCapabilityRequest };
    expect(request.account_id).toBe(bitbucket.id);
    const snapshot = fixtureContextualCapabilities(
      bitbucket,
      request.target,
      "none",
    );
    return {
      ...snapshot,
      facets: snapshot.facets.map((facet) => {
        const declaration = capabilities.facets.find(
          (entry) => entry.facet === facet.facet,
        )!;
        const access = {
          state: declaration.state,
          reason: declaration.reason,
        };
        return {
          ...facet,
          saved_read: access,
          synchronize: access,
          observation:
            facet.facet === "repositories" ? ("partial" as const) : "unknown",
        };
      }),
    };
  });
  const read = mockTauriCommandResult(
    "collaboration_repositories",
    savedRepositories,
  );
  const select = mockTauriCommandResult(
    "collaboration_select_repository",
    "11",
  );
  const refresh = mockTauriCommandResult("collaboration_refresh", {
    job_id: "repository-discovery",
  });
  const items = mockTauriCommandResult("collaboration_items", { items: [] });
  const detail = mockTauriCommandResult("collaboration_detail", null);
  const hydrate = mockTauriCommandResult("collaboration_hydrate_detail", {
    job_id: "must-not-admit",
  });
  return { read, select, refresh, items, detail, hydrate, demand };
}

describe("manual Bitbucket Cloud account connection", () => {
  it("uses a token-only password form with the three repository scopes and no automatic credential lookup", () => {
    const discover = vi.spyOn(collaboration.transport, "discoverGithubCli");
    const connect = mockTauriCommandResult(
      "collaboration_connect_bitbucket_cloud",
      bitbucket,
    );
    const { container } = mount(<ConnectBitbucketCloudForm />);
    const input = screen.getByLabelText("Bitbucket Cloud API token");
    expect(container.querySelectorAll("input")).toHaveLength(1);
    expect(input).toHaveAttribute("type", "password");
    expect(input).toHaveAttribute("autocomplete", "off");
    expect(input).toBeRequired();
    expect(screen.getByText(/read:user:bitbucket/)).toHaveTextContent(
      /read:user:bitbucket, read:workspace:bitbucket, and read:repository:bitbucket/,
    );
    expect(screen.getByText(/Connect with an API token/)).toHaveTextContent(
      /Pull requests aren’t supported yet. Issues and an inbox aren’t available for this provider./,
    );
    expect(discover).not.toHaveBeenCalled();
    expect(connect).not.toHaveBeenCalled();
  });

  it("clears the secret before native validation and submits once without query or mutation retention", async () => {
    let finish!: (account: RemoteAccount) => void;
    const connect = mockTauriCommand(
      "collaboration_connect_bitbucket_cloud",
      () =>
        new Promise<RemoteAccount>((resolve) => {
          finish = resolve;
        }),
    );
    const user = userEvent.setup();
    const { cache } = mount(<ConnectBitbucketCloudForm />);
    const input = screen.getByLabelText("Bitbucket Cloud API token");
    await user.type(input, "  synthetic-bitbucket-secret  ");
    await user.click(
      screen.getByRole("button", { name: "Connect Bitbucket account" }),
    );
    fireEvent.submit(input.closest("form")!);
    expect(connect).toHaveBeenCalledExactlyOnceWith({
      token: "synthetic-bitbucket-secret",
    });
    expect(input).toHaveValue("");
    expect(input).toBeDisabled();
    expect(cache.getMutationCache().getAll()).toEqual([]);
    expect(cache.getQueryCache().getAll()).toEqual([]);
    await act(async () => finish(bitbucket));
    expect(
      await screen.findByText(/Connected to Bitbucket as bitbucket-user/),
    ).toHaveTextContent("Choose repositories to sync.");
  });

  it("sanitizes rejection, keeps saved accounts and permits an explicit fresh-token retry", async () => {
    const connect = mockTauriCommand(
      "collaboration_connect_bitbucket_cloud",
      () =>
        Promise.reject({
          code: "permission_denied",
          message: "synthetic-secret-provider-echo",
        }),
    );
    const user = userEvent.setup();
    const { cache } = mount(<ConnectBitbucketCloudForm />);
    const accountsKey = collaborationKeys.accounts(collaboration.getVersion());
    const saved = savedAccounts([fixtureAccount]);
    cache.setQueryData(accountsKey, saved);
    const input = screen.getByLabelText("Bitbucket Cloud API token");
    await user.type(input, "synthetic-secret-provider-echo");
    await user.click(
      screen.getByRole("button", { name: "Connect Bitbucket account" }),
    );
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "This account does not have access",
    );
    expect(screen.getByRole("alert")).not.toHaveTextContent("synthetic-secret");
    expect(input).toHaveValue("");
    expect(input).toBeEnabled();
    expect(cache.getQueryData(accountsKey)).toEqual(saved);
    expect(connect).toHaveBeenCalledOnce();
    await user.type(input, "synthetic-fresh-token");
    await user.click(
      screen.getByRole("button", { name: "Connect Bitbucket account" }),
    );
    expect(connect).toHaveBeenNthCalledWith(2, {
      token: "synthetic-fresh-token",
    });
  });

  it("does not carry a late native completion into a reopened form", async () => {
    let finish!: (account: RemoteAccount) => void;
    mockTauriCommand(
      "collaboration_connect_bitbucket_cloud",
      () =>
        new Promise<RemoteAccount>((resolve) => {
          finish = resolve;
        }),
    );
    const user = userEvent.setup();
    const view = mount(<ConnectBitbucketCloudForm />);
    await user.type(
      screen.getByLabelText("Bitbucket Cloud API token"),
      "synthetic-token",
    );
    await user.click(
      screen.getByRole("button", { name: "Connect Bitbucket account" }),
    );
    view.unmount();
    mount(<ConnectBitbucketCloudForm />);
    await act(async () => finish(bitbucket));
    expect(
      screen.queryByText(/Connected to Bitbucket as/),
    ).not.toBeInTheDocument();
    expect(screen.getByLabelText("Bitbucket Cloud API token")).toHaveValue("");
  });

  it("keeps rate-limited verification manual and never renders provider secret text", async () => {
    const connect = mockTauriCommand(
      "collaboration_connect_bitbucket_cloud",
      () =>
        Promise.reject({
          code: "rate_limited",
          message: "synthetic-token-echo",
        }),
    );
    const user = userEvent.setup();
    mount(<ConnectBitbucketCloudForm />);
    const input = screen.getByLabelText("Bitbucket Cloud API token");
    await user.type(input, "synthetic-token");
    await user.click(
      screen.getByRole("button", { name: "Connect Bitbucket account" }),
    );
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Bitbucket asked Gitru to wait. Try connecting again later.",
    );
    expect(screen.getByRole("alert")).not.toHaveTextContent(
      /automatically|synthetic-token/,
    );
    expect(input).toHaveValue("");
    expect(input).toBeEnabled();
    expect(connect).toHaveBeenCalledOnce();
  });

  it("exposes account host handoff without token or disconnect controls in a child view", async () => {
    mockWindows("tab-webview:synthetic");
    managerMocks();
    const connect = mockTauriCommandResult(
      "collaboration_connect_bitbucket_cloud",
      bitbucket,
    );
    mount(
      <>
        <AccountManager />
        <ConnectBitbucketCloudForm />
      </>,
    );
    expect(
      await screen.findByText("Bitbucket Cloud · bitbucket.org"),
    ).toBeVisible();
    expect(screen.getByRole("button", { name: "Accounts" })).toBeVisible();
    expect(
      screen.queryByLabelText("Bitbucket Cloud API token"),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Disconnect bitbucket-user" }),
    ).not.toBeInTheDocument();
    expect(connect).not.toHaveBeenCalled();
  });

  it("rechecks caller trust on submit and does not dispatch from a changed child label", async () => {
    const connect = mockTauriCommandResult(
      "collaboration_connect_bitbucket_cloud",
      bitbucket,
    );
    const user = userEvent.setup();
    mount(<ConnectBitbucketCloudForm />);
    const input = screen.getByLabelText("Bitbucket Cloud API token");
    await user.type(input, "synthetic-token");
    mockWindows("tab-webview:synthetic");
    fireEvent.submit(input.closest("form")!);
    expect(connect).not.toHaveBeenCalled();
  });

  it("opens only the fixed official token guide and never submits the entered secret", async () => {
    const open = mockTauriCommandResult("open_external_url", undefined);
    const connect = mockTauriCommandResult(
      "collaboration_connect_bitbucket_cloud",
      bitbucket,
    );
    const user = userEvent.setup();
    mount(<ConnectBitbucketCloudForm />);
    await user.type(
      screen.getByLabelText("Bitbucket Cloud API token"),
      "synthetic-private-token",
    );
    await user.click(
      screen.getByRole("button", { name: "Create a Bitbucket API token" }),
    );
    expect(open).toHaveBeenCalledExactlyOnceWith({
      url: "https://support.atlassian.com/bitbucket-cloud/docs/create-an-api-token/",
    });
    expect(connect).not.toHaveBeenCalled();
  });

  it("reports a failed browser open without echoing its error or connecting", async () => {
    const open = mockTauriCommand("open_external_url", () =>
      Promise.reject(new Error("synthetic-opener-details")),
    );
    const connect = mockTauriCommandResult(
      "collaboration_connect_bitbucket_cloud",
      bitbucket,
    );
    const user = userEvent.setup();
    mount(<ConnectBitbucketCloudForm />);
    await user.click(
      screen.getByRole("button", { name: "Create a Bitbucket API token" }),
    );
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Could not open your browser. Try again.",
    );
    expect(screen.getByRole("alert")).not.toHaveTextContent(
      "synthetic-opener-details",
    );
    expect(open).toHaveBeenCalledOnce();
    expect(connect).not.toHaveBeenCalled();
  });

  it("refreshes ordinary saved accounts after native acceptance without removing the other provider", async () => {
    let connected = false;
    const accounts = mockTauriCommand("collaboration_accounts", () =>
      savedAccounts(connected ? [fixtureAccount, bitbucket] : [fixtureAccount]),
    );
    mockTauriCommandResult("collaboration_capabilities", capabilities);
    mockTauriCommandResult("collaboration_discover_github_cli", {
      status: "not_installed",
      accounts: [],
    });
    mockTauriCommandResult("collaboration_changes_since", {
      revision: "10",
      authorization_view: "1",
      changes: [],
      has_more: false,
      reset_required: false,
    });
    vi.spyOn(collaboration.transport, "listen").mockResolvedValue(() => {});
    const connect = mockTauriCommand(
      "collaboration_connect_bitbucket_cloud",
      () => {
        connected = true;
        return bitbucket;
      },
    );
    const user = userEvent.setup();
    mount(<AccountManager />, true);
    await act(async () => collaboration.wake());
    expect(
      await screen.findByText("GitHub · https://github.com"),
    ).toBeVisible();
    await user.type(
      screen.getByLabelText("Bitbucket Cloud API token"),
      "synthetic-manual-token",
    );
    await user.click(
      screen.getByRole("button", { name: "Connect Bitbucket account" }),
    );
    expect(
      await screen.findByText("Bitbucket Cloud · bitbucket.org"),
    ).toBeVisible();
    expect(screen.getByText("GitHub · https://github.com")).toBeVisible();
    expect(
      screen.getByText("Inbox isn’t supported by this connection in Gitru."),
    ).toBeVisible();
    expect(
      screen.queryByText(/Reconnect with a credential/),
    ).not.toBeInTheDocument();
    expect(connect).toHaveBeenCalledExactlyOnceWith({
      token: "synthetic-manual-token",
    });
    expect(accounts.mock.calls.length).toBeGreaterThan(1);
  });
});

describe("Bitbucket repository-only workspace", () => {
  it.each([
    "pull_request",
    "issue",
    "notification",
  ] as const)("reads saved offline repositories for %s without admitting unsupported feed reads or demand", async (kind) => {
    const boundary = repositoryOnlyReads();
    const user = userEvent.setup();
    mount(<CollaborationWorkspace kind={kind} />, true);
    await act(async () => collaboration.wake());
    expect(await screen.findByText("Feature not supported")).toBeVisible();
    expect(screen.getByRole("button", { name: "Refresh" })).toBeDisabled();
    if (kind === "notification")
      await user.click(screen.getByRole("button", { name: "Repositories" }));
    expect(await screen.findByText(repository.full_name)).toBeVisible();
    expect(screen.getByText("Offline · saved data")).toBeVisible();
    expect(boundary.read).toHaveBeenCalledWith({ accountId: bitbucket.id });
    expect(boundary.items).not.toHaveBeenCalled();
    expect(boundary.detail).not.toHaveBeenCalled();
    expect(boundary.hydrate).not.toHaveBeenCalled();
    expect(boundary.refresh).not.toHaveBeenCalled();
    await waitFor(() => expect(boundary.demand.acquire).toHaveBeenCalled());
    for (const [payload] of boundary.demand.acquire.mock.calls) {
      expect(
        (payload as { request: AcquireDemandRequest }).request,
      ).toMatchObject({
        account_id: bitbucket.id,
        target: { kind: "repositories" },
      });
    }
  });

  it("discovers and selects a UUID repository through the existing account-scoped SDK commands", async () => {
    const boundary = repositoryOnlyReads();
    const user = userEvent.setup();
    mount(<CollaborationWorkspace kind="pull_request" />, true);
    await act(async () => collaboration.wake());
    expect(await screen.findByText(repository.full_name)).toBeVisible();
    await user.click(
      screen.getByRole("button", { name: "Discover repositories" }),
    );
    expect(boundary.refresh).toHaveBeenCalledExactlyOnceWith({
      request: { account_id: bitbucket.id, repository_id: null, kind: null },
    });
    await user.click(
      screen.getByRole("checkbox", { name: repository.full_name }),
    );
    expect(boundary.select).toHaveBeenCalledExactlyOnceWith({
      accountId: bitbucket.id,
      repositoryId: repository.id,
      selected: true,
    });
    await waitFor(() =>
      expect(boundary.read.mock.calls.length).toBeGreaterThan(1),
    );
    expect(boundary.items).not.toHaveBeenCalled();
    expect(boundary.detail).not.toHaveBeenCalled();
    expect(boundary.hydrate).not.toHaveBeenCalled();
  });
});

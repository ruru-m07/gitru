import {
  collaboration,
  collaborationKeys,
  type RemoteItemKind,
} from "@gitru/collaboration-client";
import type {
  AcquireDemandRequest,
  CapabilitySnapshot,
  ContextCapabilityRequest,
  DetailQuery,
  RemoteAccount,
} from "@gitru/commands";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { mockWindows } from "@tauri-apps/api/mocks";
import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import {
  fixtureAccount,
  fixtureContextualCapabilities,
  fixtureGitlabAccount,
  fixtureGitlabCapabilities,
  fixtureItem,
  fixturePage,
  fixtureRepositories,
} from "../../../tests/fixtures/collaboration";
import {
  fixtureBody,
  fixtureMetadata,
} from "../../../tests/fixtures/resource-detail";
import { mockForegroundDemand } from "../../../tests/mocks/collaboration-demand";
import {
  mockTauriCommand,
  mockTauriCommandResult,
} from "../../../tests/mocks/tauri";
import { AccountManager, ConnectGitlabForm } from "./account-manager";
import { CollaborationWorkspace } from "./workspace";

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

function managerMocks(
  account: RemoteAccount = fixtureGitlabAccount,
  capability: CapabilitySnapshot = fixtureGitlabCapabilities,
) {
  mockTauriCommandResult("collaboration_accounts", savedAccounts([account]));
  mockTauriCommandResult("collaboration_capabilities", capability);
  mockTauriCommandResult("collaboration_discover_github_cli", {
    status: "not_installed",
    accounts: [],
  });
}

function commonGitlabReads(kind: Exclude<RemoteItemKind, "notification">) {
  const pull = kind === "pull_request";
  const providerId = pull ? "9007199254741993" : "9007199254742993";
  const repository = {
    ...fixtureRepositories.repositories[0],
    id: "gitlab:repo:9007199254740993",
    account_id: fixtureGitlabAccount.id,
    provider_id: "9007199254740993",
    full_name: "group/subgroup/日本語-project",
    web_url: "https://gitlab.com/group/subgroup/project",
  };
  const summary = {
    ...fixtureItem,
    id: `gitlab:${pull ? "pull" : "issue"}:${providerId}`,
    account_id: fixtureGitlabAccount.id,
    repository_id: repository.id,
    provider_id: providerId,
    kind,
    number: "67",
    title: `GitLab saved ${pull ? "MR" : "issue"} summary`,
    body: "Older GitLab list description",
    web_url: `${repository.web_url}/-/${pull ? "merge_requests" : "issues"}/67`,
    head_oid: pull ? "a".repeat(40) : null,
    is_draft: pull ? true : null,
  };
  const metadata = fixtureMetadata(kind);
  metadata.values = {
    ...metadata.values,
    title: `GitLab authoritative ${pull ? "MR" : "issue"} header`,
    state: "open",
    state_reason: null,
    web_url: summary.web_url,
    author: {
      provider_id: "9007199254743993",
      login: "gitlab-detail-author",
      web_url: null,
    },
    labels: [{ provider_id: null, name: "workflow::ready", color: null }],
    assignees: [
      {
        provider_id: "9007199254744993",
        login: "gitlab-assignee",
        web_url: null,
      },
    ],
    milestone: { ...metadata.values.milestone!, title: "GitLab milestone" },
    is_draft: pull ? true : null,
    head: pull
      ? { name: "feature/local", oid: "a".repeat(40), repository: null }
      : null,
    base: null,
    merged_at: null,
  };
  const body = fixtureBody({
    subject_id: summary.id,
    metadata,
    body: {
      state: "known",
      text: `Independent cached GitLab ${kind} description Δ`,
    },
  });
  body.evidence.authorization_epoch = fixtureGitlabAccount.authorization_epoch;
  body.evidence.sync = { ...fixturePage.sync, state: "offline" };
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
  mockTauriCommandResult(
    "collaboration_accounts",
    savedAccounts([fixtureGitlabAccount]),
  );
  mockTauriCommandResult("collaboration_repositories", {
    ...fixtureRepositories,
    repositories: [repository],
  });
  const items = mockTauriCommandResult("collaboration_items", {
    ...fixturePage,
    items: [summary],
  });
  mockTauriCommandResult("collaboration_item", {
    pending_intent: null,

    item: summary,
    revision: "10",
    authorization_view: "1",
  });
  const detail = mockTauriCommand("collaboration_detail", (payload) => {
    const { query } = payload as { query: DetailQuery };
    expect(query).toMatchObject({
      account_id: fixtureGitlabAccount.id,
      subject_id: summary.id,
      facet: "body",
    });
    return body;
  });
  mockTauriCommandResult("collaboration_draft", {
    account_id: fixtureGitlabAccount.id,
    subject_id: summary.id,
    body: "Private GitLab authored text",
    generation: "3",
  });
  mockTauriCommand("collaboration_contextual_capabilities", (payload) => {
    const { request } = payload as { request: ContextCapabilityRequest };
    expect(request.account_id).toBe(fixtureGitlabAccount.id);
    const snapshot = fixtureContextualCapabilities(
      fixtureGitlabAccount,
      request.target,
      "none",
    );
    return {
      ...snapshot,
      facets: snapshot.facets.map((facet) => {
        const implemented = [
          "repositories",
          "pull_requests",
          "issues",
          "pull_details",
          "issue_details",
        ].includes(facet.facet);
        const access = implemented
          ? { state: "supported" as const, reason: null }
          : facet.saved_read;
        return {
          ...facet,
          saved_read: access,
          synchronize: access,
          observation: implemented ? ("complete" as const) : facet.observation,
        };
      }),
    };
  });
  const hydrate = mockTauriCommandResult("collaboration_hydrate_detail", {
    job_id: "must-not-admit",
  });
  const refresh = mockTauriCommandResult("collaboration_refresh", {
    job_id: "must-not-refresh",
  });
  return { summary, metadata, body, detail, items, demand, hydrate, refresh };
}

describe("manual GitLab account connection", () => {
  it("clears the password before native validation and submits once without query/mutation credential retention", async () => {
    let finish!: (account: RemoteAccount) => void;
    const connect = mockTauriCommand(
      "collaboration_connect_gitlab",
      () =>
        new Promise<RemoteAccount>((resolve) => {
          finish = resolve;
        }),
    );
    const user = userEvent.setup();
    const { cache } = mount(<ConnectGitlabForm />);
    const input = screen.getByLabelText("GitLab personal access token");
    expect(input).toHaveAttribute("type", "password");
    expect(input).toHaveAttribute("autocomplete", "off");
    await user.type(input, "  synthetic-gitlab-secret  ");
    await user.click(
      screen.getByRole("button", { name: "Connect GitLab account" }),
    );
    fireEvent.submit(input.closest("form")!);
    expect(connect).toHaveBeenCalledExactlyOnceWith({
      token: "synthetic-gitlab-secret",
    });
    expect(input).toHaveValue("");
    expect(input).toBeDisabled();
    expect(cache.getMutationCache().getAll()).toEqual([]);
    expect(cache.getQueryCache().getAll()).toEqual([]);
    await act(async () => finish(fixtureGitlabAccount));
    expect(
      await screen.findByText(/Connected to GitLab as gitlab-user/),
    ).toBeVisible();
  });

  it("sanitizes a rejected credential and allows an explicit new attempt without disturbing saved accounts", async () => {
    const connect = mockTauriCommand("collaboration_connect_gitlab", () =>
      Promise.reject({
        code: "permission_denied",
        message: "synthetic-secret-provider-echo",
      }),
    );
    const user = userEvent.setup();
    const { cache } = mount(<ConnectGitlabForm />);
    cache.setQueryData(
      collaborationKeys.accounts(collaboration.getVersion()),
      savedAccounts([fixtureAccount]),
    );
    const input = screen.getByLabelText("GitLab personal access token");
    await user.type(input, "synthetic-secret-provider-echo");
    await user.click(
      screen.getByRole("button", { name: "Connect GitLab account" }),
    );
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "This account does not have access",
    );
    expect(screen.getByRole("alert")).not.toHaveTextContent("synthetic-secret");
    expect(input).toHaveValue("");
    expect(input).toBeEnabled();
    expect(connect).toHaveBeenCalledOnce();
    expect(
      cache.getQueryData(
        collaborationKeys.accounts(collaboration.getVersion()),
      ),
    ).toEqual(savedAccounts([fixtureAccount]));
    await user.type(input, "synthetic-second-token");
    await user.click(
      screen.getByRole("button", { name: "Connect GitLab account" }),
    );
    expect(connect).toHaveBeenCalledTimes(2);
  });

  it("does not carry a late connection message into a reopened form", async () => {
    let finish!: (account: RemoteAccount) => void;
    mockTauriCommand(
      "collaboration_connect_gitlab",
      () =>
        new Promise<RemoteAccount>((resolve) => {
          finish = resolve;
        }),
    );
    const user = userEvent.setup();
    const view = mount(<ConnectGitlabForm />);
    await user.type(
      screen.getByLabelText("GitLab personal access token"),
      "synthetic-token",
    );
    await user.click(
      screen.getByRole("button", { name: "Connect GitLab account" }),
    );
    view.unmount();
    mount(<ConnectGitlabForm />);
    await act(async () => finish(fixtureGitlabAccount));
    expect(
      screen.queryByText(/Connected to GitLab as/),
    ).not.toBeInTheDocument();
    expect(screen.getByLabelText("GitLab personal access token")).toHaveValue(
      "",
    );
  });

  it("keeps rate-limited credential validation manual instead of promising an automatic connection retry", async () => {
    const connect = mockTauriCommand("collaboration_connect_gitlab", () =>
      Promise.reject({ code: "rate_limited", message: "synthetic-token-echo" }),
    );
    const user = userEvent.setup();
    mount(<ConnectGitlabForm />);
    const input = screen.getByLabelText("GitLab personal access token");
    await user.type(input, "synthetic-token");
    await user.click(
      screen.getByRole("button", { name: "Connect GitLab account" }),
    );
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "GitLab asked Gitru to wait. Try connecting again later.",
    );
    expect(screen.getByRole("alert")).not.toHaveTextContent(
      /automatically|synthetic-token/,
    );
    expect(input).toBeEnabled();
    expect(input).toHaveValue("");
    expect(connect).toHaveBeenCalledOnce();
    await user.type(input, "synthetic-manual-retry");
    await user.click(
      screen.getByRole("button", { name: "Connect GitLab account" }),
    );
    expect(connect).toHaveBeenCalledTimes(2);
  });

  it("offers only the account host handoff in a child view", async () => {
    mockWindows("tab-webview:synthetic");
    managerMocks();
    const connect = mockTauriCommandResult(
      "collaboration_connect_gitlab",
      fixtureGitlabAccount,
    );
    mount(
      <>
        <AccountManager />
        <ConnectGitlabForm />
      </>,
    );
    expect(await screen.findByText("GitLab · gitlab.com")).toBeVisible();
    expect(screen.getByRole("button", { name: "Accounts" })).toBeVisible();
    expect(
      screen.queryByLabelText("GitLab personal access token"),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByLabelText("Personal access token"),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Disconnect gitlab-user" }),
    ).not.toBeInTheDocument();
    expect(connect).not.toHaveBeenCalled();
  });

  it("rechecks the native view label on submit", async () => {
    const connect = mockTauriCommandResult(
      "collaboration_connect_gitlab",
      fixtureGitlabAccount,
    );
    const user = userEvent.setup();
    mount(<ConnectGitlabForm />);
    const input = screen.getByLabelText("GitLab personal access token");
    await user.type(input, "synthetic-token");
    mockWindows("tab-webview:synthetic");
    fireEvent.submit(input.closest("form")!);
    expect(connect).not.toHaveBeenCalled();
  });

  it("opens the fixed public GitLab token settings without passing any form credential", async () => {
    const open = mockTauriCommandResult("open_external_url", undefined);
    const user = userEvent.setup();
    mount(<ConnectGitlabForm />);
    await user.type(
      screen.getByLabelText("GitLab personal access token"),
      "synthetic-private-token",
    );
    await user.click(
      screen.getByRole("button", { name: "Create a GitLab token" }),
    );
    expect(open).toHaveBeenCalledExactlyOnceWith({
      url: "https://gitlab.com/-/user_settings/personal_access_tokens",
    });
  });
});

describe("provider capability presentation", () => {
  it.each([
    "pull_request",
    "issue",
  ] as const)("renders supported GitLab %s cached Body and common metadata through the ordinary workspace", async (kind) => {
    const fixture = commonGitlabReads(kind);
    const user = userEvent.setup();
    const { cache } = mount(<CollaborationWorkspace kind={kind} />, true);
    await act(async () => collaboration.wake());
    const row = await screen.findByRole("button", {
      name: new RegExp(fixture.summary.title),
    });
    expect(screen.getByRole("button", { name: "Refresh" })).toBeEnabled();
    expect(row).toHaveAttribute("aria-pressed", "false");
    await user.click(row);
    const selected = within(
      await screen.findByRole("article", { name: "Saved item detail" }),
    );
    expect(await selected.findByText(fixture.body.body.text!)).toBeVisible();
    expect(
      await selected.findByRole("heading", {
        name: fixture.metadata.values.title!,
      }),
    ).toBeVisible();
    expect(selected.queryByText(fixture.summary.body!)).not.toBeInTheDocument();
    const header = within(
      selected.getByLabelText("Selected resource metadata"),
    );
    expect(header.getByText("workflow::ready")).toBeVisible();
    expect(header.getByText("@gitlab-assignee")).toBeVisible();
    expect(header.getByText("GitLab milestone")).toBeVisible();
    expect(header.getByText(/@gitlab-detail-author/)).toBeVisible();
    expect(header.getByText("#67")).toBeVisible();
    if (kind === "pull_request") {
      expect(header.getByText("Draft")).toBeVisible();
      expect(header.getByText("feature/local")).toBeVisible();
      expect(header.getByText("a".repeat(40))).toBeVisible();
      expect(header.getByText("No branch reference")).toBeVisible();
      expect(
        selected.getByRole("button", {
          name: "Merge pull request unavailable",
        }),
      ).toBeDisabled();
    } else {
      expect(header.queryByText("Draft")).not.toBeInTheDocument();
      expect(header.queryByText("Head branch / SHA")).not.toBeInTheDocument();
    }
    expect(await selected.findByLabelText("Private draft")).toHaveValue(
      "Private GitLab authored text",
    );
    expect(fixture.items).toHaveBeenCalledWith({
      query: expect.objectContaining({
        account_id: fixtureGitlabAccount.id,
        kind,
      }),
    });
    expect(fixture.detail).toHaveBeenCalledTimes(1);
    expect(
      cache.getQueryData(
        collaborationKeys.detail(fixtureGitlabAccount, {
          account_id: fixtureGitlabAccount.id,
          subject_id: fixture.summary.id,
          facet: "body",
          cursor: null,
          limit: 50,
        }),
      ),
    ).toEqual(fixture.body);
    await waitFor(() => {
      const bodyDemands = fixture.demand.acquire.mock.calls.filter(
        ([payload]) =>
          (payload as { request: AcquireDemandRequest }).request.target.kind ===
          "detail",
      );
      expect(bodyDemands).toHaveLength(1);
      expect(bodyDemands[0][0]).toMatchObject({
        request: {
          account_id: fixtureGitlabAccount.id,
          target: { subject_id: fixture.summary.id, facet: "body" },
        },
      });
    });
    expect(fixture.hydrate).not.toHaveBeenCalled();
    expect(fixture.refresh).not.toHaveBeenCalled();
  });

  it("identifies GitLab and its unsupported inbox without a false reconnect promise", async () => {
    managerMocks();
    mount(<AccountManager />);
    expect(await screen.findByText("GitLab · gitlab.com")).toBeVisible();
    expect(
      await screen.findByText(
        "Inbox isn’t supported by this connection in Gitru.",
      ),
    ).toBeVisible();
    expect(
      screen.queryByText(/Reconnect with a credential/),
    ).not.toBeInTheDocument();
    expect(screen.getByLabelText("Personal access token")).toBeVisible();
    expect(screen.getByLabelText("GitLab personal access token")).toBeVisible();
    expect(
      screen.queryByText(/GitLab and Bitbucket connections are planned/),
    ).not.toBeInTheDocument();
  });

  it("suggests a credential update only for an implemented inbox with missing scope", async () => {
    const account = { ...fixtureAccount, notifications_supported: false };
    managerMocks(account, {
      ...fixtureGitlabCapabilities,
      account_id: account.id,
      instance: {
        id: "github:https://github.com/",
        provider: "github",
        base_url: "https://github.com/",
      },
      inbox_semantics: "native_notifications",
      facets: [
        { facet: "inbox", state: "unavailable", reason: "missing_scope" },
      ],
    });
    mount(<AccountManager />);
    expect(
      await screen.findByText(
        /Reconnect with a credential that supports inbox/,
      ),
    ).toBeVisible();
    expect(screen.queryByText(/Inbox isn’t supported/)).not.toBeInTheDocument();
  });

  it("keeps unknown and delayed prior-account capability results from suggesting additional permissions", async () => {
    let finish!: (snapshot: CapabilitySnapshot) => void;
    managerMocks();
    const capabilities = mockTauriCommand(
      "collaboration_capabilities",
      () =>
        new Promise<CapabilitySnapshot>((resolve) => {
          finish = resolve;
        }),
    );
    const view = mount(<AccountManager />);
    await waitFor(() => expect(capabilities).toHaveBeenCalledOnce());
    expect(
      screen.queryByText(/Reconnect with a credential/),
    ).not.toBeInTheDocument();
    await act(async () =>
      finish({
        ...fixtureGitlabCapabilities,
        account_id: "another-actor-account",
        inbox_semantics: "native_notifications",
        facets: [
          { facet: "inbox", state: "unavailable", reason: "missing_scope" },
        ],
      }),
    );
    expect(
      screen.queryByText(/Reconnect with a credential/),
    ).not.toBeInTheDocument();
    view.unmount();
  });

  it("does not apply a delayed GitLab capability result to a GitHub actor with the same login", async () => {
    const gitlab = { ...fixtureGitlabAccount, login: fixtureAccount.login };
    const github = { ...fixtureAccount, notifications_supported: false };
    let finishGitlab!: (snapshot: CapabilitySnapshot) => void;
    managerMocks(gitlab);
    const capabilities = mockTauriCommand(
      "collaboration_capabilities",
      (payload) => {
        if ((payload as { accountId: string }).accountId === gitlab.id)
          return new Promise<CapabilitySnapshot>((resolve) => {
            finishGitlab = resolve;
          });
        return {
          ...fixtureGitlabCapabilities,
          account_id: github.id,
          instance: {
            id: "github:https://github.com/",
            provider: "github" as const,
            base_url: "https://github.com/",
          },
          inbox_semantics: "native_notifications" as const,
          facets: [
            {
              facet: "inbox" as const,
              state: "unavailable" as const,
              reason: "missing_scope" as const,
            },
          ],
        };
      },
    );
    const { cache } = mount(<AccountManager />);
    await waitFor(() =>
      expect(capabilities).toHaveBeenCalledWith({ accountId: gitlab.id }),
    );
    await act(async () => {
      cache.setQueryData(
        collaborationKeys.accounts(collaboration.getVersion()),
        savedAccounts([github]),
      );
    });
    expect(
      await screen.findByText(
        /Reconnect with a credential that supports inbox/,
      ),
    ).toBeVisible();
    expect(screen.queryByText("GitLab · gitlab.com")).not.toBeInTheDocument();
    await act(async () => finishGitlab(fixtureGitlabCapabilities));
    expect(
      screen.getByText(/Reconnect with a credential that supports inbox/),
    ).toBeVisible();
    expect(
      screen.queryByText("Inbox isn’t supported by this connection in Gitru."),
    ).not.toBeInTheDocument();
  });

  it("honors a declared repository-only GitLab capability fixture without admitting unsupported issue reads or feed intents", async () => {
    const demand = mockForegroundDemand();
    mockTauriCommandResult(
      "collaboration_accounts",
      savedAccounts([fixtureGitlabAccount]),
    );
    // This historical declaration deliberately excludes reads, independently of
    // the current adapter's supported MR/issue capabilities exercised above.
    const declaredRepositoryOnly = fixtureGitlabCapabilities;
    mockTauriCommand("collaboration_contextual_capabilities", (payload) => {
      const { request } = payload as { request: ContextCapabilityRequest };
      const snapshot = fixtureContextualCapabilities(
        fixtureGitlabAccount,
        request.target,
        "none",
      );
      return {
        ...snapshot,
        facets: snapshot.facets.map((facet) => {
          const declaration = declaredRepositoryOnly.facets.find(
            (entry) => entry.facet === facet.facet,
          )!;
          const access = {
            state: declaration.state,
            reason: declaration.reason,
          };
          return { ...facet, saved_read: access, synchronize: access };
        }),
      };
    });
    const repository = {
      ...fixtureRepositories.repositories[0],
      account_id: fixtureGitlabAccount.id,
      id: "nested-gitlab-project",
      full_name: "group/subgroup/日本語-project",
      web_url: "https://gitlab.com/group/subgroup/project",
      selected: false,
    };
    mockTauriCommandResult("collaboration_repositories", {
      ...fixtureRepositories,
      repositories: [repository],
    });
    const select = mockTauriCommandResult(
      "collaboration_select_repository",
      "11",
    );
    const items = mockTauriCommandResult("collaboration_items", { items: [] });
    const refresh = mockTauriCommandResult("collaboration_refresh", {
      job_id: "unexpected",
    });
    const hydrate = mockTauriCommandResult("collaboration_hydrate_detail", {
      job_id: "unexpected",
    });
    const user = userEvent.setup();
    mount(<CollaborationWorkspace kind="issue" />);
    expect(await screen.findByText("Feature not supported")).toBeVisible();
    expect(screen.getByRole("button", { name: "Refresh" })).toBeDisabled();
    const checkbox = await screen.findByRole("checkbox");
    expect(checkbox).toBeEnabled();
    expect(select).not.toHaveBeenCalled();
    await user.click(checkbox);
    expect(select).toHaveBeenCalledExactlyOnceWith({
      accountId: fixtureGitlabAccount.id,
      repositoryId: repository.id,
      selected: true,
    });
    expect(items).not.toHaveBeenCalled();
    expect(refresh).not.toHaveBeenCalled();
    expect(hydrate).not.toHaveBeenCalled();
    for (const call of demand.acquire.mock.calls) {
      const payload = call[0] as { request: { target: { kind: string } } };
      expect(payload.request.target.kind).toBe("repositories");
    }
  });
});

import { collaboration, collaborationKeys } from "@gitru/collaboration-client";
import type {
  ContextCapabilityRequest,
  GithubCliDiscovery,
  ItemPage,
  LocalDraft,
} from "@gitru/commands";
import {
  onlineManager,
  QueryClient,
  QueryClientProvider,
} from "@tanstack/react-query";
import { mockWindows } from "@tauri-apps/api/mocks";
import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  fixtureAccount,
  fixtureAccounts,
  fixtureContextualCapabilities,
  fixtureGithubCli,
  fixtureGitlabAccount,
  fixtureItem,
  fixturePage,
  fixtureRepositories,
} from "../../../tests/fixtures/collaboration";
import { mockForegroundDemand } from "../../../tests/mocks/collaboration-demand";
import {
  mockTauriCommand,
  mockTauriCommandResult,
} from "../../../tests/mocks/tauri";
import { AccountManager, ConnectGithubForm } from "./account-manager";
import { ProviderLink } from "./provider-link";
import { CollaborationWorkspace } from "./workspace";

const caches: QueryClient[] = [];
beforeEach(() => {
  mockForegroundDemand();
  mockTauriCommandResult("collaboration_discover_github_cli", {
    status: "not_installed",
    accounts: [],
  });
});
afterEach(() => {
  for (const cache of caches.splice(0)) cache.clear();
  onlineManager.setOnline(true);
});

function mount(component: React.ReactNode) {
  const cache = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  caches.push(cache);
  const view = render(
    <QueryClientProvider client={cache}>{component}</QueryClientProvider>,
  );
  return { ...view, cache };
}

function readMocks(page: ItemPage = fixturePage) {
  mockTauriCommandResult("collaboration_accounts", fixtureAccounts);
  mockTauriCommandResult("collaboration_repositories", fixtureRepositories);
  const items = mockTauriCommandResult("collaboration_items", page);
  mockTauriCommandResult("collaboration_item", {
    pending_intent: null,

    item: fixtureItem,
    revision: "10",
    authorization_view: "1",
  });
  mockTauriCommandResult("collaboration_draft", null);
  mockTauriCommand("collaboration_contextual_capabilities", (payload) => {
    const { request } = payload as {
      request: import("@gitru/commands").ContextCapabilityRequest;
    };
    return fixtureContextualCapabilities(fixtureAccount, request.target);
  });
  return items;
}

describe("collaboration workbench", () => {
  it("distinguishes providers with the same login and actor ID while selecting exact account keys", async () => {
    // jsdom cannot evaluate the browser top-layer selectors used by Floating UI.
    const matches = Element.prototype.matches;
    vi.spyOn(Element.prototype, "matches").mockImplementation(function (
      this: Element,
      selector,
    ) {
      return [":modal", ":fullscreen", ":popover-open"].includes(selector)
        ? false
        : matches.call(this, selector);
    });
    const github = {
      ...fixtureAccount,
      login: "same-login",
      actor_id: fixtureGitlabAccount.actor_id,
    };
    const gitlab = { ...fixtureGitlabAccount, login: github.login };
    const gitlabRepositories = {
      ...fixtureRepositories,
      repositories: [
        {
          ...fixtureRepositories.repositories[0],
          id: "gitlab-project",
          account_id: gitlab.id,
          full_name: "gitlab-group/cached-project",
          web_url: "https://gitlab.com/gitlab-group/cached-project",
          selected: false,
        },
      ],
    };
    const items = readMocks();
    mockTauriCommandResult("collaboration_accounts", {
      ...fixtureAccounts,
      accounts: [github, gitlab],
    });
    const repositories = mockTauriCommand(
      "collaboration_repositories",
      (payload) =>
        (payload as { accountId: string }).accountId === gitlab.id
          ? gitlabRepositories
          : fixtureRepositories,
    );
    const capabilities = mockTauriCommand(
      "collaboration_contextual_capabilities",
      (payload) => {
        const { request } = payload as { request: ContextCapabilityRequest };
        const isGitlab = request.account_id === gitlab.id;
        const snapshot = fixtureContextualCapabilities(
          isGitlab ? gitlab : github,
          request.target,
          isGitlab ? "none" : "native_notifications",
        );
        if (isGitlab) {
          for (const facet of snapshot.facets) {
            if (facet.facet === "repositories") continue;
            const access = {
              state: "unsupported" as const,
              reason:
                facet.facet === "inbox"
                  ? ("provider_semantics" as const)
                  : ("not_implemented" as const),
            };
            facet.saved_read = access;
            facet.synchronize = access;
            facet.observation = "unknown";
          }
        }
        return snapshot;
      },
    );
    const user = userEvent.setup();
    const { cache } = mount(<CollaborationWorkspace kind="pull_request" />);
    expect(await screen.findByText(fixtureItem.title)).toBeVisible();
    expect(screen.getByLabelText("Provider account")).toHaveTextContent(
      "GitHub · @same-login",
    );
    await user.click(screen.getByLabelText("Provider account"));
    expect(
      await screen.findByRole("option", { name: "GitHub · @same-login" }),
    ).toBeVisible();
    await user.click(
      screen.getByRole("option", { name: "GitLab · @same-login" }),
    );
    expect(
      await screen.findByText("gitlab-group/cached-project"),
    ).toBeVisible();
    expect(screen.getByLabelText("Provider account")).toHaveTextContent(
      "GitLab · @same-login",
    );
    expect(screen.queryByText(fixtureItem.title)).not.toBeInTheDocument();
    expect(repositories).toHaveBeenCalledWith({ accountId: gitlab.id });
    expect(capabilities).toHaveBeenCalledWith({
      request: expect.objectContaining({
        account_id: gitlab.id,
        authorization_epoch: gitlab.authorization_epoch,
      }),
    });
    expect(cache.getQueryData(collaborationKeys.repositories(gitlab))).toEqual(
      gitlabRepositories,
    );
    expect(
      items.mock.calls.every(
        ([payload]) =>
          (payload as { query: { account_id: string } }).query.account_id ===
          github.id,
      ),
    ).toBe(true);
    await user.click(screen.getByLabelText("Provider account"));
    await user.click(
      await screen.findByRole("option", { name: "GitHub · @same-login" }),
    );
    expect(await screen.findByText(fixtureItem.title)).toBeVisible();
    expect(screen.getByLabelText("Provider account")).toHaveTextContent(
      "GitHub · @same-login",
    );
    expect(cache.getQueryData(collaborationKeys.repositories(github))).toEqual(
      fixtureRepositories,
    );
  });

  it("bounds repository rendering and lets search find repositories beyond the first page", async () => {
    readMocks();
    mockTauriCommandResult("collaboration_repositories", {
      ...fixtureRepositories,
      repositories: Array.from({ length: 301 }, (_, index) => ({
        ...fixtureRepositories.repositories[0],
        id: `repo-${index}`,
        full_name: `example-org/repo-${index}`,
        selected: false,
      })),
    });
    const user = userEvent.setup();
    mount(<CollaborationWorkspace kind="pull_request" />);
    expect(
      await screen.findByText(/Showing the first 100 of 301/),
    ).toBeVisible();
    expect(screen.getAllByRole("checkbox")).toHaveLength(100);
    await user.type(screen.getByLabelText("Filter repositories"), "repo-250");
    expect(screen.getAllByRole("checkbox")).toHaveLength(1);
    expect(screen.getByText("example-org/repo-250")).toBeVisible();
  });
  it("reads actual saved rows while browser networking is offline, without refresh requests", async () => {
    onlineManager.setOnline(false);
    const items = readMocks();
    mount(<CollaborationWorkspace kind="pull_request" />);
    expect(await screen.findByText(fixtureItem.title)).toBeVisible();
    expect(screen.getByText("Offline · saved data")).toBeVisible();
    expect(screen.getByText("Partial history")).toBeVisible();
    expect(items).toHaveBeenCalledWith({
      query: {
        account_id: fixtureAccount.id,
        kind: "pull_request",
        repository_id: null,
        state: "open",
        search: null,
        cursor: null,
        limit: 50,
      },
    });
    // Native boundary fails closed: a hidden provider refresh would fail this test.
  });

  it("distinguishes missing history from an empty synced scope", async () => {
    readMocks({
      ...fixturePage,
      items: [],
      coverage: {
        state: "missing",
        validated_at: null,
        remote_has_more: false,
      },
    });
    const view = mount(<CollaborationWorkspace kind="issue" />);
    expect(await screen.findByText("Not synced yet")).toBeVisible();
    view.unmount();
    mockTauriCommandResult("collaboration_items", {
      ...fixturePage,
      items: [],
      coverage: {
        state: "complete",
        validated_at: "2026-10-02T05:00:00Z",
        remote_has_more: false,
      },
    });
    mount(<CollaborationWorkspace kind="issue" />);
    expect(await screen.findByText("Nothing here yet")).toBeVisible();
  });

  it("renders cached details as text and saves drafts only after the local commit", async () => {
    readMocks();
    let finish!: (draft: LocalDraft) => void;
    let submitted!: LocalDraft;
    const save = mockTauriCommand("collaboration_save_draft", (payload) => {
      submitted = (payload as { draft: LocalDraft }).draft;
      return new Promise<LocalDraft>((resolve) => {
        finish = resolve;
      });
    });
    const user = userEvent.setup();
    const { cache } = mount(<CollaborationWorkspace kind="pull_request" />);
    await user.click(await screen.findByText(fixtureItem.title));
    expect(
      await screen.findByText(/This description is saved locally/),
    ).toBeVisible();
    const draft = await screen.findByLabelText("Private draft");
    await user.type(draft, "My saved draft");
    await user.click(screen.getByRole("button", { name: "Save draft" }));
    expect(draft).toBeDisabled();
    expect(screen.getByRole("button", { name: "Saving…" })).toBeDisabled();
    expect(save).toHaveBeenCalledOnce();
    expect(submitted).toEqual({
      account_id: fixtureAccount.id,
      subject_id: fixtureItem.id,
      body: "My saved draft",
      generation: "0",
    });
    await act(async () => {
      finish({ ...submitted, generation: "1" });
    });
    expect(screen.getByRole("button", { name: "Save draft" })).toBeDisabled();
    expect(draft).toHaveValue("My saved draft");
    await user.type(draft, " and my unsaved changes");
    await act(async () => {
      cache.setQueryData(
        collaborationKeys.draft(fixtureAccount, fixtureItem.id),
        { ...submitted, body: "Changed in another tab", generation: "2" },
      );
    });
    expect(draft).toHaveValue("My saved draft and my unsaved changes");
    expect(
      await screen.findByText(/This draft changed in another tab/),
    ).toBeVisible();
    expect(screen.getByRole("button", { name: "Save draft" })).toBeDisabled();
    await user.click(
      screen.getByRole("button", { name: "Reload saved draft" }),
    );
    expect(draft).toHaveValue("Changed in another tab");
    expect(screen.getByLabelText("Previous draft text")).toHaveValue(
      "My saved draft and my unsaved changes",
    );
  });

  it("makes inbox credential capability explicit", async () => {
    readMocks();
    mockTauriCommandResult("collaboration_accounts", {
      ...fixtureAccounts,
      accounts: [{ ...fixtureAccount, notifications_supported: false }],
    });
    mockTauriCommand("collaboration_contextual_capabilities", (payload) => {
      const { request } = payload as {
        request: import("@gitru/commands").ContextCapabilityRequest;
      };
      const snapshot = fixtureContextualCapabilities(
        fixtureAccount,
        request.target,
      );
      return {
        ...snapshot,
        facets: snapshot.facets.map((facet) =>
          facet.facet === "inbox"
            ? {
                ...facet,
                saved_read: { state: "unavailable", reason: "missing_scope" },
                synchronize: { state: "unavailable", reason: "missing_scope" },
              }
            : facet,
        ),
      };
    });
    mount(<CollaborationWorkspace kind="notification" />);
    expect(await screen.findByText("Permission required")).toBeVisible();
  });
});

describe("provider credential form", () => {
  it("clears the uncontrolled token before awaiting native connection and keeps it out of query mutations", async () => {
    let finish!: (account: typeof fixtureAccount) => void;
    const connect = mockTauriCommand(
      "collaboration_connect_github",
      () =>
        new Promise<typeof fixtureAccount>((resolve) => {
          finish = resolve;
        }),
    );
    const user = userEvent.setup();
    const { cache } = mount(<ConnectGithubForm />);
    const input = screen.getByLabelText("Personal access token");
    await user.type(input, "ghp_fixture_secret");
    await user.click(screen.getByRole("button", { name: "Connect account" }));
    expect(connect).toHaveBeenCalledWith({ token: "ghp_fixture_secret" });
    expect(input).toHaveValue("");
    expect(input).toBeDisabled();
    expect(cache.getMutationCache().getAll()).toEqual([]);
    await act(async () => {
      finish(fixtureAccount);
    });
    expect(await screen.findByText(/Connected as example-user/)).toBeVisible();
  });

  it("restricts credential entry to the main account window", async () => {
    mockWindows("tab-webview:fixture");
    mockTauriCommandResult("collaboration_accounts", fixtureAccounts);
    mount(<AccountManager />);
    expect(
      await screen.findByText("Connected accounts are shared across tabs."),
    ).toBeVisible();
    expect(screen.getByRole("button", { name: "Accounts" })).toBeVisible();
    expect(
      screen.queryByLabelText("Personal access token"),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: /Disconnect/ }),
    ).not.toBeInTheDocument();
  });

  it("discovers multiple CLI accounts and connects only the chosen opaque candidate", async () => {
    const discovery = mockTauriCommandResult(
      "collaboration_discover_github_cli",
      fixtureGithubCli,
    );
    let finish!: (account: typeof fixtureAccount) => void;
    const connect = mockTauriCommand(
      "collaboration_connect_github_cli",
      () =>
        new Promise<typeof fixtureAccount>((resolve) => {
          finish = resolve;
        }),
    );
    const user = userEvent.setup();
    const { cache } = mount(<ConnectGithubForm />);
    expect(await screen.findByText("@second-user")).toBeVisible();
    expect(screen.getByText("Active in gh")).toBeVisible();
    expect(discovery).toHaveBeenCalledWith({});
    await user.click(
      screen.getByRole("button", {
        name: "Use GitHub CLI account second-user",
      }),
    );
    expect(connect).toHaveBeenCalledWith({ candidateId: "fixture-cli-second" });
    expect(connect).toHaveBeenCalledTimes(1);
    expect(screen.getByLabelText("Personal access token")).toBeDisabled();
    expect(screen.getByRole("button", { name: "Check again" })).toBeDisabled();
    expect(cache.getMutationCache().getAll()).toEqual([]);
    expect(
      JSON.stringify(
        cache
          .getQueryCache()
          .getAll()
          .map((query) => query.state.data),
      ),
    ).not.toContain("token");
    await act(async () => finish(fixtureAccount));
    expect(await screen.findByText(/Connected as example-user/)).toBeVisible();
  });

  it("keeps manual PAT connection available when CLI is missing or fails", async () => {
    const discovery = mockTauriCommandResult<GithubCliDiscovery>(
      "collaboration_discover_github_cli",
      {
        status: "not_installed",
        accounts: [],
      },
    );
    const user = userEvent.setup();
    mount(<ConnectGithubForm />);
    expect(await screen.findByText(/GitHub CLI wasn’t found/)).toBeVisible();
    expect(screen.getByLabelText("Personal access token")).toBeEnabled();
    discovery.mockRejectedValueOnce({
      code: "provider",
      message: "secret CLI diagnostics",
    });
    await user.click(screen.getByRole("button", { name: "Check again" }));
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Could not check GitHub CLI accounts",
    );
    expect(screen.getByRole("alert")).not.toHaveTextContent(
      "secret CLI diagnostics",
    );
    expect(screen.getByLabelText("Personal access token")).toBeEnabled();
    discovery.mockResolvedValueOnce(fixtureGithubCli);
    await user.click(screen.getByRole("button", { name: "Check again" }));
    expect(await screen.findByText("@second-user")).toBeVisible();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("allows expired CLI candidates to be rediscovered and retried without clearing manual PAT input", async () => {
    const discovery = mockTauriCommandResult(
      "collaboration_discover_github_cli",
      fixtureGithubCli,
    );
    const connect = mockTauriCommand<typeof fixtureAccount>(
      "collaboration_connect_github_cli",
      () => {
        throw { code: "stale_view", message: "sensitive native diagnostics" };
      },
    );
    const user = userEvent.setup();
    mount(<ConnectGithubForm />);
    await screen.findByText("@second-user");
    await user.type(
      screen.getByLabelText("Personal access token"),
      "ghp_unsent_manual",
    );
    await user.click(
      screen.getByRole("button", {
        name: "Use GitHub CLI account second-user",
      }),
    );
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Check again and choose the account",
    );
    expect(screen.getByLabelText("Personal access token")).toHaveValue(
      "ghp_unsent_manual",
    );
    discovery.mockResolvedValueOnce({
      ...fixtureGithubCli,
      accounts: [{ ...fixtureGithubCli.accounts[1], id: "renewed-candidate" }],
    });
    connect.mockResolvedValueOnce(fixtureAccount);
    await user.click(screen.getByRole("button", { name: "Check again" }));
    await waitFor(() => expect(discovery).toHaveBeenCalledTimes(2));
    await user.click(
      screen.getByRole("button", {
        name: "Use GitHub CLI account second-user",
      }),
    );
    expect(connect).toHaveBeenLastCalledWith({
      candidateId: "renewed-candidate",
    });
    expect(await screen.findByText(/Connected as example-user/)).toBeVisible();
  });

  it("bounds CLI account rows while filtering can reach later accounts", async () => {
    mockTauriCommandResult("collaboration_discover_github_cli", {
      status: "available",
      accounts: Array.from({ length: 21 }, (_, index) => ({
        ...fixtureGithubCli.accounts[0],
        id: `candidate-${index}`,
        login: `cli-user-${index}`,
        active: false,
      })),
    });
    const user = userEvent.setup();
    mount(<ConnectGithubForm />);
    expect(await screen.findByText(/Showing the first 10 of 21/)).toBeVisible();
    expect(
      screen.getAllByRole("button", { name: /Use GitHub CLI account/ }),
    ).toHaveLength(10);
    await user.type(
      screen.getByLabelText("Filter GitHub CLI accounts"),
      "cli-user-20",
    );
    expect(
      screen.getAllByRole("button", { name: /Use GitHub CLI account/ }),
    ).toHaveLength(1);
    expect(screen.getByText("@cli-user-20")).toBeVisible();
  });

  it("does not offer credentials from CLI accounts requiring authentication", async () => {
    mockTauriCommandResult("collaboration_discover_github_cli", {
      status: "available",
      accounts: [
        {
          ...fixtureGithubCli.accounts[0],
          availability: "auth_required",
        },
        {
          ...fixtureGithubCli.accounts[1],
          availability: "unavailable",
        },
      ],
    });
    mount(<ConnectGithubForm />);
    expect(
      await screen.findByText("Sign in again with gh auth login"),
    ).toBeVisible();
    expect(screen.getByText("Account unavailable")).toBeVisible();
    for (const button of screen.getAllByRole("button", {
      name: /Use GitHub CLI account/,
    })) {
      expect(button).toBeDisabled();
    }
    expect(screen.getByLabelText("Personal access token")).toBeEnabled();
  });

  it("never discovers or imports CLI credentials from a tab window", async () => {
    mockWindows("tab-webview:fixture");
    const discover = mockTauriCommandResult(
      "collaboration_discover_github_cli",
      fixtureGithubCli,
    );
    mockTauriCommandResult("collaboration_accounts", fixtureAccounts);
    mount(<AccountManager />);
    await screen.findByText("Connected accounts are shared across tabs.");
    expect(discover).not.toHaveBeenCalled();
    expect(
      screen.queryByRole("button", { name: /Use GitHub CLI account/ }),
    ).not.toBeInTheDocument();
  });

  it("does not echo provider error content containing a token", async () => {
    mockTauriCommand("collaboration_connect_github", () => {
      throw {
        code: "provider",
        message: "ghp_fixture_secret invalid credential",
      };
    });
    const user = userEvent.setup();
    mount(<ConnectGithubForm />);
    await user.type(
      screen.getByLabelText("Personal access token"),
      "ghp_fixture_secret",
    );
    await user.click(screen.getByRole("button", { name: "Connect account" }));
    expect(await screen.findByRole("alert")).not.toHaveTextContent(
      "ghp_fixture_secret",
    );
  });
});

it("routes token creation through the native HTTPS command without submitting the PAT form", async () => {
  const open = mockTauriCommandResult("open_external_url", undefined);
  const connect = mockTauriCommandResult(
    "collaboration_connect_github",
    fixtureAccount,
  );
  const user = userEvent.setup();
  mount(<ConnectGithubForm />);
  await user.click(
    screen.getByRole("button", { name: "Create a fine-grained token" }),
  );
  await waitFor(() =>
    expect(open).toHaveBeenLastCalledWith({
      url: "https://github.com/settings/personal-access-tokens/new?name=Gitru&description=Read%20pull%20requests%20and%20issues&expires_in=30&pull_requests=read&issues=read",
    }),
  );
  await user.click(
    screen.getByRole("button", { name: "Create a classic token" }),
  );
  await waitFor(() =>
    expect(open).toHaveBeenLastCalledWith({
      url: "https://github.com/settings/tokens/new",
    }),
  );
  open.mockRejectedValueOnce("fixture native opener failure");
  await user.click(
    screen.getByRole("button", { name: "Create a classic token" }),
  );
  expect(await screen.findByRole("alert")).toHaveTextContent(
    "Could not open your browser. Try again.",
  );
  expect(screen.getByLabelText("Personal access token")).toBeEnabled();
  expect(connect).not.toHaveBeenCalled();
});

it("never opens unsafe provider links and routes valid links through native HTTPS validation", async () => {
  const view = mount(<ProviderLink url="javascript:alert('fixture')" />);
  expect(
    screen.queryByRole("button", { name: "Open on provider" }),
  ).not.toBeInTheDocument();
  view.rerender(
    <ProviderLink url="https://github.com/example-org/engine/pull/42" />,
  );
  const open = mockTauriCommandResult("open_external_url", undefined);
  await userEvent
    .setup()
    .click(screen.getByRole("button", { name: "Open on provider" }));
  await waitFor(() =>
    expect(open).toHaveBeenCalledWith({
      url: "https://github.com/example-org/engine/pull/42",
    }),
  );
});

it("retains disconnect failures across account lifecycle changes and can retry credential cleanup", async () => {
  let current = fixtureAccounts;
  mockTauriCommand("collaboration_accounts", () => current);
  mockTauriCommand("collaboration_changes_since", () => ({
    revision: current.revision,
    authorization_view: current.authorization_view,
    has_more: false,
    reset_required: false,
    changes: [],
  }));
  vi.spyOn(collaboration.transport, "listen").mockResolvedValue(() => {});
  let attempts = 0;
  const disconnect = mockTauriCommand("collaboration_disconnect", () => {
    attempts += 1;
    current = {
      accounts: [
        { ...fixtureAccount, state: "disconnected", authorization_epoch: "2" },
      ],
      revision: "11",
      authorization_view: "2",
    };
    if (attempts === 1)
      throw {
        code: "credential_store_unavailable",
        message: "The vault is locked",
      };
    return "11";
  });
  const user = userEvent.setup();
  const { cache } = mount(<AccountManager />);
  const stop = collaboration.installBridge(cache);
  try {
    await collaboration.wake();
    await screen.findByRole("button", { name: "Disconnect example-user" });
    const privateKey = collaborationKeys.item(fixtureAccount, fixtureItem.id);
    cache.setQueryData(privateKey, { private: "cached discussion" });
    await user.click(
      screen.getByRole("button", { name: "Disconnect example-user" }),
    );
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "system credential store is unavailable",
    );
    expect(await screen.findByText("Disconnected")).toBeVisible();
    expect(cache.getQueryData(privateKey)).toBeUndefined();
    await user.click(
      screen.getByRole("button", {
        name: "Retry credential cleanup for example-user",
      }),
    );
    await waitFor(() =>
      expect(screen.queryByRole("alert")).not.toBeInTheDocument(),
    );
    expect(disconnect).toHaveBeenCalledTimes(2);
    expect(cache.getQueryData(privateKey)).toBeUndefined();
  } finally {
    stop();
  }
});

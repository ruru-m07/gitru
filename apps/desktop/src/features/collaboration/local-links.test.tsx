import { collaboration } from "@gitru/collaboration-client";
import type {
  ContextCapabilityRequest,
  LocalLinkInspection,
} from "@gitru/commands";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import {
  createMemoryHistory,
  createRootRoute,
  createRoute,
  createRouter,
  defaultParseSearch,
  RouterProvider,
  useSearch,
} from "@tanstack/react-router";
import { mockWindows } from "@tauri-apps/api/mocks";
import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useAppStore } from "@/store/use-app-store";
import {
  fixtureAccount,
  fixtureAccounts,
  fixtureContextualCapabilities,
  fixturePage,
  fixtureRepositories,
} from "../../../tests/fixtures/collaboration";
import {
  fixtureInspection,
  fixtureInstanceId,
  fixtureLink,
  fixtureNavigation,
} from "../../../tests/fixtures/local-links";
import {
  mockTauriCommand,
  mockTauriCommandResult,
} from "../../../tests/mocks/tauri";
import { LinkedCollaborationRoute } from "./linked-collaboration-route";
import { LocalClonePicker } from "./local-clone-picker";
import {
  type LocalLinkRouteTarget,
  localLinkRoutePath,
  localLinkSearch,
  localLinkTarget,
  parseLocalLinkSearch,
} from "./local-link-navigation";
import {
  LocalRepositoryLinksButton,
  LocalRepositoryLinksPanel,
} from "./local-repository-links";
import { LocalTransportSettings } from "./local-transport-settings";

// Persistence is outside this navigation fixture; every operational native
// command continues through the fail-closed Tauri mock boundary.
vi.mock("@tauri-apps/plugin-store", () => ({
  Store: {
    load: async () => ({
      get: async () => null,
      set: async () => {},
      save: async () => {},
    }),
  },
}));

const caches: QueryClient[] = [];
const stops: (() => void)[] = [];
let inspection: LocalLinkInspection;
const initialStore = useAppStore.getState();
const registered = {
  id: "registered-a",
  name: "My registered clone",
  path: "/synthetic/clone-a",
  origin: null,
  current_branch: null,
  ahead_behind: null,
  has_uncommitted_changes: false,
  last_updated: 0,
};
function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((finish) => {
    resolve = finish;
  });
  return { promise, resolve };
}
const target: LocalLinkRouteTarget = {
  ...fixtureNavigation,
  link_id: fixtureLink.id,
  generation: fixtureLink.generation,
};
beforeEach(() => {
  vi.spyOn(window, "scrollTo").mockImplementation(() => {});
  const matches = Element.prototype.matches;
  vi.spyOn(Element.prototype, "matches").mockImplementation(function (
    this: Element,
    selector: string,
  ) {
    // jsdom does not implement top-layer state; nwsapi otherwise recursively
    // delegates these selectors back to Element.matches during Select layout.
    return [":modal", ":fullscreen", ":popover-open"].includes(selector)
      ? false
      : matches.call(this, selector);
  });
  inspection = fixtureInspection();
  vi.spyOn(collaboration.transport, "listen").mockResolvedValue(() => {});
  vi.spyOn(collaboration.transport, "listenLocalChanges").mockResolvedValue(
    () => {},
  );
  mockTauriCommandResult("collaboration_accounts", fixtureAccounts);
  mockTauriCommand("collaboration_local_links", () => inspection);
  mockTauriCommandResult("collaboration_changes_since", {
    revision: "10",
    authorization_view: "1",
    changes: [],
    has_more: false,
    reset_required: false,
  });
  mockTauriCommandResult("collaboration_repositories", fixtureRepositories);
  mockTauriCommand("collaboration_contextual_capabilities", (payload) =>
    fixtureContextualCapabilities(
      fixtureAccount,
      (payload as { request: ContextCapabilityRequest }).request.target,
    ),
  );
});
afterEach(() => {
  for (const stop of stops.splice(0)) stop();
  for (const cache of caches.splice(0)) cache.clear();
  useAppStore.setState(initialStore);
});
async function mount(element: React.ReactNode) {
  const cache = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  caches.push(cache);
  stops.push(collaboration.installBridge(cache));
  await act(async () => {
    await collaboration.wake();
  });
  return render(
    <QueryClientProvider client={cache}>{element}</QueryClientProvider>,
  );
}
describe("explicit local repository links", () => {
  it("requires an explicit endpoint/account choice and sends only native opaque preview authority", async () => {
    const other = {
      ...inspection.snapshot.resolutions[0].candidates[0],
      id: "candidate-b",
      account_id: "other-account",
      actor_id: "999",
      repository: {
        ...fixtureRepositories.repositories[0],
        account_id: "other-account",
      },
    };
    inspection.snapshot.resolutions[0].candidates.push(other);
    const confirm = mockTauriCommand("collaboration_confirm_local_link", () => {
      inspection = { ...inspection, preview_id: "native-preview-2" };
      return { link: fixtureLink, revision: "10", authorization_view: "1" };
    });
    await mount(
      <LocalRepositoryLinksPanel
        localRepositoryId="registered-a"
        onNavigate={vi.fn()}
      />,
    );
    expect(
      await screen.findByRole("button", { name: /Link @example-user/ }),
    ).toBeVisible();
    expect(confirm).not.toHaveBeenCalled();
    await userEvent.click(
      screen.getByRole("button", { name: /Link @other-account/ }),
    );
    await waitFor(() =>
      expect(confirm).toHaveBeenCalledExactlyOnceWith({
        request: {
          preview_id: "native-preview-1",
          candidate_id: "candidate-b",
          replace_link_id: null,
        },
      }),
    );
  });
  it("retires a failed preview until explicit reinspection and retains the saved link", async () => {
    const confirm = mockTauriCommand("collaboration_confirm_local_link", () => {
      throw { code: "stale_view" };
    });
    await mount(
      <LocalRepositoryLinksPanel
        localRepositoryId="registered-a"
        onNavigate={vi.fn()}
      />,
    );
    await userEvent.click(
      await screen.findByRole("button", { name: /Link @example-user/ }),
    );
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Inspect remotes again",
    );
    expect(
      screen.getByRole("button", { name: /Link @example-user/ }),
    ).toBeDisabled();
    expect(screen.getByText("example-org/engine")).toBeVisible();
    inspection = { ...inspection, preview_id: "new-preview" };
    await userEvent.click(
      screen.getByRole("button", { name: "Inspect remotes again" }),
    );
    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: /Link @example-user/ }),
      ).toBeEnabled(),
    );
    expect(confirm).toHaveBeenCalledTimes(1);
  });
  it("permits a native-authorized unique account when another account has hidden competing identity claims", async () => {
    const candidate = {
      ...inspection.snapshot.resolutions[0].candidates[0],
      id: "safe-account-b-candidate",
      account_id: "account-b",
      actor_id: "actor-b",
      repository: {
        ...fixtureRepositories.repositories[0],
        account_id: "account-b",
      },
    };
    // Native excludes every candidate for ambiguous A, including its hidden
    // historical claim, while retaining B's unique authorized repository.
    inspection.snapshot.links = [];
    inspection.snapshot.resolutions = [
      {
        endpoint: candidate.endpoint,
        state: "ambiguous",
        candidates: [candidate],
      },
    ];
    const confirm = mockTauriCommandResult("collaboration_confirm_local_link", {
      link: {
        ...fixtureLink,
        account_id: candidate.account_id,
        actor_id: candidate.actor_id,
        repository: candidate.repository,
      },
      revision: "10",
      authorization_view: "1",
    });
    await mount(
      <LocalRepositoryLinksPanel
        localRepositoryId={registered.id}
        onNavigate={vi.fn()}
      />,
    );
    const button = await screen.findByRole("button", {
      name: /Link @account-b/,
    });
    expect(button).toBeEnabled();
    expect(
      screen.queryByRole("button", { name: /Link @example-user/ }),
    ).not.toBeInTheDocument();
    expect(screen.getByText(/Only unambiguous account choices/)).toBeVisible();
    await userEvent.click(button);
    await waitFor(() =>
      expect(confirm).toHaveBeenCalledExactlyOnceWith({
        request: {
          preview_id: "native-preview-1",
          candidate_id: candidate.id,
          replace_link_id: null,
        },
      }),
    );
  });
  it("changes and removes authored links through captured generations without selecting a repository", async () => {
    const confirm = mockTauriCommandResult("collaboration_confirm_local_link", {
      link: fixtureLink,
      revision: "10",
      authorization_view: "1",
    });
    const remove = mockTauriCommandResult(
      "collaboration_remove_local_link",
      "10",
    );
    const select = mockTauriCommandResult(
      "collaboration_select_repository",
      "10",
    );
    await mount(
      <LocalRepositoryLinksPanel
        localRepositoryId="registered-a"
        onNavigate={vi.fn()}
      />,
    );
    await userEvent.click(
      await screen.findByRole("button", { name: "Change link" }),
    );
    await userEvent.click(
      screen.getByRole("button", { name: /Replace with @example-user/ }),
    );
    await waitFor(() =>
      expect(confirm).toHaveBeenCalledExactlyOnceWith({
        request: {
          preview_id: "native-preview-1",
          candidate_id: "candidate-a",
          replace_link_id: fixtureLink.id,
        },
      }),
    );
    await userEvent.click(screen.getByRole("button", { name: "Remove link" }));
    await waitFor(() =>
      expect(remove).toHaveBeenCalledExactlyOnceWith({
        id: fixtureLink.id,
        generation: fixtureLink.generation,
      }),
    );
    expect(select).not.toHaveBeenCalled();
  });
  it.each([
    "ambiguous",
    "remote_changed",
    "local_repository_missing",
    "unavailable",
  ] as const)("keeps %s authored evidence removable while hiding unavailable provider fields", async (state) => {
    inspection.snapshot.links = [{ ...fixtureLink, state, repository: null }];
    inspection.snapshot.resolutions = [
      { ...inspection.snapshot.resolutions[0], state, candidates: [] },
    ];
    const remove = mockTauriCommandResult(
      "collaboration_remove_local_link",
      "10",
    );
    await mount(
      <LocalRepositoryLinksPanel
        localRepositoryId="registered-a"
        onNavigate={vi.fn()}
      />,
    );
    expect(await screen.findByText("Saved local association")).toBeVisible();
    expect(screen.queryByText("example-org/engine")).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Open pull requests" }),
    ).not.toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Remove link" }));
    expect(remove).toHaveBeenCalledOnce();
  });
  it("native-validates both Git-to-collaboration directions without provider or Git mutation commands", async () => {
    const validate = mockTauriCommandResult(
      "collaboration_validate_local_navigation",
      fixtureNavigation,
    );
    const navigate = vi.fn();
    const refresh = mockTauriCommandResult("collaboration_refresh", {
      job_id: "forbidden",
    });
    const hydrate = mockTauriCommandResult("collaboration_hydrate_detail", {
      job_id: "forbidden",
    });
    await mount(
      <LocalRepositoryLinksPanel
        localRepositoryId="registered-a"
        onNavigate={navigate}
      />,
    );
    await userEvent.click(
      await screen.findByRole("button", { name: "Open issues" }),
    );
    await waitFor(() =>
      expect(navigate).toHaveBeenCalledWith(
        fixtureNavigation,
        fixtureLink,
        "issue",
      ),
    );
    expect(validate).toHaveBeenCalledExactlyOnceWith({
      request: {
        local_repository_id: "registered-a",
        link_id: fixtureLink.id,
        generation: fixtureLink.generation,
        direction: "collaboration",
      },
    });
    expect(refresh).not.toHaveBeenCalled();
    expect(hydrate).not.toHaveBeenCalled();
  });
  it("chooses among registered clones explicitly and rejects a mismatched account receipt", async () => {
    mockTauriCommandResult("collaboration_local_clones", {
      clones: [
        {
          local_repository_id: "registered-a",
          local_repository_name: "Clone A",
          link_id: "link-a",
          generation: "1",
          state: "linked",
        },
        {
          local_repository_id: "registered-b",
          local_repository_name: "Clone B",
          link_id: "link-b",
          generation: "2",
          state: "linked",
        },
      ],
    });
    const validate = mockTauriCommandResult(
      "collaboration_validate_local_navigation",
      { ...fixtureNavigation, local_repository_id: "registered-b" },
    );
    const navigate = vi.fn(async () => {});
    await mount(
      <LocalClonePicker
        account={fixtureAccount}
        instanceId={fixtureInstanceId}
        repositoryId="fixture-repository"
        onNavigate={navigate}
      />,
    );
    await screen.findByRole("button", { name: "Open Clone B" });
    expect(validate).not.toHaveBeenCalled();
    await userEvent.click(screen.getByRole("button", { name: "Open Clone B" }));
    await waitFor(() =>
      expect(navigate).toHaveBeenCalledExactlyOnceWith("registered-b"),
    );
    mockTauriCommandResult("collaboration_validate_local_navigation", {
      ...fixtureNavigation,
      account_id: "other-account",
    });
    await userEvent.click(screen.getByRole("button", { name: "Open Clone A" }));
    expect(await screen.findByRole("alert")).toBeVisible();
    expect(navigate).toHaveBeenCalledTimes(1);
  });
  it("distinguishes same-name clones with known local registrations and validates the exact chosen ID", async () => {
    useAppStore.getState().setRepositories([
      {
        ...registered,
        id: "registered-a",
        name: "project",
        path: "/synthetic/clone-a",
      },
      {
        ...registered,
        id: "registered-b",
        name: "project",
        path: "/synthetic/clone-b",
      },
    ]);
    mockTauriCommandResult("collaboration_local_clones", {
      clones: ["a", "b", "opaque"].map((suffix) => ({
        local_repository_id: `registered-${suffix}`,
        local_repository_name: "project",
        link_id: `link-${suffix}`,
        generation: "1",
        state: "linked",
      })),
    });
    const validate = mockTauriCommandResult(
      "collaboration_validate_local_navigation",
      { ...fixtureNavigation, local_repository_id: "registered-b" },
    );
    const navigate = vi.fn(async () => {});
    await mount(
      <LocalClonePicker
        account={fixtureAccount}
        instanceId={fixtureInstanceId}
        repositoryId="fixture-repository"
        onNavigate={navigate}
      />,
    );
    expect(
      await screen.findByText("Local path: /synthetic/clone-a"),
    ).toBeVisible();
    expect(screen.getByText("Local path: /synthetic/clone-b")).toBeVisible();
    expect(screen.getByText("Registration: registered-opaque")).toBeVisible();
    const chosen = within(
      screen.getByRole("article", { name: "Local clone registered-b" }),
    ).getByRole("button", { name: "Open project" });
    expect(chosen).toHaveAccessibleDescription(
      "Local path: /synthetic/clone-b",
    );
    expect(validate).not.toHaveBeenCalled();
    await userEvent.click(chosen);
    await waitFor(() =>
      expect(validate).toHaveBeenCalledExactlyOnceWith({
        request: {
          local_repository_id: "registered-b",
          link_id: "link-b",
          generation: "1",
          direction: "git",
        },
      }),
    );
    expect(navigate).toHaveBeenCalledExactlyOnceWith("registered-b");
  });
  it("preserves exact route IDs through URL serialization and rejects incomplete query targets", () => {
    const url = new URL(
      localLinkRoutePath("issue", target),
      "https://localhost",
    );
    const decoded = parseLocalLinkSearch(defaultParseSearch(url.search));
    expect(localLinkTarget(decoded)).toEqual(
      Object.fromEntries(
        Object.entries(target).filter(([key]) => key !== "selected"),
      ),
    );
    expect(parseLocalLinkSearch({ account_id: fixtureAccount.id })).toEqual({
      invalidLocalLink: true,
    });
    expect(
      parseLocalLinkSearch({ ...localLinkSearch(target), repository_id: "" }),
    ).toEqual({ invalidLocalLink: true });
  });
  it("fails closed on a retired route receipt before account or broad feed reads", async () => {
    mockTauriCommandResult("collaboration_validate_local_navigation", {
      ...fixtureNavigation,
      authorization_epoch: "2",
    });
    const feed = mockTauriCommandResult("collaboration_items", fixturePage);
    await mount(<LinkedCollaborationRoute kind="issue" target={target} />);
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "another account or repository will not be substituted",
    );
    expect(feed).not.toHaveBeenCalled();
  });
  it("keeps an unselected linked repository scoped and waits for explicit selection", async () => {
    mockTauriCommandResult("collaboration_validate_local_navigation", {
      ...fixtureNavigation,
      selected: false,
    });
    mockTauriCommandResult("collaboration_repositories", {
      ...fixtureRepositories,
      repositories: [
        { ...fixtureRepositories.repositories[0], selected: false },
      ],
    });
    mockTauriCommand("collaboration_contextual_capabilities", (payload) => {
      const snapshot = fixtureContextualCapabilities(
        fixtureAccount,
        (payload as { request: ContextCapabilityRequest }).request.target,
      );
      return {
        ...snapshot,
        facets: snapshot.facets.map((facet) => ({
          ...facet,
          synchronize: {
            state: "unavailable",
            reason: "temporarily_unavailable",
          },
          sync: {
            ...facet.sync,
            state: "rate_limited",
            next_retry_at: "2099-01-01T00:00:00.000Z",
          },
        })),
      };
    });
    const feed = mockTauriCommandResult("collaboration_items", fixturePage);
    const select = mockTauriCommandResult(
      "collaboration_select_repository",
      "10",
    );
    await mount(<LinkedCollaborationRoute kind="issue" target={target} />);
    const button = await screen.findByRole("button", {
      name: "Select this repository",
    });
    expect(button).toBeEnabled(); // local selection remains available during quota cooldown
    expect(select).not.toHaveBeenCalled();
    for (const [payload] of feed.mock.calls)
      expect(
        (payload as { query: { repository_id: string } }).query.repository_id,
      ).toBe("fixture-repository");
    await userEvent.click(button);
    await waitFor(() =>
      expect(select).toHaveBeenCalledExactlyOnceWith({
        accountId: fixtureAccount.id,
        repositoryId: "fixture-repository",
        selected: true,
      }),
    );
  });
  it("never mounts main transport mutation controls in a child webview", async () => {
    mockWindows("tab-webview:fixture");
    const save = mockTauriCommandResult(
      "collaboration_save_transport_binding",
      {},
    );
    await mount(<LocalTransportSettings />);
    expect(
      screen.queryByRole("button", { name: "Repository transports" }),
    ).not.toBeInTheDocument();
    expect(save).not.toHaveBeenCalled();
  });

  it("opens the explicitly chosen registered clone through the existing tab and repository store", async () => {
    mockTauriCommandResult("collaboration_local_clones", {
      clones: [
        {
          local_repository_id: registered.id,
          local_repository_name: registered.name,
          link_id: fixtureLink.id,
          generation: fixtureLink.generation,
          state: "linked",
        },
      ],
    });
    mockTauriCommandResult(
      "collaboration_validate_local_navigation",
      fixtureNavigation,
    );
    const list = mockTauriCommandResult("list_repositories", [registered]);
    const refresh = mockTauriCommandResult("collaboration_refresh", {
      job_id: "forbidden",
    });
    const root = createRootRoute();
    const start = createRoute({
      getParentRoute: () => root,
      path: "/",
      component: () => (
        <LocalClonePicker
          account={fixtureAccount}
          instanceId={fixtureInstanceId}
          repositoryId="fixture-repository"
        />
      ),
    });
    const git = createRoute({
      getParentRoute: () => root,
      path: "/app/git",
      component: () => <p>Registered Git view</p>,
    });
    const router = createRouter({
      routeTree: root.addChildren([start, git]),
      history: createMemoryHistory({ initialEntries: ["/"] }),
    });
    await mount(<RouterProvider router={router} />);
    expect(list).not.toHaveBeenCalled();
    await userEvent.click(
      await screen.findByRole("button", { name: `Open ${registered.name}` }),
    );
    expect(await screen.findByText("Registered Git view")).toBeVisible();
    expect(list).toHaveBeenCalledExactlyOnceWith({ refreshStale: false });
    const store = useAppStore.getState();
    expect(
      store.tabs.find((tab) => tab.id === store.activeTabId),
    ).toMatchObject({
      repositoryId: registered.id,
      routePath: "/app/git",
      title: registered.name,
    });
    expect(store.sessionsById[store.activeSessionId ?? ""]).toMatchObject({
      repositoryId: registered.id,
      routePath: "/app/git",
    });
    expect(store.selectedRepository).toEqual(registered);
    expect(refresh).not.toHaveBeenCalled();
  });

  it("writes a reloadable exact Git-to-Issues target without automatically selecting or refreshing", async () => {
    useAppStore.getState().setRepositories([registered]);
    useAppStore.getState().syncActiveTab({ repositoryId: registered.id });
    const validate = mockTauriCommandResult(
      "collaboration_validate_local_navigation",
      fixtureNavigation,
    );
    const feed = mockTauriCommandResult("collaboration_items", {
      ...fixturePage,
      items: [],
    });
    const select = mockTauriCommandResult(
      "collaboration_select_repository",
      "10",
    );
    const refresh = mockTauriCommandResult("collaboration_refresh", {
      job_id: "forbidden",
    });
    const root = createRootRoute();
    const start = createRoute({
      getParentRoute: () => root,
      path: "/",
      component: LocalRepositoryLinksButton,
    });
    const issues = createRoute({
      getParentRoute: () => root,
      path: "/app/issues",
      validateSearch: parseLocalLinkSearch,
      component: () => {
        const search = parseLocalLinkSearch(
          useSearch({ strict: false, structuralSharing: false }),
        );
        return (
          <LinkedCollaborationRoute
            kind="issue"
            target={localLinkTarget(search)}
            invalid={search.invalidLocalLink}
          />
        );
      },
    });
    const router = createRouter({
      routeTree: root.addChildren([start, issues]),
      history: createMemoryHistory({ initialEntries: ["/"] }),
    });
    await mount(<RouterProvider router={router} />);
    await userEvent.click(
      await screen.findByRole("button", { name: "Linked collaboration" }),
    );
    await userEvent.click(
      await screen.findByRole("button", { name: "Open issues" }),
    );
    await screen.findByRole("heading", { name: "Issues" });
    await waitFor(() => expect(feed).toHaveBeenCalled());
    expect(validate).toHaveBeenCalledTimes(2); // click plus route/reload authority
    expect(
      localLinkTarget(
        parseLocalLinkSearch(
          defaultParseSearch(router.state.location.searchStr),
        ),
      ),
    ).toEqual(localLinkTarget(parseLocalLinkSearch(localLinkSearch(target))));
    const store = useAppStore.getState();
    expect(
      store.tabs.find((tab) => tab.id === store.activeTabId)?.routePath,
    ).toBe(localLinkRoutePath("issue", target));
    for (const [payload] of feed.mock.calls)
      expect(
        (payload as { query: { repository_id: string } }).query.repository_id,
      ).toBe("fixture-repository");
    expect(select).not.toHaveBeenCalled();
    expect(refresh).not.toHaveBeenCalled();
  });

  it("waits for current installation access and never broadens a missing repository target", async () => {
    mockTauriCommandResult(
      "collaboration_validate_local_navigation",
      fixtureNavigation,
    );
    const pending =
      deferred<ReturnType<typeof fixtureContextualCapabilities>>();
    mockTauriCommand(
      "collaboration_contextual_capabilities",
      () => pending.promise,
    );
    const repositories = mockTauriCommandResult("collaboration_repositories", {
      ...fixtureRepositories,
      repositories: [],
    });
    const feed = mockTauriCommandResult("collaboration_items", fixturePage);
    await mount(<LinkedCollaborationRoute kind="issue" target={target} />);
    expect(
      await screen.findByText("Reading linked installation"),
    ).toBeVisible();
    expect(repositories).not.toHaveBeenCalled();
    expect(feed).not.toHaveBeenCalled();
    await act(async () => pending.resolve(fixtureContextualCapabilities()));
    expect(
      await screen.findByText("Linked repository unavailable"),
    ).toBeVisible();
    expect(feed).not.toHaveBeenCalled();
  });

  it("suppresses old clone names and pending navigation when the actor changes", async () => {
    const other = {
      ...fixtureAccount,
      id: "other-account",
      actor_id: "999",
      login: "other-user",
    };
    mockTauriCommand("collaboration_local_clones", (payload) => ({
      clones: [
        {
          local_repository_id: registered.id,
          local_repository_name:
            (payload as { request: { account_id: string } }).request
              .account_id === fixtureAccount.id
              ? "Actor A clone"
              : "Actor B clone",
          link_id: fixtureLink.id,
          generation: fixtureLink.generation,
          state: "linked",
        },
      ],
    }));
    const pending = deferred<typeof fixtureNavigation>();
    mockTauriCommand(
      "collaboration_validate_local_navigation",
      () => pending.promise,
    );
    const navigate = vi.fn(async () => {});
    const cache = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    });
    caches.push(cache);
    const view = render(
      <QueryClientProvider client={cache}>
        <LocalClonePicker
          key={fixtureAccount.id}
          account={fixtureAccount}
          instanceId={fixtureInstanceId}
          repositoryId="fixture-repository"
          onNavigate={navigate}
        />
      </QueryClientProvider>,
    );
    await userEvent.click(
      await screen.findByRole("button", { name: "Open Actor A clone" }),
    );
    view.rerender(
      <QueryClientProvider client={cache}>
        <LocalClonePicker
          key={other.id}
          account={other}
          instanceId={fixtureInstanceId}
          repositoryId="fixture-repository"
          onNavigate={navigate}
        />
      </QueryClientProvider>,
    );
    expect(screen.queryByText("Actor A clone")).not.toBeInTheDocument();
    expect(
      await screen.findByRole("button", { name: "Open Actor B clone" }),
    ).toBeVisible();
    await act(async () => pending.resolve(fixtureNavigation));
    expect(navigate).not.toHaveBeenCalled();
  });

  it("keeps legacy remote observation errors explicit without inventing endpoint choices", async () => {
    inspection = fixtureInspection({
      remotes: null,
      observation_error: "unsupported_legacy_configuration",
      preview_id: null,
    });
    inspection.snapshot.resolutions = [];
    await mount(
      <LocalRepositoryLinksPanel
        localRepositoryId={registered.id}
        onNavigate={vi.fn()}
      />,
    );
    expect(
      await screen.findByText(/unsupported legacy configuration/),
    ).toBeVisible();
    expect(
      screen.queryByRole("button", { name: /Link @/ }),
    ).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Remove link" })).toBeEnabled();
  });

  it("edits and removes transport mappings only after explicit main-window inspection with native generations", async () => {
    const binding = {
      id: "binding-a",
      instance_id: fixtureInstanceId,
      transport: "ssh" as const,
      host: "github-work",
      port: 22,
      path_prefix: "git",
      layout: "subgroups" as const,
      generation: "9007199254740994",
    };
    inspection.snapshot.bindings = [binding];
    useAppStore.getState().setRepositories([registered]);
    const save = mockTauriCommand(
      "collaboration_save_transport_binding",
      () => {
        inspection = {
          ...inspection,
          snapshot: {
            ...inspection.snapshot,
            bindings_generation: "9007199254740995",
            bindings: [
              {
                ...binding,
                host: "github-new",
                generation: "9007199254740995",
              },
            ],
          },
        };
        return inspection.snapshot.bindings[0];
      },
    );
    const remove = mockTauriCommandResult(
      "collaboration_remove_transport_binding",
      "10",
    );
    await mount(<LocalTransportSettings />);
    expect(save).not.toHaveBeenCalled();
    await userEvent.click(
      screen.getByRole("button", { name: "Repository transports" }),
    );
    const choose = screen.getByRole("combobox", {
      name: "Registered local repository",
    });
    const user = userEvent.setup();
    await user.click(choose);
    const option = await screen.findByRole("option", { name: registered.name });
    await user.hover(option);
    await user.click(option);
    await userEvent.click(
      await screen.findByRole("button", { name: "Edit mapping" }),
    );
    await userEvent.clear(
      screen.getByRole("textbox", { name: "Exact transport host" }),
    );
    await userEvent.type(
      screen.getByRole("textbox", { name: "Exact transport host" }),
      "github-new",
    );
    await userEvent.click(
      screen.getByRole("button", { name: "Save mapping changes" }),
    );
    await waitFor(() =>
      expect(save).toHaveBeenCalledExactlyOnceWith({
        request: {
          instance_id: fixtureInstanceId,
          transport: "ssh",
          host: "github-new",
          port: 22,
          path_prefix: "git",
          layout: "subgroups",
          expected_bindings_generation: "9007199254740993",
          replace: { id: binding.id, generation: binding.generation },
        },
      }),
    );
    await waitFor(() =>
      expect(
        screen.queryByRole("button", { name: "Save mapping changes" }),
      ).not.toBeInTheDocument(),
    );
    await userEvent.click(
      screen.getByRole("button", { name: "Remove mapping" }),
    );
    expect(remove).toHaveBeenCalledExactlyOnceWith({
      id: binding.id,
      generation: "9007199254740995",
      expectedBindingsGeneration: "9007199254740995",
    });
  });

  it("adds a provider-instance mapping only after choosing an existing installation and layout", async () => {
    useAppStore.getState().setRepositories([registered]);
    const other = {
      ...fixtureAccount,
      id: "work-account",
      actor_id: "work-actor",
      login: "work-user",
      host: "https://git.example.test",
      provider: "gitlab" as const,
    };
    mockTauriCommandResult("collaboration_accounts", {
      ...fixtureAccounts,
      accounts: [fixtureAccount, other],
    });
    const profile = mockTauriCommand(
      "collaboration_capabilities",
      (payload) => {
        const account =
          (payload as { accountId: string }).accountId === other.id
            ? other
            : fixtureAccount;
        return {
          account_id: account.id,
          instance: fixtureContextualCapabilities(account).instance,
          facets: [],
          inbox_semantics: "none",
          revision: "10",
          authorization_view: "1",
        };
      },
    );
    const save = mockTauriCommand(
      "collaboration_save_transport_binding",
      (payload) => ({
        ...(payload as { request: object }).request,
        id: "new-binding",
        generation: "9007199254740994",
      }),
    );
    const select = mockTauriCommandResult(
      "collaboration_select_repository",
      "10",
    );
    await mount(<LocalTransportSettings />);
    const user = userEvent.setup();
    async function choose(label: string, name: string) {
      await user.click(screen.getByRole("combobox", { name: label }));
      const option = await screen.findByRole("option", { name });
      await user.hover(option);
      await user.click(option);
    }
    await user.click(
      screen.getByRole("button", { name: "Repository transports" }),
    );
    await choose("Registered local repository", registered.name);
    await screen.findByRole("combobox", {
      name: "Provider installation account",
    });
    expect(profile).not.toHaveBeenCalled();
    expect(
      screen.queryByRole("button", { name: "Add transport mapping" }),
    ).not.toBeInTheDocument();
    await choose(
      "Provider installation account",
      `@${other.login} (${other.host})`,
    );
    await screen.findByText(`Installation: gitlab:${other.host}/`);
    await user.type(
      screen.getByRole("textbox", { name: "Exact transport host" }),
      "git-work-alias",
    );
    await user.type(
      screen.getByRole("textbox", { name: "Path prefix" }),
      "scm",
    );
    await choose("Repository path layout", "Namespace subgroups / repository");
    expect(save).not.toHaveBeenCalled();
    await user.click(
      screen.getByRole("button", { name: "Add transport mapping" }),
    );
    await waitFor(() =>
      expect(save).toHaveBeenCalledExactlyOnceWith({
        request: {
          instance_id: `gitlab:${other.host}/`,
          transport: "https",
          host: "git-work-alias",
          port: 443,
          path_prefix: "scm",
          layout: "subgroups",
          expected_bindings_generation: "9007199254740993",
          replace: null,
        },
      }),
    );
    expect(profile).toHaveBeenCalledExactlyOnceWith({ accountId: other.id });
    expect(select).not.toHaveBeenCalled();
  });

  it("requests main settings from a child with no candidate or mutation payload", async () => {
    mockWindows("tab-webview:fixture");
    const emit = mockTauriCommandResult("plugin:event|emit_to", undefined);
    const save = mockTauriCommandResult(
      "collaboration_save_transport_binding",
      {},
    );
    await mount(
      <LocalRepositoryLinksPanel
        localRepositoryId={registered.id}
        onNavigate={vi.fn()}
      />,
    );
    await userEvent.click(
      await screen.findByRole("button", {
        name: "Configure transport mapping…",
      }),
    );
    expect(emit).toHaveBeenCalledExactlyOnceWith({
      target: { kind: "Webview", label: "main" },
      event: "gitru:open-account-settings",
      payload: undefined,
    });
    expect(save).not.toHaveBeenCalled();
  });
});

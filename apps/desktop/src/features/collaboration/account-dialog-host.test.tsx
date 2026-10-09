import { collaborationKeys } from "@gitru/collaboration-client";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { StrictMode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useAppStore } from "@/store/use-app-store";
import {
  fixtureAccounts,
  fixtureGithubCli,
} from "../../../tests/fixtures/collaboration";
import { fixtureInspection } from "../../../tests/fixtures/local-links";
import {
  mockTauriCommand,
  mockTauriCommandResult,
} from "../../../tests/mocks/tauri";
import { ACCOUNT_SETTINGS_OPEN_EVENT } from "./account-dialog-events";
import { AccountDialogHost } from "./account-dialog-host";
import { AccountSettingsButton } from "./account-manager";

const native = vi.hoisted(() => ({
  label: "main",
  listen: vi.fn(),
  emitTo: vi.fn(),
  setFocus: vi.fn(),
  suspend: vi.fn(),
}));
vi.mock("@tauri-apps/api/webview", () => ({
  getCurrentWebview: () => ({
    label: native.label,
    listen: native.listen,
    emitTo: native.emitTo,
    setFocus: native.setFocus,
  }),
}));
vi.mock("@/components/webview-tab-host", () => ({
  setTabWebviewsSuspended: native.suspend,
}));
vi.mock("@tauri-apps/plugin-store", () => ({
  Store: {
    load: async () => ({
      get: async () => null,
      set: async () => {},
      save: async () => {},
    }),
  },
}));

type Handler = () => void;
let handler: Handler | undefined;
const caches: QueryClient[] = [];
const initialStore = useAppStore.getState();
beforeEach(() => {
  const matches = Element.prototype.matches;
  vi.spyOn(Element.prototype, "matches").mockImplementation(function (
    this: Element,
    selector: string,
  ) {
    // jsdom has no top layer; its CSS matcher recurses for these three
    // unsupported selectors used by Floating UI during Select positioning.
    return [":modal", ":fullscreen", ":popover-open"].includes(selector)
      ? false
      : matches.call(this, selector);
  });
  native.label = "main";
  handler = undefined;
  native.listen.mockReset().mockImplementation(async (_event, receive) => {
    handler = receive;
    return () => {
      if (handler === receive) handler = undefined;
    };
  });
  native.emitTo.mockReset().mockImplementation(async () => handler?.());
  native.setFocus.mockReset().mockResolvedValue(undefined);
  native.suspend.mockReset().mockResolvedValue(undefined);
  mockTauriCommandResult("collaboration_accounts", fixtureAccounts);
  mockTauriCommandResult("collaboration_diagnostics", {
    generated_at: "2026-10-08T00:00:00.000Z",
    revision: fixtureAccounts.revision,
    accounts: [],
    ready_jobs: 0,
    deferred_jobs: 0,
    oldest_job_age_seconds: null,
    accounts_in_cooldown: 0,
    latency: {
      sample_count: 0,
      total_milliseconds: 0,
      maximum_milliseconds: null,
      p50_upper_bound_milliseconds: null,
      p95_upper_bound_milliseconds: null,
      p99_upper_bound_milliseconds: null,
    },
    storage: {
      cache_usage_available: false,
      logical_bytes: null,
      indexed_logical_bytes: null,
      database_bytes: null,
      wal_bytes: null,
      wal_observation_supported: false,
      wal_busy: null,
      wal_log_frames: null,
      wal_checkpointed_frames: null,
    },
  });
  mockTauriCommandResult("collaboration_discover_github_cli", fixtureGithubCli);
});
afterEach(async () => {
  await act(async () => {
    await Promise.resolve();
  });
  for (const cache of caches.splice(0)) cache.clear();
  useAppStore.setState(initialStore);
});

function mount(component: React.ReactNode) {
  const cache = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  caches.push(cache);
  return {
    ...render(
      <QueryClientProvider client={cache}>{component}</QueryClientProvider>,
    ),
    cache,
  };
}

function host() {
  return (
    <>
      <AccountSettingsButton />
      <AccountDialogHost />
    </>
  );
}

describe("main account dialog host", () => {
  it("chooses a registered transport repository inside the real suspended account dialog", async () => {
    useAppStore.getState().setRepositories([
      {
        id: "registered-a",
        name: "project",
        path: "/synthetic/a",
        origin: null,
        current_branch: null,
        ahead_behind: null,
        has_uncommitted_changes: false,
        last_updated: 0,
      },
      {
        id: "registered-b",
        name: "project",
        path: "/synthetic/b",
        origin: null,
        current_branch: null,
        ahead_behind: null,
        has_uncommitted_changes: false,
        last_updated: 0,
      },
    ]);
    const inspect = mockTauriCommand(
      "collaboration_local_links",
      (payload) => ({
        ...fixtureInspection(),
        local_repository_id: (payload as { localRepositoryId: string })
          .localRepositoryId,
      }),
    );
    const save = mockTauriCommandResult(
      "collaboration_save_transport_binding",
      {},
    );
    mount(host());
    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: "Accounts" }));
    const dialog = await screen.findByRole("dialog", {
      name: "Connected accounts",
    });
    await user.click(
      within(dialog).getByRole("button", { name: "Repository transports" }),
    );
    const body = within(dialog).getByRole("region", {
      name: "Account and repository settings",
    });
    await user.click(
      within(body).getByRole("combobox", {
        name: "Registered local repository",
      }),
    );
    const choice = await screen.findByRole("option", {
      name: "project · /synthetic/b",
    });
    expect(choice).toBeVisible();
    expect(
      within(dialog).getByRole("heading", { name: "Connected accounts" }),
    ).toBeVisible();
    expect(inspect).not.toHaveBeenCalled();
    await user.hover(choice);
    await user.click(choice);
    await waitFor(() =>
      expect(inspect).toHaveBeenCalledExactlyOnceWith({
        localRepositoryId: "registered-b",
      }),
    );
    expect(
      await within(body).findByRole("combobox", {
        name: "Provider installation account",
      }),
    ).toBeVisible();
    expect(native.suspend).toHaveBeenCalledWith(true);
    expect(save).not.toHaveBeenCalled();
  });
  it("routes a child button to the explicit main webview without mounting credential controls", async () => {
    native.label = "tab-webview:inbox";
    const discover = mockTauriCommandResult(
      "collaboration_discover_github_cli",
      fixtureGithubCli,
    );
    mount(
      <>
        <AccountSettingsButton />
        <AccountDialogHost />
      </>,
    );
    await userEvent
      .setup()
      .click(screen.getByRole("button", { name: "Accounts" }));
    expect(native.emitTo).toHaveBeenCalledWith(
      { kind: "Webview", label: "main" },
      ACCOUNT_SETTINGS_OPEN_EVENT,
    );
    expect(native.listen).not.toHaveBeenCalled();
    expect(native.suspend).not.toHaveBeenCalled();
    expect(discover).not.toHaveBeenCalled();
    expect(
      screen.queryByLabelText("Personal access token"),
    ).not.toBeInTheDocument();
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("mounts credential controls only after native children are hidden and main is focused", async () => {
    let finish!: () => void;
    native.suspend.mockImplementation((suspended: boolean) =>
      suspended
        ? new Promise<void>((resolve) => {
            finish = resolve;
          })
        : Promise.resolve(),
    );
    const discover = mockTauriCommandResult(
      "collaboration_discover_github_cli",
      fixtureGithubCli,
    );
    const user = userEvent.setup();
    mount(host());
    await user.click(screen.getByRole("button", { name: "Accounts" }));
    await waitFor(() => expect(native.suspend).toHaveBeenCalledWith(true));
    expect(native.setFocus).not.toHaveBeenCalled();
    expect(discover).not.toHaveBeenCalled();
    expect(
      screen.queryByLabelText("Personal access token"),
    ).not.toBeInTheDocument();
    await act(async () => finish());
    expect(await screen.findByLabelText("Personal access token")).toBeVisible();
    expect(native.setFocus).toHaveBeenCalledOnce();
    await user.click(screen.getByRole("button", { name: "Close" }));
    await waitFor(() => expect(native.suspend).toHaveBeenLastCalledWith(false));
    expect(
      screen.queryByLabelText("Personal access token"),
    ).not.toBeInTheDocument();
  });

  it("rediscovers metadata when reopened and discards the closed dialog's discovery query", async () => {
    const discover = mockTauriCommandResult(
      "collaboration_discover_github_cli",
      fixtureGithubCli,
    );
    const user = userEvent.setup();
    const { cache } = mount(host());
    expect(discover).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "Accounts" }));
    expect(await screen.findByText("@second-user")).toBeVisible();
    expect(discover).toHaveBeenCalledOnce();
    await user.keyboard("{Escape}");
    await waitFor(() =>
      expect(cache.getQueryData(collaborationKeys.githubCli)).toBeUndefined(),
    );
    await user.click(screen.getByRole("button", { name: "Accounts" }));
    expect(await screen.findByText("@second-user")).toBeVisible();
    expect(discover).toHaveBeenCalledTimes(2);
    expect(screen.getAllByRole("dialog")).toHaveLength(1);
  });

  it("shows a safe request error without opening a local dialog when event delivery fails", async () => {
    native.label = "tab-webview:inbox";
    native.emitTo.mockRejectedValueOnce(new Error("private native diagnostic"));
    mount(<AccountSettingsButton />);
    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: "Accounts" }));
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Could not open account settings",
    );
    expect(screen.getByRole("alert")).not.toHaveTextContent(
      "private native diagnostic",
    );
    expect(
      screen.queryByLabelText("Personal access token"),
    ).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Accounts" }));
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("unwinds failed child suspension and can open successfully on another request", async () => {
    native.suspend.mockRejectedValueOnce(
      new Error("private native diagnostic"),
    );
    const discover = mockTauriCommandResult(
      "collaboration_discover_github_cli",
      fixtureGithubCli,
    );
    const user = userEvent.setup();
    mount(host());
    await user.click(screen.getByRole("button", { name: "Accounts" }));
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Could not open account settings",
    );
    expect(native.suspend).toHaveBeenLastCalledWith(false);
    expect(discover).not.toHaveBeenCalled();
    expect(
      screen.queryByLabelText("Personal access token"),
    ).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Accounts" }));
    expect(await screen.findByLabelText("Personal access token")).toBeVisible();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("removes a listener that resolves after unmount without affecting a newer dialog owner", async () => {
    let finishListener!: (unlisten: () => void) => void;
    const lateUnlisten = vi.fn();
    native.listen.mockImplementationOnce((_event, receive) => {
      handler = receive;
      return new Promise<() => void>((resolve) => {
        finishListener = resolve;
      });
    });
    const previous = mount(<AccountDialogHost />);
    previous.unmount();
    const current = mount(host());
    await userEvent
      .setup()
      .click(screen.getByRole("button", { name: "Accounts" }));
    expect(await screen.findByLabelText("Personal access token")).toBeVisible();
    await act(async () => finishListener(lateUnlisten));
    expect(lateUnlisten).toHaveBeenCalledOnce();
    expect(native.suspend).toHaveBeenLastCalledWith(true);
    current.unmount();
    await waitFor(() => expect(native.suspend).toHaveBeenLastCalledWith(false));
  });

  it("does not expose the form after unmounting while native hiding is in flight", async () => {
    let finish!: () => void;
    native.suspend.mockImplementationOnce(
      () =>
        new Promise<void>((resolve) => {
          finish = resolve;
        }),
    );
    const user = userEvent.setup();
    const current = mount(host());
    await user.click(screen.getByRole("button", { name: "Accounts" }));
    await waitFor(() => expect(native.suspend).toHaveBeenCalledWith(true));
    current.unmount();
    await act(async () => finish());
    await waitFor(() => expect(native.suspend).toHaveBeenLastCalledWith(false));
    expect(native.setFocus).not.toHaveBeenCalled();
    expect(
      screen.queryByLabelText("Personal access token"),
    ).not.toBeInTheDocument();
  });

  it("keeps native children hidden when an obsolete opening finishes after a new host acquires them", async () => {
    let finishPreviousHide!: () => void;
    native.suspend.mockImplementationOnce(
      () =>
        new Promise<void>((resolve) => {
          finishPreviousHide = resolve;
        }),
    );
    const user = userEvent.setup();
    const previous = mount(host());
    await user.click(screen.getByRole("button", { name: "Accounts" }));
    await waitFor(() => expect(native.suspend).toHaveBeenCalledWith(true));
    previous.unmount();
    const current = mount(host());
    await user.click(screen.getByRole("button", { name: "Accounts" }));
    await act(async () => finishPreviousHide());
    expect(await screen.findByLabelText("Personal access token")).toBeVisible();
    expect(native.suspend.mock.calls.every(([suspended]) => suspended)).toBe(
      true,
    );
    current.unmount();
    await waitFor(() => expect(native.suspend).toHaveBeenLastCalledWith(false));
  });

  it("keeps one working main listener after a StrictMode effect probe", async () => {
    const current = mount(<StrictMode>{host()}</StrictMode>);
    await userEvent
      .setup()
      .click(screen.getByRole("button", { name: "Accounts" }));
    expect(await screen.findByLabelText("Personal access token")).toBeVisible();
    expect(screen.getAllByRole("dialog")).toHaveLength(1);
    current.unmount();
    await waitFor(() => expect(native.suspend).toHaveBeenLastCalledWith(false));
  });
});

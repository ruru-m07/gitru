import { collaborationKeys } from "@gitru/collaboration-client";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { StrictMode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  fixtureAccounts,
  fixtureGithubCli,
} from "../../../tests/fixtures/collaboration";
import { mockTauriCommandResult } from "../../../tests/mocks/tauri";
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

type Handler = () => void;
let handler: Handler | undefined;
const caches: QueryClient[] = [];
beforeEach(() => {
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
  mockTauriCommandResult("collaboration_discover_github_cli", fixtureGithubCli);
});
afterEach(async () => {
  await act(async () => {
    await Promise.resolve();
  });
  for (const cache of caches.splice(0)) cache.clear();
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

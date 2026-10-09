import {
  type CommandRecoveryDetail,
  collaborationKeys,
} from "@gitru/collaboration-client";
import {
  onlineManager,
  QueryClient,
  QueryClientProvider,
} from "@tanstack/react-query";
import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { fixtureAccount } from "../../../tests/fixtures/collaboration";
import {
  mockTauriCommand,
  mockTauriCommandResult,
} from "../../../tests/mocks/tauri";
import { CommandRecoveryButton } from "./command-recovery";

const account = fixtureAccount;
const commandId = "11111111-1111-4111-8111-111111111111";
function detail(): CommandRecoveryDetail {
  return {
    command: {
      account_id: account.id,
      command_id: commandId,
      target_id: "issue",
      target_kind: "issue",
      operation_kind: "fixture.edit",
      payload_version: 1,
      state: "conflict",
      admitted_at: "2026-10-08T00:00:00Z",
      attempt_count: 0,
      paused: false,
      quarantined: false,
      attention: null,
      replacement_id: null,
      blocked_reason: null,
    },
    context: {
      account_id: account.id,
      command_id: commandId,
      expected_generation: "1",
      expected_epoch: account.authorization_epoch,
      authorization_view: "1",
      review_token: "review-1",
    },
    fields: [
      {
        field: "body",
        base: { known: true, value: "Original body" },
        remote: { known: true, value: "Provider body" },
        desired: { known: true, value: "Saved <script>text</script>" },
        comparison: "conflict",
        editable: true,
      },
    ],
    can_retry: false,
    can_cancel: true,
    can_pause: false,
    can_replace: true,
    reason: null,
    revision: "1",
  };
}
// jsdom has no browser top layer. Its selector engine delegates these states
// back to Element.matches, recursively, when Base UI checks pointer containment.
// Keep real pointer interactions and delegate every supported selector normally.
beforeEach(() => {
  const matches = Element.prototype.matches;
  vi.spyOn(Element.prototype, "matches").mockImplementation(function (
    this: Element,
    selector,
  ) {
    if (selector === ":modal" || selector === ":fullscreen") return false;
    return matches.call(this, selector);
  });
});

const caches: QueryClient[] = [];
afterEach(() => {
  for (const cache of caches.splice(0)) cache.clear();
  onlineManager.setOnline(true);
});
async function mount(current = detail()) {
  const cache = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  caches.push(cache);
  const list = mockTauriCommandResult("collaboration_command_recovery_list", {
    commands: [current.command],
    next_cursor: null,
    revision: "1",
    authorization_view: "1",
  });
  const read = mockTauriCommandResult(
    "collaboration_command_recovery_detail",
    current,
  );
  render(
    <QueryClientProvider client={cache}>
      <CommandRecoveryButton accounts={[account]} />
    </QueryClientProvider>,
  );
  const user = userEvent.setup();
  expect(list).not.toHaveBeenCalled();
  await user.click(screen.getByRole("button", { name: "Saved changes" }));
  await user.click(await screen.findByText("Issue change"));
  await screen.findByRole("region", { name: "Saved change review" });
  return { cache, user, list, read };
}

it("reads preserved history offline and exposes only permitted local actions for an unknown codec", async () => {
  onlineManager.setOnline(false);
  const unknown = detail();
  unknown.fields = [];
  unknown.can_cancel = false;
  unknown.can_replace = false;
  unknown.command.quarantined = true;
  unknown.reason = "This operation is not supported by this version.";
  const { user, list, read } = await mount(unknown);
  expect(screen.getByText(unknown.reason)).toBeVisible();
  expect(
    screen.queryByRole("button", { name: "Save new resolution" }),
  ).not.toBeInTheDocument();
  expect(
    screen.queryByRole("button", { name: "Resume delivery" }),
  ).not.toBeInTheDocument();
  expect(
    screen.queryByRole("button", { name: "Cancel before sending" }),
  ).not.toBeInTheDocument();
  const exported = mockTauriCommandResult(
    "collaboration_command_recovery_export",
    true,
  );
  await user.click(
    screen.getByRole("button", { name: "Export original change" }),
  );
  expect(await screen.findByText("Saved change exported.")).toBeVisible();
  expect(exported).toHaveBeenCalledWith({ context: unknown.context });
  expect(list).toHaveBeenCalledWith({
    query: {
      account_id: account.id,
      target_id: null,
      cursor: null,
      include_terminal: false,
      limit: 50,
    },
  });
  expect(read).toHaveBeenCalledWith({ accountId: account.id, commandId });
});

it("preserves edited text through a new native review, and retries a lost local receipt with the same UUIDs", async () => {
  const original = detail();
  const { user, cache, read } = await mount(original);
  expect(screen.getByText("Saved <script>text</script>")).toBeVisible();
  expect(document.querySelector("script")).toBeNull();
  await user.click(screen.getByRole("button", { name: "Edit resolution" }));
  const input = screen.getByRole("textbox", { name: "Description resolution" });
  await user.clear(input);
  await user.type(input, "My merged text 🌿");
  const fresh: CommandRecoveryDetail = {
    ...original,
    context: {
      ...original.context,
      expected_generation: "2",
      review_token: "review-2",
    },
    fields: [
      {
        ...original.fields[0],
        remote: { known: true, value: "New provider body" },
      },
    ],
    revision: "2",
  };
  read.mockReturnValue(fresh);
  await act(async () => {
    cache.setQueryData(
      collaborationKeys.commandRecoveryDetail(account, commandId),
      fresh,
    );
  });
  expect(input).toHaveValue("My merged text 🌿");
  await waitFor(() =>
    expect(
      screen.getByRole("button", { name: "Save new resolution" }),
    ).toBeDisabled(),
  );
  await user.click(
    screen.getByRole("button", { name: "Review latest values" }),
  );
  expect(screen.getByText("New provider body")).toBeVisible();
  expect(input).toHaveValue("My merged text 🌿");
  let first = true;
  const replace = mockTauriCommand(
    "collaboration_command_recovery_replace",
    (args) => {
      if (first) {
        first = false;
        throw new Error("Receipt interrupted");
      }
      const { request } = args as {
        request: { action_id: string; new_command_id: string };
      };
      return {
        account_id: account.id,
        action_id: request.action_id,
        command_id: commandId,
        replacement_id: request.new_command_id,
        state: "superseded",
        paused: false,
        remote_may_have_happened: false,
        revision: "3",
      };
    },
  );
  await user.click(screen.getByRole("button", { name: "Save new resolution" }));
  await screen.findByRole("alert");
  expect(input).toHaveValue("My merged text 🌿");
  await user.click(
    screen.getByRole("button", { name: "Retry previous local action" }),
  );
  await screen.findByText(
    "Resolution saved as a new change. The original history is retained.",
  );
  expect(replace).toHaveBeenCalledTimes(2);
  expect(replace.mock.calls[0]).toEqual(replace.mock.calls[1]);
  expect(replace.mock.calls[1][0]).toMatchObject({
    request: {
      context: fresh.context,
      fields: [{ field: "body", choice: "edited", value: "My merged text 🌿" }],
    },
  });
});

it("distinguishes pause after an attempt from cancellation and retains an edit across dialog closure", async () => {
  const current = detail();
  current.command.state = "outcome_unknown";
  current.command.attempt_count = 1;
  current.can_cancel = false;
  current.can_replace = false;
  current.can_pause = true;
  const { user } = await mount(current);
  expect(
    screen.queryByRole("button", { name: "Cancel before sending" }),
  ).not.toBeInTheDocument();
  expect(
    screen.getByText(/Pausing future delivery does not undo it/),
  ).toBeVisible();
  const action = mockTauriCommand(
    "collaboration_command_recovery_action",
    (args) => {
      const { request } = args as { request: { action_id: string } };
      return {
        account_id: account.id,
        action_id: request.action_id,
        command_id: commandId,
        replacement_id: null,
        state: "outcome_unknown",
        paused: true,
        remote_may_have_happened: true,
        revision: "2",
      };
    },
  );
  await user.click(
    screen.getByRole("button", { name: "Pause future delivery" }),
  );
  await screen.findByText(
    "Future delivery is paused. A prior provider action may already have happened.",
  );
  expect(action).toHaveBeenCalledWith({
    request: {
      context: current.context,
      action: "pause",
      action_id: expect.any(String),
    },
  });
  await waitFor(() =>
    expect(screen.getByRole("button", { name: /^Close$/ })).toBeEnabled(),
  );
  await user.click(screen.getByRole("button", { name: "Edit resolution" }));
  await user.clear(
    screen.getByRole("textbox", { name: "Description resolution" }),
  );
  await user.type(
    screen.getByRole("textbox", { name: "Description resolution" }),
    "Retain until I choose",
  );
  await user.click(screen.getByRole("button", { name: /^Close$/ }));
  await user.click(screen.getByRole("button", { name: "Saved changes" }));
  await user.click(await screen.findByRole("button", { name: /Issue change/ }));
  expect(
    await screen.findByRole("textbox", { name: "Description resolution" }),
  ).toHaveValue("Retain until I choose");
});

it("retains resolution text and blocks new actions when refreshing its local review fails", async () => {
  const { user, cache, read } = await mount();
  await user.click(screen.getByRole("button", { name: "Edit resolution" }));
  const input = screen.getByRole("textbox", { name: "Description resolution" });
  await user.clear(input);
  await user.type(input, "Keep this text");
  read.mockImplementation(() => {
    throw new Error("Local review unavailable");
  });
  await act(async () => {
    await cache.invalidateQueries({
      queryKey: collaborationKeys.commandRecoveryDetail(account, commandId),
    });
  });
  await screen.findByRole("alert");
  expect(input).toHaveValue("Keep this text");
  expect(
    screen.getByRole("button", { name: "Save new resolution" }),
  ).toBeDisabled();
  expect(
    screen.getByRole("button", { name: "Cancel before sending" }),
  ).toBeDisabled();
  read.mockReturnValue(detail());
  await user.click(screen.getByRole("button", { name: "Reload change" }));
  await waitFor(() =>
    expect(screen.queryByRole("alert")).not.toBeInTheDocument(),
  );
  expect(input).toHaveValue("Keep this text");
  expect(
    screen.getByRole("button", { name: "Save new resolution" }),
  ).toBeEnabled();
});

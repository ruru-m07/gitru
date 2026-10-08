import type {
  ProviderInboxActionsSnapshot,
  RemoteAccount,
} from "@gitru/collaboration-client";
import {
  onlineManager,
  QueryClient,
  QueryClientProvider,
} from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, expect, it } from "vitest";
import { fixtureAccount } from "../../../tests/fixtures/collaboration";
import {
  mockTauriCommand,
  mockTauriCommandResult,
} from "../../../tests/mocks/tauri";
import { ProviderInboxActions } from "./provider-inbox-actions";

const account = fixtureAccount;
function snapshot(): ProviderInboxActionsSnapshot {
  return {
    account_id: account.id,
    subject_id: "thread",
    authorization_epoch: account.authorization_epoch,
    authorization_view: "1",
    activity_version: "activity-1",
    revision: "1",
    actions: [
      {
        action: "mark_read",
        availability: "available",
        reason: null,
        activity_policy: "best_effort_current_item",
      },
      {
        action: "mark_done",
        availability: "unsupported",
        reason: "not_implemented",
        activity_policy: "best_effort_current_item",
      },
    ],
  };
}
const caches: QueryClient[] = [];
afterEach(() => {
  for (const cache of caches.splice(0)) cache.clear();
  onlineManager.setOnline(true);
});
function mount(current = snapshot(), providerAccount: RemoteAccount = account) {
  const cache = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  caches.push(cache);
  const query = mockTauriCommandResult(
    "collaboration_provider_inbox_actions",
    current,
  );
  render(
    <QueryClientProvider client={cache}>
      <ProviderInboxActions account={providerAccount} notificationId="thread" />
    </QueryClientProvider>,
  );
  return { user: userEvent.setup(), query };
}

it("admits offline provider intent once and reuses the exact request after a lost local receipt", async () => {
  onlineManager.setOnline(false);
  let first = true;
  const queue = mockTauriCommand(
    "collaboration_queue_provider_inbox_action",
    (args) => {
      if (first) {
        first = false;
        throw new Error("Lost local response");
      }
      const { request } = args as { request: { command_id: string } };
      return {
        account_id: account.id,
        command_id: request.command_id,
        revision: "2",
        duplicate: true,
      };
    },
  );
  const { user, query } = mount();
  const button = await screen.findByRole("button", {
    name: "Mark read on GitHub",
  });
  expect(queue).not.toHaveBeenCalled();
  expect(
    screen.getByText(/activity that arrives before delivery/),
  ).toBeVisible();
  await user.click(button);
  await screen.findByRole("alert");
  expect(button).toBeDisabled();
  await user.click(
    screen.getByRole("button", { name: "Retry saving this action" }),
  );
  expect(
    await screen.findByText(
      "Saved locally. Open Saved changes to follow delivery.",
    ),
  ).toBeVisible();
  expect(queue).toHaveBeenCalledTimes(2);
  expect(queue.mock.calls[0]).toEqual(queue.mock.calls[1]);
  expect(queue.mock.calls[0][0]).toEqual({
    request: {
      account_id: account.id,
      authorization_epoch: account.authorization_epoch,
      authorization_view: "1",
      subject_id: "thread",
      expected_activity_version: "activity-1",
      command_id: expect.any(String),
      action: "mark_read",
      activity_policy: "best_effort_current_item",
    },
  });
  expect(query).toHaveBeenCalledWith({
    query: { account_id: account.id, subject_id: "thread" },
  });
});

it("keeps GitHub done unavailable and uses GitLab's typed done action without a read mutation", async () => {
  const gitlab = {
    ...account,
    provider: "gitlab" as const,
    host: "gitlab.com",
  };
  const current = snapshot();
  current.actions[0] = {
    ...current.actions[0],
    availability: "unsupported",
    reason: "provider_semantics",
  };
  current.actions[1] = {
    ...current.actions[1],
    availability: "available",
    reason: null,
  };
  const queue = mockTauriCommand(
    "collaboration_queue_provider_inbox_action",
    (args) => {
      const { request } = args as { request: { command_id: string } };
      return {
        account_id: account.id,
        command_id: request.command_id,
        revision: "2",
        duplicate: false,
      };
    },
  );
  const local = mockTauriCommandResult(
    "collaboration_set_local_inbox_state",
    undefined,
  );
  const { user } = mount(current, gitlab);
  expect(
    await screen.findByRole("button", { name: "Mark read on GitLab" }),
  ).toBeDisabled();
  await user.click(screen.getByRole("button", { name: "Mark done on GitLab" }));
  await screen.findByRole("status");
  expect(queue.mock.calls[0][0]).toMatchObject({
    request: { action: "mark_done" },
  });
  expect(local).not.toHaveBeenCalled();
});

it("does not admit an unavailable or unsupported operation from the saved native policy", async () => {
  const current = snapshot();
  current.actions[0] = {
    ...current.actions[0],
    availability: "unavailable",
    reason: "pending_command",
  };
  const queue = mockTauriCommandResult(
    "collaboration_queue_provider_inbox_action",
    undefined,
  );
  const { user } = mount(current);
  const read = await screen.findByRole("button", {
    name: "Mark read on GitHub",
  });
  const done = screen.getByRole("button", { name: "Mark done on GitHub" });
  expect(read).toBeDisabled();
  expect(done).toBeDisabled();
  expect(
    screen.getByText("A saved change is already waiting for this item."),
  ).toBeVisible();
  await user.click(read);
  await user.click(done);
  expect(queue).not.toHaveBeenCalled();
});

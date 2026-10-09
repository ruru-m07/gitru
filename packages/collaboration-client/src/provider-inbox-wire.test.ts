import {
  collaborationQueueProviderInboxAction,
  ProviderInboxActionsSnapshotSchema,
  QueueProviderInboxActionRequestSchema,
} from "@gitru/commands";
import { afterEach, expect, it, vi } from "vitest";

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
afterEach(() => invoke.mockReset());
const request = {
  account_id: "account",
  authorization_epoch: "2",
  authorization_view: "3",
  subject_id: "notification",
  expected_activity_version: "activity",
  command_id: "same-command",
  action: "mark_read" as const,
  activity_policy: "best_effort_current_item" as const,
};
it("keeps action availability, nullable reasons and the explicit provider activity policy typed", () => {
  const value = {
    account_id: "account",
    subject_id: "notification",
    authorization_epoch: "2",
    authorization_view: "3",
    activity_version: "activity",
    actions: [
      {
        action: "mark_read",
        availability: "available",
        reason: null,
        activity_policy: "best_effort_current_item",
      },
    ],
    revision: "4",
  };
  expect(ProviderInboxActionsSnapshotSchema.parse(value)).toEqual(value);
  expect(
    QueueProviderInboxActionRequestSchema.safeParse({
      ...request,
      activity_policy: "guarded_cas",
    }).success,
  ).toBe(false);
  expect(
    QueueProviderInboxActionRequestSchema.safeParse({
      ...request,
      action: "restore_unread",
    }).success,
  ).toBe(false);
});
it("transmits exact local admission identity and returns a local receipt", async () => {
  const receipt = {
    account_id: "account",
    command_id: request.command_id,
    revision: "4",
    duplicate: true,
  };
  invoke.mockResolvedValue(receipt);
  expect(await collaborationQueueProviderInboxAction({ request })).toEqual(
    receipt,
  );
  expect(invoke).toHaveBeenCalledWith(
    "collaboration_queue_provider_inbox_action",
    { request },
  );
});

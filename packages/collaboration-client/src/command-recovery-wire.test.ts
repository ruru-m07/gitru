import {
  CommandFieldReviewSchema,
  CommandRecoveryActionRequestSchema,
  CommandRecoveryReplaceRequestSchema,
  collaborationCommandRecoveryAction,
  collaborationCommandRecoveryExport,
} from "@gitru/commands";
import { afterEach, expect, it, vi } from "vitest";

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
afterEach(() => invoke.mockReset());
const context = {
  account_id: "account",
  command_id: "command",
  expected_generation: "7",
  expected_epoch: "3",
  authorization_view: "5",
  review_token: "native-proof",
};

it("preserves unknown evidence, explicit empty body and native conflict enums", () => {
  const field = {
    field: "body",
    base: { known: true, value: null },
    remote: { known: false, value: null },
    desired: { known: true, value: "Saved text" },
    comparison: "unknown",
    editable: true,
  };
  expect(CommandFieldReviewSchema.parse(field)).toEqual(field);
  expect(
    CommandFieldReviewSchema.safeParse({
      ...field,
      comparison: "last_write_wins",
    }).success,
  ).toBe(false);
  const request = {
    context,
    action_id: "action",
    new_command_id: "replacement",
    fields: [{ field: "body", choice: "edited", value: null }],
  };
  expect(CommandRecoveryReplaceRequestSchema.parse(request)).toEqual(request);
  expect(
    CommandRecoveryReplaceRequestSchema.safeParse({
      ...request,
      fields: [{ field: "permission", choice: "edited", value: "true" }],
    }).success,
  ).toBe(false);
});

it("carries exact local-action identity and generation without permitting a generic retry command", async () => {
  const request = {
    context,
    action_id: "original-action",
    action: "pause" as const,
  };
  invoke.mockResolvedValue({
    account_id: "account",
    action_id: request.action_id,
    command_id: "command",
    replacement_id: null,
    state: "outcome_unknown",
    paused: true,
    remote_may_have_happened: true,
    revision: "8",
  });
  const receipt = await collaborationCommandRecoveryAction({ request });
  expect(receipt.remote_may_have_happened).toBe(true);
  expect(invoke).toHaveBeenCalledWith("collaboration_command_recovery_action", {
    request,
  });
  expect(
    CommandRecoveryActionRequestSchema.safeParse({
      ...request,
      action: "retry",
    }).success,
  ).toBe(false);
});

it("exports only an inspected native context, with no renderer destination or arbitrary text", async () => {
  invoke.mockResolvedValue(false);
  expect(await collaborationCommandRecoveryExport({ context })).toBe(false);
  expect(invoke).toHaveBeenCalledWith("collaboration_command_recovery_export", {
    context,
  });
});

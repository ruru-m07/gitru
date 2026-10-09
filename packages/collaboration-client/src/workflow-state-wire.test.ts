import {
  WorkflowStateRequestSchema,
  WorkflowStateSnapshotSchema,
} from "@gitru/commands";
import { afterEach, describe, expect, it, vi } from "vitest";
import {
  collaboration,
  type RemoteAccount,
  StaleAuthorizationError,
  type WorkflowStateRequest,
  type WorkflowStateSnapshot,
} from "./index";

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
afterEach(() => invoke.mockReset());

const account: RemoteAccount = {
  id: "account-a",
  actor_id: "actor-a",
  provider: "github",
  host: "github.com",
  authorization_epoch: "9007199254740993",
  login: "fixture",
  state: "active",
  display_name: null,
  notifications_supported: true,
};
const context = {
  account_id: account.id,
  subject_id: "pull-42",
  authorization_epoch: account.authorization_epoch,
  authorization_view: "11",
  review_token: "native-review-token",
};

describe("cached workflow state wire", () => {
  it("preserves available and explicit unavailable evidence", () => {
    const available = WorkflowStateSnapshotSchema.parse({
      context,
      current_state: "open",
      availability: "available",
      reason: null,
      pending_intent: null,
      revision: "20",
      authorization_view: context.authorization_view,
    });
    expect(available.current_state).toBe("open");
    expect(available.reason).toBeNull();

    const unavailable = WorkflowStateSnapshotSchema.parse({
      context: null,
      current_state: null,
      availability: "unavailable",
      reason: "merged_pull_request",
      pending_intent: null,
      revision: "21",
      authorization_view: "12",
    });
    expect(unavailable.context).toBeNull();
    expect(unavailable.reason).toBe("merged_pull_request");
    expect(
      WorkflowStateSnapshotSchema.safeParse({
        ...unavailable,
        reason: "force_reopen",
      }).success,
    ).toBe(false);
    expect(
      WorkflowStateSnapshotSchema.safeParse({
        ...unavailable,
        availability: "available",
      }).success,
    ).toBe(false);
  });

  it("requires explicit best-effort consent for a typed desired state", () => {
    const request = WorkflowStateRequestSchema.parse({
      context,
      command_id: "123e4567-e89b-42d3-a456-426614174000",
      desired_state: "closed",
      accept_best_effort: true,
    });
    expect(request.desired_state).toBe("closed");
    expect(
      WorkflowStateRequestSchema.safeParse({
        ...request,
        accept_best_effort: false,
      }).success,
    ).toBe(false);
    expect(
      WorkflowStateRequestSchema.safeParse({
        ...request,
        desired_state: "merged",
      }).success,
    ).toBe(false);
  });

  it("uses local snapshot IPC separately from durable state intent admission", async () => {
    const snapshot: WorkflowStateSnapshot = WorkflowStateSnapshotSchema.parse({
      context,
      current_state: "open",
      availability: "available",
      reason: null,
      pending_intent: null,
      revision: "20",
      authorization_view: context.authorization_view,
    });
    const request: WorkflowStateRequest = {
      context,
      command_id: "123e4567-e89b-42d3-a456-426614174000",
      desired_state: "closed",
      accept_best_effort: true,
    };
    invoke.mockResolvedValueOnce(snapshot).mockResolvedValueOnce({
      account_id: account.id,
      command_id: request.command_id,
      admitted_revision: "21",
      duplicate: false,
    });

    const client = collaboration.forAccount(account);
    expect(await client.workflowStateSnapshot(context.subject_id)).toEqual(
      snapshot,
    );
    expect(await client.submitWorkflowState(request)).toMatchObject({
      account_id: account.id,
      command_id: request.command_id,
    });
    expect(invoke.mock.calls).toEqual([
      [
        "collaboration_workflow_state_snapshot",
        { accountId: account.id, subjectId: context.subject_id },
      ],
      ["collaboration_submit_workflow_state", { request }],
    ]);
  });

  it("rejects foreign native context and receipts at the account boundary", async () => {
    const foreign = WorkflowStateSnapshotSchema.parse({
      context: { ...context, subject_id: "other" },
      current_state: "open",
      availability: "available",
      reason: null,
      pending_intent: null,
      revision: "20",
      authorization_view: context.authorization_view,
    });
    invoke.mockResolvedValue(foreign);
    await expect(
      collaboration
        .forAccount(account)
        .workflowStateSnapshot(context.subject_id),
    ).rejects.toBeInstanceOf(StaleAuthorizationError);

    invoke.mockReset();
    await expect(
      collaboration.forAccount(account).submitWorkflowState({
        context: { ...context, account_id: "other" },
        command_id: "123e4567-e89b-42d3-a456-426614174000",
        desired_state: "closed",
        accept_best_effort: true,
      }),
    ).rejects.toBeInstanceOf(StaleAuthorizationError);
    expect(invoke).not.toHaveBeenCalled();

    invoke.mockResolvedValue({
      account_id: "other",
      command_id: "123e4567-e89b-42d3-a456-426614174000",
      admitted_revision: "21",
      duplicate: false,
    });
    await expect(
      collaboration.forAccount(account).submitWorkflowState({
        context,
        command_id: "123e4567-e89b-42d3-a456-426614174000",
        desired_state: "closed",
        accept_best_effort: true,
      }),
    ).rejects.toBeInstanceOf(StaleAuthorizationError);
  });
});

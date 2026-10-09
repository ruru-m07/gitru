import {
  GuardedMergePreviewSchema,
  GuardedMergeRequestSchema,
} from "@gitru/commands";
import { afterEach, describe, expect, it, vi } from "vitest";
import {
  collaboration,
  type GuardedMergePreview,
  type GuardedMergeRequest,
  type RemoteAccount,
  StaleAuthorizationError,
} from "./index";

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
afterEach(() => invoke.mockReset());
const account: RemoteAccount = {
  id: "merge-a",
  actor_id: "actor",
  provider: "github",
  host: "github.com",
  login: "fixture",
  authorization_epoch: "7",
  state: "active",
  display_name: null,
  notifications_supported: false,
};
const context = {
  account_id: account.id,
  subject_id: "pull-1",
  authorization_epoch: "7",
  authorization_view: "9",
  expected_head: "a".repeat(40),
  grant_id: "123e4567-e89b-42d3-a456-426614174000",
};
const preview: GuardedMergePreview = {
  context,
  expected_head: context.expected_head,
  methods: ["squash"],
  can_push: true,
  mergeable: true,
  provider_mergeability: "clean",
  reason: null,
  observed_at: "2026-10-08T03:00:00Z",
  expires_in_seconds: 60,
  authorization_view: "9",
};
const request: GuardedMergeRequest = {
  context,
  command_id: "123e4567-e89b-42d3-a456-426614174001",
  method: "squash",
  confirm_inspected_head: true,
};
describe("guarded merge wire", () => {
  it("requires explicit consent and coherent provider evidence", () => {
    expect(GuardedMergePreviewSchema.parse(preview).context).toEqual(context);
    for (const change of [
      { can_push: false },
      { mergeable: null },
      { methods: [] },
      { reason: "head_changed" },
      { expected_head: "b".repeat(40) },
      { expires_in_seconds: 61 },
    ])
      expect(
        GuardedMergePreviewSchema.safeParse({ ...preview, ...change }).success,
      ).toBe(false);
    for (const consent of [false, "true", "false", 1])
      expect(
        GuardedMergeRequestSchema.safeParse({
          ...request,
          confirm_inspected_head: consent,
        }).success,
      ).toBe(false);
    expect(GuardedMergeRequestSchema.parse(request)).toEqual(request);
  });
  it("uses only explicit preview/submit IPC and binds returned account/head", async () => {
    invoke.mockResolvedValueOnce(preview).mockResolvedValueOnce({
      account_id: account.id,
      command_id: request.command_id,
      admitted_revision: "10",
      duplicate: false,
    });
    const client = collaboration.forAccount(account);
    expect(await client.previewGuardedMerge(context.subject_id)).toEqual(
      preview,
    );
    expect((await client.submitGuardedMerge(request)).duplicate).toBe(false);
    expect(invoke.mock.calls.map(([name, args]) => [name, args])).toEqual([
      [
        "collaboration_preview_guarded_merge",
        { query: { account_id: account.id, subject_id: context.subject_id } },
      ],
      ["collaboration_submit_guarded_merge", { request }],
    ]);
  });
  it("rejects foreign preview and intent without sending a merge", async () => {
    invoke.mockResolvedValue({
      ...preview,
      context: { ...context, account_id: "other" },
    });
    await expect(
      collaboration.forAccount(account).previewGuardedMerge(context.subject_id),
    ).rejects.toBeInstanceOf(StaleAuthorizationError);
    invoke.mockClear();
    await expect(
      collaboration.forAccount(account).submitGuardedMerge({
        ...request,
        context: { ...context, authorization_epoch: "6" },
      }),
    ).rejects.toBeInstanceOf(StaleAuthorizationError);
    expect(invoke).not.toHaveBeenCalled();
  });
  it("discards an online preview completed after account disconnect", async () => {
    let release!: (v: GuardedMergePreview) => void;
    invoke.mockImplementation(
      () =>
        new Promise<GuardedMergePreview>((resolve) => {
          release = resolve;
        }),
    );
    const pending = collaboration
      .forAccount(account)
      .previewGuardedMerge(context.subject_id);
    invoke.mockResolvedValue(undefined);
    await collaboration.disconnect(account.id);
    release(preview);
    await expect(pending).rejects.toBeInstanceOf(StaleAuthorizationError);
  });
});

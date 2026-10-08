import { afterEach, describe, expect, it, vi } from "vitest";
import {
  collaboration,
  type PullCreationPreview,
  type PullDraftSnapshot,
  type RemoteAccount,
  StaleAuthorizationError,
  type SubmitPullRequest,
} from "./index";

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
afterEach(() => invoke.mockReset());
const account: RemoteAccount = {
  id: "pull-account",
  actor_id: "42",
  provider: "github",
  host: "github.com",
  authorization_epoch: "9007199254740993",
  login: "writer",
  state: "active",
  display_name: null,
  notifications_supported: true,
};
const key = {
  account_id: account.id,
  draft_id: "123e4567-e89b-42d3-a456-426614174000",
  repository_id: "github:repo:77",
};
const values = {
  title: "Create from published branch",
  body: "Draft stays local",
  source_branch: "feature",
  base_branch: "main",
  local_repository_id: "clone-id",
  link_id: "link-id",
  link_generation: "3",
  is_draft: true,
};
const snapshot: PullDraftSnapshot = {
  key,
  values,
  generation: "7",
  can_preview: true,
  reason: null,
  submission: null,
  published: null,
  revision: "20",
  authorization_view: "11",
};
const context = {
  key,
  draft_generation: "7",
  authorization_epoch: account.authorization_epoch,
  authorization_view: "11",
  grant_id: "223e4567-e89b-42d3-a456-426614174000",
  source_oid: "a".repeat(40),
  base_oid: "b".repeat(40),
};
const preview: PullCreationPreview = {
  context,
  reason: null,
  values,
  local_source_oid: context.source_oid,
  observed_source_oid: context.source_oid,
  observed_base_oid: context.base_oid,
  can_push: true,
  observed_at: "2026-10-08T00:00:00Z",
  expires_in_seconds: 60,
  authorization_view: "11",
};
const request: SubmitPullRequest = {
  context,
  command_id: "323e4567-e89b-42d3-a456-426614174000",
  policy: "best_effort_current_branches",
  confirm_current_branches: true,
};

describe("pull creation wire", () => {
  it("separates local reads/save/recovery, explicit preview and exact admission", async () => {
    invoke
      .mockResolvedValueOnce(snapshot)
      .mockResolvedValueOnce(snapshot)
      .mockResolvedValueOnce({
        account_id: account.id,
        drafts: [],
        next_cursor: null,
        revision: "20",
        authorization_view: "11",
      })
      .mockResolvedValueOnce(preview)
      .mockResolvedValueOnce({
        account_id: account.id,
        command_id: request.command_id,
        admitted_revision: "21",
        duplicate: true,
      });
    const client = collaboration.forAccount(account);
    await expect(client.pullDraft(key)).resolves.toEqual(snapshot);
    await expect(
      client.savePullDraft({
        key,
        authorization_view: "11",
        expected_generation: "6",
        values,
      }),
    ).resolves.toEqual(snapshot);
    await client.pullDrafts({ cursor: null, limit: 50 });
    await expect(
      client.previewPullCreation({
        key,
        draft_generation: "7",
        authorization_view: "11",
      }),
    ).resolves.toEqual(preview);
    await expect(client.submitPull(request)).resolves.toMatchObject({
      duplicate: true,
    });
    expect(invoke.mock.calls).toEqual([
      ["collaboration_pull_draft", { key }],
      [
        "collaboration_save_pull_draft",
        {
          request: {
            key,
            authorization_epoch: account.authorization_epoch,
            authorization_view: "11",
            expected_generation: "6",
            values,
          },
        },
      ],
      [
        "collaboration_pull_drafts",
        { query: { account_id: account.id, cursor: null, limit: 50 } },
      ],
      [
        "collaboration_preview_pull_creation",
        {
          request: {
            key,
            draft_generation: "7",
            authorization_epoch: account.authorization_epoch,
            authorization_view: "11",
          },
        },
      ],
      ["collaboration_submit_pull", { request }],
    ]);
  });
  it.each([
    "account_id",
    "repository_id",
    "draft_id",
  ] as const)("rejects a response for a different %s", async (field) => {
    invoke.mockResolvedValue({
      ...snapshot,
      key: {
        ...key,
        [field]:
          field === "draft_id"
            ? "423e4567-e89b-42d3-a456-426614174000"
            : "foreign",
      },
    });
    await expect(
      collaboration.forAccount(account).pullDraft(key),
    ).rejects.toBeInstanceOf(StaleAuthorizationError);
  });
  it("rejects grants for another generation, view, epoch or observed head", async () => {
    for (const patch of [
      { draft_generation: "8" },
      { authorization_view: "12" },
      { authorization_epoch: "2" },
      { source_oid: "c".repeat(40) },
      { key: { ...key, repository_id: "foreign" } },
    ]) {
      invoke.mockResolvedValueOnce({
        ...preview,
        context: { ...context, ...patch },
      });
      await expect(
        collaboration.forAccount(account).previewPullCreation({
          key,
          draft_generation: "7",
          authorization_view: "11",
        }),
      ).rejects.toBeInstanceOf(StaleAuthorizationError);
    }
  });
  it("rejects foreign save/preview/send before native invocation", async () => {
    const client = collaboration.forAccount(account);
    const foreign = { ...key, account_id: "foreign" };
    await expect(
      client.savePullDraft({
        key: foreign,
        authorization_view: "11",
        expected_generation: "7",
        values,
      }),
    ).rejects.toBeInstanceOf(StaleAuthorizationError);
    await expect(
      client.previewPullCreation({
        key: foreign,
        authorization_view: "11",
        draft_generation: "7",
      }),
    ).rejects.toBeInstanceOf(StaleAuthorizationError);
    await expect(
      client.submitPull({ ...request, context: { ...context, key: foreign } }),
    ).rejects.toBeInstanceOf(StaleAuthorizationError);
    expect(invoke).not.toHaveBeenCalled();
  });
});

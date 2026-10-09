import {
  IssueDraftPageSchema,
  IssueDraftSnapshotSchema,
  SubmitIssueRequestSchema,
} from "@gitru/commands";
import { afterEach, describe, expect, it, vi } from "vitest";
import {
  collaboration,
  type IssueDraftSnapshot,
  type RemoteAccount,
  StaleAuthorizationError,
  type SubmitIssueRequest,
} from "./index";

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
afterEach(() => invoke.mockReset());

const account: RemoteAccount = {
  id: "account-issue-create",
  actor_id: "actor-issue-create",
  provider: "github",
  host: "github.com",
  authorization_epoch: "9007199254740993",
  login: "fixture",
  state: "active",
  display_name: null,
  notifications_supported: true,
};
const key = {
  account_id: account.id,
  draft_id: "123e4567-e89b-42d3-a456-426614174000",
  repository_id: "github:repo:77",
};
const context = {
  account_id: account.id,
  repository_id: key.repository_id,
  authorization_epoch: account.authorization_epoch,
  authorization_view: "11",
  review_token: "a".repeat(64),
};
const snapshot = (): IssueDraftSnapshot =>
  IssueDraftSnapshotSchema.parse({
    ...key,
    title: "Saved issue",
    body: "Authored locally",
    generation: "7",
    context,
    availability: "available",
    reason: null,
    submission: null,
    published: null,
    revision: "20",
    authorization_view: context.authorization_view,
  });

describe("durable issue creation wire", () => {
  it("keeps pending and published evidence distinct from submission authority", () => {
    const pending = IssueDraftSnapshotSchema.parse({
      ...snapshot(),
      context: null,
      availability: "unavailable",
      reason: "pending_submission",
      submission: {
        command_id: "223e4567-e89b-42d3-a456-426614174000",
        draft_generation: "6",
        state: "unknown",
        attempt_count: 1,
        quarantined: false,
        attention: null,
      },
    });
    expect(pending.published).toBeNull();
    expect(
      IssueDraftSnapshotSchema.safeParse({
        ...pending,
        availability: "available",
      }).success,
    ).toBe(false);
    const published = IssueDraftSnapshotSchema.parse({
      ...pending,
      submission: {
        ...pending.submission!,
        draft_generation: "7",
        state: "confirmed",
      },
      reason: "already_submitted",
      published: {
        subject_id: "github:issue:99",
        provider_id: "99",
        number: "12",
        url: "https://github.com/owner/project/issues/12",
        command_id: "223e4567-e89b-42d3-a456-426614174000",
      },
    });
    expect(published.published?.subject_id).toBe("github:issue:99");
  });

  it("uses separate cache-only draft, CAS save, admission and recovery commands", async () => {
    const saved = { ...snapshot(), title: "Updated issue", generation: "8" };
    const request: SubmitIssueRequest = SubmitIssueRequestSchema.parse({
      context,
      draft_id: key.draft_id,
      draft_generation: saved.generation,
      command_id: "223e4567-e89b-42d3-a456-426614174000",
      accept_background_delivery: true,
    });
    const page = IssueDraftPageSchema.parse({
      account_id: account.id,
      drafts: [
        {
          draft_id: key.draft_id,
          repository_id: key.repository_id,
          title: saved.title,
          preview: saved.body,
          generation: saved.generation,
          submission: null,
        },
      ],
      next_cursor: null,
      revision: "22",
      authorization_view: context.authorization_view,
    });
    invoke
      .mockResolvedValueOnce(snapshot())
      .mockResolvedValueOnce(saved)
      .mockResolvedValueOnce({
        account_id: account.id,
        command_id: request.command_id,
        admitted_revision: "21",
        duplicate: false,
      })
      .mockResolvedValueOnce(page);
    const client = collaboration.forAccount(account);
    expect(
      await client.issueDraft({
        draft_id: key.draft_id,
        repository_id: key.repository_id,
      }),
    ).toEqual(snapshot());
    expect(
      await client.saveIssueDraft({
        draft_id: key.draft_id,
        repository_id: key.repository_id,
        authorization_view: context.authorization_view,
        expected_generation: "7",
        title: saved.title,
        body: saved.body,
      }),
    ).toEqual(saved);
    expect(await client.submitIssue(request)).toMatchObject({
      command_id: request.command_id,
    });
    expect(await client.issueDrafts({ cursor: null, limit: 50 })).toEqual(page);
    expect(invoke.mock.calls).toEqual([
      ["collaboration_issue_draft", { key }],
      [
        "collaboration_save_issue_draft",
        {
          request: {
            ...key,
            authorization_epoch: account.authorization_epoch,
            authorization_view: context.authorization_view,
            expected_generation: "7",
            title: saved.title,
            body: saved.body,
          },
        },
      ],
      ["collaboration_submit_issue", { request }],
      [
        "collaboration_issue_drafts",
        { query: { account_id: account.id, cursor: null, limit: 50 } },
      ],
    ]);
  });

  it("rejects foreign draft, repository, context and recovery identities", async () => {
    const client = collaboration.forAccount(account);
    invoke.mockResolvedValueOnce({ ...snapshot(), repository_id: "other" });
    await expect(
      client.issueDraft({
        draft_id: key.draft_id,
        repository_id: key.repository_id,
      }),
    ).rejects.toBeInstanceOf(StaleAuthorizationError);
    invoke.mockReset();
    await expect(
      client.submitIssue({
        context: { ...context, account_id: "other" },
        draft_id: key.draft_id,
        draft_generation: "7",
        command_id: "223e4567-e89b-42d3-a456-426614174000",
        accept_background_delivery: true,
      }),
    ).rejects.toBeInstanceOf(StaleAuthorizationError);
    expect(invoke).not.toHaveBeenCalled();
    invoke.mockResolvedValue({
      account_id: "other",
      drafts: [],
      next_cursor: null,
      revision: "22",
      authorization_view: context.authorization_view,
    });
    await expect(
      client.issueDrafts({ cursor: null, limit: 50 }),
    ).rejects.toBeInstanceOf(StaleAuthorizationError);
  });
});

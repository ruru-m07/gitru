import {
  CommentDraftPageSchema,
  CommentDraftSnapshotSchema,
  CreatedCommentPageSchema,
  SendCommentRequestSchema,
} from "@gitru/commands";
import { afterEach, describe, expect, it, vi } from "vitest";
import {
  type CommentDraftSnapshot,
  collaboration,
  type RemoteAccount,
  type SendCommentRequest,
  StaleAuthorizationError,
} from "./index";

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
afterEach(() => invoke.mockReset());

const account: RemoteAccount = {
  id: "account-comment",
  actor_id: "actor-comment",
  provider: "github",
  host: "https://github.com",
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
  review_token: "native-comment-review-token",
};
const snapshot = (): CommentDraftSnapshot =>
  CommentDraftSnapshotSchema.parse({
    account_id: account.id,
    subject_id: context.subject_id,
    body: "Saved comment",
    generation: "7",
    context,
    availability: "available",
    reason: null,
    submission: null,
    revision: "20",
    authorization_view: context.authorization_view,
  });

describe("dedicated comment draft and submission wire", () => {
  it("keeps availability, pending evidence and explicit background consent coherent", () => {
    expect(snapshot().context).toEqual(context);
    const unavailable = CommentDraftSnapshotSchema.parse({
      ...snapshot(),
      context: null,
      availability: "unavailable",
      reason: "pending_submission",
      submission: {
        command_id: "123e4567-e89b-42d3-a456-426614174000",
        draft_generation: "7",
        state: "unknown",
        attempt_count: 1,
        quarantined: false,
        attention: "Review the saved change",
      },
    });
    expect(unavailable.submission?.state).toBe("unknown");
    expect(
      CommentDraftSnapshotSchema.safeParse({
        ...unavailable,
        availability: "available",
      }).success,
    ).toBe(false);
    const request = SendCommentRequestSchema.parse({
      context,
      draft_generation: "7",
      command_id: "123e4567-e89b-42d3-a456-426614174000",
      accept_background_delivery: true,
    });
    expect(request.accept_background_delivery).toBe(true);
    expect(
      SendCommentRequestSchema.safeParse({
        ...request,
        accept_background_delivery: false,
      }).success,
    ).toBe(false);
  });

  it("uses separate local draft, save, admission and confirmed-history commands", async () => {
    const saved = { ...snapshot(), body: "Updated comment", generation: "8" };
    const request: SendCommentRequest = {
      context: saved.context!,
      draft_generation: saved.generation,
      command_id: "123e4567-e89b-42d3-a456-426614174000",
      accept_background_delivery: true,
    };
    const page = CreatedCommentPageSchema.parse({
      account_id: account.id,
      subject_id: context.subject_id,
      comments: [],
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
    expect(await client.commentDraft(context.subject_id)).toEqual(snapshot());
    expect(
      await client.saveCommentDraft({
        subject_id: context.subject_id,
        authorization_view: context.authorization_view,
        expected_generation: "7",
        body: saved.body,
      }),
    ).toEqual(saved);
    expect(await client.sendComment(request)).toMatchObject({
      command_id: request.command_id,
    });
    expect(
      await client.createdComments({
        subject_id: context.subject_id,
        cursor: null,
        limit: 25,
      }),
    ).toEqual(page);
    expect(invoke.mock.calls).toEqual([
      [
        "collaboration_comment_draft",
        { accountId: account.id, subjectId: context.subject_id },
      ],
      [
        "collaboration_save_comment_draft",
        {
          request: {
            account_id: account.id,
            subject_id: context.subject_id,
            authorization_epoch: account.authorization_epoch,
            authorization_view: context.authorization_view,
            expected_generation: "7",
            body: saved.body,
          },
        },
      ],
      ["collaboration_send_comment", { request }],
      [
        "collaboration_created_comments",
        {
          query: {
            account_id: account.id,
            subject_id: context.subject_id,
            cursor: null,
            limit: 25,
          },
        },
      ],
    ]);
  });

  it("rejects foreign comment context and created-history identity", async () => {
    invoke.mockResolvedValueOnce({
      ...snapshot(),
      subject_id: "other",
    });
    const client = collaboration.forAccount(account);
    await expect(
      client.commentDraft(context.subject_id),
    ).rejects.toBeInstanceOf(StaleAuthorizationError);
    invoke.mockReset();
    await expect(
      client.sendComment({
        context: { ...context, account_id: "other" },
        draft_generation: "7",
        command_id: "123e4567-e89b-42d3-a456-426614174000",
        accept_background_delivery: true,
      }),
    ).rejects.toBeInstanceOf(StaleAuthorizationError);
    expect(invoke).not.toHaveBeenCalled();
    invoke.mockResolvedValue({
      account_id: account.id,
      subject_id: "other",
      comments: [],
      next_cursor: null,
      revision: "22",
      authorization_view: context.authorization_view,
    });
    await expect(
      client.createdComments({
        subject_id: context.subject_id,
        cursor: null,
        limit: 25,
      }),
    ).rejects.toBeInstanceOf(StaleAuthorizationError);
  });

  it("lists dedicated comment drafts through a bounded local recovery query", async () => {
    const page = CommentDraftPageSchema.parse({
      account_id: account.id,
      drafts: [
        {
          subject_id: context.subject_id,
          preview: "Saved comment",
          generation: "7",
        },
      ],
      next_cursor: null,
      revision: "20",
      authorization_view: context.authorization_view,
    });
    invoke.mockResolvedValueOnce(page);
    expect(
      await collaboration.forAccount(account).commentDrafts({
        cursor: null,
        limit: 50,
      }),
    ).toEqual(page);
    expect(invoke).toHaveBeenCalledWith("collaboration_comment_drafts", {
      query: { account_id: account.id, cursor: null, limit: 50 },
    });
    invoke.mockReset();
    invoke.mockResolvedValue({ ...page, account_id: "other" });
    await expect(
      collaboration
        .forAccount(account)
        .commentDrafts({ cursor: null, limit: 50 }),
    ).rejects.toBeInstanceOf(StaleAuthorizationError);
  });
});

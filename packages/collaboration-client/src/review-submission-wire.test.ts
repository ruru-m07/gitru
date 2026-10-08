import { afterEach, describe, expect, it, vi } from "vitest";
import {
  collaboration,
  type RemoteAccount,
  type ReviewDraftSnapshot,
  StaleAuthorizationError,
  type SubmitReviewRequest,
} from "./index";

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
afterEach(() => invoke.mockReset());
const account: RemoteAccount = {
  id: "review-account",
  actor_id: "42",
  provider: "github",
  host: "github.com",
  authorization_epoch: "9007199254740993",
  login: "writer",
  state: "active",
  display_name: null,
  notifications_supported: true,
};
const key = { account_id: account.id, subject_id: "pull-77" };
const context = {
  ...key,
  authorization_epoch: account.authorization_epoch,
  authorization_view: "11",
  review_token: "c".repeat(64),
  review_context: {
    base_oid: "a".repeat(40),
    head_oid: "b".repeat(40),
    base_repository_provider_id: "77",
    source_repository_provider_id: "77",
    metadata_facet_revision: "8",
  },
};
const snapshot: ReviewDraftSnapshot = {
  key,
  event: "comment",
  body: "Review stays local",
  comments: [],
  generation: "7",
  context,
  availability: "available",
  reason: null,
  submission: null,
  revision: "20",
  authorization_view: "11",
};
const request: SubmitReviewRequest = {
  context,
  draft_generation: "7",
  command_id: "323e4567-e89b-42d3-a456-426614174000",
  accept_background_delivery: true,
  accept_best_effort_race: true,
};
describe("review submission wire", () => {
  it("routes five explicit local commands and preserves decimal epochs", async () => {
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
      .mockResolvedValueOnce({
        account_id: account.id,
        command_id: request.command_id,
        admitted_revision: "21",
        duplicate: true,
      })
      .mockResolvedValueOnce({
        ...key,
        reviews: [],
        next_cursor: null,
        revision: "21",
        authorization_view: "11",
      });
    const client = collaboration.forAccount(account);
    expect(await client.reviewDraft(key.subject_id)).toEqual(snapshot);
    await client.saveReviewDraft({
      key,
      authorization_view: "11",
      expected_generation: "6",
      event: "comment",
      body: snapshot.body,
      comments: [],
    });
    await client.reviewDrafts({ cursor: null, limit: 50 });
    expect(await client.submitReview(request)).toMatchObject({
      duplicate: true,
    });
    await client.submittedReviews({
      subject_id: key.subject_id,
      cursor: null,
      limit: 25,
    });
    expect(invoke.mock.calls.map(([name]) => name)).toEqual([
      "collaboration_review_draft",
      "collaboration_save_review_draft",
      "collaboration_review_drafts",
      "collaboration_submit_review",
      "collaboration_submitted_reviews",
    ]);
    expect(invoke.mock.calls[1][1]).toMatchObject({
      request: { key, authorization_epoch: account.authorization_epoch },
    });
    expect(invoke.mock.calls[3][1]).toEqual({ request });
  });
  it.each([
    "account_id",
    "subject_id",
  ] as const)("rejects a draft for another %s", async (field) => {
    invoke.mockResolvedValue({
      ...snapshot,
      key: { ...key, [field]: "foreign" },
    });
    await expect(
      collaboration.forAccount(account).reviewDraft(key.subject_id),
    ).rejects.toBeInstanceOf(StaleAuthorizationError);
  });
  it("rejects mismatched epoch/view/subject authority in a response", async () => {
    for (const patch of [
      { authorization_epoch: "2" },
      { authorization_view: "12" },
      { subject_id: "foreign" },
      { account_id: "foreign" },
    ]) {
      invoke.mockResolvedValueOnce({
        ...snapshot,
        context: { ...context, ...patch },
      });
      await expect(
        collaboration.forAccount(account).reviewDraft(key.subject_id),
      ).rejects.toBeInstanceOf(StaleAuthorizationError);
    }
  });
  it("refuses a foreign save or submit before native invocation", async () => {
    const client = collaboration.forAccount(account);
    await expect(
      client.saveReviewDraft({
        key: { ...key, account_id: "foreign" },
        authorization_view: "11",
        expected_generation: "7",
        event: "approve",
        body: "",
        comments: [],
      }),
    ).rejects.toBeInstanceOf(StaleAuthorizationError);
    await expect(
      client.submitReview({
        ...request,
        context: { ...context, authorization_epoch: "2" },
      }),
    ).rejects.toBeInstanceOf(StaleAuthorizationError);
    expect(invoke).not.toHaveBeenCalled();
  });
  it("rejects a foreign exact receipt and provider history", async () => {
    invoke.mockResolvedValueOnce({
      account_id: account.id,
      command_id: "other",
      admitted_revision: "21",
      duplicate: false,
    });
    await expect(
      collaboration.forAccount(account).submitReview(request),
    ).rejects.toBeInstanceOf(StaleAuthorizationError);
    invoke.mockResolvedValueOnce({
      ...key,
      subject_id: "other",
      reviews: [],
      next_cursor: null,
      revision: "21",
      authorization_view: "11",
    });
    await expect(
      collaboration.forAccount(account).submittedReviews({
        subject_id: key.subject_id,
        cursor: null,
        limit: 25,
      }),
    ).rejects.toBeInstanceOf(StaleAuthorizationError);
  });
});

import {
  IssueDraftV2SnapshotSchema,
  IssueMetadataPageSchema,
  IssueMetadataReferenceSchema,
  SaveIssueDraftV2RequestSchema,
  SubmitIssueV2RequestSchema,
} from "@gitru/commands";
import { afterEach, expect, it, vi } from "vitest";
import {
  collaboration,
  type IssueDraftV2Snapshot,
  type RemoteAccount,
} from "./index";

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
afterEach(() => invoke.mockReset());
const account: RemoteAccount = {
  id: "account",
  actor_id: "7",
  provider: "github",
  host: "github.com",
  authorization_epoch: "9007199254740993",
  state: "active",
  login: "fixture",
  display_name: null,
  notifications_supported: true,
};
const key = {
  account_id: account.id,
  repository_id: "repo",
  draft_id: "123e4567-e89b-42d3-a456-426614174000",
};
const context = {
  account_id: account.id,
  repository_id: key.repository_id,
  authorization_epoch: account.authorization_epoch,
  authorization_view: "11",
  review_token: "a".repeat(64),
};
const metadata = {
  labels: [{ provider_id: "9007199254740995", name: "bug", color: null }],
  assignees: [{ provider_id: "7", login: "fixture" }],
  milestone: null,
};
const snapshot = (): IssueDraftV2Snapshot => ({
  draft: {
    ...key,
    title: "Saved",
    body: "Authored",
    generation: "1",
    context,
    availability: "available",
    reason: null,
    submission: null,
    published: null,
    revision: "20",
    authorization_view: "11",
  },
  metadata,
  metadata_outcome: null,
});

it("preserves the native adjacent-tag references, nullable options and lowercase states", () => {
  for (const reference of [
    { kind: "label", value: metadata.labels[0] },
    { kind: "assignee", value: metadata.assignees[0] },
    {
      kind: "milestone",
      value: { provider_id: "99", number: "3", title: "Release" },
    },
  ])
    expect(IssueMetadataReferenceSchema.parse(reference)).toEqual(reference);
  expect(IssueMetadataReferenceSchema.safeParse("Label").success).toBe(false);
  expect(
    IssueMetadataReferenceSchema.safeParse({
      kind: "label",
      value: metadata.assignees[0],
    }).success,
  ).toBe(false);
  const page = {
    account_id: account.id,
    repository_id: key.repository_id,
    kind: "labels",
    options: [
      {
        reference: { kind: "label", value: metadata.labels[0] },
        availability: "unknown",
        reason: "unobserved",
      },
    ],
    next_cursor: null,
    coverage: { state: "missing", validated_at: null, remote_has_more: false },
    freshness: "unknown",
    sync: {
      state: "idle",
      last_success_at: null,
      next_retry_at: null,
      error: null,
    },
    revision: "20",
    authorization_view: "11",
    catalog_revision: null,
  };
  expect(IssueMetadataPageSchema.parse(page)).toEqual(page);
  expect(IssueDraftV2SnapshotSchema.parse(snapshot())).toEqual(snapshot());
});

it("uses real generated V2 commands and preserves metadata, false consent and opaque counters", async () => {
  const save = SaveIssueDraftV2RequestSchema.parse({
    ...key,
    authorization_epoch: account.authorization_epoch,
    authorization_view: "11",
    expected_generation: "0",
    title: "Saved",
    body: "Authored",
    metadata,
  });
  const submit = SubmitIssueV2RequestSchema.parse({
    context,
    draft_id: key.draft_id,
    draft_generation: "1",
    command_id: "223e4567-e89b-42d3-a456-426614174000",
    accept_background_delivery: true,
    accept_metadata_best_effort: false,
  });
  invoke
    .mockResolvedValueOnce(snapshot())
    .mockResolvedValueOnce(snapshot())
    .mockResolvedValueOnce({
      account_id: account.id,
      command_id: submit.command_id,
      admitted_revision: "21",
      duplicate: false,
    });
  await collaboration.forAccount(account).issueDraftV2(key);
  await collaboration.forAccount(account).saveIssueDraftV2(save);
  await collaboration.forAccount(account).submitIssueV2(submit);
  expect(invoke.mock.calls).toEqual([
    ["collaboration_issue_draft_v2", { key }],
    ["collaboration_save_issue_draft_v2", { request: save }],
    ["collaboration_submit_issue_v2", { request: submit }],
  ]);
  expect(
    SubmitIssueV2RequestSchema.safeParse({
      ...submit,
      accept_metadata_best_effort: undefined,
    }).success,
  ).toBe(false);
  expect(
    SubmitIssueV2RequestSchema.safeParse({
      ...submit,
      accept_metadata_best_effort: "false",
    }).success,
  ).toBe(false);
  expect(
    SubmitIssueV2RequestSchema.safeParse({
      ...submit,
      accept_background_delivery: "false",
    }).success,
  ).toBe(false);
});

it.each([
  "queued",
  "unknown",
] as const)("preserves a same-generation %s submission without claiming completed creation", (state) => {
  const pending = snapshot();
  pending.draft.context = null;
  pending.draft.availability = "unavailable";
  pending.draft.reason = "pending_submission";
  pending.draft.submission = {
    command_id: "pending",
    draft_generation: pending.draft.generation,
    state,
    attempt_count: 1,
    quarantined: false,
    attention: null,
  };
  expect(IssueDraftV2SnapshotSchema.parse(pending)).toEqual(pending);
  expect(
    IssueDraftV2SnapshotSchema.safeParse({
      ...pending,
      draft: { ...pending.draft, reason: "already_submitted" },
    }).success,
  ).toBe(false);
});

it("preserves a canonical historical publication independently of a newer pending submission", () => {
  const saved = snapshot();
  saved.draft.context = null;
  saved.draft.availability = "unavailable";
  saved.draft.reason = "pending_submission";
  saved.draft.submission = {
    command_id: "pending",
    draft_generation: "1",
    state: "unknown",
    attempt_count: 1,
    quarantined: false,
    attention: null,
  };
  saved.draft.published = {
    command_id: "historical",
    subject_id: "issue",
    provider_id: "9007199254740997",
    number: "4",
    url: "https://github.com/owner/repo/issues/4",
  };
  saved.metadata_outcome = {
    command_id: "historical",
    fields: [{ field: "labels", result: "unobserved", reason: "malformed" }],
    needs_attention: true,
  };
  expect(IssueDraftV2SnapshotSchema.parse(saved)).toEqual(saved);
});

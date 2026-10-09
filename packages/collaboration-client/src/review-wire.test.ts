import {
  collaborationDetail,
  collaborationHydrateDetail,
  DetailEntrySchema,
  NativeDetailPayloadSchema,
  ReviewThreadV1Schema,
  ReviewV1Schema,
} from "@gitru/commands";
import { afterEach, describe, expect, it, vi } from "vitest";

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
afterEach(() => invoke.mockReset());

const baseOid = "b".repeat(40);
const headOid = "a".repeat(40);
const context = {
  base_oid: baseOid,
  head_oid: headOid,
  base_repository_provider_id: "target-1",
  source_repository_provider_id: "source-1",
  metadata_facet_revision: "10",
};

const review = {
  context,
  reviewer: null,
  decision: "unknown" as const,
  provider_state: "FUTURE_PROVIDER_DECISION",
  reviewed_commit_oid: null,
  submitted_at: null,
};

const thread = {
  context,
  thread_id: "discussion-1",
  root_comment_id: null,
  comment_id: "note-1",
  parent_comment_id: null,
  review_id: null,
  author: null,
  created_at: "2026-10-08T00:00:00Z",
  updated_at: "2026-10-08T00:01:00Z",
  anchor: null,
  provider_outdated: null,
  provider_resolved: false,
  native: null,
};

function validation(field: string) {
  return {
    field,
    validated_at: "2026-10-08T00:02:00Z",
    source: "fixture/reviews/v1",
    adapter_version: 1,
  };
}

function entry(
  kind: "review.v1" | "review_thread.v1",
  value: typeof review | typeof thread,
) {
  const fields =
    kind === "review.v1"
      ? ["body", "author", "state", "updated_at", "head_oid", "review"]
      : ["body", "author", "updated_at", "head_oid", "review_thread"];
  return {
    id: `${kind}:1`,
    provider_id: "1",
    author: null,
    title: null,
    state: kind === "review.v1" ? "FUTURE_PROVIDER_DECISION" : null,
    body: { state: "known" as const, text: "Saved as text" },
    observed_body_state: "known" as const,
    updated_at: "2026-10-08T00:01:00Z",
    head_oid: headOid,
    native: { kind, value },
    field_mask: fields,
    field_validations: fields.map(validation),
  };
}

describe("generated native review wire families", () => {
  it("preserves unknown decisions, nullable commit identity, and explicit provider flags", () => {
    expect(ReviewV1Schema.parse(review)).toEqual(review);
    expect(ReviewThreadV1Schema.parse(thread)).toEqual(thread);
    expect(
      NativeDetailPayloadSchema.parse({ kind: "review.v1", value: review }),
    ).toEqual({ kind: "review.v1", value: review });
    expect(
      NativeDetailPayloadSchema.parse({
        kind: "review_thread.v1",
        value: thread,
      }),
    ).toEqual({ kind: "review_thread.v1", value: thread });
  });

  it("keeps review and thread field evidence in separate native families", () => {
    const summary = entry("review.v1", review);
    const discussion = entry("review_thread.v1", thread);
    expect(DetailEntrySchema.parse(summary).native?.kind).toBe("review.v1");
    expect(DetailEntrySchema.parse(discussion).native?.kind).toBe(
      "review_thread.v1",
    );
    for (const bad of [
      { ...summary, field_mask: summary.field_mask.slice(0, -1) },
      {
        ...discussion,
        field_validations: discussion.field_validations.slice(0, -1),
      },
      { ...summary, field_mask: [...summary.field_mask, "review_thread"] },
      { ...discussion, field_mask: [...discussion.field_mask, "review"] },
      { ...summary, field_mask: [...summary.field_mask, "review"] },
      {
        ...discussion,
        field_validations: [
          ...discussion.field_validations,
          validation("review_thread"),
        ],
      },
    ])
      expect(DetailEntrySchema.safeParse(bad).success).toBe(false);
  });

  it("keeps each 50-row saved read separate from explicit epoch-scoped hydration", async () => {
    const query = {
      account_id: "actor",
      subject_id: "repository:67",
      facet: "review_threads" as const,
      cursor: null,
      limit: 50,
    };
    const request = {
      account_id: "actor",
      authorization_epoch: "9007199254740993",
      subject_id: query.subject_id,
      facet: "review_threads" as const,
    };
    invoke.mockResolvedValue({});
    await collaborationDetail({ query });
    await collaborationHydrateDetail({ request });
    expect(invoke.mock.calls).toEqual([
      ["collaboration_detail", { query }],
      ["collaboration_hydrate_detail", { request }],
    ]);
  });
});

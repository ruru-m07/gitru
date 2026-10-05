import {
  collaborationDetail,
  collaborationHydrateDetail,
  DetailEntrySchema,
  NativeDetailPayloadSchema,
} from "@gitru/commands";
import { afterEach, describe, expect, it, vi } from "vitest";

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
afterEach(() => invoke.mockReset());

const fields = [
  "task_content",
  "task_creator_login",
  "task_creator_display_name",
  "task_state",
  "task_created_at",
  "task_updated_at",
  "task_pending",
  "task_resolved_at",
  "task_resolver",
  "task_resolver_login",
  "task_resolver_display_name",
  "task_comment_id",
];
const native = {
  kind: "task.v1",
  value: {
    content: { state: "known", text: "" },
    observed_content_state: "omitted",
    creator: {
      provider_id: "44444444-4444-4444-8444-444444444444",
      kind: "future_actor",
      login: null,
      display_name: null,
    },
    state: "<future-task-state>",
    created_at: "2026-10-03T00:00:00Z",
    updated_at: "2026-10-04T00:00:00Z",
    pending: false,
    resolved_at: null,
    resolved_by: null,
    comment_id: "9007199254740993",
  },
};
function validation(field: string) {
  return {
    field,
    validated_at: "2026-10-04T00:00:00Z",
    source: "bitbucket.tasks.v1",
    adapter_version: 1,
  };
}
const entry = {
  id: "bitbucket_cloud:task:33333333-3333-4333-8333-333333333333:67:9007199254740993",
  provider_id: "33333333-3333-4333-8333-333333333333:67:9007199254740993",
  author: null,
  title: null,
  state: null,
  body: { state: "not_loaded", text: null },
  observed_body_state: "not_loaded",
  updated_at: null,
  head_oid: null,
  native,
  field_mask: fields,
  field_validations: fields.map(validation),
};

describe("generated native Task wire family", () => {
  it("preserves all twelve Task fields, opaque future actor/state, known empty, nullable resolver and pending false", () => {
    const parsed = DetailEntrySchema.parse(JSON.parse(JSON.stringify(entry)));
    expect(parsed.native?.kind).toBe("task.v1");
    if (parsed.native?.kind !== "task.v1")
      throw new Error("Expected typed task");
    expect(parsed.native.value.content.text).toBe("");
    expect(parsed.native.value.observed_content_state).toBe("omitted");
    expect(parsed.native.value.pending).toBe(false);
    expect(parsed.native.value.resolved_by).toBeNull();
    expect(parsed.native.value.creator.kind).toBe("future_actor");
    expect(parsed.native.value.state).toBe("<future-task-state>");
    expect(parsed.native.value.comment_id).toBe("9007199254740993");
    expect(parsed.field_mask).toHaveLength(12);
    expect(parsed.field_validations).toHaveLength(12);
    for (const bad of [
      "task.v1",
      { kind: "task.v2", value: native.value },
      { kind: "task.v1" },
      { ...native, value: { ...native.value, pending: "false" } },
      { ...native, value: { ...native.value, pending: 0 } },
      { ...native, value: { ...native.value, creator: null } },
      { ...native, value: { ...native.value, content: "raw-string" } },
    ])
      expect(NativeDetailPayloadSchema.safeParse(bad).success).toBe(false);
  });

  it("preserves unobserved pending and current oversized state independently of retained known content/validation", () => {
    const result = DetailEntrySchema.parse({
      ...entry,
      native: {
        ...native,
        value: {
          ...native.value,
          content: { state: "known", text: "Earlier saved task text" },
          observed_content_state: "oversized",
          pending: null,
          resolved_by: {
            provider_id: "55555555-5555-4555-8555-555555555555",
            kind: "app_user",
            login: null,
            display_name: null,
          },
        },
      },
      field_mask: ["task_updated_at", "task_resolver"],
      field_validations: [
        validation("task_content"),
        validation("task_updated_at"),
        validation("task_resolver"),
      ],
    });
    if (result.native?.kind !== "task.v1")
      throw new Error("Expected typed task");
    expect(result.native.value.pending).toBeNull();
    expect(result.native.value.observed_content_state).toBe("oversized");
    expect(result.native.value.content.text).toBe("Earlier saved task text");
    expect(result.field_mask).not.toContain("task_content");
    expect(result.field_validations.map((proof) => proof.field)).toContain(
      "task_content",
    );
    expect(result.field_validations.map((proof) => proof.field)).not.toContain(
      "task_resolver_login",
    );
  });

  it.each([
    "mask",
    "validation",
  ] as const)("rejects Task, participant and generic cross-family %s evidence", (evidence) => {
    const participant = {
      kind: "participant.v1",
      value: {
        user: {
          provider_id: "44444444-4444-4444-8444-444444444444",
          login: null,
          display_name: null,
        },
        role: null,
        approved: false,
        state: null,
        participated_at: null,
      },
    };
    for (const [payload, own, foreign] of [
      [native, "task_pending", "body"],
      [native, "task_pending", "participant_approved"],
      [participant, "participant_approved", "task_pending"],
      [null, "body", "task_pending"],
    ] as const) {
      expect(
        DetailEntrySchema.safeParse({
          ...entry,
          native: payload,
          field_mask: [evidence === "mask" ? foreign : own],
          field_validations: [
            validation(evidence === "validation" ? foreign : own),
          ],
        }).success,
      ).toBe(false);
    }
  });

  it("rejects duplicate or unknown authority rather than enlarging the three field families", () => {
    expect(
      DetailEntrySchema.safeParse({
        ...entry,
        field_mask: [...fields, "task_pending"],
      }).success,
    ).toBe(false);
    expect(
      DetailEntrySchema.safeParse({
        ...entry,
        field_validations: [
          ...entry.field_validations,
          validation("task_pending"),
        ],
      }).success,
    ).toBe(false);
    expect(
      DetailEntrySchema.safeParse({
        ...entry,
        field_mask: ["task_future_field"],
      }).success,
    ).toBe(false);
  });

  it("invokes a 50-row saved cursor read independently from explicit epoch-scoped Task sync", async () => {
    const query = {
      account_id: "actor",
      subject_id: "compound-repository-uuid:67",
      facet: "tasks" as const,
      cursor: "opaque-local-cursor",
      limit: 50,
    };
    const request = {
      account_id: "actor",
      authorization_epoch: "9007199254740993",
      subject_id: query.subject_id,
      facet: "tasks" as const,
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

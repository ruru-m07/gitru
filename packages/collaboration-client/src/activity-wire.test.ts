import {
  ActivityEventSchema,
  collaborationDetail,
  collaborationHydrateDetail,
  DetailEntrySchema,
  NativeDetailPayloadSchema,
} from "@gitru/commands";
import { afterEach, describe, expect, it, vi } from "vitest";

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
afterEach(() => invoke.mockReset());

const observedAt = "2026-10-08T00:00:00Z";
const native = {
  kind: "activity.v1" as const,
  value: {
    kind: "future_provider_event",
    supported: false,
    occurred_at: null,
    description: null,
  },
};

function entry() {
  return {
    id: "github:activity:9007199254740993",
    provider_id: "9007199254740993",
    author: null,
    title: "Unknown saved event",
    state: null,
    body: { state: "omitted" as const, text: null },
    observed_body_state: "omitted" as const,
    updated_at: observedAt,
    head_oid: null,
    native,
    field_mask: ["activity", "body", "updated_at"],
    field_validations: ["activity", "body", "updated_at"].map((field) => ({
      field,
      validated_at: observedAt,
      source: "github.activity.v1",
      adapter_version: 1,
    })),
  };
}

describe("generated native Activity wire family", () => {
  it("preserves unknown provider kinds, false support, and nullable evidence", () => {
    expect(ActivityEventSchema.parse(native.value)).toEqual(native.value);
    expect(NativeDetailPayloadSchema.parse(native)).toEqual(native);
    const parsed = DetailEntrySchema.parse(entry());
    expect(parsed.native?.kind).toBe("activity.v1");
    if (parsed.native?.kind !== "activity.v1")
      throw new Error("Expected typed activity payload");
    expect(parsed.native.value.supported).toBe(false);
    expect(parsed.native.value.occurred_at).toBeNull();
    expect(parsed.native.value.description).toBeNull();
  });

  it("rejects malformed values and evidence from another detail family", () => {
    for (const bad of [
      { ...native.value, supported: "false" },
      { ...native.value, supported: 0 },
      { ...native.value, occurred_at: 1 },
      { ...native.value, description: false },
      { ...native.value, kind: null },
    ])
      expect(ActivityEventSchema.safeParse(bad).success).toBe(false);
    for (const bad of [
      { ...entry(), field_mask: ["activity", "head_oid"] },
      {
        ...entry(),
        field_validations: [
          {
            field: "review",
            validated_at: observedAt,
            source: "github.activity.v1",
            adapter_version: 1,
          },
        ],
      },
    ])
      expect(DetailEntrySchema.safeParse(bad).success).toBe(false);
  });

  it("keeps the local read separate from explicit epoch-scoped hydration", async () => {
    const query = {
      account_id: "actor",
      subject_id: "github:pull:1:67",
      facet: "activity" as const,
      cursor: null,
      limit: 50,
    };
    const request = {
      account_id: query.account_id,
      authorization_epoch: "9007199254740993",
      subject_id: query.subject_id,
      facet: "activity" as const,
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

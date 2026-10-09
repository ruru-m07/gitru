import {
  CheckV1Schema,
  collaborationDetail,
  collaborationHydrateDetail,
  DetailEntrySchema,
  NativeDetailPayloadSchema,
} from "@gitru/commands";
import { afterEach, describe, expect, it, vi } from "vitest";

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
afterEach(() => invoke.mockReset());

const headOid = "a".repeat(40);
const checkRun = {
  kind: "check_run" as const,
  name: "build",
  state: {
    kind: "check_run" as const,
    status: "completed",
    conclusion: "success",
  },
  description: { state: "known" as const, text: "Build completed" },
  producer: "fixture-ci",
  started_at: "2026-10-07T00:00:00Z",
  completed_at: "2026-10-07T00:01:00Z",
  updated_at: "2026-10-07T00:01:00Z",
  allow_failure: null,
};
const commitStatus = {
  kind: "commit_status" as const,
  name: "deploy",
  state: { kind: "commit_status" as const, state: "pending" },
  description: { state: "omitted" as const, text: null },
  producer: null,
  started_at: null,
  completed_at: null,
  updated_at: null,
  allow_failure: true,
};

function entry(value: typeof checkRun | typeof commitStatus) {
  const validation = (field: "check" | "head_oid") => ({
    field,
    validated_at: "2026-10-07T00:00:00Z",
    source: "fixture/checks/v1",
    adapter_version: 1,
  });
  return {
    id: `github:check:${value.name}`,
    provider_id: value.name,
    author: null,
    title: null,
    state: null,
    body: { state: "not_loaded", text: null },
    observed_body_state: "not_loaded",
    updated_at: null,
    head_oid: headOid,
    native: { kind: "check.v1", value },
    field_mask: ["check", "head_oid"],
    field_validations: [validation("check"), validation("head_oid")],
  };
}

describe("generated native Check wire family", () => {
  it("keeps check-run conclusions and commit-status states distinct", () => {
    const run = CheckV1Schema.parse(checkRun);
    const status = CheckV1Schema.parse(commitStatus);
    expect(run.state).toEqual({
      kind: "check_run",
      status: "completed",
      conclusion: "success",
    });
    expect(status.state).toEqual({
      kind: "commit_status",
      state: "pending",
    });
    expect(run.allow_failure).toBeNull();
    expect(status.allow_failure).toBe(true);
    expect(DetailEntrySchema.parse(entry(checkRun)).head_oid).toBe(headOid);
    expect(DetailEntrySchema.parse(entry(commitStatus)).native?.kind).toBe(
      "check.v1",
    );
  });

  it("preserves unknown provider values without crossing native state families", () => {
    const futureRun = {
      ...checkRun,
      state: {
        kind: "check_run" as const,
        status: "future_status",
        conclusion: "future_conclusion",
      },
    };
    expect(CheckV1Schema.parse(futureRun).state).toEqual(futureRun.state);
    for (const bad of [
      { ...checkRun, state: commitStatus.state },
      { ...commitStatus, state: checkRun.state },
      { ...checkRun, allow_failure: false },
      { ...checkRun, state: { kind: "check_run", status: "completed" } },
      { ...commitStatus, state: { kind: "commit_status" } },
    ])
      expect(CheckV1Schema.safeParse(bad).success).toBe(false);
    expect(
      NativeDetailPayloadSchema.safeParse({
        kind: "check.v1",
        value: { ...checkRun, kind: "commit_status" },
      }).success,
    ).toBe(false);
  });

  it("requires exact saved check and head evidence without cross-family validations", () => {
    const valid = entry(checkRun);
    for (const bad of [
      { ...valid, field_mask: ["check"] },
      { ...valid, field_mask: ["check", "head_oid", "check"] },
      { ...valid, field_mask: ["check", "head_oid", "body"] },
      { ...valid, field_validations: [] },
      {
        ...valid,
        field_validations: valid.field_validations.slice(0, 1),
      },
      {
        ...valid,
        field_validations: [
          valid.field_validations[0],
          valid.field_validations[0],
        ],
      },
      {
        ...valid,
        field_validations: [
          valid.field_validations[0],
          { ...valid.field_validations[1], field: "body" },
        ],
      },
    ])
      expect(DetailEntrySchema.safeParse(bad).success).toBe(false);
  });

  it("invokes a 50-row local read independently from explicit epoch-scoped sync", async () => {
    const query = {
      account_id: "actor",
      subject_id: "repository:67",
      facet: "checks" as const,
      cursor: null,
      limit: 50,
    };
    const request = {
      account_id: "actor",
      authorization_epoch: "9007199254740993",
      subject_id: query.subject_id,
      facet: "checks" as const,
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

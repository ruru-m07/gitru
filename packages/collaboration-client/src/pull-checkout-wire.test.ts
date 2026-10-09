import { afterEach, describe, expect, it, vi } from "vitest";
import {
  collaboration,
  type PullCheckoutPlan,
  type PullCheckoutReceipt,
  type RemoteAccount,
} from "./index";

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
afterEach(() => invoke.mockReset());

const account: RemoteAccount = {
  id: "account-a",
  actor_id: "actor-a",
  provider: "github",
  host: "https://github.com",
  authorization_epoch: "9007199254740993",
  login: "fixture",
  state: "active",
  display_name: null,
  notifications_supported: true,
};
const plan: PullCheckoutPlan = {
  plan_id: "opaque-native-plan",
  local_repository_id: "registered-a",
  local_repository_name: "project",
  source_repository: "owner/project",
  source_remote: "origin",
  source_branch: "feature",
  expected_oid: "a".repeat(40),
  local_branch: "review/42",
  metadata_validated_at: "2026-10-05T00:00:00Z",
  metadata_stale: false,
  inspection: {
    current_branch: "main",
    current_head_oid: "b".repeat(40),
    detached: false,
    dirty: false,
    operation: "clean",
    object_available: false,
    action: "fetch_and_create_branch",
  },
};
const receipt: PullCheckoutReceipt = {
  local_repository_id: plan.local_repository_id,
  branch: plan.local_branch,
  oid: plan.expected_oid,
  fetched: true,
  git_reported_failure: false,
};

describe("pull checkout IPC wire", () => {
  it("sends only stable identities for planning and an opaque plan ID for execution", async () => {
    invoke.mockResolvedValueOnce(plan).mockResolvedValueOnce(receipt);
    const request = {
      instance_id: "github:https://github.com/",
      subject_id: "pull-42",
      local_repository_id: "registered-a",
      link_id: "link-a",
      link_generation: "9007199254740994",
      local_branch: "review/42",
      path: "/caller-controlled/path",
      url: "https://attacker.invalid/repository",
      remote: "caller-remote",
      oid: "c".repeat(40),
    };
    const checkout = collaboration.forAccount(account);

    await expect(checkout.planPullCheckout(request)).resolves.toEqual(plan);
    await expect(
      checkout.executePullCheckout("opaque-native-plan"),
    ).resolves.toEqual(receipt);

    expect(invoke.mock.calls).toEqual([
      [
        "collaboration_plan_pull_checkout",
        {
          request: {
            account_id: account.id,
            authorization_epoch: account.authorization_epoch,
            instance_id: request.instance_id,
            subject_id: request.subject_id,
            local_repository_id: request.local_repository_id,
            link_id: request.link_id,
            link_generation: request.link_generation,
            local_branch: request.local_branch,
          },
        },
      ],
      [
        "collaboration_execute_pull_checkout",
        { request: { plan_id: "opaque-native-plan" } },
      ],
    ]);
    const wire = invoke.mock.calls.map(([, payload]) => payload);
    for (const forbidden of ["path", "url", "remote", "oid"])
      expect(JSON.stringify(wire)).not.toContain(`\"${forbidden}\"`);
  });
});

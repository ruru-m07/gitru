import {
  AcquireDemandRequestSchema,
  collaborationAcquireDemand,
  collaborationDemandActivity,
  collaborationReleaseDemand,
  collaborationRenewDemand,
  DemandLeaseReceiptSchema,
  DemandOwnerActivitySchema,
  type DemandTarget,
} from "@gitru/commands";
import type { Event } from "@tauri-apps/api/event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { collaboration } from "./index";

type ActivityPayload = {
  owner_label: string;
  activity: { generation: string; active: boolean };
};
const { invoke, listen, currentWebview } = vi.hoisted(() => ({
  invoke: vi.fn(),
  listen: vi.fn(),
  currentWebview: vi.fn(() => ({ label: "owner-a" })),
}));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
vi.mock("@tauri-apps/api/event", () => ({ listen }));
vi.mock("@tauri-apps/api/webview", () => ({
  getCurrentWebview: currentWebview,
}));
afterEach(() => {
  invoke.mockReset();
  listen.mockReset();
  currentWebview.mockClear();
});

describe("generated foreground demand wire", () => {
  it("round-trips every typed target and exact string actor epoch/owner generation through acquire", async () => {
    const targets: DemandTarget[] = [
      {
        kind: "repositories",
        repository_id: null,
        subject_id: null,
        facet: null,
      },
      { kind: "inbox", repository_id: null, subject_id: null, facet: null },
      {
        kind: "pull_requests",
        repository_id: null,
        subject_id: null,
        facet: null,
      },
      {
        kind: "issues",
        repository_id: "9007199254740993",
        subject_id: null,
        facet: null,
      },
      {
        kind: "detail",
        repository_id: null,
        subject_id: "9007199254740994",
        facet: "body",
      },
    ];
    const receipt = DemandLeaseReceiptSchema.parse({
      lease_id: "opaque-native-id",
      owner_generation: "9007199254740995",
      expires_in_seconds: 45,
      renew_after_seconds: 15,
    });
    invoke.mockResolvedValue(receipt);
    for (const target of targets) {
      const request = AcquireDemandRequestSchema.parse({
        account_id: "account",
        authorization_epoch: "9007199254740996",
        owner_generation: receipt.owner_generation,
        target,
        owner_label: "forged-caller-is-not-a-wire-field",
      });
      expect(await collaborationAcquireDemand({ request })).toEqual(receipt);
      expect(invoke).toHaveBeenLastCalledWith("collaboration_acquire_demand", {
        request,
      });
      expect(request).not.toHaveProperty("owner_label");
      expect(request.authorization_epoch).toBe("9007199254740996");
    }
    expect(() =>
      AcquireDemandRequestSchema.parse({
        account_id: "account",
        authorization_epoch: "1",
        owner_generation: "1",
        target: { ...targets[0], kind: "http" },
      }),
    ).toThrow();
  });

  it("keeps native activity, atomic renewal and release distinct from provider refresh", async () => {
    const activity = DemandOwnerActivitySchema.parse({
      generation: "9007199254740997",
      active: false,
    });
    invoke.mockResolvedValueOnce(activity);
    expect(await collaborationDemandActivity({})).toEqual(activity);
    expect(invoke).toHaveBeenLastCalledWith(
      "collaboration_demand_activity",
      {},
    );
    const request = {
      owner_generation: "9007199254740998",
      leases: [
        {
          lease_id: "lease-a",
          account_id: "account-a",
          authorization_epoch: "9007199254740999",
        },
        {
          lease_id: "lease-b",
          account_id: "account-b",
          authorization_epoch: "2",
        },
      ],
    };
    const result = {
      leases: request.leases.map((lease) => ({
        lease_id: lease.lease_id,
        owner_generation: request.owner_generation,
        expires_in_seconds: 45,
        renew_after_seconds: 15,
      })),
    };
    invoke.mockResolvedValueOnce(result);
    expect(await collaborationRenewDemand({ request })).toEqual(result);
    expect(invoke).toHaveBeenLastCalledWith("collaboration_renew_demand", {
      request,
    });
    invoke.mockResolvedValueOnce(undefined);
    await collaborationReleaseDemand({ request: { lease_id: "lease-a" } });
    expect(invoke).toHaveBeenLastCalledWith("collaboration_release_demand", {
      request: { lease_id: "lease-a" },
    });
    expect(invoke.mock.calls.map((call) => call[0])).toEqual([
      "collaboration_demand_activity",
      "collaboration_renew_demand",
      "collaboration_release_demand",
    ]);
  });

  it("accepts activity events only for the actual current native webview label", async () => {
    const unlisten = vi.fn();
    listen.mockResolvedValue(unlisten);
    const changed = vi.fn();
    const stop = await collaboration.transport.listenDemandActivity(changed);
    expect(currentWebview).toHaveBeenCalledOnce();
    expect(listen.mock.calls[0][0]).toBe("collaboration:owner-activity");
    const handler = listen.mock.calls[0][1] as (
      event: Event<ActivityPayload>,
    ) => void;
    handler({
      event: "collaboration:owner-activity",
      id: 1,
      payload: {
        owner_label: "another-view",
        activity: { generation: "999", active: true },
      },
    });
    expect(changed).not.toHaveBeenCalled();
    const activity = { generation: "9007199254740993", active: false };
    handler({
      event: "collaboration:owner-activity",
      id: 2,
      payload: { owner_label: "owner-a", activity },
    });
    expect(changed).toHaveBeenCalledExactlyOnceWith(activity);
    stop();
    expect(unlisten).toHaveBeenCalledOnce();
  });
});

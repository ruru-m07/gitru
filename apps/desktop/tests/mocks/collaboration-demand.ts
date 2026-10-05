import { collaboration } from "@gitru/collaboration-client";
import type {
  AcquireDemandRequest,
  DemandOwnerActivity,
  RenewDemandRequest,
} from "@gitru/commands";
import { vi } from "vitest";
import { mockTauriCommand } from "./tauri";

/** Explicit native lease boundary; content commands remain fail-closed. */
export function mockForegroundDemand() {
  let activity: DemandOwnerActivity = { generation: "1", active: true };
  let notify: ((next: DemandOwnerActivity) => void) | null = null;
  let sequence = 0;
  vi.spyOn(collaboration.transport, "listenDemandActivity").mockImplementation(
    async (listener) => {
      notify = listener;
      return () => {
        if (notify === listener) notify = null;
      };
    },
  );
  const lease = (leaseId: string, ownerGeneration: string) => ({
    lease_id: leaseId,
    owner_generation: ownerGeneration,
    expires_in_seconds: 45,
    renew_after_seconds: 15,
  });
  const observe = mockTauriCommand(
    "collaboration_demand_activity",
    () => activity,
  );
  const acquire = mockTauriCommand(
    "collaboration_acquire_demand",
    (payload) => {
      const { request } = payload as { request: AcquireDemandRequest };
      return lease(`fixture-lease-${++sequence}`, request.owner_generation);
    },
  );
  const renew = mockTauriCommand("collaboration_renew_demand", (payload) => {
    const { request } = payload as { request: RenewDemandRequest };
    return {
      leases: request.leases.map((entry) =>
        lease(entry.lease_id, request.owner_generation),
      ),
    };
  });
  const release = mockTauriCommand(
    "collaboration_release_demand",
    () => undefined,
  );
  return {
    observe,
    acquire,
    renew,
    release,
    emit: (next: DemandOwnerActivity) => {
      activity = next;
      notify?.(next);
    },
  };
}

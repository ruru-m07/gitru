import type {
  AcquireDemandRequest,
  DemandLeaseReceipt,
  DemandOwnerActivity,
  DemandRenewalReceipt,
  DemandTarget,
  RenewDemandRequest,
} from "@gitru/commands";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  type DemandAccount,
  DemandCoordinator,
  type DemandTransport,
} from "./demand-coordinator";

const account: DemandAccount = {
  id: "account-a",
  actor_id: "actor-a",
  authorization_epoch: "1",
  provider: "github",
  host: "github.com",
  state: "active",
};
const target: DemandTarget = {
  kind: "detail",
  repository_id: null,
  subject_id: "issue:7",
  facet: "body",
};
const receipt = (
  leaseId: string,
  ownerGeneration = "1",
): DemandLeaseReceipt => ({
  lease_id: leaseId,
  owner_generation: ownerGeneration,
  expires_in_seconds: 45,
  renew_after_seconds: 15,
});
const deferred = <T>() => {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((done, failed) => {
    resolve = done;
    reject = failed;
  });
  return { promise, resolve, reject };
};
const flush = async () => {
  for (let i = 0; i < 12; i += 1) await Promise.resolve();
};
const coordinators: DemandCoordinator[] = [];
function fixture(
  initial: DemandOwnerActivity = { generation: "1", active: true },
) {
  let activity = initial;
  let emit!: (value: DemandOwnerActivity) => void;
  let sequence = 0;
  const unlisten = vi.fn();
  const transport = {
    demandActivity: vi.fn(async () => activity),
    acquireDemand: vi.fn(async (request: AcquireDemandRequest) =>
      receipt(`lease-${++sequence}`, request.owner_generation),
    ),
    renewDemand: vi.fn(async (request: RenewDemandRequest) => ({
      leases: request.leases.map((lease) =>
        receipt(lease.lease_id, request.owner_generation),
      ),
    })),
    releaseDemand: vi.fn(async () => {}),
    listenDemandActivity: vi.fn(async (listener) => {
      emit = listener;
      return unlisten;
    }),
  } satisfies DemandTransport;
  const coordinator = new DemandCoordinator(transport);
  coordinators.push(coordinator);
  return {
    coordinator,
    transport,
    unlisten,
    emit: (next: DemandOwnerActivity) => {
      activity = next;
      emit(next);
    },
  };
}

beforeEach(() => {
  vi.useFakeTimers();
  vi.setSystemTime(0);
});
afterEach(() => {
  for (const coordinator of coordinators.splice(0)) coordinator.stop();
  vi.useRealTimers();
});

describe("ephemeral foreground SDK demand", () => {
  it("keeps bridge-only and pre-bridge retains free of activity IPC", async () => {
    const { coordinator, transport } = fixture();
    coordinator.attach();
    await flush();
    expect(transport.demandActivity).not.toHaveBeenCalled();
    coordinator.stop();
    coordinator.retain(account, target);
    await flush();
    expect(transport.acquireDemand).not.toHaveBeenCalled();
    coordinator.attach();
    await flush();
    expect(transport.acquireDemand).toHaveBeenCalledTimes(1);
  });

  it("coalesces equivalent views and a StrictMode remount with one atomic liveness heartbeat", async () => {
    const { coordinator, transport } = fixture();
    coordinator.attach();
    const first = coordinator.retain(account, target);
    first.release();
    const replay = coordinator.retain(account, target);
    const second = coordinator.retain(account, { ...target });
    await flush();
    expect(transport.acquireDemand).toHaveBeenCalledTimes(1);
    await vi.advanceTimersByTimeAsync(15_000);
    expect(transport.renewDemand).toHaveBeenCalledExactlyOnceWith({
      owner_generation: "1",
      leases: [
        {
          lease_id: "lease-1",
          account_id: "account-a",
          authorization_epoch: "1",
        },
      ],
    });
    replay.release();
    await vi.advanceTimersByTimeAsync(15_000);
    expect(transport.releaseDemand).not.toHaveBeenCalled();
    expect(transport.renewDemand).toHaveBeenCalledTimes(2);
    second.release();
    await vi.advanceTimersByTimeAsync(0);
    expect(transport.releaseDemand).toHaveBeenCalledExactlyOnceWith({
      lease_id: "lease-1",
    });
    await vi.advanceTimersByTimeAsync(60_000);
    expect(transport.renewDemand).toHaveBeenCalledTimes(2);
    coordinator.retain(account, target);
    await flush();
    expect(transport.acquireDemand).toHaveBeenCalledTimes(2);
  });

  it.each([
    "release",
    "account-reset",
    "owner-replacement",
    "bridge-stop",
  ])("releases a raw late acquisition after %s before it can renew", async (cause) => {
    const { coordinator, transport, emit } = fixture();
    const pending = deferred<DemandLeaseReceipt>();
    transport.acquireDemand.mockImplementationOnce(() => pending.promise);
    coordinator.attach();
    const handle = coordinator.retain(account, target);
    await flush();
    if (cause === "release") {
      handle.release();
      await vi.advanceTimersByTimeAsync(0);
    } else if (cause === "account-reset") coordinator.clear(account.id);
    else if (cause === "owner-replacement")
      emit({ generation: "2", active: false });
    else coordinator.stop();
    pending.resolve(receipt("late-old-lease"));
    await flush();
    expect(transport.releaseDemand).toHaveBeenCalledWith({
      lease_id: "late-old-lease",
    });
    await vi.advanceTimersByTimeAsync(30_000);
    expect(transport.renewDemand).not.toHaveBeenCalled();
  });

  it("does not allow a late initial getter or older native event to reactivate an owner", async () => {
    const { coordinator, transport, emit } = fixture();
    const getter = deferred<DemandOwnerActivity>();
    transport.demandActivity.mockImplementationOnce(() => getter.promise);
    coordinator.attach();
    coordinator.retain(account, target);
    await flush();
    emit({ generation: "9007199254740994", active: false });
    getter.resolve({ generation: "9007199254740993", active: true });
    await flush();
    expect(transport.acquireDemand).not.toHaveBeenCalled();
    emit({ generation: "9007199254740995", active: true });
    await flush();
    expect(transport.acquireDemand.mock.calls[0][0].owner_generation).toBe(
      "9007199254740995",
    );
    emit({ generation: "9007199254740994", active: false });
    await vi.advanceTimersByTimeAsync(15_000);
    expect(transport.renewDemand).toHaveBeenCalledTimes(1);
  });

  it("waits for authoritative activation and reacquires current targets after hide/reopen", async () => {
    const { coordinator, transport, emit } = fixture({
      generation: "1",
      active: false,
    });
    coordinator.attach();
    coordinator.retain(account, target);
    await flush();
    await vi.advanceTimersByTimeAsync(60_000);
    expect(transport.acquireDemand).not.toHaveBeenCalled();
    emit({ generation: "2", active: true });
    await flush();
    emit({ generation: "3", active: false });
    expect(transport.releaseDemand).toHaveBeenCalledWith({
      lease_id: "lease-1",
    });
    await vi.advanceTimersByTimeAsync(60_000);
    expect(transport.renewDemand).not.toHaveBeenCalled();
    emit({ generation: "4", active: true });
    await flush();
    expect(transport.acquireDemand).toHaveBeenCalledTimes(2);
    expect(transport.acquireDemand.mock.calls[1][0].owner_generation).toBe("4");
  });

  it("captures mutable account/target inputs and isolates a reset from another actor", async () => {
    const { coordinator, transport } = fixture();
    coordinator.attach();
    const input = { ...account };
    const selection = { ...target };
    coordinator.retain(input, selection);
    input.authorization_epoch = "999";
    selection.subject_id = "wrong-later-subject";
    coordinator.retain(
      {
        ...account,
        id: "account-b",
        actor_id: "actor-b",
        authorization_epoch: "7",
      },
      target,
    );
    await flush();
    expect(transport.acquireDemand.mock.calls[0][0]).toMatchObject({
      account_id: "account-a",
      authorization_epoch: "1",
      target: { subject_id: "issue:7" },
    });
    coordinator.clear("account-a");
    coordinator.retain({ ...account, authorization_epoch: "2" }, target);
    await flush();
    await vi.advanceTimersByTimeAsync(15_000);
    expect(transport.renewDemand.mock.calls[0][0].leases).toEqual([
      {
        lease_id: "lease-2",
        account_id: "account-b",
        authorization_epoch: "7",
      },
      {
        lease_id: "lease-3",
        account_id: "account-a",
        authorization_epoch: "2",
      },
    ]);
  });

  it("bounds records and native handles while reusing references and freeing final-release capacity", async () => {
    const { coordinator, transport } = fixture();
    coordinator.attach();
    const handles = Array.from({ length: 128 }, (_, index) =>
      coordinator.retain(account, { ...target, subject_id: String(index) }),
    );
    const excess = coordinator.retain(account, {
      ...target,
      subject_id: "overflow",
    });
    const failure = vi.fn();
    excess.subscribe(failure);
    await flush();
    expect(transport.acquireDemand).toHaveBeenCalledTimes(16);
    expect(failure.mock.calls[0][0]).toMatchObject({ code: "busy" });
    await vi.advanceTimersByTimeAsync(15_000);
    expect(transport.renewDemand.mock.calls[0][0].leases).toHaveLength(16);
    handles[0].release();
    await vi.advanceTimersByTimeAsync(0);
    coordinator.retain(account, { ...target, subject_id: "new-after-release" });
    await flush();
    expect(transport.acquireDemand).toHaveBeenCalledTimes(17);
  });

  it("never automatically retries permission failure, Busy or inactive accounts on heartbeat ticks", async () => {
    const { coordinator, transport, emit } = fixture();
    transport.acquireDemand.mockRejectedValue({
      code: "permission_denied",
      message: "Denied",
    });
    coordinator.attach();
    coordinator.retain(account, target);
    coordinator.retain(
      { ...account, id: "disconnected", state: "disconnected" },
      target,
    );
    await flush();
    await vi.advanceTimersByTimeAsync(90_000);
    expect(transport.acquireDemand).toHaveBeenCalledTimes(1);
    expect(transport.renewDemand).not.toHaveBeenCalled();
    emit({ generation: "2", active: true });
    await flush();
    expect(transport.acquireDemand).toHaveBeenCalledTimes(1);
    coordinator.clear(account.id);
    coordinator.retain(account, target);
    await flush();
    expect(transport.acquireDemand).toHaveBeenCalledTimes(2);
  });

  it("does not overlap renewals or adopt a renewal that arrives after its TTL", async () => {
    const { coordinator, transport } = fixture();
    const pending = deferred<DemandRenewalReceipt>();
    transport.renewDemand.mockImplementationOnce(() => pending.promise);
    coordinator.attach();
    const handle = coordinator.retain(account, target);
    const failure = vi.fn();
    handle.subscribe(failure);
    await flush();
    await vi.advanceTimersByTimeAsync(60_000);
    expect(transport.renewDemand).toHaveBeenCalledTimes(1);
    expect(
      failure.mock.calls.some(([value]) => value?.code === "stale_view"),
    ).toBe(true);
    expect(transport.acquireDemand).toHaveBeenCalledTimes(2);
    expect(transport.demandActivity).toHaveBeenCalledTimes(2);
    pending.resolve({ leases: [receipt("lease-1")] });
    await flush();
    await vi.advanceTimersByTimeAsync(15_000);
    expect(transport.renewDemand).toHaveBeenCalledTimes(2);
    expect(transport.renewDemand.mock.calls[1][0].leases[0].lease_id).toBe(
      "lease-2",
    );
  });

  it.each([
    "busy",
    "permission_denied",
  ])("makes one authoritative expiry repair and never spins after %s", async (code) => {
    const { coordinator, transport, emit } = fixture();
    coordinator.attach();
    coordinator.retain(account, target);
    await flush();
    transport.acquireDemand.mockRejectedValueOnce({ code });
    vi.setSystemTime(46_000);
    await vi.advanceTimersByTimeAsync(15_000);
    expect(transport.demandActivity).toHaveBeenCalledTimes(2);
    expect(transport.acquireDemand).toHaveBeenCalledTimes(2);
    await vi.advanceTimersByTimeAsync(90_000);
    emit({ generation: "2", active: true });
    await flush();
    expect(transport.demandActivity).toHaveBeenCalledTimes(2);
    expect(transport.acquireDemand).toHaveBeenCalledTimes(2);
  });

  it("fences a delayed expiry repair across actor reset and follows only the replacement target", async () => {
    const { coordinator, transport } = fixture();
    coordinator.attach();
    coordinator.retain(account, target);
    await flush();
    const repair = deferred<DemandOwnerActivity>();
    transport.demandActivity.mockImplementationOnce(() => repair.promise);
    vi.setSystemTime(46_000);
    await vi.advanceTimersByTimeAsync(15_000);
    coordinator.clear(account.id);
    coordinator.retain(
      { ...account, actor_id: "new-actor", authorization_epoch: "2" },
      { ...target, subject_id: "new-issue" },
    );
    await flush();
    repair.resolve({ generation: "1", active: true });
    await flush();
    expect(transport.acquireDemand).toHaveBeenCalledTimes(2);
    expect(transport.acquireDemand.mock.calls[1][0]).toMatchObject({
      authorization_epoch: "2",
      target: { subject_id: "new-issue" },
    });
    expect(transport.demandActivity).toHaveBeenCalledTimes(2);
  });

  it("leaves an expired target suspended after an inactive repair until new native activation", async () => {
    const { coordinator, transport, emit } = fixture();
    coordinator.attach();
    coordinator.retain(account, target);
    await flush();
    transport.demandActivity.mockResolvedValueOnce({
      generation: "2",
      active: false,
    });
    vi.setSystemTime(46_000);
    await vi.advanceTimersByTimeAsync(90_000);
    expect(transport.acquireDemand).toHaveBeenCalledTimes(1);
    expect(transport.demandActivity).toHaveBeenCalledTimes(2);
    emit({ generation: "3", active: true });
    await flush();
    expect(transport.acquireDemand).toHaveBeenCalledTimes(2);
  });

  it("does not misclassify a still-current account when a pending atomic batch crosses another account reset", async () => {
    const { coordinator, transport } = fixture();
    const pending = deferred<DemandRenewalReceipt>();
    transport.renewDemand.mockImplementationOnce(() => pending.promise);
    coordinator.attach();
    coordinator.retain(account, target);
    const other = coordinator.retain(
      { ...account, id: "account-b", actor_id: "actor-b" },
      target,
    );
    const failure = vi.fn();
    other.subscribe(failure);
    await flush();
    await vi.advanceTimersByTimeAsync(15_000);
    coordinator.clear(account.id);
    pending.reject({ code: "stale_view" });
    await flush();
    expect(failure).toHaveBeenLastCalledWith(null);
    await vi.advanceTimersByTimeAsync(15_000);
    expect(transport.renewDemand.mock.calls[1][0].leases).toEqual([
      {
        lease_id: "lease-2",
        account_id: "account-b",
        authorization_epoch: "1",
      },
    ]);
  });

  it.each([
    "getter",
    "acquire",
  ])("repairs startup %s NotReady once after a successful readiness wake", async (boundary) => {
    const { coordinator, transport } = fixture();
    if (boundary === "getter")
      transport.demandActivity.mockRejectedValueOnce({ code: "not_ready" });
    else transport.acquireDemand.mockRejectedValueOnce({ code: "not_ready" });
    coordinator.attach();
    const failure = vi.fn();
    coordinator.retain(account, target).subscribe(failure);
    await flush();
    expect(failure).toHaveBeenLastCalledWith({ code: "not_ready" });
    await vi.advanceTimersByTimeAsync(90_000);
    expect(transport.demandActivity).toHaveBeenCalledTimes(1);
    coordinator.ready();
    await flush();
    expect(failure).toHaveBeenLastCalledWith(null);
    expect(transport.demandActivity).toHaveBeenCalledTimes(2);
    expect(transport.acquireDemand).toHaveBeenCalledTimes(
      boundary === "getter" ? 1 : 2,
    );
  });

  it("never spins repeated startup failure or retries permanent denials on readiness wakes", async () => {
    const { coordinator, transport } = fixture();
    transport.acquireDemand.mockRejectedValue({ code: "not_ready" });
    coordinator.attach();
    coordinator.retain(account, target);
    await flush();
    coordinator.ready();
    await flush();
    for (let i = 0; i < 10; i += 1) coordinator.ready();
    await flush();
    await vi.advanceTimersByTimeAsync(90_000);
    expect(transport.acquireDemand).toHaveBeenCalledTimes(2);
    expect(transport.demandActivity).toHaveBeenCalledTimes(2);
    coordinator.clear();
    transport.acquireDemand.mockRejectedValue({ code: "permission_denied" });
    coordinator.retain(account, target);
    await flush();
    coordinator.ready();
    await flush();
    expect(transport.acquireDemand).toHaveBeenCalledTimes(3);
    expect(transport.demandActivity).toHaveBeenCalledTimes(2);
  });

  it("does not revive the old actor from a startup repair delayed across reset and teardown", async () => {
    const { coordinator, transport } = fixture();
    transport.acquireDemand.mockRejectedValueOnce({ code: "not_ready" });
    coordinator.attach();
    coordinator.retain(account, target);
    await flush();
    const repair = deferred<DemandOwnerActivity>();
    transport.demandActivity.mockImplementationOnce(() => repair.promise);
    coordinator.ready();
    await flush();
    coordinator.clear(account.id);
    coordinator.stop();
    repair.resolve({ generation: "1", active: true });
    await flush();
    expect(transport.acquireDemand).toHaveBeenCalledTimes(1);
    coordinator.attach();
    coordinator.retain(
      { ...account, actor_id: "actor-new", authorization_epoch: "2" },
      { ...target, subject_id: "new-subject" },
    );
    await flush();
    expect(transport.acquireDemand.mock.calls[1][0]).toMatchObject({
      authorization_epoch: "2",
      target: { subject_id: "new-subject" },
    });
  });

  it("rejects an invalid or late acquisition without an immediate reacquire loop", async () => {
    const { coordinator, transport } = fixture();
    const pending = deferred<DemandLeaseReceipt>();
    transport.acquireDemand.mockImplementationOnce(() => pending.promise);
    coordinator.attach();
    coordinator.retain(account, target);
    await flush();
    vi.setSystemTime(46_000);
    pending.resolve(receipt("expired-before-delivery"));
    await flush();
    expect(transport.acquireDemand).toHaveBeenCalledTimes(1);
    expect(transport.releaseDemand).toHaveBeenCalledWith({
      lease_id: "expired-before-delivery",
    });
    await vi.advanceTimersByTimeAsync(90_000);
    expect(transport.acquireDemand).toHaveBeenCalledTimes(1);
  });

  it("stops all listeners/timers and fences an initial getter across bridge disposal/reinstallation", async () => {
    const { coordinator, transport, unlisten } = fixture();
    const getter = deferred<DemandOwnerActivity>();
    transport.demandActivity.mockImplementationOnce(() => getter.promise);
    coordinator.attach();
    coordinator.retain(account, target);
    await flush();
    coordinator.stop();
    expect(unlisten).toHaveBeenCalledTimes(1);
    coordinator.attach();
    coordinator.retain(account, { ...target, subject_id: "new-subject" });
    await flush();
    getter.resolve({ generation: "2", active: true });
    await flush();
    expect(transport.acquireDemand).toHaveBeenCalledTimes(1);
    expect(transport.acquireDemand.mock.calls[0][0].target.subject_id).toBe(
      "new-subject",
    );
    coordinator.stop();
    expect(vi.getTimerCount()).toBe(0);
  });
});

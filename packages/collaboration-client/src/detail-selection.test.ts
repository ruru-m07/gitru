import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { DetailSelectionCoordinator } from "./detail-selection";

const identity = ["account", "actor", "1", "pull", "body"] as const;
beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

describe("selected detail admission", () => {
  it("coalesces StrictMode/remount intent and failures until the parent head changes", async () => {
    const coordinator = new DetailSelectionCoordinator();
    const hydrate = vi.fn().mockRejectedValue(new Error("offline"));
    const first = coordinator.retain(identity, hydrate);
    expect(hydrate).not.toHaveBeenCalled();
    await expect(first.hydrate("head-a")).rejects.toThrow("offline");
    first.release();
    const replay = coordinator.retain(identity, hydrate);
    await expect(replay.hydrate("head-a")).resolves.toBe(false);
    const overlapping = coordinator.retain(identity, hydrate);
    await expect(overlapping.hydrate("head-a")).resolves.toBe(false);
    hydrate.mockResolvedValue({ queued: true });
    await expect(replay.hydrate("head-b")).resolves.toBe(true);
    await expect(overlapping.hydrate("head-b")).resolves.toBe(false);
    expect(hydrate).toHaveBeenCalledTimes(2);
    replay.release();
    overlapping.release();
    await vi.runAllTimersAsync();
  });

  it("isolates actor/epoch/subject and deactivates old targets before subsequent dispatch", async () => {
    const coordinator = new DetailSelectionCoordinator();
    const old = vi.fn().mockResolvedValue({ queued: true });
    const current = vi.fn().mockResolvedValue({ queued: true });
    const oldLease = coordinator.retain(identity, old);
    const switched = coordinator.retain(
      ["account", "other", "2", "pull", "body"],
      current,
    );
    const otherSubject = coordinator.retain(
      ["account", "other", "2", "issue", "body"],
      current,
    );
    await oldLease.hydrate("same-head");
    await switched.hydrate("same-head");
    await otherSubject.hydrate("same-head");
    expect(old).toHaveBeenCalledTimes(1);
    expect(current).toHaveBeenCalledTimes(2);
    coordinator.clear("account");
    await expect(oldLease.hydrate("late-head")).resolves.toBe(false);
    await expect(switched.hydrate("late-head")).resolves.toBe(false);
    expect(old).toHaveBeenCalledTimes(1);
    expect(current).toHaveBeenCalledTimes(2);
    oldLease.release();
    switched.release();
    otherSubject.release();
  });

  it("retires departed views while permitting an intentional later visit", async () => {
    const coordinator = new DetailSelectionCoordinator();
    const hydrate = vi.fn().mockResolvedValue({ queued: true });
    const first = coordinator.retain(identity, hydrate);
    await first.hydrate("head");
    first.release();
    await expect(first.hydrate("new-head")).resolves.toBe(false);
    await vi.runAllTimersAsync();
    const revisit = coordinator.retain(identity, hydrate);
    await revisit.hydrate("head");
    expect(hydrate).toHaveBeenCalledTimes(2);
    revisit.release();
    coordinator.clear();
    await vi.runAllTimersAsync();
    await expect(revisit.hydrate("new-head")).resolves.toBe(false);
  });

  it("bounds active bookkeeping and releases capacity on teardown without initiating reads", async () => {
    const coordinator = new DetailSelectionCoordinator();
    const hydrate = vi.fn().mockResolvedValue({ queued: true });
    const leases = Array.from({ length: 128 }, (_, index) =>
      coordinator.retain(
        ["account", "actor", "1", String(index), "body"],
        hydrate,
      ),
    );
    const overflow = coordinator.retain(
      ["account", "actor", "1", "overflow", "body"],
      hydrate,
    );
    await expect(overflow.hydrate("head")).resolves.toBe(false);
    expect(hydrate).not.toHaveBeenCalled();
    coordinator.clear();
    for (const lease of leases) lease.release();
    const next = coordinator.retain(identity, hydrate);
    await expect(next.hydrate("head")).resolves.toBe(true);
    expect(hydrate).toHaveBeenCalledTimes(1);
    next.release();
    coordinator.clear();
  });
});

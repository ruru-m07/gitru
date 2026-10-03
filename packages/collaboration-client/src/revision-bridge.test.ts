import { describe, expect, it, vi } from "vitest";
import {
  compareRevisions,
  type RevisionBatch,
  RevisionBridge,
} from "./revision-bridge";

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((done) => {
    resolve = done;
  });
  return { promise, resolve };
}

const batch = (
  from: string | null,
  to: string,
  extra: Partial<RevisionBatch<string>> = {},
): RevisionBatch<string> => ({
  changes: [to],
  fromExclusive: from,
  toInclusive: to,
  reset: false,
  hasMore: false,
  ...extra,
});

describe("RevisionBridge", () => {
  it("compares revisions beyond JavaScript integer precision", () => {
    expect(compareRevisions("9007199254740993", "9007199254740992")).toBe(1);
    expect(compareRevisions("002", "2")).toBe(0);
    expect(() => compareRevisions("-1", "0")).toThrow();
  });

  it("subscribes before snapshot and catches up hints arriving during a read", async () => {
    const pending = deferred<RevisionBatch<string>>();
    let wake!: () => void;
    const catchUp = vi
      .fn()
      .mockImplementationOnce(() => pending.promise)
      .mockResolvedValueOnce(batch("1", "2"));
    const apply = vi.fn();
    const bridge = new RevisionBridge(
      {
        listen: async (callback) => {
          wake = callback;
          return () => {};
        },
        catchUp,
      },
      apply,
    );
    const started = bridge.start();
    await Promise.resolve();
    wake();
    wake();
    pending.resolve(batch(null, "1"));
    await started;
    expect(catchUp.mock.calls).toEqual([[null], ["1"]]);
    expect(apply).toHaveBeenCalledTimes(2);
    bridge.stop();
  });

  it("repairs a missed revision range with an authoritative reset", async () => {
    const catchUp = vi
      .fn()
      .mockResolvedValueOnce(batch(null, "1"))
      .mockResolvedValueOnce(batch("3", "4"))
      .mockResolvedValueOnce(batch(null, "4", { reset: true }));
    const apply = vi.fn();
    const bridge = new RevisionBridge(
      { listen: async () => () => {}, catchUp },
      apply,
    );
    await bridge.start();
    await bridge.wake();
    expect(catchUp.mock.calls).toEqual([[null], ["1"], [null]]);
    expect(apply).toHaveBeenCalledTimes(2);
    expect(apply.mock.calls[1]?.[0].reset).toBe(true);
    bridge.stop();
  });

  it("ignores delayed reads and unregisters a listener resolved after stop", async () => {
    const registration = deferred<() => void>();
    const unlisten = vi.fn();
    const apply = vi.fn();
    const catchUp = vi.fn();
    const bridge = new RevisionBridge(
      { listen: () => registration.promise, catchUp },
      apply,
    );
    const started = bridge.start();
    bridge.stop();
    registration.resolve(unlisten);
    await started;
    expect(unlisten).toHaveBeenCalledOnce();
    expect(catchUp).not.toHaveBeenCalled();
    expect(apply).not.toHaveBeenCalled();
  });

  it("stops on errors and retries only when another wake arrives", async () => {
    const catchUp = vi
      .fn()
      .mockRejectedValueOnce(new Error("local storage unavailable"))
      .mockResolvedValueOnce(batch(null, "1"));
    const error = vi.fn();
    const bridge = new RevisionBridge(
      { listen: async () => () => {}, catchUp },
      vi.fn(),
      error,
    );
    await bridge.start();
    expect(catchUp).toHaveBeenCalledOnce();
    expect(error).toHaveBeenCalledOnce();
    await bridge.wake();
    expect(catchUp).toHaveBeenCalledTimes(2);
    bridge.stop();
  });

  it("rejects a stalled page instead of spinning", async () => {
    const error = vi.fn();
    const catchUp = vi
      .fn()
      .mockResolvedValueOnce(batch(null, "1"))
      .mockResolvedValueOnce(batch("1", "1", { hasMore: true }));
    const bridge = new RevisionBridge(
      { listen: async () => () => {}, catchUp },
      vi.fn(),
      error,
    );
    await bridge.start();
    await bridge.wake();
    expect(catchUp).toHaveBeenCalledTimes(2);
    expect(error).toHaveBeenCalledOnce();
    bridge.stop();
  });

  it("does not publish a read resolved after stop and can restart immediately", async () => {
    const oldRead = deferred<RevisionBatch<string>>();
    const apply = vi.fn();
    const catchUp = vi
      .fn()
      .mockImplementationOnce(() => oldRead.promise)
      .mockResolvedValueOnce(batch(null, "2", { reset: true }));
    const bridge = new RevisionBridge(
      { listen: async () => () => {}, catchUp },
      apply,
    );
    const first = bridge.start();
    await Promise.resolve();
    bridge.stop();
    await bridge.start();
    oldRead.resolve(batch(null, "1"));
    await first;
    expect(apply).toHaveBeenCalledOnce();
    expect(apply.mock.calls[0]?.[0].toInclusive).toBe("2");
    bridge.stop();
  });
});

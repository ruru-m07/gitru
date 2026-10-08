import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { waitForHarnessManifest } from "./e2e-collaboration-harness-startup";

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

describe("retained native manifest startup", () => {
  it("returns an immediately ready manifest without leaving timers", async () => {
    const manifest = { role: "main", scenario_generation: "1" };
    const read = vi.fn(async () => manifest);
    await expect(waitForHarnessManifest(read)).resolves.toBe(manifest);
    expect(read).toHaveBeenCalledTimes(1);
    expect(vi.getTimerCount()).toBe(0);
  });

  it("retries transient native startup and installs only after readiness", async () => {
    const manifest = { role: "main", scenario_generation: "1" };
    const read = vi
      .fn<() => Promise<typeof manifest>>()
      .mockRejectedValueOnce({ code: "not_ready" })
      .mockRejectedValueOnce({ code: "not_ready" })
      .mockResolvedValue(manifest);
    const install = vi.fn();
    const result = waitForHarnessManifest(read)
      .then(install)
      .then(
        () => ({ ready: true }),
        (failure) => ({ failure }),
      );
    await vi.advanceTimersByTimeAsync(100);
    expect(install).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(100);
    expect(await result).toEqual({ ready: true });
    expect(read).toHaveBeenCalledTimes(3);
    expect(install).toHaveBeenCalledExactlyOnceWith(manifest);
    expect(vi.getTimerCount()).toBe(0);
  });

  it("fails after 25 seconds when native startup never becomes ready", async () => {
    const read = vi.fn(async () => {
      throw { code: "not_ready" };
    });
    const assertion = expect(waitForHarnessManifest(read)).rejects.toThrow(
      "Retained native harness startup exceeded 25000ms",
    );
    await vi.advanceTimersByTimeAsync(25_000);
    await assertion;
    const attempts = read.mock.calls.length;
    expect(attempts).toBeLessThanOrEqual(251);
    await vi.advanceTimersByTimeAsync(25_000);
    expect(read).toHaveBeenCalledTimes(attempts);
    expect(vi.getTimerCount()).toBe(0);
  });

  it.each([
    "resolve",
    "reject",
  ] as const)("ignores a held manifest that %ss after the startup deadline", async (settlement) => {
    let resolve!: (manifest: object) => void;
    let reject!: (error: unknown) => void;
    const held = new Promise<object>((yes, no) => {
      resolve = yes;
      reject = no;
    });
    const read = vi.fn(() => held);
    const install = vi.fn();
    const assertion = expect(
      waitForHarnessManifest(read).then(install),
    ).rejects.toThrow("Retained native harness startup exceeded 25000ms");
    await vi.advanceTimersByTimeAsync(25_000);
    await assertion;
    if (settlement === "resolve") resolve({ role: "main" });
    else reject({ code: "not_ready" });
    await vi.advanceTimersByTimeAsync(25_000);
    expect(install).not.toHaveBeenCalled();
    expect(read).toHaveBeenCalledTimes(1);
    expect(vi.getTimerCount()).toBe(0);
  });

  it.each([
    { code: "auth_required" },
    { code: "permission_denied" },
    { code: "stale_view" },
    { code: "provider" },
    { code: "busy" },
    { message: "not_ready" },
    "not_ready",
    null,
  ])("does not retry any other failure: %j", async (failure) => {
    const read = vi.fn(async () => {
      throw failure;
    });
    await expect(waitForHarnessManifest(read)).rejects.toBe(failure);
    await vi.advanceTimersByTimeAsync(25_000);
    expect(read).toHaveBeenCalledTimes(1);
    expect(vi.getTimerCount()).toBe(0);
  });
});

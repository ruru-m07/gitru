import { describe, expect, it } from "vitest";
import {
  AuthorizationFence,
  StaleAuthorizationError,
} from "./authorization-fence";

describe("AuthorizationFence", () => {
  it("rejects a delayed IPC completion after disconnect", async () => {
    const fence = new AuthorizationFence();
    let finish!: (value: string) => void;
    const result = fence.read(
      "account-a",
      () =>
        new Promise<string>((resolve) => {
          finish = resolve;
        }),
    );
    fence.invalidate("account-a");
    finish("private cached content");
    await expect(result).rejects.toBeInstanceOf(StaleAuthorizationError);
  });

  it("does not discard unrelated account reads", async () => {
    const fence = new AuthorizationFence();
    const result = fence.read("account-b", async () => "other account");
    fence.invalidate("account-a");
    await expect(result).resolves.toBe("other account");
  });

  it("rejects all pre-reset reads and aborted local queries", async () => {
    const fence = new AuthorizationFence();
    const old = fence.read("account", async () => "cached content");
    fence.invalidate();
    await expect(old).rejects.toBeInstanceOf(StaleAuthorizationError);
    const controller = new AbortController();
    controller.abort();
    let calls = 0;
    await expect(
      fence.read(
        "account",
        async () => {
          calls += 1;
          return "value";
        },
        controller.signal,
      ),
    ).rejects.toBeInstanceOf(StaleAuthorizationError);
    expect(calls).toBe(0);
  });
});

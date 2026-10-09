import {
  collaborationConnectBitbucketCloud,
  RemoteAccountSchema,
} from "@gitru/commands";
import { afterEach, describe, expect, it, vi } from "vitest";

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
afterEach(() => invoke.mockReset());

const account = {
  id: "bitbucket-actor-bound-account",
  provider: "bitbucket_cloud",
  host: "bitbucket.org",
  actor_id: "11111111-1111-4111-8111-111111111111",
  authorization_epoch: "9007199254740994",
  login: "same-nickname",
  display_name: null,
  state: "active",
  notifications_supported: false,
};

describe("generated Bitbucket Cloud connection wire", () => {
  it("sends only the transient API token and preserves UUID identity and the string epoch", async () => {
    const result = RemoteAccountSchema.parse(account);
    invoke.mockResolvedValue(result);
    expect(
      await collaborationConnectBitbucketCloud({
        token: "synthetic-manual-api-token",
      }),
    ).toEqual(result);
    expect(invoke).toHaveBeenCalledExactlyOnceWith(
      "collaboration_connect_bitbucket_cloud",
      { token: "synthetic-manual-api-token" },
    );
    expect(result.actor_id).toBe(account.actor_id);
    expect(result.authorization_epoch).toBe("9007199254740994");
    expect(result.display_name).toBeNull();
    expect(result.notifications_supported).toBe(false);
  });

  it("rejects numeric identity and epoch values instead of coercing them on the wire", () => {
    expect(
      RemoteAccountSchema.safeParse({ ...account, actor_id: 123 }).success,
    ).toBe(false);
    expect(
      RemoteAccountSchema.safeParse({
        ...account,
        authorization_epoch: 9007199254740992,
      }).success,
    ).toBe(false);
  });
});

import {
  collaborationConnectGitlab,
  RemoteAccountSchema,
} from "@gitru/commands";
import { afterEach, describe, expect, it, vi } from "vitest";

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
afterEach(() => invoke.mockReset());

const account = {
  id: "gitlab-actor-bound-account",
  provider: "gitlab",
  host: "gitlab.com",
  actor_id: "9007199254740993",
  authorization_epoch: "9007199254740994",
  login: "fixture-user",
  display_name: null,
  state: "active",
  notifications_supported: false,
};

describe("generated GitLab connection wire", () => {
  it("sends only a transient token to the fixed command and retains provider/string actor identity", async () => {
    const result = RemoteAccountSchema.parse(account);
    invoke.mockResolvedValue(result);
    expect(
      await collaborationConnectGitlab({ token: "synthetic-manual-pat" }),
    ).toEqual(result);
    expect(invoke).toHaveBeenCalledExactlyOnceWith(
      "collaboration_connect_gitlab",
      { token: "synthetic-manual-pat" },
    );
    expect(result.actor_id).toBe("9007199254740993");
    expect(result.authorization_epoch).toBe("9007199254740994");
    expect(result.display_name).toBeNull();
    expect(result.notifications_supported).toBe(false);
  });

  it("rejects non-string identity in returned account schemas", () => {
    expect(
      RemoteAccountSchema.safeParse({ ...account, actor_id: 9007199254740992 })
        .success,
    ).toBe(false);
  });
});

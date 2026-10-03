import {
  AccountSnapshotSchema,
  collaborationAccounts,
  collaborationConnectGithubCli,
  collaborationDiscoverGithubCli,
  collaborationItem,
  collaborationItems,
  GithubCliDiscoverySchema,
  ItemPageSchema,
  ItemQuerySchema,
} from "@gitru/commands";
import { afterEach, describe, expect, it, vi } from "vitest";

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
afterEach(() => invoke.mockReset());

describe("generated collaboration wire contract", () => {
  it("accepts metadata-only CLI discovery states without credentials", () => {
    const discovery = GithubCliDiscoverySchema.parse({
      status: "available",
      accounts: [
        {
          id: "opaque-candidate",
          login: "fixture",
          host: "github.com",
          active: true,
          availability: "ready",
        },
        {
          id: "stale-candidate",
          login: "stale",
          host: "github.com",
          active: false,
          availability: "auth_required",
        },
      ],
    });
    expect(discovery.accounts[0]?.id).toBe("opaque-candidate");
    expect(discovery.accounts[1]?.availability).toBe("auth_required");
  });

  it("accepts native serde names and explicit nulls", () => {
    const snapshot = AccountSnapshotSchema.parse({
      accounts: [
        {
          id: "account",
          provider: "github",
          host: "https://github.com",
          actor_id: "1",
          login: "fixture",
          display_name: null,
          authorization_epoch: "9007199254740993",
          state: "active",
          notifications_supported: true,
        },
      ],
      revision: "9007199254740994",
      authorization_view: "4",
    });
    expect(snapshot.accounts[0]?.display_name).toBeNull();
    expect(snapshot.accounts[0]?.provider).toBe("github");
    expect(
      ItemQuerySchema.parse({
        account_id: "account",
        kind: "pull_request",
        repository_id: null,
        state: null,
        search: null,
        cursor: null,
        limit: 50,
      }).repository_id,
    ).toBeNull();
    const page = ItemPageSchema.parse({
      items: [
        {
          id: "item",
          account_id: "account",
          repository_id: null,
          provider_id: "2",
          kind: "notification",
          number: null,
          title: "fixture",
          body: null,
          body_omitted: true,
          author: null,
          web_url: null,
          state: "unread",
          updated_at: "2026-10-02T00:00:00Z",
          head_oid: null,
          is_draft: null,
          reason: null,
          unread: true,
        },
      ],
      revision: "5",
      authorization_view: "4",
      next_cursor: null,
      coverage: { state: "partial", validated_at: null, remote_has_more: true },
      sync: {
        state: "rate_limited",
        last_success_at: null,
        next_retry_at: null,
        error: {
          code: "rate_limited",
          message: "Wait",
          retry_after_seconds: 30,
        },
      },
    });
    expect(page.items[0]?.is_draft).toBeNull();
    expect(page.sync.state).toBe("rate_limited");
  });

  it("invokes only user parameters and never serializes injected native webviews", async () => {
    invoke.mockResolvedValue({});
    await collaborationAccounts({});
    await collaborationDiscoverGithubCli({});
    await collaborationConnectGithubCli({ candidateId: "opaque-candidate" });
    await collaborationItem({ accountId: "account", itemId: "item" });
    const query = {
      account_id: "account",
      kind: "issue" as const,
      repository_id: null,
      state: null,
      search: null,
      cursor: null,
      limit: 50,
    };
    await collaborationItems({ query });
    expect(invoke.mock.calls).toEqual([
      ["collaboration_accounts", {}],
      ["collaboration_discover_github_cli", {}],
      ["collaboration_connect_github_cli", { candidateId: "opaque-candidate" }],
      ["collaboration_item", { accountId: "account", itemId: "item" }],
      ["collaboration_items", { query }],
    ]);
  });
});

import {
  AccountSnapshotSchema,
  CapabilitySnapshotSchema,
  collaborationAccounts,
  collaborationCapabilities,
  collaborationConnectGithubCli,
  collaborationDiscoverGithubCli,
  collaborationItem,
  collaborationItems,
  collaborationResolveResource,
  GithubCliDiscoverySchema,
  ItemPageSchema,
  ItemQuerySchema,
  ResourceResolutionSchema,
} from "@gitru/commands";
import { afterEach, describe, expect, it, vi } from "vitest";

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
afterEach(() => invoke.mockReset());

describe("generated collaboration wire contract", () => {
  it("preserves typed capability states, large native identities, and metadata-only resolution", async () => {
    const instance = {
      id: "gitlab:https://git.example:8443/gitlab/",
      provider: "gitlab",
      base_url: "https://git.example:8443/gitlab/",
    };
    const capabilities = CapabilitySnapshotSchema.parse({
      account_id: "actor",
      instance,
      facets: [
        { facet: "pull_requests", state: "supported", reason: null },
        { facet: "merge", state: "unsupported", reason: "not_implemented" },
        { facet: "inbox", state: "unavailable", reason: "missing_scope" },
      ],
      inbox_semantics: "todos",
      revision: "9007199254740993",
      authorization_view: "3",
    });
    expect(capabilities.facets[0]?.reason).toBeNull();
    const resolution = ResourceResolutionSchema.parse({
      state: "resolved",
      resource: {
        account_id: "actor",
        instance_id: instance.id,
        id: "opaque-pull",
        kind: "pull_request",
        provider_id: "9007199254740994",
      },
      candidates: [],
      revision: "9007199254740993",
      authorization_view: "3",
    });
    expect(resolution.resource?.provider_id).toBe("9007199254740994");
    expect(
      ResourceResolutionSchema.parse({
        ...resolution,
        state: "unresolved",
        resource: null,
      }).resource,
    ).toBeNull();
    invoke
      .mockResolvedValueOnce(capabilities)
      .mockResolvedValueOnce(resolution);
    await collaborationCapabilities({ accountId: "actor" });
    const locator = {
      instance_id: instance.id,
      kind: "pull_request" as const,
      locator_kind: "repository_number" as const,
      value: "7",
      repository_path: "group/subgroup/project",
    };
    await collaborationResolveResource({ accountId: "actor", locator });
    expect(invoke.mock.calls).toEqual([
      ["collaboration_capabilities", { accountId: "actor" }],
      ["collaboration_resolve_resource", { accountId: "actor", locator }],
    ]);
  });
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

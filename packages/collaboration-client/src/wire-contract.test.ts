import {
  AccountSnapshotSchema,
  CapabilitySnapshotSchema,
  ContextualCapabilitySnapshotSchema,
  collaborationAccounts,
  collaborationCapabilities,
  collaborationConnectGithubCli,
  collaborationContextualCapabilities,
  collaborationDetail,
  collaborationDiscoverGithubCli,
  collaborationHydrateDetail,
  collaborationItem,
  collaborationItems,
  collaborationResolveResource,
  DetailSnapshotSchema,
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
  it("preserves contextual targets, independent modes and observation states without injected webviews", async () => {
    const target = {
      kind: "resource" as const,
      instance_id: "github:https://github.com/",
      repository_id: null,
      resource_id: "9007199254740995",
      resource_kind: "pull_request" as const,
    };
    const snapshot = ContextualCapabilitySnapshotSchema.parse({
      account_id: "actor",
      authorization_epoch: "9007199254740993",
      instance: {
        id: target.instance_id,
        provider: "github",
        base_url: "https://github.com/",
      },
      target,
      facets: [
        {
          facet: "pull_details",
          saved_read: { state: "supported", reason: null },
          synchronize: {
            state: "unavailable",
            reason: "temporarily_unavailable",
          },
          remote_write: { state: "unsupported", reason: "not_implemented" },
          observation: "oversized",
          can_recheck_access: false,
          sync: {
            state: "rate_limited",
            next_retry_at: "2099-10-03T12:00:00Z",
            last_success_at: null,
            error: null,
          },
        },
      ],
      inbox_semantics: "native_notifications",
      revision: "9007199254740994",
      authorization_view: "3",
    });
    expect(snapshot.target.resource_id).toBe("9007199254740995");
    expect(snapshot.target.repository_id).toBeNull();
    expect(snapshot.facets[0].saved_read.reason).toBeNull();
    for (const observation of [
      "unknown",
      "not_loaded",
      "partial",
      "complete",
      "empty",
      "omitted",
      "oversized",
    ] as const)
      expect(
        ContextualCapabilitySnapshotSchema.parse({
          ...snapshot,
          facets: [{ ...snapshot.facets[0], observation }],
        }).facets[0].observation,
      ).toBe(observation);
    const request = {
      account_id: "actor",
      authorization_epoch: "9007199254740993",
      target,
    };
    invoke.mockResolvedValue(snapshot);
    await collaborationContextualCapabilities({ request });
    expect(invoke).toHaveBeenCalledWith(
      "collaboration_contextual_capabilities",
      { request },
    );
  });
  it("preserves detail missingness, nullable authoritative body and string revisions through distinct read/hydrate commands", async () => {
    const snapshot = DetailSnapshotSchema.parse({
      subject_id: "pull",
      body: { state: "known", text: null },
      entries: [],
      next_cursor: null,
      revision: "9007199254740993",
      authorization_view: "4",
      evidence: {
        facet: "body",
        availability: "ready",
        coverage: {
          state: "complete",
          validated_at: "2026-10-03T00:00:00Z",
          remote_has_more: false,
        },
        freshness: "stale",
        stale_at: null,
        facet_revision: "9007199254740993",
        authorization_epoch: "3",
        access_reason: null,
        source: {
          source: "fixture/body/v1",
          adapter_version: 1,
          field_mask: ["body"],
          provider_updated_at: null,
          observed_at: "2026-10-03T00:00:00Z",
        },
        value_source: null,
        saved_empty: null,
        observed_state: "known",
        sync: {
          state: "offline",
          last_success_at: null,
          next_retry_at: null,
          error: null,
        },
      },
    });
    expect(snapshot.body.text).toBeNull();
    expect(snapshot.evidence.facet_revision).toBe("9007199254740993");
    for (const state of ["not_loaded", "omitted", "oversized"] as const)
      expect(
        DetailSnapshotSchema.parse({ ...snapshot, body: { state, text: null } })
          .body.state,
      ).toBe(state);
    invoke.mockResolvedValue(snapshot);
    const query = {
      account_id: "actor",
      subject_id: "pull",
      facet: "body" as const,
      cursor: null,
      limit: 100,
    };
    await collaborationDetail({ query });
    const request = {
      account_id: "actor",
      authorization_epoch: "3",
      subject_id: "pull",
      facet: "body" as const,
    };
    await collaborationHydrateDetail({ request });
    expect(invoke.mock.calls).toEqual([
      ["collaboration_detail", { query }],
      ["collaboration_hydrate_detail", { request }],
    ]);
  });
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

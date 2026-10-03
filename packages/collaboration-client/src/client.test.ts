import type {
  AccountSnapshot,
  CapabilitySnapshot,
  ChangePage,
  ItemPage,
  RemoteAccount,
  ResourceLocator,
  ResourceResolution,
} from "@gitru/commands";
import { QueryClient } from "@tanstack/react-query";
import { describe, expect, it, vi } from "vitest";
import { StaleAuthorizationError } from "./authorization-fence";
import {
  CollaborationClient,
  type CollaborationTransport,
  collaborationErrorMessage,
  collaborationKeys,
} from "./client";

const account: RemoteAccount = {
  id: "account-a",
  provider: "github",
  host: "https://github.com",
  actor_id: "1",
  login: "fixture",
  display_name: null,
  authorization_epoch: "1",
  state: "active",
  notifications_supported: true,
};
const snapshot: AccountSnapshot = {
  accounts: [account],
  revision: "1",
  authorization_view: "1",
};
const page: ItemPage = {
  items: [],
  revision: "1",
  authorization_view: "1",
  next_cursor: null,
  coverage: {
    state: "complete",
    validated_at: "2026-10-02T00:00:00Z",
    remote_has_more: false,
  },
  sync: {
    state: "idle",
    last_success_at: null,
    next_retry_at: null,
    error: null,
  },
};
const changePage = (
  revision: string,
  view = "1",
  changes: ChangePage["changes"] = [],
): ChangePage => ({
  revision,
  authorization_view: view,
  reset_required: false,
  has_more: false,
  changes,
});

function transport(
  overrides: Partial<CollaborationTransport> = {},
): CollaborationTransport {
  const unexpected = async () => {
    throw new Error("Unexpected transport operation");
  };
  return {
    accounts: unexpected,
    connectGithub: unexpected,
    discoverGithubCli: unexpected,
    connectGithubCli: unexpected,
    disconnect: unexpected,
    repositories: unexpected,
    selectRepository: unexpected,
    items: unexpected,
    item: unexpected,
    refresh: unexpected,
    changesSince: unexpected,
    saveDraft: unexpected,
    draft: unexpected,
    capabilities: unexpected,
    resolveResource: unexpected,
    listen: unexpected,
    ...overrides,
  };
}

describe("CollaborationClient", () => {
  it("binds capability and local resolver reads to the account and fences late lifecycle responses", async () => {
    const locator: ResourceLocator = {
      instance_id: "github:https://github.com/",
      kind: "pull_request",
      locator_kind: "repository_number",
      value: "67",
      repository_path: "owner/project",
    };
    const capability: CapabilitySnapshot = {
      account_id: account.id,
      instance: {
        id: locator.instance_id,
        provider: "github",
        base_url: "https://github.com/",
      },
      facets: [],
      inbox_semantics: "native_notifications",
      revision: "1",
      authorization_view: "1",
    };
    const resolution: ResourceResolution = {
      state: "unresolved",
      resource: null,
      candidates: [],
      revision: "1",
      authorization_view: "1",
    };
    let finish!: (result: ResourceResolution) => void;
    const capabilities = vi.fn().mockResolvedValue(capability);
    const resolveResource = vi.fn(
      () =>
        new Promise<ResourceResolution>((resolve) => {
          finish = resolve;
        }),
    );
    const client = new CollaborationClient(
      transport({ capabilities, resolveResource, disconnect: async () => "2" }),
    );
    const handle = client.forAccount(account);
    expect(await handle.capabilities()).toEqual(capability);
    expect(capabilities).toHaveBeenCalledWith(account.id);
    const pending = handle.resolveResource(locator);
    expect(resolveResource).toHaveBeenCalledWith(account.id, locator);
    await client.disconnect(account.id);
    finish(resolution);
    await expect(pending).rejects.toBeInstanceOf(StaleAuthorizationError);
  });

  it("refreshes unresolved identities and capability observations on provider changes without invalidating drafts", async () => {
    let next = changePage("1");
    const client = new CollaborationClient(
      transport({
        listen: async () => () => {},
        changesSince: async () => next,
      }),
    );
    const cache = new QueryClient();
    const stop = client.installBridge(cache);
    await client.wake();
    const locator: ResourceLocator = {
      instance_id: "github:https://github.com/",
      kind: "repository",
      locator_kind: "repository_path",
      value: "owner/project",
      repository_path: null,
    };
    const resourceKey = collaborationKeys.resource(account, locator);
    const capabilityKey = collaborationKeys.capabilities(account);
    const draftKey = collaborationKeys.draft(account, "draft");
    for (const key of [resourceKey, capabilityKey, draftKey])
      cache.setQueryData(key, "cached");
    next = changePage("2", "1", [
      {
        revision: "2",
        account_id: account.id,
        scope: "repositories",
        reset: false,
      },
    ]);
    await client.wake();
    expect(cache.getQueryState(resourceKey)?.isInvalidated).toBe(true);
    expect(cache.getQueryState(capabilityKey)?.isInvalidated).toBe(true);
    expect(cache.getQueryState(draftKey)?.isInvalidated).toBe(false);
    stop();
    cache.clear();
  });
  it("discovers only CLI metadata without changing the saved authorization view", async () => {
    const discovery = {
      status: "available" as const,
      accounts: [
        {
          id: "opaque-candidate",
          login: "fixture",
          host: "github.com",
          active: true,
          availability: "ready" as const,
        },
      ],
    };
    const discoverGithubCli = vi.fn().mockResolvedValue(discovery);
    const client = new CollaborationClient(transport({ discoverGithubCli }));
    expect(await client.discoverGithubCli()).toEqual(discovery);
    expect(discoverGithubCli).toHaveBeenCalledWith();
    expect(client.getVersion()).toBe(0);
  });

  it("connects a CLI account using its opaque candidate and fences obsolete private reads", async () => {
    let finish!: (snapshot: typeof page) => void;
    const connectGithubCli = vi.fn().mockResolvedValue(account);
    const client = new CollaborationClient(
      transport({
        connectGithubCli,
        items: () =>
          new Promise((resolve) => {
            finish = resolve;
          }),
      }),
    );
    const oldRead = client.forAccount(account).items({
      kind: "issue",
      repository_id: null,
      state: null,
      search: null,
      cursor: null,
      limit: 50,
    });
    expect(await client.connectGithubCli("opaque-candidate")).toBe(account);
    expect(connectGithubCli).toHaveBeenCalledWith("opaque-candidate");
    expect(client.getVersion()).toBe(1);
    finish(page);
    await expect(oldRead).rejects.toBeInstanceOf(StaleAuthorizationError);
  });

  it("keeps local account views intact when CLI credential import fails", async () => {
    const client = new CollaborationClient(
      transport({
        connectGithubCli: async () => {
          throw { code: "auth_required" };
        },
      }),
    );
    await expect(client.connectGithubCli("expired-candidate")).rejects.toEqual({
      code: "auth_required",
    });
    expect(client.getVersion()).toBe(0);
  });

  it("creates inert account handles and reads locally without provider refresh", async () => {
    const items = vi.fn().mockResolvedValue(page);
    const refresh = vi.fn();
    const client = new CollaborationClient(
      transport({ accounts: async () => snapshot, items, refresh }),
    );
    const handle = client.forAccount(account);
    expect(items).not.toHaveBeenCalled();
    await client.accounts();
    await handle.items({
      kind: "issue",
      repository_id: null,
      state: "open",
      search: null,
      cursor: null,
      limit: 50,
    });
    expect(items).toHaveBeenCalledWith({
      account_id: account.id,
      kind: "issue",
      repository_id: null,
      state: "open",
      search: null,
      cursor: null,
      limit: 50,
    });
    expect(refresh).not.toHaveBeenCalled();
  });

  it("partitions query keys by account authorization epoch", () => {
    expect(collaborationKeys.item(account, "item")).not.toEqual(
      collaborationKeys.item({ ...account, authorization_epoch: "2" }, "item"),
    );
    expect(collaborationKeys.item(account, "item")).not.toEqual(
      collaborationKeys.item({ ...account, id: "account-b" }, "item"),
    );
  });

  it("removes account projections immediately and fences late reads on disconnect", async () => {
    let finishDisconnect!: (value: string) => void;
    let finishRead!: (value: ItemPage) => void;
    const client = new CollaborationClient(
      transport({
        accounts: async () => snapshot,
        listen: async () => () => {},
        changesSince: async () => changePage("1"),
        disconnect: () =>
          new Promise((resolve) => {
            finishDisconnect = resolve;
          }),
        items: () =>
          new Promise((resolve) => {
            finishRead = resolve;
          }),
      }),
    );
    const cache = new QueryClient();
    const stop = client.installBridge(cache);
    await client.wake();
    await client.accounts();
    cache.setQueryData(collaborationKeys.item(account, "private-item"), {
      private: true,
    });
    const pendingRead = client.forAccount(account).items({
      kind: "issue",
      repository_id: null,
      state: null,
      search: null,
      cursor: null,
      limit: 50,
    });
    const disconnected = client.disconnect(account.id);
    expect(
      cache.getQueryData(collaborationKeys.item(account, "private-item")),
    ).toBeUndefined();
    finishRead(page);
    await expect(pendingRead).rejects.toBeInstanceOf(StaleAuthorizationError);
    finishDisconnect("2");
    await disconnected;
    stop();
    cache.clear();
  });

  it("rejects an obsolete authorization view and does not render raw error data", async () => {
    const client = new CollaborationClient(
      transport({
        accounts: async () => snapshot,
        items: async () => ({
          ...page,
          authorization_view: "0",
          revision: "0",
        }),
      }),
    );
    await client.accounts();
    await expect(
      client.forAccount(account).items({
        kind: "issue",
        repository_id: null,
        state: null,
        search: null,
        cursor: null,
        limit: 50,
      }),
    ).rejects.toBeInstanceOf(StaleAuthorizationError);
    expect(
      collaborationErrorMessage({
        code: "provider",
        message: "private token ghp_something",
      }),
    ).not.toContain("ghp_");
    expect(collaborationErrorMessage("secret payload")).not.toContain("secret");
  });

  it("invalidates only matching repository item scopes and leaves drafts untouched", async () => {
    let next = changePage("1");
    const client = new CollaborationClient(
      transport({
        listen: async () => () => {},
        changesSince: async () => next,
      }),
    );
    const cache = new QueryClient();
    const stop = client.installBridge(cache);
    await client.wake();
    const issueKey = collaborationKeys.items(account, {
      account_id: account.id,
      kind: "issue",
      repository_id: "repo-a",
      state: null,
      search: null,
      cursor: null,
      limit: 50,
    });
    const otherKey = collaborationKeys.items(account, {
      account_id: account.id,
      kind: "issue",
      repository_id: "repo-b",
      state: null,
      search: null,
      cursor: null,
      limit: 50,
    });
    const draftKey = collaborationKeys.draft(account, "draft");
    cache.setQueryData(issueKey, page);
    cache.setQueryData(otherKey, page);
    cache.setQueryData(draftKey, "draft");
    next = changePage("2", "1", [
      {
        revision: "2",
        account_id: account.id,
        scope: "repo:repo-a:issue",
        reset: false,
      },
    ]);
    await client.wake();
    expect(cache.getQueryState(issueKey)?.isInvalidated).toBe(true);
    expect(cache.getQueryState(otherKey)?.isInvalidated).toBe(false);
    expect(cache.getQueryState(draftKey)?.isInvalidated).toBe(false);
    stop();
    cache.clear();
  });

  it("fences draft commit responses after an account lifecycle reset", async () => {
    let finish!: (draft: {
      account_id: string;
      subject_id: string;
      body: string;
      generation: string;
    }) => void;
    const client = new CollaborationClient(
      transport({
        disconnect: async () => "2",
        saveDraft: () =>
          new Promise((resolve) => {
            finish = resolve;
          }),
      }),
    );
    const saved = client.forAccount(account).saveDraft({
      subject_id: "item",
      body: "private draft",
      generation: "0",
    });
    await client.disconnect(account.id);
    finish({
      account_id: account.id,
      subject_id: "item",
      body: "private draft",
      generation: "1",
    });
    await expect(saved).rejects.toBeInstanceOf(StaleAuthorizationError);
  });
});

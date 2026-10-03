import type {
  AccountSnapshot,
  ChangePage,
  ItemPage,
  RemoteAccount,
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
    drafts: unexpected,
    exportDraft: unexpected,
    listen: unexpected,
    ...overrides,
  };
}

describe("CollaborationClient", () => {
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

describe("authored draft recovery", () => {
  it("keeps authored caches across disconnect/replacement while clearing provider projections", async () => {
    let revision = "1";
    const client = new CollaborationClient(
      transport({
        listen: async () => () => {},
        changesSince: async () => changePage(revision, revision),
        disconnect: async () => {
          revision = "2";
          return revision;
        },
        connectGithub: async () => {
          revision = "3";
          return { ...account, authorization_epoch: "3" };
        },
      }),
    );
    const cache = new QueryClient();
    const stop = client.installBridge(cache);
    await client.wake();
    const draft = {
      account_id: account.id,
      subject_id: "missing",
      body: "My text",
      generation: "1",
    };
    const key = collaborationKeys.draft(account, "missing");
    const listKey = collaborationKeys.drafts(account, {
      account_id: account.id,
      cursor: null,
      limit: 50,
    });
    const remoteKey = collaborationKeys.item(account, "private-provider-data");
    cache.setQueryData(key, draft);
    cache.setQueryData(listKey, { drafts: [], next_cursor: null });
    cache.setQueryData(remoteKey, { body: "Provider body" });
    await client.disconnect(account.id);
    expect(cache.getQueryData(key)).toEqual(draft);
    expect(cache.getQueryState(key)?.isInvalidated).toBe(true);
    expect(cache.getQueryData(listKey)).toBeDefined();
    expect(cache.getQueryState(listKey)?.isInvalidated).toBe(true);
    expect(cache.getQueryData(remoteKey)).toBeUndefined();
    await client.connectGithub("fixture");
    expect(
      collaborationKeys.draft(
        { ...account, authorization_epoch: "3" },
        "missing",
      ),
    ).toEqual(key);
    expect(
      collaborationKeys.draft(
        { ...account, id: "actor-b", actor_id: "2" },
        "missing",
      ),
    ).not.toEqual(key);
    expect(cache.getQueryData(key)).toEqual(draft);
    stop();
    cache.clear();
  });

  it("fences delayed authored reads at disconnect and when a subject observer is cancelled", async () => {
    let finish!: (value: import("@gitru/commands").LocalDraft | null) => void;
    const client = new CollaborationClient(
      transport({
        disconnect: async () => "2",
        connectGithub: async () => ({ ...account, authorization_epoch: "3" }),
        draft: () =>
          new Promise((resolve) => {
            finish = resolve;
          }),
      }),
    );
    const pending = client.forAccount(account).draft("old-subject");
    await client.disconnect(account.id);
    finish({
      account_id: account.id,
      subject_id: "old-subject",
      body: "old actor text",
      generation: "1",
    });
    await expect(pending).rejects.toBeInstanceOf(StaleAuthorizationError);
    const replacing = client.forAccount(account).draft("old-subject");
    await client.connectGithub("fixture");
    finish(null);
    await expect(replacing).rejects.toBeInstanceOf(StaleAuthorizationError);
    const abort = new AbortController();
    const cancelled = client
      .forAccount(account)
      .draft("old-subject", abort.signal);
    abort.abort();
    finish(null);
    await expect(cancelled).rejects.toBeInstanceOf(StaleAuthorizationError);
  });

  it("retains authored text but invalidates it when the durable stream requires reset", async () => {
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
    const key = collaborationKeys.draft(account, "missing");
    cache.setQueryData(key, { body: "User's retained text", generation: "1" });
    next = { ...changePage("3"), reset_required: true };
    await client.wake();
    await Promise.resolve();
    expect(cache.getQueryData(key)).toEqual({
      body: "User's retained text",
      generation: "1",
    });
    expect(cache.getQueryState(key)?.isInvalidated).toBe(true);
    stop();
    cache.clear();
  });

  it("binds local list/export arguments to the actor and invalidates only its authored projections", async () => {
    let next = changePage("1");
    const drafts = vi.fn().mockResolvedValue({ drafts: [], next_cursor: null });
    const exportDraft = vi.fn().mockResolvedValue(false);
    const client = new CollaborationClient(
      transport({
        drafts,
        exportDraft,
        listen: async () => () => {},
        changesSince: async () => next,
      }),
    );
    const cache = new QueryClient();
    const stop = client.installBridge(cache);
    await client.wake();
    const handle = client.forAccount(account);
    await handle.drafts({ cursor: "cursor", limit: 50 });
    expect(drafts).toHaveBeenCalledWith({
      account_id: account.id,
      cursor: "cursor",
      limit: 50,
    });
    expect(await handle.exportDraft("missing", "7")).toBe(false);
    expect(exportDraft).toHaveBeenCalledWith(account.id, "missing", "7");
    const own = collaborationKeys.drafts(account, {
      account_id: account.id,
      cursor: null,
      limit: 50,
    });
    const other = collaborationKeys.draft(
      { ...account, id: "actor-b" },
      "missing",
    );
    const remote = collaborationKeys.item(account, "provider");
    for (const key of [own, other, remote]) cache.setQueryData(key, "data");
    next = changePage("2", "1", [
      { revision: "2", account_id: account.id, scope: "drafts", reset: false },
    ]);
    await client.wake();
    expect(cache.getQueryState(own)?.isInvalidated).toBe(true);
    expect(cache.getQueryState(other)?.isInvalidated).toBe(false);
    expect(cache.getQueryState(remote)?.isInvalidated).toBe(false);
    stop();
    cache.clear();
  });
});

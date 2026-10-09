import type {
  IssueDraftV2Snapshot,
  IssueMetadataPage,
  RemoteAccount,
  SaveIssueDraftV2Request,
} from "@gitru/commands";
import { QueryClient } from "@tanstack/react-query";
import { describe, expect, it, vi } from "vitest";
import { StaleAuthorizationError } from "./authorization-fence";
import {
  CollaborationClient,
  type CollaborationTransport,
  collaborationKeys,
} from "./client";

const account: RemoteAccount = {
  id: "account-a",
  actor_id: "7",
  provider: "github",
  host: "github.com",
  login: "fixture",
  display_name: null,
  authorization_epoch: "2",
  state: "active",
  notifications_supported: true,
};
const key = {
  account_id: account.id,
  repository_id: "github:repo:99",
  draft_id: "123e4567-e89b-42d3-a456-426614174000",
};
const selection = () => ({
  labels: [{ provider_id: "1", name: "bug", color: "ff0000" }],
  assignees: [{ provider_id: "7", login: "fixture" }],
  milestone: { provider_id: "88", number: "3", title: "Release" },
});
function draft(): IssueDraftV2Snapshot {
  return {
    draft: {
      ...key,
      title: "Keep title",
      body: "Keep body",
      generation: "1",
      context: {
        account_id: account.id,
        repository_id: key.repository_id,
        authorization_epoch: "2",
        authorization_view: "1",
        review_token: "a".repeat(64),
      },
      availability: "available",
      reason: null,
      submission: null,
      published: null,
      revision: "1",
      authorization_view: "1",
    },
    metadata: selection(),
    metadata_outcome: null,
  };
}
const query = {
  account_id: account.id,
  repository_id: key.repository_id,
  kind: "labels" as const,
  search: "",
  cursor: null,
  limit: 50,
};
function page(kind: IssueMetadataPage["kind"] = "labels"): IssueMetadataPage {
  return {
    account_id: account.id,
    repository_id: key.repository_id,
    kind,
    options: [],
    next_cursor: null,
    coverage: { state: "partial", validated_at: null, remote_has_more: false },
    freshness: "unknown",
    sync: {
      state: "idle",
      last_success_at: null,
      next_retry_at: null,
      error: null,
    },
    revision: "1",
    authorization_view: "1",
    catalog_revision: null,
  };
}
function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((done) => {
    resolve = done;
  });
  return { promise, resolve };
}
function transport(
  overrides: Partial<CollaborationTransport>,
): CollaborationTransport {
  const methods: Partial<CollaborationTransport> = {
    listen: async () => () => {},
    listenRuntimeReset: async () => () => {},
    listenLocalChanges: async () => () => {},
    changesSince: async () => ({
      revision: "1",
      authorization_view: "1",
      changes: [],
      reset_required: false,
      has_more: false,
    }),
    ...overrides,
  };
  return new Proxy(methods, {
    get(target, name) {
      return (
        target[name as keyof CollaborationTransport] ??
        (() => {
          throw new Error(`Unexpected native operation: ${String(name)}`);
        })
      );
    },
  }) as CollaborationTransport;
}

describe("issue metadata authority", () => {
  it("rejects an option from another catalog family", async () => {
    const wrong = page();
    wrong.options = [
      {
        reference: {
          kind: "assignee",
          value: { provider_id: "7", login: "fixture" },
        },
        availability: "available",
        reason: null,
      },
    ];
    const client = new CollaborationClient(
      transport({ issueMetadataOptions: async () => wrong }),
    );
    await expect(
      client.forAccount(account).issueMetadataOptions(query),
    ).rejects.toBeInstanceOf(StaleAuthorizationError);
  });

  it("binds historical outcomes to the canonical creation receipt rather than a newer draft submission", async () => {
    const saved = draft();
    saved.draft.published = {
      command_id: "confirmed-create",
      provider_id: "8",
      subject_id: "github:issue:8",
      number: "3",
      url: "https://github.com/owner/repo/issues/3",
    };
    saved.draft.submission = {
      command_id: "new-generation",
      draft_generation: "2",
      state: "queued",
      attempt_count: 0,
      quarantined: false,
      attention: null,
    };
    saved.metadata_outcome = {
      command_id: "confirmed-create",
      fields: [{ field: "labels", result: "different", reason: null }],
      needs_attention: true,
    };
    const client = new CollaborationClient(
      transport({ issueDraftV2: async () => saved }),
    );
    await expect(client.forAccount(account).issueDraftV2(key)).resolves.toEqual(
      saved,
    );
    saved.metadata_outcome.command_id = "new-generation";
    await expect(
      client.forAccount(account).issueDraftV2(key),
    ).rejects.toBeInstanceOf(StaleAuthorizationError);
  });

  it("rejects a catalog from another account, repository, family or authorization view", async () => {
    for (const replacement of [
      { account_id: "other" },
      { repository_id: "other" },
      { kind: "assignees" as const },
      { authorization_view: "2" },
    ]) {
      const client = new CollaborationClient(
        transport({
          issueMetadataOptions: async () => ({ ...page(), ...replacement }),
          issueDraftV2: async () => draft(),
        }),
      );
      await client.forAccount(account).issueDraftV2(key);
      await expect(
        client.forAccount(account).issueMetadataOptions(query),
      ).rejects.toBeInstanceOf(StaleAuthorizationError);
    }
  });

  it("captures every authored selection before a held save crosses IPC", async () => {
    const done = deferred<IssueDraftV2Snapshot>();
    let captured: SaveIssueDraftV2Request | undefined;
    const client = new CollaborationClient(
      transport({
        saveIssueDraftV2: (request) => {
          captured = request;
          return done.promise;
        },
      }),
    );
    const metadata = selection();
    const expected = selection();
    const saving = client.forAccount(account).saveIssueDraftV2({
      ...key,
      authorization_view: "1",
      expected_generation: "0",
      title: "Keep title",
      body: "Keep body",
      metadata,
    });
    metadata.labels[0].name = "changed";
    metadata.assignees[0].login = "changed";
    metadata.milestone.title = "changed";
    metadata.labels.push({ provider_id: "2", name: "late", color: "ffffff" });
    expect(captured?.metadata).toEqual(expected);
    expect(captured?.authorization_epoch).toBe("2");
    done.resolve(draft());
    await saving;
  });

  it("rejects v2 draft/save bindings and a metadata outcome without its creation receipt", async () => {
    const wrong = draft();
    wrong.draft.context = { ...wrong.draft.context!, authorization_epoch: "3" };
    const client = new CollaborationClient(
      transport({ issueDraftV2: async () => wrong }),
    );
    await expect(
      client.forAccount(account).issueDraftV2(key),
    ).rejects.toBeInstanceOf(StaleAuthorizationError);
    const unproven = draft();
    unproven.metadata_outcome = {
      command_id: "unproven",
      fields: [],
      needs_attention: false,
    };
    const other = new CollaborationClient(
      transport({ issueDraftV2: async () => unproven }),
    );
    await expect(
      other.forAccount(account).issueDraftV2(key),
    ).rejects.toBeInstanceOf(StaleAuthorizationError);
  });

  it("redacts receipt and catalog authority synchronously on disconnect while preserving authored metadata", async () => {
    const done = deferred<string>();
    const client = new CollaborationClient(
      transport({ disconnect: () => done.promise }),
    );
    const cache = new QueryClient();
    const stop = client.installBridge(cache);
    const localKey = collaborationKeys.issueDraftV2(account, key);
    const catalogKey = collaborationKeys.issueMetadataOptions(account, query);
    const saved = draft();
    saved.metadata_outcome = {
      command_id: "historical",
      fields: [{ field: "labels", result: "different", reason: null }],
      needs_attention: true,
    };
    cache.setQueryData(localKey, saved);
    cache.setQueryData(catalogKey, page());
    const pending = client.disconnect(account.id);
    expect(cache.getQueryData(localKey)).toEqual({
      ...saved,
      draft: {
        ...saved.draft,
        context: null,
        availability: "unavailable",
        reason: "account_unavailable",
        published: null,
      },
      metadata_outcome: null,
    });
    expect(cache.getQueryData(catalogKey)).toBeUndefined();
    expect(cache.getQueryState(localKey)?.isInvalidated).toBe(true);
    done.resolve("2");
    await pending;
    stop();
    cache.clear();
  });

  it("fences held catalog and authored-save responses after runtime reset", async () => {
    let reset: (() => void) | undefined;
    const catalog = deferred<IssueMetadataPage>();
    const saved = deferred<IssueDraftV2Snapshot>();
    const client = new CollaborationClient(
      transport({
        issueMetadataOptions: () => catalog.promise,
        saveIssueDraftV2: () => saved.promise,
        listenRuntimeReset: async (listener) => {
          reset = listener;
          return () => {};
        },
      }),
    );
    const cache = new QueryClient();
    const stop = client.installBridge(cache);
    await client.wake();
    const read = expect(
      client.forAccount(account).issueMetadataOptions(query),
    ).rejects.toBeInstanceOf(StaleAuthorizationError);
    const save = expect(
      client.forAccount(account).saveIssueDraftV2({
        ...key,
        authorization_view: "1",
        expected_generation: "0",
        title: "Saved",
        body: "Body",
        metadata: selection(),
      }),
    ).rejects.toBeInstanceOf(StaleAuthorizationError);
    reset?.();
    catalog.resolve(page());
    saved.resolve(draft());
    await Promise.all([read, save]);
    await client.wake();
    stop();
    cache.clear();
  });

  it("invalidates only the exact repository catalog family, including repository IDs with colons", async () => {
    let revision = "1";
    const client = new CollaborationClient(
      transport({
        changesSince: async () => ({
          revision,
          authorization_view: "1",
          reset_required: false,
          has_more: false,
          changes:
            revision === "2"
              ? [
                  {
                    revision: "2",
                    account_id: account.id,
                    scope: `repository_metadata:${key.repository_id}:labels`,
                    reset: false,
                  },
                ]
              : [],
        }),
      }),
    );
    const cache = new QueryClient();
    const stop = client.installBridge(cache);
    await client.wake();
    const labels = collaborationKeys.issueMetadataOptions(account, query);
    const assignees = collaborationKeys.issueMetadataOptions(account, {
      ...query,
      kind: "assignees",
    });
    const foreign = collaborationKeys.issueMetadataOptions(account, {
      ...query,
      repository_id: "github:repo:other",
    });
    const local = collaborationKeys.issueDraftV2(account, key);
    for (const k of [labels, assignees, foreign]) cache.setQueryData(k, page());
    cache.setQueryData(local, draft());
    revision = "2";
    await client.wake();
    expect(cache.getQueryState(labels)?.isInvalidated).toBe(true);
    for (const k of [assignees, foreign, local])
      expect(cache.getQueryState(k)?.isInvalidated).toBe(false);
    stop();
    cache.clear();
  });

  it("keeps v1/v2 cache shapes apart and preserves metadata consent with the captured submission UUID", async () => {
    expect(collaborationKeys.issueDraft(account, key)).not.toEqual(
      collaborationKeys.issueDraftV2(account, key),
    );
    const pending = deferred<{
      account_id: string;
      command_id: string;
      admitted_revision: string;
      duplicate: boolean;
    }>();
    const submit = vi.fn(() => pending.promise);
    const client = new CollaborationClient(
      transport({ submitIssueV2: submit }),
    );
    const request = {
      context: draft().draft.context!,
      draft_id: key.draft_id,
      draft_generation: "1",
      command_id: "123e4567-e89b-42d3-a456-426614174001",
      accept_background_delivery: true,
      accept_metadata_best_effort: true,
    };
    const original = structuredClone(request);
    const submitted = client.forAccount(account).submitIssueV2(request);
    request.command_id = "changed";
    request.context.review_token = "changed";
    request.accept_metadata_best_effort = false;
    expect(submit).toHaveBeenCalledWith(original);
    pending.resolve({
      account_id: account.id,
      command_id: original.command_id,
      admitted_revision: "2",
      duplicate: false,
    });
    await expect(submitted).resolves.toMatchObject({
      command_id: original.command_id,
    });
  });
});

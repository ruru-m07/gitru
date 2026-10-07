import {
  collaborationInbox,
  collaborationSetLocalInboxState,
  InboxPageSchema,
  LocalInboxFilterSchema,
  SetLocalInboxStateRequestSchema,
} from "@gitru/commands";
import { afterEach, describe, expect, it, vi } from "vitest";

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
afterEach(() => invoke.mockReset());

const query = {
  account_id: "account-a",
  remote_state: "unread",
  local_state: "bookmarked" as const,
  search: "review requested",
  cursor: null,
  limit: 50,
};
const localState = {
  disposition: "done" as const,
  effective_disposition: "inbox" as const,
  bookmarked: true,
  snoozed_until: null,
  activity_updated_at: "2026-10-06T12:00:00.000000000Z",
  superseded_by_activity: true,
  generation: "9007199254740993",
};
const page = {
  entries: [
    {
      item: {
        id: "notification-a",
        account_id: query.account_id,
        repository_id: "repository-a",
        provider_id: "9007199254740994",
        kind: "notification" as const,
        number: null,
        title: "Review requested",
        body: null,
        body_omitted: false,
        author: "reviewer",
        web_url: "https://github.com/example/repository/pull/67",
        state: "unread",
        updated_at: "2026-10-07T12:00:00.000000000Z",
        head_oid: null,
        is_draft: null,
        reason: "review_requested",
        unread: true,
      },
      local: localState,
    },
  ],
  revision: "9007199254740995",
  authorization_view: "9007199254740996",
  next_cursor: null,
  coverage: {
    state: "complete" as const,
    validated_at: "2026-10-07T12:00:00.000000000Z",
    remote_has_more: false,
  },
  sync: {
    state: "idle" as const,
    last_success_at: "2026-10-07T12:00:00.000000000Z",
    next_retry_at: null,
    error: null,
  },
  evaluated_at: "2026-10-07T12:01:00.000000000Z",
  next_local_change_at: null,
};

describe("generated local inbox IPC wire", () => {
  it("preserves provider state, independent local state and opaque generations", async () => {
    const parsed = InboxPageSchema.parse(page);
    expect(parsed.entries[0].local.effective_disposition).toBe("inbox");
    expect(parsed.entries[0].local.disposition).toBe("done");
    expect(parsed.entries[0].local.bookmarked).toBe(true);
    expect(parsed.entries[0].local.superseded_by_activity).toBe(true);
    expect(parsed.entries[0].local.generation).toBe("9007199254740993");
    for (const filter of ["inbox", "snoozed", "done", "bookmarked", "all"])
      expect(LocalInboxFilterSchema.parse(filter)).toBe(filter);
    invoke.mockResolvedValue(parsed);
    await expect(collaborationInbox({ query })).resolves.toEqual(parsed);
    expect(invoke).toHaveBeenCalledExactlyOnceWith("collaboration_inbox", {
      query,
    });
  });

  it("sends a single explicit intent with the captured CAS generation", async () => {
    const request = SetLocalInboxStateRequestSchema.parse({
      account_id: query.account_id,
      authorization_epoch: "9007199254740997",
      notification_id: page.entries[0].item.id,
      mutation: "bookmark",
      disposition: null,
      bookmarked: false,
      snoozed_until: null,
      expected_generation: localState.generation,
    });
    const receipt = {
      state: {
        ...localState,
        bookmarked: false,
        generation: "9007199254740998",
      },
      revision: "9007199254740999",
      authorization_view: page.authorization_view,
    };
    invoke.mockResolvedValue(receipt);
    await expect(collaborationSetLocalInboxState({ request })).resolves.toEqual(
      receipt,
    );
    expect(invoke).toHaveBeenCalledExactlyOnceWith(
      "collaboration_set_local_inbox_state",
      { request },
    );
  });

  it("rejects numeric generations at the renderer boundary", () => {
    expect(
      SetLocalInboxStateRequestSchema.safeParse({
        account_id: query.account_id,
        authorization_epoch: "1",
        notification_id: page.entries[0].item.id,
        mutation: "disposition",
        disposition: "done",
        bookmarked: null,
        snoozed_until: null,
        expected_generation: 9007199254740992,
      }).success,
    ).toBe(false);
  });
});

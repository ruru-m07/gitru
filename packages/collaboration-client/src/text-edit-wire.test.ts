import { TextEditRequestSchema, TextEditSnapshotSchema } from "@gitru/commands";
import { afterEach, describe, expect, it, vi } from "vitest";
import {
  collaboration,
  type RemoteAccount,
  StaleAuthorizationError,
  type TextEditRequest,
  type TextEditSnapshot,
} from "./index";

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
afterEach(() => invoke.mockReset());

const account: RemoteAccount = {
  id: "account-a",
  actor_id: "actor-a",
  provider: "github",
  host: "https://github.com",
  authorization_epoch: "9007199254740993",
  login: "fixture",
  state: "active",
  display_name: null,
  notifications_supported: true,
};
const context = {
  account_id: account.id,
  subject_id: "pull-42",
  authorization_epoch: account.authorization_epoch,
  authorization_view: "11",
  review_token: "native-review-token",
};

describe("cached title and body edit wire", () => {
  it("preserves nullable known body and explicit unavailable evidence", () => {
    const available = TextEditSnapshotSchema.parse({
      context,
      title: "Cached title",
      body: null,
      availability: "available",
      reason: null,
      pending_intent: null,
      revision: "20",
      authorization_view: context.authorization_view,
    });
    expect(available.body).toBeNull();
    expect(available.reason).toBeNull();

    const unavailable = TextEditSnapshotSchema.parse({
      context: null,
      title: null,
      body: null,
      availability: "unavailable",
      reason: "missing_base",
      pending_intent: null,
      revision: "21",
      authorization_view: "12",
    });
    expect(unavailable.context).toBeNull();
    expect(unavailable.reason).toBe("missing_base");
    expect(
      TextEditSnapshotSchema.safeParse({
        ...unavailable,
        reason: "last_write_wins",
      }).success,
    ).toBe(false);
    expect(
      TextEditSnapshotSchema.safeParse({
        ...unavailable,
        availability: "available",
      }).success,
    ).toBe(false);
  });

  it("keeps unchanged fields omitted while an empty body remains an explicit clear", () => {
    const request = TextEditRequestSchema.parse({
      context,
      command_id: "123e4567-e89b-42d3-a456-426614174000",
      accept_best_effort: true,
      title: null,
      body: "",
    });
    expect(request.title).toBeNull();
    expect(request.body).toBe("");
    expect(
      TextEditRequestSchema.safeParse({
        ...request,
        accept_best_effort: false,
      }).success,
    ).toBe(false);
    expect(
      TextEditRequestSchema.safeParse({
        ...request,
        title: null,
        body: null,
      }).success,
    ).toBe(false);
  });

  it("uses local snapshot IPC separately from durable intent admission", async () => {
    const snapshot: TextEditSnapshot = TextEditSnapshotSchema.parse({
      context,
      title: "Cached title",
      body: "Cached body",
      availability: "available",
      reason: null,
      pending_intent: null,
      revision: "20",
      authorization_view: context.authorization_view,
    });
    const request: TextEditRequest = {
      context,
      command_id: "123e4567-e89b-42d3-a456-426614174000",
      accept_best_effort: true,
      title: "Changed title",
      body: null,
    };
    invoke.mockResolvedValueOnce(snapshot).mockResolvedValueOnce({
      account_id: account.id,
      command_id: request.command_id,
      admitted_revision: "21",
      duplicate: false,
    });

    const client = collaboration.forAccount(account);
    expect(await client.textEditSnapshot(context.subject_id)).toEqual(snapshot);
    expect(await client.submitTextEdit(request)).toMatchObject({
      account_id: account.id,
      command_id: request.command_id,
    });
    expect(invoke.mock.calls).toEqual([
      [
        "collaboration_text_edit_snapshot",
        {
          accountId: account.id,
          subjectId: context.subject_id,
        },
      ],
      ["collaboration_submit_text_edit", { request }],
    ]);
  });

  it("rejects foreign native context before it can cross an account boundary", async () => {
    const foreign = TextEditSnapshotSchema.parse({
      context: { ...context, subject_id: "other" },
      title: "Foreign title",
      body: null,
      availability: "available",
      reason: null,
      pending_intent: null,
      revision: "20",
      authorization_view: context.authorization_view,
    });
    invoke.mockResolvedValue(foreign);
    await expect(
      collaboration.forAccount(account).textEditSnapshot(context.subject_id),
    ).rejects.toBeInstanceOf(StaleAuthorizationError);

    invoke.mockReset();
    await expect(
      collaboration.forAccount(account).submitTextEdit({
        context: { ...context, account_id: "other" },
        command_id: "123e4567-e89b-42d3-a456-426614174000",
        accept_best_effort: true,
        title: "Changed title",
        body: null,
      }),
    ).rejects.toBeInstanceOf(StaleAuthorizationError);
    expect(invoke).not.toHaveBeenCalled();
  });
});

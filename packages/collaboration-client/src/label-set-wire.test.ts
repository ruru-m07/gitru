import { LabelSetRequestSchema, LabelSetSnapshotSchema } from "@gitru/commands";
import { afterEach, describe, expect, it, vi } from "vitest";
import {
  collaboration,
  type LabelSetRequest,
  type LabelSetSnapshot,
  type RemoteAccount,
  StaleAuthorizationError,
} from "./index";

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
afterEach(() => invoke.mockReset());

const account: RemoteAccount = {
  id: "account-a",
  actor_id: "actor-a",
  provider: "github",
  host: "github.com",
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
const bug = { provider_id: "10", name: "bug", color: "d73a4a" };
const docs = { provider_id: "11", name: "docs/#?", color: null };

function available(): LabelSetSnapshot {
  return LabelSetSnapshotSchema.parse({
    context,
    canonical_labels: [bug],
    effective_labels: [bug],
    available_labels: [bug, docs],
    catalog_complete: false,
    catalog_truncated: true,
    availability: "available",
    reason: null,
    pending_intent: null,
    revision: "20",
    authorization_view: context.authorization_view,
  });
}

describe("cached GitHub label-set wire", () => {
  it("preserves nullable colors and explicit partial catalog evidence", () => {
    const snapshot = available();
    expect(snapshot.available_labels[1]?.color).toBeNull();
    expect(snapshot.catalog_complete).toBe(false);
    expect(snapshot.catalog_truncated).toBe(true);

    expect(
      LabelSetSnapshotSchema.safeParse({
        ...snapshot,
        catalog_complete: true,
      }).success,
    ).toBe(false);
    expect(
      LabelSetSnapshotSchema.safeParse({
        ...snapshot,
        available_labels: Array.from({ length: 101 }, (_, index) => ({
          provider_id: String(index + 1),
          name: `label-${index}`,
          color: null,
        })),
      }).success,
    ).toBe(false);

    const pending = LabelSetSnapshotSchema.parse({
      ...snapshot,
      context: null,
      availability: "unavailable",
      reason: "pending_intent",
      pending_intent: {
        subject_id: context.subject_id,
        commands: [
          { command_id: "queued", state: "queued", fields: ["labels"] },
        ],
      },
    });
    expect(pending.reason).toBe("pending_intent");
  });

  it("requires explicit consent and a bounded nonempty typed delta", () => {
    const request = LabelSetRequestSchema.parse({
      context,
      command_id: "123e4567-e89b-42d3-a456-426614174000",
      add_labels: [docs],
      remove_labels: [bug],
      accept_best_effort: true,
    });
    expect(request.add_labels[0]?.name).toBe("docs/#?");
    expect(
      LabelSetRequestSchema.safeParse({
        ...request,
        accept_best_effort: false,
      }).success,
    ).toBe(false);
    expect(
      LabelSetRequestSchema.safeParse({
        ...request,
        add_labels: [],
        remove_labels: [],
      }).success,
    ).toBe(false);
    expect(
      LabelSetRequestSchema.safeParse({
        ...request,
        add_labels: [{ ...docs, provider_id: "0" }],
      }).success,
    ).toBe(false);
    expect(
      LabelSetRequestSchema.safeParse({
        ...request,
        add_labels: [docs],
        remove_labels: [{ ...bug, name: "DOCS/#?" }],
      }).success,
    ).toBe(false);
    expect(
      LabelSetRequestSchema.safeParse({
        ...request,
        add_labels: [{ ...docs, name: ".." }],
        remove_labels: [],
      }).success,
    ).toBe(false);
  });

  it("uses cache-only snapshot IPC and clones the exact admitted delta", async () => {
    const snapshot = available();
    const request: LabelSetRequest = {
      context,
      command_id: "123e4567-e89b-42d3-a456-426614174000",
      add_labels: [docs],
      remove_labels: [bug],
      accept_best_effort: true,
    };
    invoke.mockResolvedValueOnce(snapshot).mockResolvedValueOnce({
      account_id: account.id,
      command_id: request.command_id,
      admitted_revision: "21",
      duplicate: false,
    });

    const client = collaboration.forAccount(account);
    expect(await client.labelSetSnapshot(context.subject_id)).toEqual(snapshot);
    const submitted = client.submitLabelSet(request);
    request.add_labels[0] = bug;
    expect(await submitted).toMatchObject({ account_id: account.id });
    expect(invoke.mock.calls).toEqual([
      [
        "collaboration_label_set_snapshot",
        { accountId: account.id, subjectId: context.subject_id },
      ],
      [
        "collaboration_submit_label_set",
        {
          request: {
            ...request,
            add_labels: [docs],
            remove_labels: [bug],
          },
        },
      ],
    ]);
  });

  it("rejects foreign contexts and receipts at the account boundary", async () => {
    invoke.mockResolvedValue({
      ...available(),
      context: { ...context, subject_id: "other" },
    });
    await expect(
      collaboration.forAccount(account).labelSetSnapshot(context.subject_id),
    ).rejects.toBeInstanceOf(StaleAuthorizationError);

    invoke.mockReset();
    await expect(
      collaboration.forAccount(account).submitLabelSet({
        context: { ...context, account_id: "other" },
        command_id: "123e4567-e89b-42d3-a456-426614174000",
        add_labels: [docs],
        remove_labels: [],
        accept_best_effort: true,
      }),
    ).rejects.toBeInstanceOf(StaleAuthorizationError);
    expect(invoke).not.toHaveBeenCalled();

    invoke.mockResolvedValue({
      account_id: "other",
      command_id: "123e4567-e89b-42d3-a456-426614174000",
      admitted_revision: "21",
      duplicate: false,
    });
    await expect(
      collaboration.forAccount(account).submitLabelSet({
        context,
        command_id: "123e4567-e89b-42d3-a456-426614174000",
        add_labels: [docs],
        remove_labels: [],
        accept_best_effort: true,
      }),
    ).rejects.toBeInstanceOf(StaleAuthorizationError);
  });
});

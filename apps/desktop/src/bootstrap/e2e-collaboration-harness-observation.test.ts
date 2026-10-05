import { webcrypto } from "node:crypto";
import { collaboration, collaborationKeys } from "@gitru/collaboration-client";
import {
  detailQueryOptions,
  itemQueryOptions,
} from "@gitru/collaboration-client/react";
import type { LocalDraft } from "@gitru/commands";
import { QueryClient } from "@tanstack/react-query";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  fixtureAccount,
  fixtureItem,
} from "../../tests/fixtures/collaboration";
import { fixtureBody } from "../../tests/fixtures/resource-detail";
import {
  installHarnessCatchupObservation,
  observeHarnessDocument,
  syntheticFingerprint,
} from "./e2e-collaboration-harness-observation";

let client: QueryClient;
let container: HTMLElement;
beforeEach(() => {
  vi.stubGlobal("crypto", webcrypto);
  client = new QueryClient();
  container = document.createElement("section");
});
afterEach(() => client.clear());

const bodyOptions = (account = fixtureAccount) =>
  detailQueryOptions(account, {
    subject_id: fixtureItem.id,
    facet: "body",
    cursor: null,
    limit: 50,
  });
const observe = (account = fixtureAccount) =>
  observeHarnessDocument({
    client,
    container,
    documentNonce: "document-103",
    actor: "primary",
    account,
    subjectId: fixtureItem.id,
  });

describe("synthetic document observations", () => {
  it("fingerprints exact Unicode values without returning text or triggering IPC", async () => {
    const text = "Synthetic Body 日本語\nλ\n";
    client.setQueryData(
      bodyOptions().queryKey,
      fixtureBody({ body: { state: "known", text } }),
    );
    client.setQueryData(
      itemQueryOptions(fixtureAccount, fixtureItem.id).queryKey,
      {
        item: fixtureItem,
        revision: "9007199254740993",
        authorization_view: "1",
      },
    );
    container.innerHTML =
      '<article aria-label="Saved item detail"><header aria-label="Selected resource metadata"></header></article>';
    const snapshot = await observe();
    expect(snapshot.body_hash).toBe(await syntheticFingerprint(text));
    expect(snapshot.metadata_hash).toMatch(/^[a-f0-9]{64}$/);
    expect(snapshot.item.revision).toBe("9007199254740993");
    expect(snapshot.provider_visible).toBe(true);
    expect(snapshot.collaboration_query_count).toBe(2);
    expect(JSON.stringify(snapshot)).not.toContain(text);
    expect(JSON.stringify(snapshot)).not.toContain(
      "Authoritative detail title",
    );
  });

  it("observes only the exact account and epoch even when canonical subject IDs match", async () => {
    client.setQueryData(bodyOptions().queryKey, fixtureBody());
    const other = { ...fixtureAccount, id: "other-account", actor_id: "456" };
    const nextEpoch = { ...fixtureAccount, authorization_epoch: "2" };
    for (const account of [other, nextEpoch]) {
      const snapshot = await observe(account);
      expect(snapshot.body.status).toBe("absent");
      expect(snapshot.body_hash).toBeNull();
      expect(snapshot.metadata_hash).toBeNull();
      expect(snapshot.provider_visible).toBe(false);
      expect(snapshot.account_id).toBe(account.id);
    }
  });

  it("distinguishes authoritative null, authoritative empty, and an omitted observation", async () => {
    const emptyHash = await syntheticFingerprint("");
    client.setQueryData(
      bodyOptions().queryKey,
      fixtureBody({ body: { state: "known", text: null } }),
    );
    expect((await observe()).body_value_state).toBe("known");
    expect((await observe()).body_hash).toBeNull();
    client.setQueryData(
      bodyOptions().queryKey,
      fixtureBody({ body: { state: "known", text: "" } }),
    );
    expect((await observe()).body_hash).toBe(emptyHash);
    client.setQueryData(
      bodyOptions().queryKey,
      fixtureBody({ body: { state: "omitted", text: null } }),
    );
    expect((await observe()).body_value_state).toBe("omitted");
    expect((await observe()).body_hash).toBeNull();
  });

  it("observes a real cold Body receipt before foreground hydration has run", async () => {
    client.setQueryData(
      bodyOptions().queryKey,
      fixtureBody({ body: { state: "not_loaded", text: null } }),
    );
    const snapshot = await observe();
    expect(snapshot.body.status).toBe("success");
    expect(snapshot.body_value_state).toBe("not_loaded");
    expect(snapshot.body_hash).toBeNull();
    expect(snapshot.body.revision).not.toBeNull();
  });

  it("reports current saved generation separately from retained unsaved editor text", async () => {
    const savedDraft: LocalDraft = {
      account_id: fixtureAccount.id,
      subject_id: fixtureItem.id,
      body: "Synthetic saved generation two",
      generation: "2",
    };
    client.setQueryData<LocalDraft>(
      collaborationKeys.draft(fixtureAccount, fixtureItem.id),
      savedDraft,
    );
    const editor = document.createElement("textarea");
    editor.name = "private-draft";
    editor.value = "Synthetic retained generation one edit";
    const conflict = document.createElement("p");
    conflict.textContent =
      "This draft changed in another tab. Your text is still here.";
    const save = document.createElement("button");
    save.type = "submit";
    save.textContent = "Save draft";
    save.disabled = true;
    container.append(editor, conflict, save);
    const snapshot = await observe();
    expect(snapshot.draft_generation).toBe("2");
    expect(snapshot.editor_hash).toBe(await syntheticFingerprint(editor.value));
    expect(snapshot.editor_hash).not.toBe(snapshot.saved_draft_hash);
    expect(snapshot.conflict_visible).toBe(true);
    expect(snapshot.save_enabled).toBe(false);
  });

  it("refuses oversized diagnostics rather than returning unbounded content", async () => {
    await expect(syntheticFingerprint("a".repeat(1_048_577))).rejects.toThrow(
      "bounded text limit",
    );
  });

  it("observes transport receipts without changing their object or arguments", async () => {
    const receipt = {
      revision: "9007199254740993",
      authorization_view: "1",
      changes: [],
      has_more: true,
      reset_required: true,
    };
    const source = vi
      .spyOn(collaboration.transport, "changesSince")
      .mockResolvedValue(receipt);
    const observer = installHarnessCatchupObservation();
    try {
      expect(await collaboration.transport.changesSince("256")).toBe(receipt);
      expect(source).toHaveBeenCalledExactlyOnceWith("256");
      expect(observer.snapshot()).toEqual({
        reads: 1,
        pages_with_more: 1,
        resets: 1,
        last_request: "256",
        last_receipt: receipt.revision,
      });
    } finally {
      observer.stop();
    }
    expect(collaboration.transport.changesSince).toBe(source);
  });

  it("does not update diagnostic state from a delayed receipt after teardown", async () => {
    let finish!: (receipt: {
      revision: string;
      authorization_view: string;
      changes: [];
      has_more: boolean;
      reset_required: boolean;
    }) => void;
    vi.spyOn(collaboration.transport, "changesSince").mockImplementation(
      () =>
        new Promise((resolve) => {
          finish = resolve;
        }),
    );
    const observer = installHarnessCatchupObservation();
    const pending = collaboration.transport.changesSince("0");
    observer.stop();
    finish({
      revision: "1",
      authorization_view: "1",
      changes: [],
      has_more: false,
      reset_required: false,
    });
    await pending;
    expect(observer.snapshot().reads).toBe(0);
  });
});

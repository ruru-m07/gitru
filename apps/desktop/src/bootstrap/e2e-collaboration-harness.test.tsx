import { webcrypto } from "node:crypto";
import { collaboration } from "@gitru/collaboration-client";
import { detailQueryOptions } from "@gitru/collaboration-client/react";
import { QueryClient } from "@tanstack/react-query";
import { act, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { HarnessAction } from "../../e2e/protocol/collaboration-harness";
import { fixtureAccount } from "../../tests/fixtures/collaboration";
import { fixtureBody } from "../../tests/fixtures/resource-detail";
import { syntheticFingerprint } from "./e2e-collaboration-harness-observation";

const native = vi.hoisted(() => ({
  label: "main",
  listen: vi.fn(async () => vi.fn()),
  emitTo: vi.fn(async () => undefined),
  control: vi.fn(async () => undefined),
  activity: vi.fn(async () => ({ generation: "7", active: true })),
  renew: vi.fn(async () => undefined),
  release: vi.fn(async () => undefined),
  disconnect: vi.fn(async () => undefined),
  inspectOwner: vi.fn(async () => undefined),
  setOwner: vi.fn(async () => undefined),
  disposeOwner: vi.fn(async () => undefined),
  connect: vi.fn(async () => undefined),
  edit: vi.fn(),
  submit: vi.fn(),
}));
vi.mock("@tauri-apps/api/webview", () => ({
  getCurrentWebview: () => ({
    label: native.label,
    listen: native.listen,
    emitTo: native.emitTo,
  }),
}));
vi.mock("@gitru/commands", async (original) => ({
  ...(await original<object>()),
  collaborationHarnessControl: native.control,
  collaborationDemandActivity: native.activity,
  collaborationRenewDemand: native.renew,
  collaborationReleaseDemand: native.release,
  collaborationDisconnect: native.disconnect,
  collaborationInspectDemandOwner: native.inspectOwner,
  collaborationSetDemandOwnerActivity: native.setOwner,
  collaborationDisposeDemandOwner: native.disposeOwner,
  collaborationConnectGithub: native.connect,
}));
vi.mock("@gitru/collaboration-client/react", async (original) => ({
  ...(await original<object>()),
  useCollaborationAccounts: () => ({
    data: { accounts: [fixtureAccount] },
  }),
}));
// Unit lifecycle fixture only. The packaged lane mounts the actual component.
// This isolates whether a new native phase accidentally replaces its editor.
vi.mock("../features/collaboration/saved-item-detail", async () => {
  const { useState } = await import("react");
  return {
    SavedItemDetail: () => {
      const [text, setText] = useState("Synthetic authored editor");
      return (
        <article aria-label="Saved item detail">
          <form
            onSubmit={(event) => {
              event.preventDefault();
              native.submit();
            }}
          >
            <textarea
              name="private-draft"
              value={text}
              onChange={(event) => {
                native.edit(event.currentTarget.value);
                setText(event.currentTarget.value);
              }}
            />
            <button type="submit">Save draft</button>
          </form>
        </article>
      );
    },
  };
});

import { installCollaborationHarnessProbe } from "./e2e-collaboration-harness";

let probe: Awaited<ReturnType<typeof installCollaborationHarnessProbe>> | null;
beforeEach(() => {
  vi.stubGlobal("crypto", webcrypto);
  native.label = "main";
  vi.clearAllMocks();
  probe = null;
});
afterEach(async () => {
  await act(async () => probe?.stop());
});

function fixture() {
  let manifest = {
    run_nonce: "run-103",
    session_id: "session-103",
    scenario_generation: "1",
    webview_label: native.label,
    role:
      native.label === "main"
        ? ("main" as const)
        : ("concurrent_child" as const),
    actors: [
      {
        slot: "primary" as const,
        account_id: fixtureAccount.id,
        authorization_epoch: fixtureAccount.authorization_epoch,
        instance_id: "github:github.com",
        repository_id: "fixture-repository",
        subject_id: "fixture-subject",
      },
    ],
  };
  const readManifest = vi.fn(async () => structuredClone(manifest));
  const request = (action: HarnessAction) => ({
    run_nonce: manifest.run_nonce,
    scenario_generation: manifest.scenario_generation,
    request_id: crypto.randomUUID(),
    label: native.label,
    action,
  });
  return {
    readManifest,
    request,
    phase() {
      manifest = { ...manifest, scenario_generation: "2" };
    },
    retire() {
      manifest = { ...manifest, session_id: "new-session-103" };
    },
  };
}

describe("retained probe document lifetime", () => {
  it("observes actual document visibility and own native activity without setting host activity", async () => {
    const source = fixture();
    vi.spyOn(document, "visibilityState", "get").mockReturnValue("hidden");
    native.activity.mockResolvedValueOnce({
      generation: "9007199254740993",
      active: false,
    });
    await act(async () => {
      probe = await installCollaborationHarnessProbe(source.readManifest);
    });
    const receipt = await probe?.execute(
      source.request({ kind: "inspect-activity" }),
    );
    expect(receipt?.outcome).toBe("accepted");
    expect(receipt?.activity).toEqual({
      document_visibility: "hidden",
      own_activity: { generation: "9007199254740993", active: false },
    });
    expect(native.activity).toHaveBeenCalledExactlyOnceWith({});
    expect(native.setOwner).not.toHaveBeenCalled();
    expect(native.control).not.toHaveBeenCalled();
    expect(receipt?.snapshot?.mounted).toBe(false);
  });

  it("fences an old activity observation before reading native ownership", async () => {
    const source = fixture();
    await act(async () => {
      probe = await installCollaborationHarnessProbe(source.readManifest);
    });
    const old = source.request({ kind: "inspect-activity" });
    source.phase();
    expect((await probe?.execute(old))?.outcome).toBe("stale_view");
    expect(native.activity).not.toHaveBeenCalled();
  });
  it("requires the actual native view label before creating renderer state", async () => {
    const wrong = fixture();
    native.label = "tab-webview:other";
    await expect(
      installCollaborationHarnessProbe(wrong.readManifest),
    ).rejects.toThrow("actual native view");
    expect(
      document.querySelector('[aria-label="Retained collaboration fixture"]'),
    ).toBeNull();
  });

  it("keeps the same dirty editor when a provider phase changes its request fence", async () => {
    const source = fixture();
    await act(async () => {
      probe = await installCollaborationHarnessProbe(source.readManifest);
      await probe.execute(source.request({ kind: "mount", actor: "primary" }));
    });
    await waitFor(() =>
      expect(
        document.querySelector('textarea[name="private-draft"]'),
      ).not.toBeNull(),
    );
    await act(async () => {
      await probe?.execute(
        source.request({ kind: "edit-draft", variant: "first-edit" }),
      );
    });
    const before = await probe?.inspect();
    const editor = document.querySelector('textarea[name="private-draft"]');
    source.phase();
    let receipt:
      | Awaited<ReturnType<NonNullable<typeof probe>["execute"]>>
      | undefined;
    await act(async () => {
      receipt = await probe?.execute(source.request({ kind: "inspect" }));
    });
    expect(receipt?.outcome).toBe("accepted");
    expect(document.querySelector('textarea[name="private-draft"]')).toBe(
      editor,
    );
    expect((await probe?.inspect())?.editor_hash).toBe(before?.editor_hash);
    expect(before?.editor_hash).not.toBe(
      await syntheticFingerprint("Synthetic authored editor"),
    );
  });

  it("rejects old scenario and process incarnations without a DOM action", async () => {
    const source = fixture();
    await act(async () => {
      probe = await installCollaborationHarnessProbe(source.readManifest);
    });
    const old = source.request({ kind: "mount", actor: "primary" });
    source.phase();
    expect((await probe?.execute(old))?.outcome).toBe("stale_view");
    expect((await probe?.inspect())?.actor).toBeNull();
    source.retire();
    expect(
      (
        await probe?.execute(
          source.request({ kind: "mount", actor: "primary" }),
        )
      )?.outcome,
    ).toBe("stale_view");
    expect((await probe?.inspect())?.actor).toBeNull();
  });

  it("keeps a new phase local read fenced until React commits its real binding", async () => {
    const source = fixture();
    await act(async () => {
      probe = await installCollaborationHarnessProbe(source.readManifest);
      await probe.execute(source.request({ kind: "mount", actor: "primary" }));
    });
    await waitFor(() =>
      expect(
        document.querySelector('textarea[name="private-draft"]'),
      ).not.toBeNull(),
    );
    const editor = document.querySelector('textarea[name="private-draft"]');
    const localRead = vi
      .spyOn(collaboration.transport, "detail")
      .mockResolvedValue(fixtureBody());
    source.phase();
    let first:
      | Awaited<ReturnType<NonNullable<typeof probe>["execute"]>>
      | undefined;
    await act(async () => {
      first = await probe?.execute(source.request({ kind: "read-body" }));
      expect(first?.outcome).toBe("not_ready");
      expect(localRead).not.toHaveBeenCalled();
    });
    let second:
      | Awaited<ReturnType<NonNullable<typeof probe>["execute"]>>
      | undefined;
    await act(async () => {
      second = await probe?.execute(source.request({ kind: "read-body" }));
    });
    expect(second?.outcome).toBe("accepted");
    expect(localRead).toHaveBeenCalledOnce();
    expect(document.querySelector('textarea[name="private-draft"]')).toBe(
      editor,
    );
    localRead.mockRestore();
  });

  it("distinguishes actual TanStack cached cancellation from a held native SDK completion", async () => {
    const client = new QueryClient();
    const options = detailQueryOptions(fixtureAccount, {
      subject_id: "fixture-subject",
      facet: "body",
      cursor: null,
      limit: 50,
    });
    const cached = fixtureBody();
    client.setQueryData(options.queryKey, cached);
    let finish!: (body: ReturnType<typeof fixtureBody>) => void;
    const nativeRead = vi
      .spyOn(collaboration.transport, "detail")
      .mockImplementationOnce(
        () =>
          new Promise((resolve) => {
            finish = resolve;
          }),
      );
    try {
      const refetch = client.fetchQuery({ ...options, staleTime: 0 });
      await waitFor(() => expect(nativeRead).toHaveBeenCalledOnce());
      await client.cancelQueries({ queryKey: options.queryKey, exact: true });
      // Real TanStack reverts the query and resolves its old cached value. A
      // fetchQuery acceptance is therefore not the held SDK completion proof.
      await expect(refetch).resolves.toBe(cached);
      finish(fixtureBody());
    } finally {
      nativeRead.mockRestore();
      client.clear();
    }
  });

  it("reports the real SDK fence error from an obsolete held native Body", async () => {
    const source = fixture();
    await act(async () => {
      probe = await installCollaborationHarnessProbe(source.readManifest);
      await probe.execute(source.request({ kind: "mount", actor: "primary" }));
    });
    await waitFor(() =>
      expect(
        document.querySelector('textarea[name="private-draft"]'),
      ).not.toBeNull(),
    );
    let finish!: (body: ReturnType<typeof fixtureBody>) => void;
    const nativeRead = vi
      .spyOn(collaboration.transport, "detail")
      .mockImplementationOnce(
        () =>
          new Promise((resolve) => {
            finish = resolve;
          }),
      );
    const disconnect = vi
      .spyOn(collaboration.transport, "disconnect")
      .mockResolvedValue("2");
    try {
      const delayed = probe?.execute(source.request({ kind: "read-body" }));
      await waitFor(() => expect(nativeRead).toHaveBeenCalledOnce());
      // The unchanged singleton SDK invalidates its real fence. Only native
      // transport I/O is a unit double; no stale result/error is fabricated.
      await collaboration.disconnect(fixtureAccount.id);
      finish(fixtureBody());
      expect((await delayed)?.outcome).toBe("stale_view");
      expect(nativeRead).toHaveBeenCalledExactlyOnceWith({
        account_id: fixtureAccount.id,
        subject_id: "fixture-subject",
        facet: "body",
        cursor: null,
        limit: 50,
      });
    } finally {
      nativeRead.mockRestore();
      disconnect.mockRestore();
    }
  });

  it.each([
    { kind: "edit-draft", variant: "first-edit" },
    { kind: "save-draft" },
  ] satisfies HarnessAction[])("keeps $kind free of DOM side effects until the current phase commits", async (action) => {
    const source = fixture();
    await act(async () => {
      probe = await installCollaborationHarnessProbe(source.readManifest);
      await probe.execute(source.request({ kind: "mount", actor: "primary" }));
    });
    await waitFor(() =>
      expect(
        document.querySelector('textarea[name="private-draft"]'),
      ).not.toBeNull(),
    );
    const editor = document.querySelector('textarea[name="private-draft"]');
    source.phase();
    await act(async () => {
      const first = await probe?.execute(source.request(action));
      expect(first?.outcome).toBe("not_ready");
      expect(native.edit).not.toHaveBeenCalled();
      expect(native.submit).not.toHaveBeenCalled();
    });
    let second:
      | Awaited<ReturnType<NonNullable<typeof probe>["execute"]>>
      | undefined;
    await act(async () => {
      second = await probe?.execute(source.request(action));
    });
    expect(second?.outcome).toBe("accepted");
    expect(
      action.kind === "edit-draft" ? native.edit : native.submit,
    ).toHaveBeenCalledOnce();
    expect(
      action.kind === "edit-draft" ? native.submit : native.edit,
    ).not.toHaveBeenCalled();
    expect(document.querySelector('textarea[name="private-draft"]')).toBe(
      editor,
    );
  });

  it("uses the same generated controller for a real child's authority check", async () => {
    native.label = "tab-webview:ruru103:actual-child";
    native.control.mockRejectedValueOnce({ code: "permission_denied" });
    const source = fixture();
    await act(async () => {
      probe = await installCollaborationHarnessProbe(source.readManifest);
    });
    const receipt = await probe?.execute(
      source.request({ kind: "check-control-authority" }),
    );
    expect(receipt?.outcome).toBe("permission_denied");
    expect(native.control).toHaveBeenCalledWith({
      request: {
        run_nonce: "run-103",
        expected_generation: "1",
        action: "core",
        core_action: "cancel_gates",
        gate_id: null,
      },
    });
  });

  it("forwards the bounded peer packet through generated calls using its own owner receipt", async () => {
    native.label = "tab-webview:ruru103:actual-child";
    for (const command of [
      native.control,
      native.renew,
      native.release,
      native.disconnect,
      native.inspectOwner,
      native.setOwner,
      native.disposeOwner,
      native.connect,
    ])
      command.mockRejectedValue({ code: "permission_denied" });
    const source = fixture();
    const peer = {
      lease_id: "a8b2f06a-5d3e-40f7-9f98-aaf24112c2af",
      owner_label: "main" as const,
      owner_generation: "5",
      account_id: fixtureAccount.id,
      authorization_epoch: fixtureAccount.authorization_epoch,
    };
    await act(async () => {
      probe = await installCollaborationHarnessProbe(source.readManifest);
    });
    const receipt = await probe?.execute(
      source.request({ kind: "check-peer-authority", peer }),
    );
    expect(receipt?.outcome).toBe("accepted");
    expect(Object.values(receipt?.authority ?? {})).toEqual(
      Array(8).fill("permission_denied"),
    );
    expect(native.activity).toHaveBeenCalledExactlyOnceWith({});
    expect(native.renew).toHaveBeenCalledExactlyOnceWith({
      request: {
        owner_generation: "7",
        leases: [
          {
            lease_id: peer.lease_id,
            account_id: peer.account_id,
            authorization_epoch: peer.authorization_epoch,
          },
        ],
      },
    });
    expect(native.release).toHaveBeenCalledExactlyOnceWith({
      request: { lease_id: peer.lease_id },
    });
    expect(native.disconnect).toHaveBeenCalledExactlyOnceWith({
      accountId: fixtureAccount.id,
    });
    expect(native.setOwner).toHaveBeenCalledExactlyOnceWith({
      ownerLabel: "main",
      expectedGeneration: "5",
      active: false,
    });
    expect(native.disposeOwner).toHaveBeenCalledExactlyOnceWith({
      ownerLabel: "main",
      expectedGeneration: "5",
    });
    expect(native.connect).toHaveBeenCalledExactlyOnceWith({
      token: "ruru103-synthetic-not-a-personal-token",
    });
    // These are protocol-boundary unit doubles, not native denial evidence.
    // Preserve an unexpected actual acceptance for the packaged executor to
    // fail rather than normalizing it into a passing denial.
    native.release.mockResolvedValueOnce(undefined);
    const unexpected = await probe?.execute(
      source.request({ kind: "check-peer-authority", peer }),
    );
    expect(unexpected?.authority?.release_lease).toBe("accepted");
  });

  it("rejects a foreign actor or stale epoch before any peer credential/host call", async () => {
    native.label = "tab-webview:ruru103:actual-child";
    const source = fixture();
    const peer = {
      lease_id: "a8b2f06a-5d3e-40f7-9f98-aaf24112c2af",
      owner_label: "main" as const,
      owner_generation: "5",
      account_id: fixtureAccount.id,
      authorization_epoch: fixtureAccount.authorization_epoch,
    };
    await act(async () => {
      probe = await installCollaborationHarnessProbe(source.readManifest);
    });
    for (const changed of [
      { ...peer, account_id: "foreign-account" },
      { ...peer, authorization_epoch: "2" },
    ]) {
      const receipt = await probe?.execute(
        source.request({ kind: "check-peer-authority", peer: changed }),
      );
      expect(receipt?.outcome).toBe("permission_denied");
      expect(receipt?.authority).toBeUndefined();
    }
    expect(native.activity).not.toHaveBeenCalled();
    expect(native.renew).not.toHaveBeenCalled();
    expect(native.disconnect).not.toHaveBeenCalled();
    expect(native.setOwner).not.toHaveBeenCalled();
    expect(native.connect).not.toHaveBeenCalled();
  });

  it("retires listener and rendered content once without adding a pagehide lease cleanup", async () => {
    const source = fixture();
    const remove = vi.fn();
    native.listen.mockResolvedValueOnce(remove);
    await act(async () => {
      probe = await installCollaborationHarnessProbe(source.readManifest);
    });
    // An actual reload is allowed to lose JS cleanup, leaving native TTL to
    // qualify cleanup. This harness must not add a fake lease-release hook.
    window.dispatchEvent(new Event("pagehide"));
    expect(remove).not.toHaveBeenCalled();
    await act(async () => {
      probe?.stop();
      probe?.stop();
    });
    expect(remove).toHaveBeenCalledOnce();
    expect(
      document.querySelector('[aria-label="Retained collaboration fixture"]'),
    ).toBeNull();
  });
});

import { webcrypto } from "node:crypto";
import type { HarnessStatus, HarnessViewManifest } from "@gitru/commands";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  type HarnessProbeSnapshot,
  type HarnessRequest,
  type HarnessResult,
  HarnessScenarioResultSchema,
} from "../../e2e/protocol/collaboration-harness";
import { syntheticFingerprint } from "./e2e-collaboration-harness-observation";

const native = vi.hoisted(() => ({
  status: vi.fn(),
  manifest: vi.fn(),
  control: vi.fn(),
  inspectOwner: vi.fn(),
  setOwner: vi.fn(),
  navigate: vi.fn(),
  wake: vi.fn(),
  createTab: vi.fn(),
  activateTab: vi.fn(),
  closeTab: vi.fn(),
  getAll: vi.fn(),
  listen: vi.fn(),
  emitTo: vi.fn(),
  acquire: vi.fn(),
  release: vi.fn(),
  renew: vi.fn(),
  accounts: vi.fn(),
}));
vi.mock("@gitru/commands", async (original) => ({
  ...(await original<object>()),
  collaborationHarnessStatus: native.status,
  collaborationHarnessViewManifest: native.manifest,
  collaborationHarnessControl: native.control,
  collaborationInspectDemandOwner: native.inspectOwner,
  collaborationSetDemandOwnerActivity: native.setOwner,
  collaborationAcquireDemand: native.acquire,
  collaborationReleaseDemand: native.release,
  collaborationRenewDemand: native.renew,
}));
vi.mock("@gitru/collaboration-client", () => ({
  collaboration: { wake: native.wake, accounts: native.accounts },
}));
vi.mock("@tauri-apps/api/webview", () => ({
  getCurrentWebview: () => ({
    label: "main",
    listen: native.listen,
    emitTo: native.emitTo,
  }),
  Webview: { getAll: native.getAll },
}));
vi.mock("../features/collaboration/account-dialog-events", () => ({
  requestAccountSettings: vi.fn(),
}));
vi.mock("../store/use-app-store", () => ({
  useAppStore: {
    getState: () => ({
      createTab: native.createTab,
      activateTab: native.activateTab,
      closeTab: native.closeTab,
    }),
  },
}));
vi.mock("./create-router", () => ({ router: { navigate: native.navigate } }));

import { installCollaborationHarnessExecutor } from "./e2e-collaboration-harness-executor";

function statusFixture(): HarnessStatus {
  return {
    core: {
      run_nonce: "run-103",
      session_id: "session-103",
      scenario_generation: "1",
      prepared: true,
      fixture: "primary",
      phase: "one",
      revision: "18",
      actors: [],
      calls: [],
      gates: [],
      provider_call_count: "0",
      vault_load_count: "0",
      vault_store_count: "0",
      vault_delete_count: "0",
      durable_detail_requests: 0,
      demand_lease_count: 0,
      clock_elapsed_seconds: 0,
      committed_phase: null,
      committed_facet_revision: null,
    },
    child_label: null,
    hint_mode: "normal",
    held_hint_revisions: [],
    local_reads: [],
    authorized_hydrate_requests: "0",
    checkpoint: null,
    process_id: 103,
  };
}

let current: HarnessStatus;
let executor: Awaited<
  ReturnType<typeof installCollaborationHarnessExecutor>
> | null;
const events: string[] = [];
let resultListener: ((event: { payload: unknown }) => void) | null = null;
beforeEach(() => {
  vi.stubGlobal("crypto", webcrypto);
  current = statusFixture();
  executor = null;
  events.length = 0;
  resultListener = null;
  native.listen.mockImplementation(async (_event, handler) => {
    resultListener = handler;
    return () => {
      resultListener = null;
    };
  });
  native.status.mockImplementation(async () => structuredClone(current));
  native.manifest.mockResolvedValue({
    run_nonce: "run-103",
    session_id: "session-103",
    scenario_generation: "1",
    webview_label: "main",
    role: "main",
    actors: [],
  } satisfies HarnessViewManifest);
  native.inspectOwner.mockResolvedValue({ generation: "7", active: true });
  native.setOwner.mockResolvedValue({ generation: "7", active: true });
  native.control.mockImplementation(async ({ request }) => {
    events.push(`control:${request.action}:${request.core_action ?? "none"}`);
    return { status: structuredClone(current), gate_id: null };
  });
  native.getAll.mockResolvedValue([]);
  native.navigate.mockResolvedValue(undefined);
  native.createTab
    .mockReturnValueOnce({ id: "first" })
    .mockReturnValueOnce({ id: "second" });
});
afterEach(() => {
  executor?.stop();
  vi.useRealTimers();
});

function probeFixture() {
  const execute = vi.fn(
    async (request: HarnessRequest): Promise<HarnessResult> => {
      events.push(`probe:${request.action.kind}`);
      return {
        run_nonce: request.run_nonce,
        scenario_generation: request.scenario_generation,
        request_id: request.request_id,
        label: request.label,
        outcome: "accepted",
        snapshot: null,
        ...(request.action.kind === "inspect-activity"
          ? {
              activity: {
                document_visibility: "hidden" as const,
                own_activity: { generation: "7", active: false },
              },
            }
          : {}),
      };
    },
  );
  return { execute, inspect: vi.fn(), stop: vi.fn() };
}

describe("retained finite executor failure evidence", () => {
  it("preserves the first native failure and captures real observations before cleanup", async () => {
    const probe = probeFixture();
    native.control.mockImplementation(async ({ request }) => {
      events.push(`control:${request.action}:${request.core_action ?? "none"}`);
      if (request.core_action === "arm_provider_gate")
        throw { code: "provider", message: "private" };
      if (request.action === "cancel_local_reads")
        throw { code: "storage", message: "private" };
      return { status: structuredClone(current), gate_id: null };
    });
    executor = await installCollaborationHarnessExecutor(probe);
    const result = HarnessScenarioResultSchema.parse(
      await executor.runScenario("concurrent-demand"),
    );
    expect(result.stage).toBe("arm missing Body response");
    expect(result.failure).toEqual({ kind: "native_error", code: "provider" });
    expect(result.cleanup_failure).toEqual({
      kind: "native_error",
      code: "storage",
    });
    expect(result.cleanup_stage).toBe("restore_native_scenario");
    expect(result.failure_context?.activities).toEqual([
      {
        label: "main",
        document_visibility: "hidden",
        own_activity: { generation: "7", active: false },
        native_owner: { generation: "7", active: true },
        probe_failure: null,
        owner_failure: null,
      },
    ]);
    expect(events.indexOf("probe:inspect-activity")).toBeLessThan(
      events.indexOf("probe:detach"),
    );
    expect(JSON.stringify(result)).not.toContain("private");
  });

  it("accepts the new native generation from an actual false-to-true setter transition", async () => {
    native.inspectOwner
      .mockResolvedValue({ generation: "8", active: true })
      .mockResolvedValueOnce({ generation: "7", active: false });
    native.setOwner.mockResolvedValue({ generation: "8", active: true });
    native.control.mockRejectedValue({ code: "provider" });
    executor = await installCollaborationHarnessExecutor(probeFixture());
    const result = await executor.runScenario("concurrent-demand");
    expect(result.stage).toBe("arm missing Body response");
    expect(result.failure).toEqual({ kind: "native_error", code: "provider" });
    expect(native.setOwner).toHaveBeenCalledExactlyOnceWith({
      ownerLabel: "main",
      expectedGeneration: "7",
      active: true,
    });
  });

  it("waits for actual visibility activation at a newer native generation after an inactive setter", async () => {
    native.setOwner.mockResolvedValue({ generation: "7", active: false });
    native.inspectOwner
      .mockResolvedValueOnce({ generation: "7", active: false })
      .mockResolvedValueOnce({ generation: "8", active: true });
    native.control.mockRejectedValue({ code: "provider" });
    executor = await installCollaborationHarnessExecutor(probeFixture());
    const result = await executor.runScenario("concurrent-demand");
    expect(result.stage).toBe("arm missing Body response");
    expect(result.failure).toEqual({ kind: "native_error", code: "provider" });
    expect(native.inspectOwner.mock.calls.length).toBeGreaterThanOrEqual(3);
    expect(native.setOwner).toHaveBeenCalledExactlyOnceWith({
      ownerLabel: "main",
      expectedGeneration: "7",
      active: true,
    });
  });

  it("rejects an older active inspection instead of accepting a replay for the setter owner", async () => {
    native.setOwner.mockResolvedValue({ generation: "8", active: false });
    native.inspectOwner.mockResolvedValue({ generation: "7", active: true });
    const probe = probeFixture();
    executor = await installCollaborationHarnessExecutor(probe);
    const result = await executor.runScenario("concurrent-demand");
    expect(result.stage).toBe("activate the real native main demand owner");
    expect(result.failure).toEqual({ kind: "assertion" });
    expect(
      native.control.mock.calls.some(
        ([value]) => value.request.core_action === "arm_provider_gate",
      ),
    ).toBe(false);
    expect(
      probe.execute.mock.calls.some(([value]) => value.action.kind === "mount"),
    ).toBe(false);
  });

  it("keeps an actually inactive owner fenced and returns a finite settle timeout", async () => {
    vi.useFakeTimers();
    native.setOwner.mockResolvedValue({ generation: "7", active: false });
    native.inspectOwner.mockResolvedValue({ generation: "7", active: false });
    const probe = probeFixture();
    executor = await installCollaborationHarnessExecutor(probe);
    const pending = executor.runScenario("concurrent-demand");
    await vi.advanceTimersByTimeAsync(25_100);
    const result = HarnessScenarioResultSchema.parse(await pending);
    expect(result.stage).toBe("activate the real native main demand owner");
    expect(result.failure).toEqual({
      kind: "settle_timeout",
      last_action_outcome: null,
    });
    expect(result.failure_context?.activities[0].native_owner?.active).toBe(
      false,
    );
    expect(
      native.control.mock.calls.some(
        ([value]) => value.request.core_action === "arm_provider_gate",
      ),
    ).toBe(false);
    expect(
      probe.execute.mock.calls.some(([value]) => value.action.kind === "mount"),
    ).toBe(false);
  });

  it("names ordinary navigation separately from close and document handshake", async () => {
    const probe = probeFixture();
    probe.execute.mockImplementation(async (request) => ({
      run_nonce: request.run_nonce,
      scenario_generation: request.scenario_generation,
      request_id: request.request_id,
      label: request.label,
      outcome: "accepted",
      snapshot: {} as NonNullable<HarnessResult["snapshot"]>,
      ...(request.action.kind === "inspect-activity"
        ? {
            activity: {
              document_visibility: "visible" as const,
              own_activity: { generation: "7", active: true },
            },
          }
        : {}),
    }));
    native.navigate.mockRejectedValue({
      code: "not_ready",
      message: "private route",
    });
    executor = await installCollaborationHarnessExecutor(probe);
    const result = await executor.runScenario("normal-tab-lifecycle");
    expect(result.stage).toBe(
      "navigate main into the ordinary native tab host",
    );
    expect(result.failure).toEqual({ kind: "native_error", code: "not_ready" });
    expect(native.createTab).toHaveBeenCalledTimes(2);
    expect(native.getAll).not.toHaveBeenCalled();
  });
});

/** Transport doubles qualify executor ordering only, never real native gates. */
async function orderingFixture() {
  current.core.actors = [
    {
      slot: "primary",
      account_id: "ruru103:primary",
      authorization_epoch: "1",
      instance_id: "github:github.com",
      repository_id: "fixture-repository",
      subject_id: "fixture-subject",
    },
  ];
  current.core.committed_phase = "one";
  current.core.committed_facet_revision = "18";
  const hash = await syntheticFingerprint(
    "RURU-103 primary body phase one — π 🌱",
  );
  const mounted = new Set<string>();
  const documentVisibility = new Map<string, "visible" | "hidden">();
  let manualLeases = 0;
  function setManualLeases(count: number) {
    manualLeases = count;
    current.core.demand_lease_count = mounted.size + manualLeases;
  }
  const execute = vi.fn(
    async (request: HarnessRequest): Promise<HarnessResult> => {
      events.push(`view:${request.label}:${request.action.kind}`);
      if (request.action.kind === "mount") mounted.add(request.label);
      if (request.action.kind === "detach") mounted.delete(request.label);
      current.core.demand_lease_count = mounted.size + manualLeases;
      const snapshot: HarnessProbeSnapshot = {
        document_nonce:
          request.label === "main" ? "main-document" : "child-document",
        actor: "primary",
        account_id: "ruru103:primary",
        actor_id: "103001",
        subject_id: "fixture-subject",
        authorization_epoch: "1",
        mounted: mounted.has(request.label),
        provider_visible: true,
        item: {
          status: "success",
          fetching: false,
          revision: current.core.revision,
          authorization_epoch: "1",
        },
        body: {
          status: "success",
          fetching: false,
          revision: current.core.revision,
          authorization_epoch: "1",
        },
        facet_revision: current.core.committed_facet_revision,
        body_value_state: "known",
        body_hash: hash,
        metadata_hash: hash,
        draft_generation: "1",
        saved_draft_hash: hash,
        editor_hash: hash,
        save_enabled: false,
        conflict_visible: false,
        catchup: {
          reads: 0,
          pages_with_more: 0,
          resets: 0,
          last_request: null,
          last_receipt: null,
        },
        collaboration_query_count: 4,
      };
      return {
        run_nonce: request.run_nonce,
        scenario_generation: request.scenario_generation,
        request_id: request.request_id,
        label: request.label,
        outcome: "accepted",
        snapshot,
        ...(request.action.kind === "inspect-activity"
          ? {
              activity: {
                document_visibility:
                  documentVisibility.get(request.label) ?? "visible",
                own_activity: { generation: "7", active: true },
              },
            }
          : {}),
        ...(request.action.kind === "check-peer-authority"
          ? {
              authority: {
                controller: "permission_denied" as const,
                renew_lease: "permission_denied" as const,
                release_lease: "permission_denied" as const,
                disconnect: "permission_denied" as const,
                inspect_owner: "permission_denied" as const,
                set_owner: "permission_denied" as const,
                dispose_owner: "permission_denied" as const,
                connect_synthetic_pat: "permission_denied" as const,
              },
            }
          : {}),
      };
    },
  );
  native.emitTo.mockImplementation(
    async (_target, _event, request: HarnessRequest) => {
      resultListener?.({ payload: await execute(request) });
    },
  );
  const coreControl = async ({
    request,
  }: {
    request: {
      action: string;
      core_action: string | null;
      run_nonce: string;
      expected_generation: string;
    };
  }) => {
    events.push(`control:${request.action}:${request.core_action ?? "none"}`);
    if (
      request.run_nonce !== "run-103" ||
      request.expected_generation !== current.core.scenario_generation
    )
      throw { code: "stale_view" };
    if (request.action === "create_concurrent_child")
      current.child_label = "tab-webview:ruru103:child";
    if (request.action === "close_concurrent_child") current.child_label = null;
    return { status: structuredClone(current), gate_id: null };
  };
  native.control.mockImplementation(coreControl);
  return {
    probe: { execute, inspect: vi.fn(), stop: vi.fn() },
    coreControl,
    setManualLeases,
    documentVisibility,
  };
}

describe("retained finite executor scheduling order", () => {
  it("does not start the ordinary tab host until the actual probe observes a visible main document", async () => {
    const { probe, documentVisibility } = await orderingFixture();
    vi.useFakeTimers();
    documentVisibility.set("main", "hidden");
    native.navigate.mockRejectedValue({ code: "not_ready" });
    executor = await installCollaborationHarnessExecutor(probe);
    const pending = executor.runScenario("normal-tab-lifecycle");
    await vi.advanceTimersByTimeAsync(300);
    expect(native.inspectOwner).toHaveBeenCalled();
    expect(native.setOwner).toHaveBeenCalledWith({
      ownerLabel: "main",
      expectedGeneration: "7",
      active: true,
    });
    expect(native.createTab).not.toHaveBeenCalled();
    expect(native.navigate).not.toHaveBeenCalled();
    expect(native.getAll).not.toHaveBeenCalled();
    documentVisibility.set("main", "visible");
    await vi.advanceTimersByTimeAsync(100);
    const result = HarnessScenarioResultSchema.parse(await pending);
    expect(result.stage).toBe(
      "navigate main into the ordinary native tab host",
    );
    expect(result.failure).toEqual({ kind: "native_error", code: "not_ready" });
    expect(native.createTab).toHaveBeenCalledTimes(2);
    expect(native.navigate).toHaveBeenCalledOnce();
  });

  it.each([
    "main",
    "tab-webview:ruru103:child",
  ])("keeps the fixture unmounted when %s remains DOM-hidden even with a native active owner", async (label) => {
    const { probe, documentVisibility } = await orderingFixture();
    vi.useFakeTimers();
    documentVisibility.set(label, "hidden");
    executor = await installCollaborationHarnessExecutor(probe);
    const pending = executor.runScenario("disconnect");
    await vi.advanceTimersByTimeAsync(25_100);
    const result = HarnessScenarioResultSchema.parse(await pending);
    expect(result.stage).toBe(
      label === "main"
        ? "observe actual main document visibility before fixture mount"
        : "observe actual child document visibility before fixture mount",
    );
    expect(result.failure).toEqual({
      kind: "settle_timeout",
      last_action_outcome: null,
    });
    expect(
      probe.execute.mock.calls.some(
        ([request]) =>
          request.label === label && request.action.kind === "mount",
      ),
    ).toBe(false);
    expect(
      result.failure_context?.activities.find(
        (activity) => activity.label === label,
      ),
    ).toMatchObject({
      document_visibility: "hidden",
      native_owner: { active: true },
    });
    expect(
      native.control.mock.calls.some(
        ([input]) => input.request.core_action === "advance_refresh",
      ),
    ).toBe(false);
  });

  it("waits for actual admitted SDK leases after visible cached mounts before advancing the gated refresh", async () => {
    const { probe, coreControl } = await orderingFixture();
    vi.useFakeTimers();
    let armed = false;
    let leasesAdmitted = false;
    native.status.mockImplementation(async () => {
      const receipt = structuredClone(current);
      if (armed && !leasesAdmitted) receipt.core.demand_lease_count = 0;
      return receipt;
    });
    native.control.mockImplementation(async (input) => {
      const receipt = await coreControl(input);
      if (input.request.core_action === "arm_provider_gate") {
        armed = true;
        return { ...receipt, gate_id: "provider-gate" };
      }
      if (input.request.core_action === "advance_refresh") {
        expect(leasesAdmitted).toBe(true);
        throw { code: "provider" };
      }
      return receipt;
    });
    executor = await installCollaborationHarnessExecutor(probe);
    const pending = executor.runScenario("disconnect");
    await vi.advanceTimersByTimeAsync(300);
    expect(armed).toBe(true);
    expect(
      probe.execute.mock.calls.filter(
        ([request]) => request.action.kind === "mount",
      ),
    ).toHaveLength(4);
    expect(
      native.control.mock.calls.some(
        ([input]) => input.request.core_action === "advance_refresh",
      ),
    ).toBe(false);
    leasesAdmitted = true;
    await vi.advanceTimersByTimeAsync(100);
    const result = HarnessScenarioResultSchema.parse(await pending);
    expect(result.stage).toBe(
      "capture actual old provider response after due refresh",
    );
    expect(result.failure).toEqual({ kind: "native_error", code: "provider" });
  });

  it("waits for a newly committed Body facet after Completed before the authority baseline", async () => {
    const { probe, coreControl, setManualLeases } = await orderingFixture();
    vi.useFakeTimers();
    native.control.mockImplementation(async (input) => {
      const receipt = await coreControl(input);
      if (input.request.core_action === "advance_refresh") {
        current.core.calls.push({
          call_id: "1",
          scenario_generation: "1",
          slot: "primary",
          authorization_epoch: "1",
          facet: "body",
          head_oid: null,
          phase: "one",
          state: "completed",
        });
        current.core.provider_call_count = "1";
        setTimeout(() => {
          current.core.committed_facet_revision = "19";
          current.core.revision = "19";
        }, 500);
        return { ...receipt, status: structuredClone(current) };
      }
      return receipt;
    });
    const lease = {
      lease_id: "a8b2f06a-5d3e-40f7-9f98-aaf24112c2af",
      owner_generation: "7",
      authorization_epoch: "1",
      expires_at: "2099-01-01T00:00:00Z",
    };
    native.acquire.mockImplementation(async () => {
      events.push("manual:acquire");
      setManualLeases(1);
      return lease;
    });
    native.renew.mockResolvedValue({ leases: [lease] });
    native.release.mockImplementation(async () => {
      setManualLeases(0);
    });
    native.accounts.mockResolvedValue({
      accounts: [],
      revision: "19",
      authorization_view: "view",
    });
    executor = await installCollaborationHarnessExecutor(probe);
    const pending = executor.runScenario("authority");
    await vi.advanceTimersByTimeAsync(0);
    await vi.waitFor(() => expect(current.core.calls).toHaveLength(1));
    await vi.advanceTimersByTimeAsync(300);
    expect(current.core.committed_facet_revision).toBe("18");
    expect(native.acquire).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(700);
    const result = HarnessScenarioResultSchema.parse(await pending);
    expect(result.outcome).toBe("passed");
    expect(
      result.observations.some((snapshot) => snapshot.facet_revision === "19"),
    ).toBe(true);
    expect(result.authority?.provider_calls_unchanged).toBe(true);
    expect(result.authority?.native_revision_unchanged).toBe(true);
    expect(result.authority?.lease_count_restored).toBe(true);
    expect(native.acquire).toHaveBeenCalledOnce();
  });

  it("retires all leases before the phase wake and arms the provider gate before remount", async () => {
    const { probe, coreControl } = await orderingFixture();
    native.control.mockImplementation(async (input) => {
      const receipt = await coreControl(input);
      if (input.request.core_action === "phase_two")
        expect(current.core.demand_lease_count).toBe(0);
      if (input.request.core_action === "arm_provider_gate")
        return { ...receipt, gate_id: "provider-gate" };
      if (input.request.core_action === "advance_refresh")
        throw { code: "provider" };
      return receipt;
    });
    executor = await installCollaborationHarnessExecutor(probe);
    const result = await executor.runScenario("disconnect");
    expect(result.stage).toBe(
      "capture actual old provider response after due refresh",
    );
    expect(result.failure).toEqual({ kind: "native_error", code: "provider" });
    const phase = events.indexOf("control:core:phase_two");
    const gate = events.indexOf("control:core:arm_provider_gate");
    const initialChildDetach = events.indexOf(
      "view:tab-webview:ruru103:child:detach",
    );
    const mounts = events.flatMap((event, index) =>
      event.endsWith(":mount") ? [index] : [],
    );
    expect(initialChildDetach).toBeLessThan(phase);
    expect(phase).toBeLessThan(gate);
    expect(mounts).toHaveLength(4);
    expect(gate).toBeLessThan(mounts[2]);
    expect(gate).toBeLessThan(mounts[3]);
  });
});

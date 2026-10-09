import { webcrypto } from "node:crypto";
import type { HarnessStatus, HarnessViewManifest } from "@gitru/commands";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  HARNESS_DRAFT_EDITS,
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
  forAccount: vi.fn(),
  draft: vi.fn(),
  detail: vi.fn(),
  pullCommits: vi.fn(),
  hydrateDetail: vi.fn(),
  inbox: vi.fn(),
  setLocalInboxState: vi.fn(),
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
  collaboration: {
    wake: native.wake,
    accounts: native.accounts,
    forAccount: native.forAccount,
  },
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
      performance: null,
    },
    child_label: null,
    hint_mode: "normal",
    held_hint_revisions: [],
    local_reads: [],
    authorized_hydrate_requests: "0",
    checkpoint: null,
    process_id: 103,
    native_setup_started_epoch_ms: "1770000000000",
    runtime_ready_epoch_ms: "1770000000010",
    runtime_open_micros: "10000",
    performance_queries: [],
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
  native.forAccount.mockImplementation(() => ({
    draft: native.draft,
    detail: native.detail,
    pullCommits: native.pullCommits,
    hydrateDetail: native.hydrateDetail,
    inbox: native.inbox,
    setLocalInboxState: native.setLocalInboxState,
  }));
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

describe("retained restart pull-commit acceptance", () => {
  it("presents the exact saved snapshot before fresh interest without provider hydration", async () => {
    const { probe } = await orderingFixture();
    const account = {
      id: "ruru103:primary",
      provider: "github" as const,
      host: "github.com",
      actor_id: "103001",
      login: "ruru103-primary",
      display_name: null,
      authorization_epoch: "1",
      state: "active" as const,
      notifications_supported: false,
    };
    current.core.actors = [
      {
        slot: "primary",
        account_id: account.id,
        authorization_epoch: account.authorization_epoch,
        instance_id: "github:github.com",
        repository_id: "github:repository:9007199254741993",
        subject_id: "github:pull:9007199254742993",
      },
    ];
    current.core.committed_phase = "one";
    current.core.committed_facet_revision = "21";
    current.core.revision = "24";
    current.process_id = 104;
    current.checkpoint = {
      run_nonce: "run-103",
      session_id: "retired-session",
      scenario_generation: "1",
      kind: "committed_before_hint",
      gate_id: null,
      committed_phase: "one",
      committed_facet_revision: "21",
      process_id: 103,
    };
    native.accounts.mockResolvedValue({
      accounts: [account],
      revision: "24",
      authorization_view: "1",
    });
    native.draft.mockResolvedValue({ generation: "1" });
    native.detail.mockResolvedValue({
      body: {
        state: "known",
        text: "RURU-103 primary body phase one — π 🌱",
      },
      evidence: { facet_revision: "21" },
    });
    native.pullCommits.mockResolvedValue({
      subject_id: "github:pull:9007199254742993",
      context: {
        base_oid: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        head_oid: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        source_repository_provider_id: "9007199254741994",
        metadata_facet_revision: "21",
      },
      commits: [
        {
          position: 0,
          oid: "cccccccccccccccccccccccccccccccccccccccc",
        },
        {
          position: 1,
          oid: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        },
      ],
      next_cursor: null,
      completeness: { state: "complete", reason: null },
      coverage: {
        state: "complete",
        validated_at: "2026-10-01T00:00:00Z",
        remote_has_more: false,
      },
      sync: {
        state: "ready",
        last_success_at: "2026-10-01T00:00:00Z",
        next_retry_at: null,
        error: null,
      },
      freshness: "fresh",
      facet_revision: "24",
      revision: "24",
      authorization_view: "1",
    });
    const activity = "2026-10-08T00:00:00.000000000Z";
    const local = (
      id: string,
      disposition: "inbox" | "done",
      effective: "inbox" | "snoozed" | "done",
      bookmarked: boolean,
      snoozedUntil: string | null,
    ) => ({
      item: {
        id,
        account_id: account.id,
        repository_id: "github:repository:9007199254741993",
        provider_id: id.split(":").at(-1) ?? id,
        kind: "notification" as const,
        number: "1",
        title: id,
        body: null,
        body_omitted: false,
        author: "ruru103-reviewer",
        web_url: "https://github.com/x-ruru103/project/pull/1",
        state: "pending",
        updated_at: activity,
        head_oid: null,
        is_draft: null,
        reason: "review_requested",
        unread: true,
        native_inbox: null,
      },
      local: {
        disposition,
        effective_disposition: effective,
        bookmarked,
        snoozed_until: snoozedUntil,
        activity_updated_at: activity,
        superseded_by_activity: false,
        generation: "1",
      },
    });
    native.inbox.mockResolvedValue({
      total_count: 0,
      pending_intents: [],

      entries: [
        local(
          "github:notification:9007199254744991",
          "done",
          "done",
          false,
          null,
        ),
        local(
          "github:notification:9007199254744992",
          "inbox",
          "snoozed",
          false,
          "2026-10-15T00:00:00.000000000Z",
        ),
        local(
          "github:notification:9007199254744993",
          "inbox",
          "inbox",
          true,
          null,
        ),
      ],
      revision: "27",
      authorization_view: "1",
      next_cursor: null,
      coverage: {
        state: "complete",
        validated_at: activity,
        remote_has_more: false,
      },
      sync: {
        state: "ready",
        last_success_at: activity,
        next_retry_at: null,
        error: null,
      },
      evaluated_at: activity,
      next_local_change_at: "2026-10-15T00:00:00.000000000Z",
    });

    executor = await installCollaborationHarnessExecutor(probe);
    const result = HarnessScenarioResultSchema.parse(
      await executor.runScenario("restart"),
    );

    expect(result.outcome).toBe("passed");
    expect(result.pull_commits).toMatchObject({
      account_id: account.id,
      subject_id: "github:pull:9007199254742993",
      context: {
        base_oid: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        head_oid: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        source_repository_provider_id: "9007199254741994",
        metadata_facet_revision: "21",
      },
      facet_revision: "24",
      oids: [
        "cccccccccccccccccccccccccccccccccccccccc",
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
      ],
      completeness: { state: "complete", reason: null },
      cache_only: true,
      provider_call_count_before: "0",
      provider_call_count_after: "0",
      vault_load_count_before: "0",
      vault_load_count_after: "0",
    });
    expect(result.local_inbox).toMatchObject({
      account_id: account.id,
      operation: "restart_read",
      entries: [
        {
          notification_id: "github:notification:9007199254744991",
          effective_disposition: "done",
          generation: "1",
        },
        {
          notification_id: "github:notification:9007199254744992",
          effective_disposition: "snoozed",
          generation: "1",
        },
        {
          notification_id: "github:notification:9007199254744993",
          bookmarked: true,
          generation: "1",
        },
      ],
      provider_call_count_before: "0",
      provider_call_count_after: "0",
      vault_load_count_before: "0",
      vault_load_count_after: "0",
    });
    expect(native.pullCommits).toHaveBeenCalledOnce();
    expect(native.inbox).toHaveBeenCalledExactlyOnceWith({
      remote_state: null,
      local_state: "all",
      search: null,
      cursor: null,
      limit: 100,
    });
    expect(native.setLocalInboxState).not.toHaveBeenCalled();
    expect(native.hydrateDetail).not.toHaveBeenCalled();
    expect(current.core.provider_call_count).toBe("0");
    expect(current.core.vault_load_count).toBe("0");
  });
});

/** This models ordering receipts, not native storage or SDK fence behavior. */
async function retentionOrderingFixture({
  prematureCatchup = false,
  delayResetBodyCache = false,
  leakHints = null,
}: {
  prematureCatchup?: boolean;
  delayResetBodyCache?: boolean;
  leakHints?: "hold" | "drop" | null;
} = {}) {
  const { probe, coreControl } = await orderingFixture();
  const baseExecute = probe.execute.getMockImplementation();
  if (!baseExecute) throw new Error("The ordering probe is missing");
  const childLabel = "tab-webview:ruru103:child";
  async function fingerprint(value: string) {
    const hash = await syntheticFingerprint(value);
    if (!hash) throw new Error("The ordering fingerprint is missing");
    return hash;
  }
  const hashes = {
    one: await fingerprint("RURU-103 primary body phase one — π 🌱"),
    two: await fingerprint("RURU-103 primary body phase two — λ 🌿"),
    first: await fingerprint(HARNESS_DRAFT_EDITS["first-edit"]),
    second: await fingerprint(HARNESS_DRAFT_EDITS["second-edit"]),
  };
  type Projection = {
    phase: "one" | "two";
    facet: string;
    bodyRevision: string;
    catchup: HarnessProbeSnapshot["catchup"];
  };
  const projections = new Map<string, Projection>();
  const editors = new Map<string, string>();
  let draftGeneration = "1";
  let savedHash = hashes.one;
  let catchupFilled = false;
  let retentionFilled = false;
  let completePreparation: (() => void) | null = null;
  let preparationStarted: (() => void) | null = null;
  const started = new Promise<void>((resolve) => {
    preparationStarted = resolve;
  });
  const preparation = new Promise<void>((resolve) => {
    completePreparation = resolve;
  });
  let completeRead: ((outcome: "stale_view" | "cancelled") => void) | null =
    null;
  let capturedReadRevision: string | null = null;

  function projection(label: string) {
    let value = projections.get(label);
    if (!value) {
      value = {
        phase: "one",
        facet: "18",
        bodyRevision: "18",
        catchup: {
          reads: 1,
          pages_with_more: 0,
          resets: 0,
          last_request: "0",
          last_receipt: "18",
        },
      };
      projections.set(label, value);
    }
    return value;
  }
  function catchUpChild() {
    const value = projection(childLabel);
    value.phase = current.core.committed_phase === "one" ? "one" : "two";
    value.facet = current.core.committed_facet_revision ?? "18";
    value.catchup.reads += 1;
    value.catchup.last_request = value.catchup.last_receipt;
    value.catchup.last_receipt = current.core.revision;
    if (catchupFilled) {
      value.catchup.pages_with_more += 1;
      catchupFilled = false;
    }
    if (retentionFilled) {
      value.catchup.resets += 1;
      if (!delayResetBodyCache) value.bodyRevision = current.core.revision;
      retentionFilled = false;
    } else value.bodyRevision = current.core.revision;
  }
  probe.execute.mockImplementation(async (request) => {
    const receipt = await baseExecute(request);
    const value = projection(request.label);
    if (request.action.kind === "edit-draft") {
      editors.set(
        request.label,
        request.action.variant === "first-edit" ? hashes.first : hashes.second,
      );
    }
    if (request.action.kind === "save-draft") {
      savedHash = editors.get(request.label) ?? savedHash;
      draftGeneration = "2";
    }
    if (request.action.kind === "wake" && request.label === childLabel)
      catchUpChild();
    if (request.action.kind === "read-body") {
      events.push("retention:read-started");
      capturedReadRevision = current.core.revision;
      const gate = current.local_reads[0];
      if (!gate || gate.state !== "armed")
        throw new Error("The ordering read has no armed gate");
      gate.state = "held";
      const outcome = await new Promise<"stale_view" | "cancelled">(
        (resolve) => {
          completeRead = resolve;
        },
      );
      return { ...receipt, outcome, snapshot: null };
    }
    if (request.label === "main") {
      value.phase = current.core.committed_phase === "one" ? "one" : "two";
      value.facet = current.core.committed_facet_revision ?? "18";
    }
    if (!receipt.snapshot) return receipt;
    const editorHash = editors.get(request.label) ?? hashes.one;
    return {
      ...receipt,
      snapshot: {
        ...receipt.snapshot,
        body: {
          ...receipt.snapshot.body,
          revision: value.bodyRevision,
        },
        facet_revision: value.facet,
        body_hash: hashes[value.phase],
        draft_generation: draftGeneration,
        saved_draft_hash: savedHash,
        editor_hash: editorHash,
        save_enabled: editorHash !== savedHash && request.label === "main",
        conflict_visible:
          request.label === childLabel &&
          draftGeneration === "2" &&
          editorHash !== savedHash,
        catchup: { ...value.catchup },
      },
    };
  });
  native.control.mockImplementation(async (input) => {
    const receipt = await coreControl(input);
    const { action, core_action: coreAction } = input.request;
    if (action === "hold_child_hints") current.hint_mode = "hold";
    if (action === "drop_child_hints") current.hint_mode = "drop";
    if (action === "resume_hints") current.hint_mode = "normal";
    if (action === "deliver_child_hints_reverse") catchUpChild();
    if (coreAction?.startsWith("phase_"))
      current.core.phase =
        coreAction === "phase_one"
          ? "one"
          : coreAction === "phase_two"
            ? "two"
            : "not_modified";
    if (coreAction === "advance_refresh") {
      current.core.revision = (BigInt(current.core.revision) + 1n).toString();
      current.core.committed_facet_revision = current.core.revision;
      current.core.committed_phase =
        current.core.phase === "one" ? "one" : "two";
      if (current.hint_mode === "hold")
        current.held_hint_revisions.push(current.core.revision);
      if (current.hint_mode === leakHints) catchUpChild();
    }
    if (coreAction === "fill_catchup") {
      current.core.revision = (BigInt(current.core.revision) + 300n).toString();
      catchupFilled = true;
    }
    if (coreAction === "fill_retention") {
      preparationStarted?.();
      await preparation;
      current.core.revision = (
        BigInt(current.core.revision) + 4100n
      ).toString();
      retentionFilled = true;
      events.push("retention:preparation-completed");
      if (prematureCatchup) catchUpChild();
    }
    let gateId: string | null = null;
    if (action === "arm_body_read") {
      gateId = "9a81a45c-813e-4463-a5ab-9c6a521ba60d";
      current.local_reads.push({
        gate_id: gateId,
        scenario_generation: current.core.scenario_generation,
        webview_label: childLabel,
        kind: "body",
        state: "armed",
      });
    }
    if (action === "release_local_read") {
      current.local_reads[0].state = "released";
      completeRead?.("stale_view");
    }
    if (action === "cancel_local_reads") completeRead?.("cancelled");
    return { ...receipt, status: structuredClone(current), gate_id: gateId };
  });
  return {
    probe,
    started,
    completePreparation: () => completePreparation?.(),
    completeBodyCache: () => {
      projection(childLabel).bodyRevision = current.core.revision;
    },
    capturedReadRevision: () => capturedReadRevision,
  };
}

describe("retained finite executor scheduling order", () => {
  it.each([
    "hold",
    "drop",
  ] as const)("rejects observed child cache advancement while %s hints should be withheld", async (leakHints) => {
    const fixture = await retentionOrderingFixture({ leakHints });
    executor = await installCollaborationHarnessExecutor(fixture.probe);
    const result = HarnessScenarioResultSchema.parse(
      await executor.runScenario("hints-and-catchup"),
    );
    expect(result.outcome).toBe("failed");
    expect(result.stage).toBe(
      leakHints === "hold"
        ? "verify actual child cache is unchanged while hints are held"
        : "verify actual child cache is unchanged while hints are dropped",
    );
    expect(result.failure).toEqual({ kind: "assertion" });
    expect(result.obsolete_reads?.retention_reset).toBeNull();
    expect(current.local_reads).toHaveLength(0);
    expect(
      native.control.mock.calls.some(
        ([input]) => input.request.core_action === "fill_retention",
      ),
    ).toBe(false);
    expect(result.cleanup_failure).toBeNull();
  });

  it("finishes slow retention preparation before starting either bounded held-read lifetime", async () => {
    const fixture = await retentionOrderingFixture();
    vi.useFakeTimers();
    executor = await installCollaborationHarnessExecutor(fixture.probe);
    const pending = executor.runScenario("hints-and-catchup");
    await fixture.started;
    await vi.advanceTimersByTimeAsync(16_000);
    expect(current.local_reads).toHaveLength(0);
    expect(events).not.toContain("retention:read-started");
    expect(
      native.control.mock.calls.some(
        ([input]) => input.request.action === "arm_body_read",
      ),
    ).toBe(false);
    fixture.completePreparation();
    await vi.advanceTimersByTimeAsync(100);
    const result = HarnessScenarioResultSchema.parse(await pending);
    expect(result.outcome).toBe("passed");
    expect(result.obsolete_reads?.retention_reset).toBe("stale_view");
    expect(result.cleanup_failure).toBeNull();
    expect(events.indexOf("retention:preparation-completed")).toBeLessThan(
      events.indexOf("control:arm_body_read:none"),
    );
    expect(events.indexOf("retention:read-started")).toBeLessThan(
      events.lastIndexOf("view:tab-webview:ruru103:child:wake"),
    );
    expect(fixture.capturedReadRevision()).toBe(current.core.revision);
    expect(current.local_reads[0].state).toBe("released");
  });

  it("rejects premature child catchup after preparation before claiming a pre-reset read", async () => {
    const fixture = await retentionOrderingFixture({ prematureCatchup: true });
    executor = await installCollaborationHarnessExecutor(fixture.probe);
    const pending = executor.runScenario("hints-and-catchup");
    await fixture.started;
    fixture.completePreparation();
    const result = HarnessScenarioResultSchema.parse(await pending);
    expect(result.outcome).toBe("failed");
    expect(result.stage).toBe(
      "prepare real retention overflow before capturing a local read",
    );
    expect(result.failure).toEqual({ kind: "assertion" });
    expect(result.obsolete_reads?.retention_reset).toBeNull();
    expect(events).not.toContain("retention:read-started");
    expect(current.local_reads).toHaveLength(0);
    expect(result.cleanup_failure).toBeNull();
  });

  it("keeps the held return gated until the actual Body cache catches up after ResetRequired", async () => {
    const fixture = await retentionOrderingFixture({
      delayResetBodyCache: true,
    });
    vi.useFakeTimers();
    executor = await installCollaborationHarnessExecutor(fixture.probe);
    const pending = executor.runScenario("hints-and-catchup");
    await fixture.started;
    fixture.completePreparation();
    await vi.advanceTimersByTimeAsync(300);
    expect(current.local_reads[0].state).toBe("held");
    expect(events).toContain("retention:read-started");
    expect(
      native.control.mock.calls.some(
        ([input]) => input.request.action === "release_local_read",
      ),
    ).toBe(false);
    fixture.completeBodyCache();
    await vi.advanceTimersByTimeAsync(100);
    const result = HarnessScenarioResultSchema.parse(await pending);
    expect(result.outcome).toBe("passed");
    expect(result.obsolete_reads?.retention_reset).toBe("stale_view");
    expect(current.local_reads[0].state).toBe("released");
  });

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

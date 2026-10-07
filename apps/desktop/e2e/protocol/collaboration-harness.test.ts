import { afterEach, describe, expect, it, vi } from "vitest";
import {
  classifyHarnessFailure,
  createHarnessRequester,
  HARNESS_DIAGNOSTIC_TIMEOUT_MS,
  HARNESS_MAX_PENDING,
  HARNESS_REQUEST_TIMEOUT_MS,
  HarnessActionSchema,
  HarnessAuthorityEvidenceSchema,
  HarnessFailureContextSchema,
  HarnessFailureSchema,
  HarnessLocalInboxEvidenceSchema,
  HarnessObsoleteReadsSchema,
  HarnessPeerLeaseSchema,
  HarnessPerformanceViewSchema,
  HarnessPullCommitEvidenceSchema,
  type HarnessRequest,
  HarnessRequestSchema,
  HarnessResultSchema,
  HarnessScenarioError,
  HarnessScenarioResultSchema,
  HarnessScenarioSchema,
  matchesHarnessPeerLease,
  matchesHarnessRequest,
  readHarnessDiagnostic,
} from "./collaboration-harness";

const request = {
  run_nonce: "run-103",
  scenario_generation: "9007199254740993",
  request_id: "request-1",
  label: "tab-webview:ruru103:native-1",
  action: { kind: "mount", actor: "primary" },
} as const;
const receipt = {
  ...request,
  action: undefined,
  outcome: "accepted",
  snapshot: null,
};

describe("retained collaboration renderer protocol", () => {
  it("bounds performance samples to finite fixed native cases", () => {
    const sample = { duration_ms: 4.25, payload_bytes: 4096 };
    const cases = [
      ["list_local_ipc", "react_useful_content"],
      ["search_local_ipc", "react_useful_content"],
      ["detail_local_ipc", "react_useful_content"],
      ["list_local_ipc", "sdk_ipc"],
      ["search_local_ipc", "sdk_ipc"],
      ["detail_local_ipc", "sdk_ipc"],
      ["list_memory_hit", "query_memory"],
      ["search_memory_hit", "query_memory"],
      ["detail_memory_hit", "query_memory"],
    ].map(([name, boundary]) => ({ name, boundary, samples: [sample] }));
    const view = {
      phase: "restart",
      webview_label: "main",
      role: "main",
      sample_count: 10,
      account_id: "ruru103:alternate",
      first_useful_epoch_ms: 1_800_000_000_000,
      navigation_to_first_useful_ms: 120,
      exact_first_title: "RURU-125 cached pull 4999 alternate repository 4",
      cases,
    };
    expect(HarnessPerformanceViewSchema.safeParse(view).success).toBe(true);
    for (const changed of [
      { ...view, samples: [{ duration_ms: Number.NaN }] },
      { ...view, token: "credential" },
      { ...view, cases: [...cases, cases[0]] },
      {
        ...view,
        cases: cases.map((entry, index) =>
          index === 0
            ? { ...entry, samples: [{ ...sample, payload_bytes: 9_000_000 }] }
            : entry,
        ),
      },
    ])
      expect(HarnessPerformanceViewSchema.safeParse(changed).success).toBe(
        false,
      );
  });

  it("bounds restart pull-commit evidence to exact typed cache facts", () => {
    const evidence = {
      account_id: "ruru103:primary",
      subject_id: "github:pull:9007199254742993",
      context: {
        base_oid: "b".repeat(40),
        head_oid: "a".repeat(40),
        source_repository_provider_id: "9007199254741994",
        metadata_facet_revision: "21",
      },
      facet_revision: "24",
      oids: ["c".repeat(40), "a".repeat(40)],
      completeness: { state: "complete", reason: null },
      cache_only: true,
      provider_call_count_before: "0",
      provider_call_count_after: "0",
      vault_load_count_before: "0",
      vault_load_count_after: "0",
    };
    expect(HarnessPullCommitEvidenceSchema.safeParse(evidence).success).toBe(
      true,
    );
    expect(
      HarnessPullCommitEvidenceSchema.safeParse({
        ...evidence,
        oids: Array.from({ length: 9 }, () => "c".repeat(40)),
      }).success,
    ).toBe(false);
    expect(
      HarnessPullCommitEvidenceSchema.safeParse({
        ...evidence,
        provider_response: "raw private payload",
      }).success,
    ).toBe(false);
  });

  it("bounds local inbox restart evidence to the three fixed projections", () => {
    const entry = {
      notification_id: "github:notification:9007199254744991",
      disposition: "done",
      effective_disposition: "done",
      bookmarked: false,
      snoozed_until: null,
      activity_updated_at: "2026-10-08T00:00:00.000000000Z",
      superseded_by_activity: false,
      generation: "1",
    } as const;
    const evidence = {
      account_id: "ruru103:primary",
      operation: "restart_read",
      entries: [
        entry,
        {
          ...entry,
          notification_id: "github:notification:9007199254744992",
          disposition: "inbox",
          effective_disposition: "snoozed",
          snoozed_until: "2026-10-15T00:00:00.000000000Z",
        },
        {
          ...entry,
          notification_id: "github:notification:9007199254744993",
          disposition: "inbox",
          effective_disposition: "inbox",
          bookmarked: true,
        },
      ],
      provider_call_count_before: "0",
      provider_call_count_after: "0",
      vault_load_count_before: "0",
      vault_load_count_after: "0",
    };
    expect(HarnessLocalInboxEvidenceSchema.safeParse(evidence).success).toBe(
      true,
    );
    expect(
      HarnessLocalInboxEvidenceSchema.safeParse({
        ...evidence,
        entries: [...evidence.entries, entry],
      }).success,
    ).toBe(false);
    expect(
      HarnessLocalInboxEvidenceSchema.safeParse({
        ...evidence,
        entries: [
          { ...entry, notification_id: "provider-selected-id" },
          ...evidence.entries.slice(1),
        ],
      }).success,
    ).toBe(false);
  });

  it("classifies failures without persisting arbitrary exception text", () => {
    expect(
      classifyHarnessFailure({ code: "permission_denied", message: "private" }),
    ).toEqual({ kind: "native_error", code: "permission_denied" });
    expect(
      classifyHarnessFailure(new Error("private provider response")),
    ).toEqual({ kind: "unclassified" });
    expect(classifyHarnessFailure({ code: "caller-selected-code" })).toEqual({
      kind: "unclassified",
    });
    const failure = {
      kind: "settle_timeout",
      last_action_outcome: "not_ready",
    } as const;
    expect(
      classifyHarnessFailure(new HarnessScenarioError(failure, "private")),
    ).toEqual(failure);
    expect(
      HarnessFailureSchema.parse({
        kind: "action_rejected",
        outcome: "not_ready",
        native_code: "not_ready",
      }),
    ).toEqual({
      kind: "action_rejected",
      outcome: "not_ready",
      native_code: "not_ready",
    });
    for (const value of [
      { kind: "native_error", code: "provider", message: "private" },
      { kind: "assertion", stack: "private path" },
      { kind: "action_rejected", outcome: "accepted" },
      { kind: "settle_timeout", last_action_outcome: "timeout means stale" },
    ])
      expect(HarnessFailureSchema.safeParse(value).success).toBe(false);
  });

  it("bounds failure visibility and actual owner observations to four native labels", () => {
    const activity = {
      label: "main",
      document_visibility: "hidden",
      own_activity: { generation: "9007199254740993", active: false },
      native_owner: { generation: "9007199254740993", active: false },
      probe_failure: null,
      owner_failure: null,
    };
    const context = {
      status: null,
      status_failure: null,
      activities: [activity],
    };
    expect(
      HarnessFailureContextSchema.parse(context).activities[0].native_owner
        ?.generation,
    ).toBe("9007199254740993");
    for (const value of [
      { ...context, activities: Array.from({ length: 5 }, () => activity) },
      {
        ...context,
        activities: [{ ...activity, document_visibility: "pretend-visible" }],
      },
      {
        ...context,
        activities: [
          { ...activity, own_activity: { generation: "1", active: "true" } },
        ],
      },
      { ...context, activities: [{ ...activity, raw_error: "private" }] },
    ])
      expect(HarnessFailureContextSchema.safeParse(value).success).toBe(false);
    expect(
      HarnessActionSchema.safeParse({
        kind: "inspect-activity",
        owner_label: "arbitrary",
      }).success,
    ).toBe(false);
  });
  it("keeps exact decimal generations and rejects caller programs or text", () => {
    expect(HarnessRequestSchema.parse(request).scenario_generation).toBe(
      "9007199254740993",
    );
    for (const payload of [
      { ...request, scenario_generation: 9007199254740993 },
      { ...request, scenario_generation: "01" },
      { ...request, scenario_generation: "18446744073709551616" },
      { ...request, path: "/tmp/other" },
      { ...request, action: { kind: "eval", script: "location.reload()" } },
      { ...request, action: { kind: "mount", actor: "third" } },
      {
        ...request,
        action: { kind: "edit-draft", variant: "first-edit", body: "custom" },
      },
    ]) {
      expect(HarnessRequestSchema.safeParse(payload).success).toBe(false);
    }
  });

  it("requires the exact run, scenario, request and native target incarnation", () => {
    const { action: _, ...envelope } = request;
    const parsedRequest = HarnessRequestSchema.parse(request);
    const result = HarnessResultSchema.parse({
      ...envelope,
      outcome: "accepted",
      snapshot: null,
    });
    expect(matchesHarnessRequest(parsedRequest, result)).toBe(true);
    for (const altered of [
      { ...result, run_nonce: "old-run" },
      { ...result, scenario_generation: "9007199254740992" },
      { ...result, request_id: "old-request" },
      { ...result, label: "tab-webview:ruru103:old-native-1" },
    ]) {
      expect(matchesHarnessRequest(parsedRequest, altered)).toBe(false);
    }
  });

  it("rejects broad diagnostic payloads and bounds all driver choices", () => {
    const { action: _, ...envelope } = request;
    expect(
      HarnessResultSchema.safeParse({
        ...envelope,
        outcome: "failed",
        snapshot: null,
        provider_error: "raw private response",
      }).success,
    ).toBe(false);
    expect(
      HarnessActionSchema.safeParse({ kind: "wake", token: "pat" }).success,
    ).toBe(false);
    expect(
      HarnessScenarioSchema.safeParse("arbitrary-native-method").success,
    ).toBe(false);
    expect(
      HarnessRequestSchema.safeParse({
        ...request,
        label: "https://foreign.example/",
      }).success,
    ).toBe(false);
    expect(
      HarnessRequestSchema.safeParse({
        ...request,
        request_id: "a".repeat(129),
      }).success,
    ).toBe(false);
    expect(HarnessResultSchema.safeParse(receipt).success).toBe(false);
  });

  it("keeps persisted driver receipts finite and excludes diagnostic text programs", () => {
    const result = {
      scenario: "authority",
      outcome: "failed",
      stage: "deny native controller from actual child",
      status: null,
      observations: [],
      pull_commits: null,
      local_inbox: null,
    };
    expect(HarnessScenarioResultSchema.safeParse(result).success).toBe(true);
    for (const changed of [
      { ...result, private_body: "Synthetic text must remain a hash" },
      { ...result, token: "arbitrary token" },
      { ...result, stage: "x".repeat(161) },
      { ...result, scenario: "arbitrary native invocation" },
    ])
      expect(HarnessScenarioResultSchema.safeParse(changed).success).toBe(
        false,
      );
  });

  it("bounds a peer packet to one main native handle and the manifest primary", () => {
    const peer = {
      lease_id: "a8b2f06a-5d3e-40f7-9f98-aaf24112c2af",
      owner_label: "main",
      owner_generation: "9007199254740993",
      account_id: "ruru103:primary",
      authorization_epoch: "1",
    } as const;
    const manifest = {
      run_nonce: "run-103",
      session_id: "session-103",
      scenario_generation: "1",
      webview_label: request.label,
      role: "concurrent_child" as const,
      actors: [
        {
          slot: "primary" as const,
          account_id: peer.account_id,
          authorization_epoch: peer.authorization_epoch,
          instance_id: "github:github.com",
          repository_id: "fixture-repository",
          subject_id: "fixture-subject",
        },
      ],
    };
    expect(matchesHarnessPeerLease(peer, manifest)).toBe(true);
    for (const changed of [
      { ...peer, lease_id: "caller-selected-path" },
      { ...peer, owner_label: request.label },
      { ...peer, owner_generation: "01" },
      { ...peer, authorization_epoch: "18446744073709551616" },
      { ...peer, account_id: "x".repeat(257) },
      { ...peer, target: "arbitrary resource" },
      { ...peer, token: "arbitrary credential" },
    ])
      expect(HarnessPeerLeaseSchema.safeParse(changed).success).toBe(false);
    expect(
      matchesHarnessPeerLease(
        { ...peer, account_id: "foreign-account" },
        manifest,
      ),
    ).toBe(false);
    expect(
      matchesHarnessPeerLease({ ...peer, authorization_epoch: "2" }, manifest),
    ).toBe(false);
    expect(matchesHarnessPeerLease(peer, { ...manifest, role: "main" })).toBe(
      false,
    );
    expect(
      matchesHarnessPeerLease(peer, { ...manifest, role: "normal_tab" }),
    ).toBe(false);
    expect(
      HarnessActionSchema.safeParse({ kind: "check-peer-authority", peer })
        .success,
    ).toBe(true);
  });

  it("persists finite authority outcomes and cleanup without native lease handles", () => {
    const evidence = {
      scope: "native-secondary-window",
      target: "manifest-primary-body",
      checks: {
        controller: "permission_denied",
        renew_lease: "permission_denied",
        release_lease: "permission_denied",
        disconnect: "permission_denied",
        inspect_owner: "permission_denied",
        set_owner: "permission_denied",
        dispose_owner: "permission_denied",
        connect_synthetic_pat: "permission_denied",
      },
      main_renewed: true,
      main_released: true,
      lease_count_restored: true,
      account_snapshot_unchanged: true,
      native_revision_unchanged: true,
      native_owner_unchanged: true,
      provider_calls_unchanged: true,
      vault_access_unchanged: true,
    };
    expect(HarnessAuthorityEvidenceSchema.safeParse(evidence).success).toBe(
      true,
    );
    for (const changed of [
      { ...evidence, lease_id: "opaque-native-handle" },
      { ...evidence, token: "arbitrary credential" },
      {
        ...evidence,
        checks: { ...evidence.checks, connect_synthetic_pat: "denied" },
      },
      {
        ...evidence,
        checks: { ...evidence.checks, raw_error: "private response" },
      },
    ])
      expect(HarnessAuthorityEvidenceSchema.safeParse(changed).success).toBe(
        false,
      );
    // A failed actual check remains observable; it cannot become a fabricated
    // denial through the receipt parser.
    expect(
      HarnessAuthorityEvidenceSchema.parse({
        ...evidence,
        checks: { ...evidence.checks, release_lease: "accepted" },
        main_renewed: false,
      }).checks.release_lease,
    ).toBe("accepted");
  });

  it("keeps obsolete read evidence explicit without accepting a failed request as a fence", () => {
    expect(
      HarnessObsoleteReadsSchema.parse({
        retention_reset: "stale_view",
        disconnect: "cancelled",
      }),
    ).toEqual({ retention_reset: "stale_view", disconnect: "cancelled" });
    expect(
      HarnessObsoleteReadsSchema.parse({
        retention_reset: "request_failed",
        disconnect: "accepted",
      }).retention_reset,
    ).toBe("request_failed");
    for (const changed of [
      { retention_reset: "timeout means stale", disconnect: null },
      {
        retention_reset: "stale_view",
        disconnect: null,
        raw_error: "private response",
      },
    ])
      expect(HarnessObsoleteReadsSchema.safeParse(changed).success).toBe(false);
  });
});

describe("compiled main request lifetime", () => {
  afterEach(() => vi.useRealTimers());

  it("bounds uncancellable diagnostic reads to three seconds and consumes late rejection", async () => {
    vi.useFakeTimers();
    let reject!: (error: unknown) => void;
    const read = readHarnessDiagnostic(
      () =>
        new Promise<never>((_, fail) => {
          reject = fail;
        }),
    );
    await vi.advanceTimersByTimeAsync(HARNESS_DIAGNOSTIC_TIMEOUT_MS);
    await expect(read).resolves.toEqual({
      value: null,
      failure: { kind: "diagnostic_timeout" },
    });
    reject(new Error("late private provider text"));
    await Promise.resolve();
    expect(vi.getTimerCount()).toBe(0);
  });

  it("records only native codes for rejected diagnostic observations", async () => {
    await expect(
      readHarnessDiagnostic(async () => {
        throw { code: "storage", message: "private" };
      }),
    ).resolves.toEqual({
      value: null,
      failure: { kind: "native_error", code: "storage" },
    });
  });

  function fixture() {
    let listener: ((payload: unknown) => void) | null = null;
    const remove = vi.fn();
    const emit = vi.fn(async (_request: HarnessRequest) => undefined);
    const requester = createHarnessRequester({
      runNonce: request.run_nonce,
      generation: () => request.scenario_generation,
      allowedLabels: () => ["main", request.label],
      transport: {
        listen: async (next) => {
          listener = next;
          return remove;
        },
        emit,
      },
    });
    const respond = (pending: HarnessRequest, override: object = {}) => {
      const { action: _, ...envelope } = pending;
      listener?.({
        ...envelope,
        outcome: "accepted",
        snapshot: null,
        ...override,
      });
    };
    return { requester, emit, remove, respond };
  }

  it("registers before emission and ignores a different view/run/generation receipt", async () => {
    const { requester, emit, respond, remove } = fixture();
    let settled = false;
    const result = requester.request(request.label, { kind: "inspect" });
    void result.then(() => {
      settled = true;
    });
    await vi.waitFor(() => expect(emit).toHaveBeenCalledOnce());
    const sent = emit.mock.calls[0][0];
    respond(sent, { label: "main" });
    respond(sent, { run_nonce: "another-run" });
    respond(sent, { scenario_generation: "9007199254740992" });
    await Promise.resolve();
    expect(settled).toBe(false);
    respond(sent);
    expect((await result).request_id).toBe(sent.request_id);
    requester.stop();
    expect(remove).toHaveBeenCalledOnce();
  });

  it("rejects unissued targets without dispatching anything", async () => {
    const { requester, emit } = fixture();
    await expect(
      requester.request("tab-webview:foreign", { kind: "inspect" }),
    ).rejects.toThrow("not issued");
    expect(emit).not.toHaveBeenCalled();
    requester.stop();
  });

  it("bounds pending reads and rejects every outstanding request on teardown", async () => {
    const { requester, emit, remove } = fixture();
    const reads = Array.from({ length: HARNESS_MAX_PENDING }, () =>
      requester.request(request.label, { kind: "read-item" }),
    );
    const results = Promise.allSettled(reads);
    await vi.waitFor(() =>
      expect(emit).toHaveBeenCalledTimes(HARNESS_MAX_PENDING),
    );
    await expect(
      requester.request(request.label, { kind: "inspect" }),
    ).rejects.toThrow("limit reached");
    requester.stop();
    expect(
      (await results).every((result) => result.status === "rejected"),
    ).toBe(true);
    expect(remove).toHaveBeenCalledOnce();
    await expect(
      requester.request(request.label, { kind: "inspect" }),
    ).rejects.toThrow("stopped");
  });

  it("times out a lost receipt and frees its request slot", async () => {
    vi.useFakeTimers();
    const { requester, emit, respond } = fixture();
    const result = requester.request(request.label, { kind: "inspect" });
    const rejected = expect(result).rejects.toThrow("did not complete");
    await vi.advanceTimersByTimeAsync(HARNESS_REQUEST_TIMEOUT_MS);
    await rejected;
    const replacement = requester.request(request.label, { kind: "inspect" });
    await vi.advanceTimersByTimeAsync(0);
    respond(emit.mock.calls[1][0]);
    expect((await replacement).outcome).toBe("accepted");
    requester.stop();
  });

  it("removes a listener that resolves after document disposal", async () => {
    let finish!: (remove: () => void) => void;
    const remove = vi.fn();
    const requester = createHarnessRequester({
      runNonce: request.run_nonce,
      generation: () => request.scenario_generation,
      allowedLabels: () => [request.label],
      transport: {
        listen: () =>
          new Promise<() => void>((resolve) => {
            finish = resolve;
          }),
        emit: async () => undefined,
      },
    });
    requester.stop();
    finish(remove);
    await Promise.resolve();
    expect(remove).toHaveBeenCalledOnce();
  });

  it("cannot accept a delayed result after the native scenario has retired", async () => {
    let generation = request.scenario_generation as string;
    let listener!: (payload: unknown) => void;
    let sent!: HarnessRequest;
    const requester = createHarnessRequester({
      runNonce: request.run_nonce,
      generation: () => generation,
      allowedLabels: () => [request.label],
      transport: {
        listen: async (next) => {
          listener = next;
          return () => undefined;
        },
        emit: async (value) => {
          sent = value;
        },
      },
    });
    const result = requester.request(request.label, { kind: "read-body" });
    const rejected = expect(result).rejects.toThrow("did not complete");
    await vi.waitFor(() => expect(sent).toBeDefined());
    generation = "9007199254740994";
    const { action: _, ...envelope } = sent;
    listener({ ...envelope, outcome: "accepted", snapshot: null });
    await rejected;
    requester.stop();
  });

  it("cannot accept a delayed receipt from a native label retired while the request was pending", async () => {
    let labels = [request.label] as string[];
    let listener!: (payload: unknown) => void;
    let sent!: HarnessRequest;
    const requester = createHarnessRequester({
      runNonce: request.run_nonce,
      generation: () => request.scenario_generation,
      allowedLabels: () => labels,
      transport: {
        listen: async (next) => {
          listener = next;
          return () => undefined;
        },
        emit: async (value) => {
          sent = value;
        },
      },
    });
    const pending = requester.request(request.label, { kind: "read-body" });
    const rejected = expect(pending).rejects.toThrow("did not complete");
    await vi.waitFor(() => expect(sent).toBeDefined());
    labels = [];
    const { action: _, ...envelope } = sent;
    listener({ ...envelope, outcome: "accepted", snapshot: null });
    await rejected;
    requester.stop();
  });

  it("fails listener installation without emitting and observes its failure after teardown", async () => {
    const emit = vi.fn();
    const requester = createHarnessRequester({
      runNonce: request.run_nonce,
      generation: () => request.scenario_generation,
      allowedLabels: () => [request.label],
      transport: {
        listen: async () => {
          throw new Error("Native registration failed");
        },
        emit,
      },
    });
    await expect(
      requester.request(request.label, { kind: "inspect" }),
    ).rejects.toThrow("registration failed");
    expect(emit).not.toHaveBeenCalled();
    requester.stop();
  });
});

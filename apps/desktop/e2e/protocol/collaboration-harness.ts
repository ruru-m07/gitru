/** Finite renderer protocol. Imported only by the retained harness build/tests. */

import {
  DetailValueStateSchema,
  ErrorCodeSchema,
  HarnessStatusSchema,
  type HarnessViewManifest,
  PullCommitCompletenessSchema,
  PullCommitContextSchema,
} from "@gitru/commands";
import { z } from "zod";

export const HARNESS_REQUEST_EVENT = "gitru:e2e-collaboration-harness:request";
export const HARNESS_RESULT_EVENT = "gitru:e2e-collaboration-harness:result";
export const HARNESS_MAX_PENDING = 8;
export const HARNESS_REQUEST_TIMEOUT_MS = 10_000;
export const HARNESS_DIAGNOSTIC_TIMEOUT_MS = 3_000;

const nonce = z
  .string()
  .min(1)
  .max(128)
  .regex(/^[a-zA-Z0-9:_-]+$/);
const decimal = z
  .string()
  .max(20)
  .regex(/^(0|[1-9]\d*)$/)
  .refine((value) => {
    try {
      return BigInt(value) <= 18_446_744_073_709_551_615n;
    } catch {
      return false;
    }
  });
const label = z
  .string()
  .min(1)
  .max(128)
  .regex(/^[a-zA-Z0-9:_-]+$/);
const hash = z.string().regex(/^[a-f0-9]{64}$/);

export const HarnessActorSchema = z.enum(["primary", "alternate"]);
export type HarnessActor = z.infer<typeof HarnessActorSchema>;

/** A native-issued main lease, never a caller-selected target or program. */
export const HarnessPeerLeaseSchema = z
  .object({
    lease_id: z.string().uuid(),
    owner_label: z.literal("main"),
    owner_generation: decimal,
    account_id: z.string().min(1).max(256),
    authorization_epoch: decimal,
  })
  .strict();
export type HarnessPeerLease = z.infer<typeof HarnessPeerLeaseSchema>;

export function matchesHarnessPeerLease(
  peer: HarnessPeerLease,
  manifest: HarnessViewManifest,
) {
  const primary = manifest.actors.find((actor) => actor.slot === "primary");
  return (
    manifest.role === "concurrent_child" &&
    manifest.webview_label !== peer.owner_label &&
    peer.owner_label === "main" &&
    primary?.account_id === peer.account_id &&
    primary.authorization_epoch === peer.authorization_epoch
  );
}

export const HarnessActionSchema = z.discriminatedUnion("kind", [
  z.object({ kind: z.literal("inspect") }).strict(),
  z.object({ kind: z.literal("inspect-activity") }).strict(),
  z.object({ kind: z.literal("mount"), actor: HarnessActorSchema }).strict(),
  z.object({ kind: z.literal("detach") }).strict(),
  z
    .object({
      kind: z.literal("edit-draft"),
      variant: z.enum(["first-edit", "second-edit"]),
    })
    .strict(),
  z.object({ kind: z.literal("save-draft") }).strict(),
  z.object({ kind: z.literal("wake") }).strict(),
  z.object({ kind: z.literal("read-item") }).strict(),
  z.object({ kind: z.literal("read-body") }).strict(),
  z
    .object({
      kind: z.literal("benchmark"),
      phase: z.enum(["warm", "restart"]),
      sample_count: z.union([z.literal(10), z.literal(30)]),
    })
    .strict(),
  z.object({ kind: z.literal("check-control-authority") }).strict(),
  z
    .object({
      kind: z.literal("check-peer-authority"),
      peer: HarnessPeerLeaseSchema,
    })
    .strict(),
]);
export type HarnessAction = z.infer<typeof HarnessActionSchema>;

export const HarnessRequestSchema = z
  .object({
    run_nonce: nonce,
    scenario_generation: decimal,
    request_id: nonce,
    label,
    action: HarnessActionSchema,
  })
  .strict();
export type HarnessRequest = z.infer<typeof HarnessRequestSchema>;

const query = z
  .object({
    status: z.enum(["absent", "pending", "success", "error"]),
    fetching: z.boolean(),
    revision: decimal.nullable(),
    authorization_epoch: decimal.nullable(),
  })
  .strict();

export const HarnessCatchupObservationSchema = z
  .object({
    reads: z.number().int().min(0).max(10_000),
    pages_with_more: z.number().int().min(0).max(10_000),
    resets: z.number().int().min(0).max(1000),
    last_request: decimal.nullable(),
    last_receipt: decimal.nullable(),
  })
  .strict();
export type HarnessCatchupObservation = z.infer<
  typeof HarnessCatchupObservationSchema
>;

export const HarnessProbeSnapshotSchema = z
  .object({
    document_nonce: nonce,
    actor: HarnessActorSchema.nullable(),
    account_id: z.string().min(1).max(256).nullable(),
    actor_id: z.string().min(1).max(256).nullable(),
    subject_id: z.string().min(1).max(256).nullable(),
    authorization_epoch: decimal.nullable(),
    mounted: z.boolean(),
    provider_visible: z.boolean(),
    item: query,
    body: query,
    facet_revision: decimal.nullable(),
    body_value_state: DetailValueStateSchema.nullable(),
    body_hash: hash.nullable(),
    metadata_hash: hash.nullable(),
    draft_generation: decimal.nullable(),
    saved_draft_hash: hash.nullable(),
    editor_hash: hash.nullable(),
    save_enabled: z.boolean(),
    conflict_visible: z.boolean(),
    catchup: HarnessCatchupObservationSchema,
    collaboration_query_count: z.number().int().min(0).max(1024),
  })
  .strict();
export type HarnessProbeSnapshot = z.infer<typeof HarnessProbeSnapshotSchema>;

const actionOutcome = z.enum([
  "accepted",
  "not_ready",
  "stale_view",
  "permission_denied",
  "busy",
  "cancelled",
  "failed",
]);
export const HarnessFailureSchema = z.discriminatedUnion("kind", [
  z.object({ kind: z.literal("native_error"), code: ErrorCodeSchema }).strict(),
  z
    .object({
      kind: z.literal("action_rejected"),
      outcome: actionOutcome.exclude(["accepted"]),
      native_code: ErrorCodeSchema.nullable().optional(),
    })
    .strict(),
  z
    .object({
      kind: z.literal("settle_timeout"),
      last_action_outcome: actionOutcome.nullable(),
    })
    .strict(),
  z.object({ kind: z.literal("assertion") }).strict(),
  z.object({ kind: z.literal("request_timeout") }).strict(),
  z.object({ kind: z.literal("diagnostic_timeout") }).strict(),
  z.object({ kind: z.literal("unclassified") }).strict(),
]);
export type HarnessFailure = z.infer<typeof HarnessFailureSchema>;

/** Carries only a compiled category; error messages never enter artifacts. */
export class HarnessScenarioError extends Error {
  constructor(
    readonly failure: HarnessFailure,
    message = "The fixed retained scenario failed",
  ) {
    super(message);
  }
}

export function classifyHarnessFailure(error: unknown): HarnessFailure {
  if (error instanceof HarnessScenarioError) return error.failure;
  if (error && typeof error === "object" && "code" in error) {
    const code = ErrorCodeSchema.safeParse(error.code);
    if (code.success) return { kind: "native_error", code: code.data };
  }
  return { kind: "unclassified" };
}

/** An observation may register a native owner; it never activates one. */
export const HarnessActivitySchema = z
  .object({ generation: decimal, active: z.boolean() })
  .strict();
export const HarnessDocumentActivitySchema = z
  .object({
    document_visibility: z.enum(["visible", "hidden"]),
    own_activity: HarnessActivitySchema,
  })
  .strict();
export type HarnessDocumentActivity = z.infer<
  typeof HarnessDocumentActivitySchema
>;

/** Bounds observation latency even when native IPC cannot be cancelled. */
export async function readHarnessDiagnostic<T>(
  read: () => Promise<T>,
  timeoutMs = HARNESS_DIAGNOSTIC_TIMEOUT_MS,
): Promise<{ value: T | null; failure: HarnessFailure | null }> {
  let timer: ReturnType<typeof setTimeout> | undefined;
  const timeout = new Promise<{
    value: null;
    failure: HarnessFailure;
  }>((resolve) => {
    timer = setTimeout(
      () => resolve({ value: null, failure: { kind: "diagnostic_timeout" } }),
      Math.min(HARNESS_DIAGNOSTIC_TIMEOUT_MS, Math.max(0, timeoutMs)),
    );
  });
  try {
    return await Promise.race([
      Promise.resolve()
        .then(read)
        .then(
          (value) => ({ value, failure: null }),
          (error: unknown) => ({
            value: null,
            failure: classifyHarnessFailure(error),
          }),
        ),
      timeout,
    ]);
  } finally {
    clearTimeout(timer);
  }
}
const obsoleteReadOutcome = z.union([
  actionOutcome,
  z.literal("request_failed"),
]);
export const HarnessObsoleteReadsSchema = z
  .object({
    retention_reset: obsoleteReadOutcome.nullable(),
    disconnect: obsoleteReadOutcome.nullable(),
  })
  .strict();
export type HarnessObsoleteReads = z.infer<typeof HarnessObsoleteReadsSchema>;
export const HarnessAuthorityChecksSchema = z
  .object({
    controller: actionOutcome,
    renew_lease: actionOutcome,
    release_lease: actionOutcome,
    disconnect: actionOutcome,
    inspect_owner: actionOutcome,
    set_owner: actionOutcome,
    dispose_owner: actionOutcome,
    connect_synthetic_pat: actionOutcome,
  })
  .strict();
export type HarnessAuthorityChecks = z.infer<
  typeof HarnessAuthorityChecksSchema
>;

export const HarnessResultSchema = z
  .object({
    run_nonce: nonce,
    scenario_generation: decimal,
    request_id: nonce,
    label,
    outcome: actionOutcome,
    snapshot: HarnessProbeSnapshotSchema.nullable(),
    authority: HarnessAuthorityChecksSchema.optional(),
    activity: HarnessDocumentActivitySchema.optional(),
    performance: z.lazy(() => HarnessPerformanceViewSchema).optional(),
    failure: HarnessFailureSchema.optional(),
  })
  .strict();
export type HarnessResult = z.infer<typeof HarnessResultSchema>;

/** Match every incarnation field, including native-issued target label. */
export function matchesHarnessRequest(
  request: HarnessRequest,
  result: HarnessResult,
): boolean {
  return (
    request.run_nonce === result.run_nonce &&
    request.scenario_generation === result.scenario_generation &&
    request.request_id === result.request_id &&
    request.label === result.label
  );
}

export interface HarnessEventTransport {
  listen(listener: (payload: unknown) => void): Promise<() => void>;
  emit(request: HarnessRequest): Promise<void>;
}

/** A bounded request table for the compiled main executor, never a sync engine. */
export function createHarnessRequester({
  transport,
  runNonce,
  generation,
  allowedLabels,
}: {
  transport: HarnessEventTransport;
  runNonce: string;
  generation: () => string;
  allowedLabels: () => readonly string[];
}) {
  let stopped = false;
  let unlisten: (() => void) | undefined;
  const pending = new Map<
    string,
    {
      request: HarnessRequest;
      finish: (result: HarnessResult | null) => void;
    }
  >();
  const ready = transport
    .listen((payload) => {
      const parsed = HarnessResultSchema.safeParse(payload);
      if (!parsed.success || stopped) return;
      const entry = pending.get(parsed.data.request_id);
      if (entry && matchesHarnessRequest(entry.request, parsed.data)) {
        entry.finish(
          entry.request.scenario_generation === generation() &&
            allowedLabels().includes(entry.request.label)
            ? parsed.data
            : null,
        );
      }
    })
    .then((remove) => {
      if (stopped) remove();
      else unlisten = remove;
    });
  // Installation failure remains observable by request(), without a detached
  // rejected promise if teardown happens before the first request.
  void ready.catch(() => undefined);

  return {
    async request(
      label: string,
      action: HarnessAction,
    ): Promise<HarnessResult> {
      if (!allowedLabels().includes(label))
        throw new Error("Harness target was not issued by the native manifest");
      await ready;
      if (stopped) throw new Error("Harness requester has stopped");
      if (!allowedLabels().includes(label))
        throw new Error("Harness target was retired during registration");
      if (pending.size >= HARNESS_MAX_PENDING)
        throw new Error("Harness pending request limit reached");
      const request = HarnessRequestSchema.parse({
        run_nonce: runNonce,
        scenario_generation: generation(),
        request_id: crypto.randomUUID(),
        label,
        action,
      });
      return new Promise<HarnessResult>((resolve, reject) => {
        const timer = setTimeout(
          () => finish(null),
          action.kind === "benchmark" ? 180_000 : HARNESS_REQUEST_TIMEOUT_MS,
        );
        function finish(result: HarnessResult | null) {
          if (!pending.delete(request.request_id)) return;
          clearTimeout(timer);
          if (result) resolve(result);
          else
            reject(
              new HarnessScenarioError(
                { kind: "request_timeout" },
                "Harness request did not complete",
              ),
            );
        }
        pending.set(request.request_id, { request, finish });
        void transport.emit(request).catch(() => finish(null));
      });
    },
    stop() {
      if (stopped) return;
      stopped = true;
      unlisten?.();
      unlisten = undefined;
      for (const entry of [...pending.values()]) entry.finish(null);
    },
  };
}

export const HarnessScenarioSchema = z.enum([
  "concurrent-demand",
  "hints-and-catchup",
  "reload-and-expiry",
  "normal-tab-lifecycle",
  "disconnect",
  "crash-before-commit",
  "crash-after-commit",
  "restart",
  "authority",
  "vault-unavailable",
  "performance",
  "performance-restart",
]);
export type HarnessScenario = z.infer<typeof HarnessScenarioSchema>;

export const HarnessVaultEvidenceSchema = z
  .object({
    scope: z.literal("synthetic-native-vault"),
    account_id: z.string().min(1).max(256),
    authorization_epoch: decimal,
    credential_error: z.literal("credential_store_unavailable"),
    credential_error_cleared: z.literal(true),
    facet_revisions: z
      .object({ before: decimal, blocked: decimal, recovered: decimal })
      .strict(),
    provider_calls: z
      .object({ before: decimal, blocked: decimal, recovered: decimal })
      .strict(),
    vault_failures: z
      .object({ before: decimal, blocked: decimal, recovered: decimal })
      .strict(),
    body_hashes: z
      .object({ before: hash, blocked: hash, recovered: hash })
      .strict(),
    draft_hashes: z
      .object({ before: hash, blocked: hash, recovered: hash })
      .strict(),
    authorization_preserved: z.literal(true),
  })
  .strict()
  .refine((value) => {
    try {
      return (
        value.provider_calls.before === value.provider_calls.blocked &&
        BigInt(value.provider_calls.recovered) >
          BigInt(value.provider_calls.blocked) &&
        BigInt(value.vault_failures.blocked) >
          BigInt(value.vault_failures.before) &&
        value.vault_failures.recovered === value.vault_failures.blocked &&
        value.body_hashes.before === value.body_hashes.blocked &&
        value.body_hashes.recovered === value.body_hashes.blocked &&
        BigInt(value.facet_revisions.blocked) >=
          BigInt(value.facet_revisions.before) &&
        BigInt(value.facet_revisions.recovered) >
          BigInt(value.facet_revisions.blocked) &&
        value.draft_hashes.before === value.draft_hashes.blocked &&
        value.draft_hashes.recovered === value.draft_hashes.blocked
      );
    } catch {
      return false;
    }
  });

export type HarnessVaultEvidence = z.infer<typeof HarnessVaultEvidenceSchema>;

export const HarnessAuthorityEvidenceSchema = z
  .object({
    scope: z.literal("native-secondary-window"),
    target: z.literal("manifest-primary-body"),
    checks: HarnessAuthorityChecksSchema,
    main_renewed: z.boolean(),
    main_released: z.boolean(),
    lease_count_restored: z.boolean(),
    account_snapshot_unchanged: z.boolean(),
    native_revision_unchanged: z.boolean(),
    native_owner_unchanged: z.boolean(),
    provider_calls_unchanged: z.boolean(),
    vault_access_unchanged: z.boolean(),
  })
  .strict();
export type HarnessAuthorityEvidence = z.infer<
  typeof HarnessAuthorityEvidenceSchema
>;

const commitOid = z.string().regex(/^(?:[a-f0-9]{40}|[a-f0-9]{64})$/);
export const HarnessPullCommitEvidenceSchema = z
  .object({
    account_id: z.string().min(1).max(256),
    subject_id: z.string().min(1).max(512),
    context: PullCommitContextSchema,
    facet_revision: decimal,
    oids: z.array(commitOid).min(1).max(8),
    completeness: PullCommitCompletenessSchema,
    cache_only: z.boolean(),
    provider_call_count_before: decimal,
    provider_call_count_after: decimal,
    vault_load_count_before: decimal,
    vault_load_count_after: decimal,
  })
  .strict();
export type HarnessPullCommitEvidence = z.infer<
  typeof HarnessPullCommitEvidenceSchema
>;

const utcTimestamp = z
  .string()
  .min(20)
  .max(40)
  .regex(
    /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d{1,9})?(?:Z|[+-]\d{2}:\d{2})$/,
  );
export const HarnessLocalInboxEvidenceSchema = z
  .object({
    account_id: z.literal("ruru103:primary"),
    operation: z.enum(["write", "restart_read"]),
    entries: z
      .array(
        z
          .object({
            notification_id: z.enum([
              "github:notification:9007199254744991",
              "github:notification:9007199254744992",
              "github:notification:9007199254744993",
            ]),
            disposition: z.enum(["inbox", "done"]),
            effective_disposition: z.enum(["inbox", "snoozed", "done"]),
            bookmarked: z.boolean(),
            snoozed_until: utcTimestamp.nullable(),
            activity_updated_at: utcTimestamp,
            superseded_by_activity: z.boolean(),
            generation: decimal,
          })
          .strict(),
      )
      .length(3),
    provider_call_count_before: decimal,
    provider_call_count_after: decimal,
    vault_load_count_before: decimal,
    vault_load_count_after: decimal,
  })
  .strict();
export type HarnessLocalInboxEvidence = z.infer<
  typeof HarnessLocalInboxEvidenceSchema
>;

const performanceDuration = z.number().finite().min(0).max(60_000);
const performanceEpoch = z
  .number()
  .finite()
  .min(1_700_000_000_000)
  .max(4_102_444_800_000);
export const HarnessPerformanceSampleSchema = z
  .object({
    duration_ms: performanceDuration,
    payload_bytes: z
      .number()
      .int()
      .min(0)
      .max(4 * 1024 * 1024)
      .nullable(),
  })
  .strict();
export const HarnessPerformanceCaseSchema = z
  .object({
    name: z.enum([
      "list_local_ipc",
      "list_memory_hit",
      "search_local_ipc",
      "search_memory_hit",
      "detail_local_ipc",
      "detail_memory_hit",
    ]),
    boundary: z.enum(["react_useful_content", "sdk_ipc", "query_memory"]),
    samples: z.array(HarnessPerformanceSampleSchema).min(1).max(30),
  })
  .strict();
export const HarnessPerformanceViewSchema = z
  .object({
    phase: z.enum(["warm", "restart"]),
    webview_label: label,
    role: z.enum(["main", "concurrent_child"]),
    sample_count: z.union([z.literal(10), z.literal(30)]),
    account_id: z.literal("ruru103:alternate"),
    landing_mode: z.enum(["automatic_navigation", "benchmark_request"]),
    benchmark_request_epoch_ms: performanceEpoch,
    workspace_mount_epoch_ms: performanceEpoch,
    first_useful_epoch_ms: performanceEpoch,
    navigation_to_first_useful_ms: performanceDuration,
    exact_first_title: z.literal(
      "RURU-125 cached pull 4999 alternate repository 4",
    ),
    cases: z.array(HarnessPerformanceCaseSchema).length(9),
  })
  .strict()
  .superRefine((value, context) => {
    if (value.workspace_mount_epoch_ms > value.first_useful_epoch_ms)
      context.addIssue({
        code: "custom",
        message: "Useful content predates its workspace mount",
      });
    if (
      value.landing_mode === "benchmark_request" &&
      value.benchmark_request_epoch_ms > value.workspace_mount_epoch_ms
    )
      context.addIssue({
        code: "custom",
        message: "Requested landing predates its benchmark request",
      });
    const keys = value.cases.map((entry) => `${entry.name}:${entry.boundary}`);
    if (new Set(keys).size !== keys.length)
      context.addIssue({
        code: "custom",
        message: "Duplicate performance case",
      });
  });
export type HarnessPerformanceView = z.infer<
  typeof HarnessPerformanceViewSchema
>;
export const HarnessPerformanceEvidenceSchema = z
  .object({
    dataset_version: z.literal(1),
    item_count: z.literal(10_000),
    account_count: z.literal(2),
    repositories_per_account: z.literal(5),
    items_per_account: z.literal(5_000),
    views: z.array(HarnessPerformanceViewSchema).length(2),
    provider_call_count_before: decimal,
    provider_call_count_after: decimal,
    vault_load_count_before: decimal,
    vault_load_count_after: decimal,
  })
  .strict();
export type HarnessPerformanceEvidence = z.infer<
  typeof HarnessPerformanceEvidenceSchema
>;

const scenarioStatus = HarnessStatusSchema.refine(
  (value) =>
    value.core.actors.length <= 2 &&
    value.core.calls.length <= 128 &&
    value.core.gates.length <= 2 &&
    value.local_reads.length <= 2 &&
    value.held_hint_revisions.length <= 128 &&
    value.performance_queries.length <= 512,
);
export const HarnessFailureContextSchema = z
  .object({
    status: scenarioStatus.nullable(),
    status_failure: HarnessFailureSchema.nullable(),
    activities: z
      .array(
        z
          .object({
            label,
            document_visibility: z.enum(["visible", "hidden"]).nullable(),
            own_activity: HarnessActivitySchema.nullable(),
            native_owner: HarnessActivitySchema.nullable(),
            probe_failure: HarnessFailureSchema.nullable(),
            owner_failure: HarnessFailureSchema.nullable(),
          })
          .strict(),
      )
      .max(4),
  })
  .strict();
export type HarnessFailureContext = z.infer<typeof HarnessFailureContextSchema>;

export const HarnessScenarioResultSchema = z
  .object({
    scenario: HarnessScenarioSchema,
    outcome: z.enum(["passed", "checkpoint", "failed"]),
    stage: z.string().min(1).max(160),
    status: scenarioStatus.nullable(),
    observations: z.array(HarnessProbeSnapshotSchema).max(32),
    authority: HarnessAuthorityEvidenceSchema.nullable().optional(),
    vault: HarnessVaultEvidenceSchema.nullable().optional(),
    pull_commits: HarnessPullCommitEvidenceSchema.nullable(),
    local_inbox: HarnessLocalInboxEvidenceSchema.nullable(),
    performance: HarnessPerformanceEvidenceSchema.nullable().optional(),
    obsolete_reads: HarnessObsoleteReadsSchema.optional(),
    failure: HarnessFailureSchema.nullable().optional(),
    cleanup_failure: HarnessFailureSchema.nullable().optional(),
    cleanup_stage: z
      .enum(["release_native_lease", "restore_native_scenario"])
      .nullable()
      .optional(),
    failure_context: HarnessFailureContextSchema.nullable().optional(),
  })
  .strict();
export type HarnessScenarioResult = z.infer<typeof HarnessScenarioResultSchema>;

declare global {
  interface Window {
    __GITRU_COLLABORATION_HARNESS__?: {
      runScenario(scenario: HarnessScenario): Promise<HarnessScenarioResult>;
    };
  }
}

// Only these fixed synthetic edits can cross the renderer action boundary.
export const HARNESS_DRAFT_EDITS = {
  "first-edit": "RURU-103 first private edit\n日本語 retained locally.\n",
  "second-edit": "RURU-103 second private edit\nλ local CAS.\n",
} as const;

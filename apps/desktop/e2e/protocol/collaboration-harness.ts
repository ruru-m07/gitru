/** Finite renderer protocol. Imported only by the retained harness build/tests. */

import {
  DetailValueStateSchema,
  HarnessStatusSchema,
  type HarnessViewManifest,
} from "@gitru/commands";
import { z } from "zod";

export const HARNESS_REQUEST_EVENT = "gitru:e2e-collaboration-harness:request";
export const HARNESS_RESULT_EVENT = "gitru:e2e-collaboration-harness:result";
export const HARNESS_MAX_PENDING = 8;
export const HARNESS_REQUEST_TIMEOUT_MS = 10_000;

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
          HARNESS_REQUEST_TIMEOUT_MS,
        );
        function finish(result: HarnessResult | null) {
          if (!pending.delete(request.request_id)) return;
          clearTimeout(timer);
          if (result) resolve(result);
          else reject(new Error("Harness request did not complete"));
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
]);
export type HarnessScenario = z.infer<typeof HarnessScenarioSchema>;

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

export const HarnessScenarioResultSchema = z
  .object({
    scenario: HarnessScenarioSchema,
    outcome: z.enum(["passed", "checkpoint", "failed"]),
    stage: z.string().min(1).max(160),
    status: HarnessStatusSchema.refine(
      (value) =>
        value.core.actors.length <= 2 &&
        value.core.calls.length <= 128 &&
        value.core.gates.length <= 2 &&
        value.local_reads.length <= 2 &&
        value.held_hint_revisions.length <= 128,
    ).nullable(),
    observations: z.array(HarnessProbeSnapshotSchema).max(32),
    authority: HarnessAuthorityEvidenceSchema.nullable().optional(),
    obsolete_reads: HarnessObsoleteReadsSchema.optional(),
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

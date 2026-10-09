/** Fixed scenarios compiled only into the retained native qualification app. */
import { collaboration } from "@gitru/collaboration-client";
import {
  collaborationAcquireDemand,
  collaborationHarnessControl,
  collaborationHarnessStatus,
  collaborationHarnessViewManifest,
  collaborationInspectDemandOwner,
  collaborationReleaseDemand,
  collaborationRenewDemand,
  collaborationSetDemandOwnerActivity,
  type DemandLeaseReceipt,
  type HarnessControlRequest,
  type InboxPage,
  type PullCommitSnapshot,
} from "@gitru/commands";
import { getCurrentWebview, Webview } from "@tauri-apps/api/webview";
import {
  classifyHarnessFailure,
  createHarnessRequester,
  HARNESS_DRAFT_EDITS,
  HARNESS_REQUEST_EVENT,
  HARNESS_RESULT_EVENT,
  type HarnessAction,
  type HarnessAuthorityEvidence,
  type HarnessFailure,
  type HarnessFailureContext,
  type HarnessLocalInboxEvidence,
  type HarnessObsoleteReads,
  HarnessPeerLeaseSchema,
  type HarnessPerformanceEvidence,
  type HarnessProbeSnapshot,
  type HarnessPullCommitEvidence,
  type HarnessScenario,
  HarnessScenarioError,
  type HarnessScenarioResult,
  HarnessScenarioSchema,
  readHarnessDiagnostic,
} from "../../e2e/protocol/collaboration-harness";
import { requestAccountSettings } from "../features/collaboration/account-dialog-events";
import { useAppStore } from "../store/use-app-store";
import { router } from "./create-router";
import type { installCollaborationHarnessProbe } from "./e2e-collaboration-harness";
import { syntheticFingerprint } from "./e2e-collaboration-harness-observation";
import { sanitizeTabWebviewLabel } from "./runtime-utils";

type Probe = Awaited<ReturnType<typeof installCollaborationHarnessProbe>>;
type NativeHarnessAction = HarnessControlRequest["action"];
type HarnessCoreAction = NonNullable<HarnessControlRequest["core_action"]>;
type BoundAction = Extract<
  HarnessAction,
  { kind: "edit-draft" | "save-draft" | "read-item" | "read-body" }
>;

function requiresHarnessBinding(action: HarnessAction): action is BoundAction {
  return ["edit-draft", "save-draft", "read-item", "read-body"].includes(
    action.kind,
  );
}

const BODY = {
  one: "RURU-103 primary body phase one — π 🌱",
  two: "RURU-103 primary body phase two — λ 🌿",
} as const;
const PULL_COMMITS = {
  accountId: "ruru103:primary",
  subjectId: "github:pull:9007199254742993",
  baseOid: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
  headOid: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
  sourceRepositoryProviderId: "9007199254741994",
  oids: [
    "cccccccccccccccccccccccccccccccccccccccc",
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
  ],
} as const;
const LOCAL_INBOX = {
  accountId: "ruru103:primary",
  doneId: "github:notification:9007199254744991",
  snoozedId: "github:notification:9007199254744992",
  bookmarkedId: "github:notification:9007199254744993",
} as const;
const STEP_TIMEOUT_MS = 25_000;
const SCENARIO_TIMEOUT_MS = 160_000;

/** Main owns only the finite controller; every view uses its production SDK. */
export async function installCollaborationHarnessExecutor(probe: Probe) {
  const view = getCurrentWebview();
  const manifest = await collaborationHarnessViewManifest({});
  if (view.label !== "main" || manifest.role !== "main")
    throw new Error("The retained scenario executor requires native main");
  const runNonce = manifest.run_nonce;
  const sessionId = manifest.session_id;
  let current = await collaborationHarnessStatus({
    request: { run_nonce: runNonce },
  });
  let alive = true;
  let running = false;
  const normalLabels = new Set<string>();
  const requester = createHarnessRequester({
    runNonce,
    generation: () => current.core.scenario_generation,
    allowedLabels: () => [
      "main",
      ...normalLabels,
      ...(current.child_label ? [current.child_label] : []),
    ],
    transport: {
      listen: async (handler) =>
        view.listen<unknown>(HARNESS_RESULT_EVENT, ({ payload }) =>
          handler(payload),
        ),
      emit: (request) =>
        view.emitTo(
          { kind: "Webview", label: request.label },
          HARNESS_REQUEST_EVENT,
          request,
        ),
    },
  });

  async function status() {
    const next = await collaborationHarnessStatus({
      request: { run_nonce: runNonce },
    });
    if (
      !alive ||
      next.core.run_nonce !== runNonce ||
      next.core.session_id !== sessionId
    )
      throw new Error("The native retained session was retired");
    current = next;
    return next;
  }

  async function control(
    action: NativeHarnessAction,
    coreAction: HarnessCoreAction | null = null,
    gateId: string | null = null,
  ) {
    await status();
    const receipt = await collaborationHarnessControl({
      request: {
        run_nonce: runNonce,
        expected_generation: current.core.scenario_generation,
        action,
        core_action: coreAction,
        gate_id: gateId,
      },
    });
    current = receipt.status;
    return receipt;
  }
  const core = (action: HarnessCoreAction, gateId: string | null = null) =>
    control("core", action, gateId);

  async function request(label: string, action: HarnessAction) {
    await status();
    if (label === "main") {
      const receipt = await probe.execute({
        run_nonce: runNonce,
        scenario_generation: current.core.scenario_generation,
        request_id: crypto.randomUUID(),
        label,
        action,
      });
      return receipt;
    }
    return requester.request(label, action);
  }

  async function runScenario(input: HarnessScenario) {
    const scenario = HarnessScenarioSchema.parse(input);
    if (!alive || running)
      throw new Error("The retained scenario executor is unavailable");
    running = true;
    const performanceScenario =
      scenario === "performance" || scenario === "performance-restart";
    const deadline =
      Date.now() + (performanceScenario ? 5 * 60_000 : SCENARIO_TIMEOUT_MS);
    const observations: HarnessProbeSnapshot[] = [];
    const obsoleteReads: HarnessObsoleteReads = {
      retention_reset: null,
      disconnect: null,
    };
    let authority: HarnessAuthorityEvidence | null = null;
    let pullCommits: HarnessPullCommitEvidence | null = null;
    let localInbox: HarnessLocalInboxEvidence | null = null;
    let performanceEvidence: HarnessPerformanceEvidence | null = null;
    let manualLease: DemandLeaseReceipt | null = null;
    let manualLeaseBaseline = 0;
    let stage = "read native fixture";
    let checkpoint = false;
    let failed = false;
    let failure: HarnessFailure | null = null;
    let cleanupFailure: HarnessFailure | null = null;
    let cleanupStage:
      | "release_native_lease"
      | "restore_native_scenario"
      | null = null;
    let failureContext: HarnessFailureContext | null = null;
    const normalTabIds = new Set<string>();
    let normalHostMounted = false;

    function require(condition: unknown) {
      if (!condition) throw new HarnessScenarioError({ kind: "assertion" });
    }
    function pullCommitEvidence(
      accountId: string,
      snapshot: PullCommitSnapshot,
      before: { provider: string; vault: string },
      after: { provider: string; vault: string },
      cacheOnly: boolean,
    ): HarnessPullCommitEvidence {
      require(accountId === PULL_COMMITS.accountId);
      require(snapshot.subject_id === PULL_COMMITS.subjectId);
      require(snapshot.context);
      if (!snapshot.context)
        throw new HarnessScenarioError({ kind: "assertion" });
      require(snapshot.context.base_oid === PULL_COMMITS.baseOid);
      require(snapshot.context.head_oid === PULL_COMMITS.headOid);
      require(
        snapshot.context.source_repository_provider_id ===
          PULL_COMMITS.sourceRepositoryProviderId,
      );
      require(/^[1-9]\d*$/.test(snapshot.context.metadata_facet_revision));
      const facetRevision = snapshot.facet_revision;
      require(facetRevision && /^[1-9]\d*$/.test(facetRevision));
      if (!facetRevision) throw new HarnessScenarioError({ kind: "assertion" });
      require(snapshot.completeness.state === "complete");
      require(snapshot.completeness.reason === null);
      require(snapshot.coverage.state === "complete");
      require(!snapshot.coverage.remote_has_more);
      require(snapshot.next_cursor === null);
      require(snapshot.commits.length === PULL_COMMITS.oids.length);
      for (const [position, oid] of PULL_COMMITS.oids.entries()) {
        require(snapshot.commits[position]?.position === position);
        require(snapshot.commits[position]?.oid === oid);
      }
      if (cacheOnly) {
        require(before.provider === "0" && after.provider === "0");
        require(before.vault === "0" && after.vault === "0");
      } else {
        require(BigInt(after.provider) > BigInt(before.provider));
        require(BigInt(after.vault) > BigInt(before.vault));
      }
      return {
        account_id: accountId,
        subject_id: snapshot.subject_id,
        context: snapshot.context,
        facet_revision: facetRevision,
        oids: snapshot.commits.map((commit) => commit.oid),
        completeness: snapshot.completeness,
        cache_only: cacheOnly,
        provider_call_count_before: before.provider,
        provider_call_count_after: after.provider,
        vault_load_count_before: before.vault,
        vault_load_count_after: after.vault,
      };
    }
    function localInboxEvidence(
      snapshot: InboxPage,
      before: { provider: string; vault: string },
      after: { provider: string; vault: string },
      operation: HarnessLocalInboxEvidence["operation"],
    ): HarnessLocalInboxEvidence {
      require(snapshot.entries.length === 3);
      require(snapshot.next_cursor === null);
      const byId = new Map(
        snapshot.entries.map((entry) => [entry.item.id, entry] as const),
      );
      const done = byId.get(LOCAL_INBOX.doneId);
      const snoozed = byId.get(LOCAL_INBOX.snoozedId);
      const bookmarked = byId.get(LOCAL_INBOX.bookmarkedId);
      require(done && snoozed && bookmarked);
      if (!done || !snoozed || !bookmarked)
        throw new HarnessScenarioError({ kind: "assertion" });
      require(
        done.local.disposition === "done" &&
          done.local.effective_disposition === "done" &&
          !done.local.bookmarked &&
          done.local.snoozed_until === null &&
          !done.local.superseded_by_activity &&
          done.local.generation === "1",
      );
      require(
        snoozed.local.disposition === "inbox" &&
          snoozed.local.effective_disposition === "snoozed" &&
          !snoozed.local.bookmarked &&
          snoozed.local.snoozed_until !== null &&
          !snoozed.local.superseded_by_activity &&
          snoozed.local.generation === "1",
      );
      require(
        bookmarked.local.disposition === "inbox" &&
          bookmarked.local.effective_disposition === "inbox" &&
          bookmarked.local.bookmarked &&
          bookmarked.local.snoozed_until === null &&
          !bookmarked.local.superseded_by_activity &&
          bookmarked.local.generation === "1",
      );
      require(
        snoozed.local.snoozed_until &&
          Date.parse(snoozed.local.snoozed_until) >
            Date.parse(snapshot.evaluated_at),
      );
      require(
        before.provider === after.provider && before.vault === after.vault,
      );
      if (operation === "restart_read")
        require(
          before.provider === "0" &&
            after.provider === "0" &&
            before.vault === "0" &&
            after.vault === "0",
        );
      return {
        account_id: LOCAL_INBOX.accountId,
        operation,
        entries: [done, snoozed, bookmarked].map((entry) => ({
          notification_id: entry.item.id as
            | typeof LOCAL_INBOX.doneId
            | typeof LOCAL_INBOX.snoozedId
            | typeof LOCAL_INBOX.bookmarkedId,
          disposition: entry.local.disposition,
          effective_disposition: entry.local.effective_disposition,
          bookmarked: entry.local.bookmarked,
          snoozed_until: entry.local.snoozed_until,
          activity_updated_at: entry.local.activity_updated_at,
          superseded_by_activity: entry.local.superseded_by_activity,
          generation: entry.local.generation,
        })) as HarnessLocalInboxEvidence["entries"],
        provider_call_count_before: before.provider,
        provider_call_count_after: after.provider,
        vault_load_count_before: before.vault,
        vault_load_count_after: after.vault,
      };
    }
    function keep(snapshot: HarnessProbeSnapshot) {
      if (observations.length < 32) observations.push(snapshot);
      return snapshot;
    }
    async function wait<T>(
      read: () => Promise<T | null>,
      cleanup = false,
      lastActionOutcome?: () =>
        | Awaited<ReturnType<typeof request>>["outcome"]
        | null,
    ) {
      const end = cleanup
        ? Date.now() + STEP_TIMEOUT_MS
        : Math.min(Date.now() + STEP_TIMEOUT_MS, deadline);
      while (alive && Date.now() < end) {
        const value = await read();
        if (value !== null) return value;
        await new Promise((resolve) => window.setTimeout(resolve, 100));
      }
      throw new HarnessScenarioError({
        kind: "settle_timeout",
        last_action_outcome: lastActionOutcome?.() ?? null,
      });
    }
    async function activate(label: string) {
      // Production native owner receipts remain authoritative. A desired
      // active host state cannot replace physical native availability.
      const owner = await collaborationInspectDemandOwner({
        ownerLabel: label,
      });
      const receipt = await collaborationSetDemandOwnerActivity({
        ownerLabel: label,
        expectedGeneration: owner.generation,
        active: true,
      });
      // A real false→true transition issues a new native generation. The
      // setter receipt, rather than the pre-set inspect, is authoritative.
      if (receipt.active) return receipt;
      return wait(async () => {
        const actual = await collaborationInspectDemandOwner({
          ownerLabel: label,
        });
        // Native show/host transitions can advance generation while waiting.
        // An older replay never proves this setter's current owner is active.
        require(BigInt(actual.generation) >= BigInt(receipt.generation));
        return actual.active ? actual : null;
      });
    }
    async function captureFailureContext(): Promise<HarnessFailureContext> {
      const labels = [
        ...new Set([
          "main",
          ...(current.child_label ? [current.child_label] : []),
          ...normalLabels,
        ]),
      ].slice(0, 4);
      // Failure-only observations run before detach, gate cancellation, or
      // route cleanup. A native own getter may register an owner; none of
      // these reads set activity or run inside authority counter assertions.
      const [native, activities] = await Promise.all([
        readHarnessDiagnostic(status),
        Promise.all(
          labels.map(async (label) => {
            const [own, owner] = await Promise.all([
              readHarnessDiagnostic(async () => {
                const receipt = await request(label, {
                  kind: "inspect-activity",
                });
                if (receipt.outcome !== "accepted" || !receipt.activity)
                  throw new HarnessScenarioError({
                    kind: "action_rejected",
                    outcome:
                      receipt.outcome === "accepted"
                        ? "failed"
                        : receipt.outcome,
                    native_code:
                      receipt.failure?.kind === "native_error"
                        ? receipt.failure.code
                        : null,
                  });
                return receipt.activity;
              }),
              readHarnessDiagnostic(() =>
                collaborationInspectDemandOwner({ ownerLabel: label }),
              ),
            ]);
            return {
              label,
              document_visibility:
                own.value?.document_visibility ??
                (label === "main" ? document.visibilityState : null),
              own_activity: own.value?.own_activity ?? null,
              native_owner: owner.value,
              probe_failure: own.failure,
              owner_failure: owner.failure,
            };
          }),
        ),
      ]);
      return {
        status: native.value,
        status_failure: native.failure,
        activities,
      };
    }
    async function inspect(label: string) {
      return requireSnapshot(label, { kind: "inspect" });
    }
    async function requestWhenBound(label: string, action: BoundAction) {
      let lastOutcome: Awaited<ReturnType<typeof request>>["outcome"] | null =
        null;
      return wait(
        async () => {
          const receipt = await request(label, action);
          lastOutcome = receipt.outcome;
          // A phase can publish its manifest before React commits the matching
          // real account binding. NotReady precedes a DOM mutation/local IPC,
          // so retry only that outcome. Accepted edits/saves run once; timeouts
          // and failed/stale/denied receipts never count as a successful action.
          return receipt.outcome === "not_ready" ? null : receipt;
        },
        false,
        () => lastOutcome,
      );
    }
    async function requireSnapshot(label: string, action: HarnessAction) {
      const receipt = requiresHarnessBinding(action)
        ? await requestWhenBound(label, action)
        : await request(label, action);
      if (receipt.outcome !== "accepted" || !receipt.snapshot)
        throw new HarnessScenarioError({
          kind: "action_rejected",
          outcome: receipt.outcome === "accepted" ? "failed" : receipt.outcome,
          native_code:
            receipt.failure?.kind === "native_error"
              ? receipt.failure.code
              : null,
        });
      return receipt.snapshot;
    }
    async function visibleDocument(label: string) {
      return wait(async () => {
        // Window show/native active receipts do not prove WebKit's document
        // visibility. Observe the actual document without overriding either
        // its DOM suspension or its independently authoritative native owner.
        const receipt = await request(label, { kind: "inspect-activity" });
        if (receipt.outcome !== "accepted" || !receipt.activity)
          throw new HarnessScenarioError({
            kind: "action_rejected",
            outcome:
              receipt.outcome === "accepted" ? "failed" : receipt.outcome,
            native_code:
              receipt.failure?.kind === "native_error"
                ? receipt.failure.code
                : null,
          });
        return receipt.activity.document_visibility === "visible"
          ? receipt.activity
          : null;
      });
    }
    async function mount(label: string, actor: "primary" | "alternate") {
      const mountingStage = stage;
      stage =
        label === "main"
          ? "observe actual main document visibility before fixture mount"
          : "observe actual child document visibility before fixture mount";
      await visibleDocument(label);
      stage = mountingStage;
      await requireSnapshot(label, { kind: "mount", actor });
      return keep(
        await wait(async () => {
          const snapshot = await inspect(label);
          return snapshot.actor === actor &&
            snapshot.mounted &&
            snapshot.item.status === "success" &&
            snapshot.draft_generation !== null &&
            snapshot.editor_hash !== null
            ? snapshot
            : null;
        }),
      );
    }
    async function child() {
      if (!current.child_label) await control("create_concurrent_child");
      const label = current.child_label;
      require(label);
      if (!label) throw new Error("The native child label is missing");
      await wait(async () => {
        const receipt = await request(label, { kind: "inspect" }).catch(
          () => null,
        );
        return receipt?.outcome === "accepted" ? true : null;
      });
      await activate(label);
      return label;
    }
    async function rendered(
      label: string,
      phase: keyof typeof BODY,
      expectedFacet?: string | null,
    ) {
      const expected = await syntheticFingerprint(BODY[phase]);
      return keep(
        await wait(async () => {
          const snapshot = await inspect(label);
          return snapshot.body_hash === expected &&
            snapshot.provider_visible &&
            snapshot.facet_revision !== null &&
            (!expectedFacet || snapshot.facet_revision === expectedFacet)
            ? snapshot
            : null;
        }),
      );
    }
    async function committed(phase: "one" | "two") {
      return wait(async () => {
        const receipt = await status();
        return receipt.core.committed_phase === phase ? receipt : null;
      });
    }
    async function refresh(
      phase: "phase_one" | "phase_two" | "phase_not_modified",
    ) {
      const previous = (await status()).core.committed_facet_revision;
      await core(phase);
      await core("advance_refresh");
      await wait(async () => {
        const receipt = await status();
        return receipt.core.committed_phase ===
          (phase === "phase_one" ? "one" : "two") &&
          receipt.core.committed_facet_revision !== previous
          ? receipt
          : null;
      });
    }
    async function providerGateHeld(id: string) {
      return wait(async () => {
        const receipt = await status();
        return receipt.core.gates.some(
          (gate) => gate.gate_id === id && gate.state === "held",
        )
          ? receipt
          : null;
      });
    }

    try {
      await status();
      if (performanceScenario) {
        if (!current.core.prepared) {
          require(scenario === "performance");
          stage = "prepare deterministic 10,000-item cache";
          await core("prepare_performance");
        }
        require(current.core.fixture === "performance");
        require(current.core.performance?.dataset_version === 1);
        require(current.core.performance?.item_count === 10_000);
      } else if (scenario !== "restart" && !current.core.prepared) {
        stage = "prepare synthetic primary account";
        await core("prepare_primary");
      }
      require(current.core.prepared);
      if (!performanceScenario) {
        stage = "wake the real main SDK";
        await collaboration.wake();
        stage = "activate the real native main demand owner";
        await activate("main");
      }

      if (performanceScenario) {
        const fixture = current.core.performance;
        require(fixture);
        if (!fixture) throw new HarnessScenarioError({ kind: "assertion" });
        const before = await status();
        const phase = scenario === "performance" ? "warm" : "restart";
        const mainSamples = scenario === "performance" ? 30 : 10;
        stage = "measure useful cached content in the real main webview";
        const main = await request("main", {
          kind: "benchmark",
          phase,
          sample_count: mainSamples,
        });
        require(main.outcome === "accepted" && main.performance);
        const label = await child();
        stage = "measure the same saved cache in a retained second native view";
        const secondary = await request(label, {
          kind: "benchmark",
          phase,
          sample_count: 10,
        });
        require(secondary.outcome === "accepted" && secondary.performance);
        if (!main.performance || !secondary.performance)
          throw new HarnessScenarioError({ kind: "assertion" });
        const after = await status();
        require(
          after.core.provider_call_count === before.core.provider_call_count &&
            after.core.vault_load_count === before.core.vault_load_count,
        );
        require(after.authorized_hydrate_requests === "0");
        performanceEvidence = {
          dataset_version: 1,
          item_count: 10_000,
          account_count: 2,
          repositories_per_account: 5,
          items_per_account: 5_000,
          views: [main.performance, secondary.performance],
          provider_call_count_before: before.core.provider_call_count,
          provider_call_count_after: after.core.provider_call_count,
          vault_load_count_before: before.core.vault_load_count,
          vault_load_count_after: after.core.vault_load_count,
        };
      } else if (scenario === "concurrent-demand") {
        stage = "arm missing Body response";
        const gate = (await core("arm_provider_gate")).gate_id;
        require(gate);
        if (!gate) throw new Error("The native provider gate is missing");
        const before = BigInt(current.core.provider_call_count);
        const label = await child();
        stage = "mount two physically visible resource owners";
        await mount("main", "primary");
        await mount(label, "primary");
        const held = await providerGateHeld(gate);
        await wait(async () =>
          (await status()).core.demand_lease_count === 2 ? true : null,
        );
        require(BigInt(held.core.provider_call_count) === before + 1n);
        require(held.core.durable_detail_requests === 0);
        const [mainOwner, childOwner] = await Promise.all([
          collaborationInspectDemandOwner({ ownerLabel: "main" }),
          collaborationInspectDemandOwner({ ownerLabel: label }),
        ]);
        require(mainOwner.active && childOwner.active);
        stage = "release one response to both real query caches";
        await core("release_provider_gate", gate);
        const main = await rendered("main", "one");
        const other = await rendered(label, "one");
        require(main.body_hash === other.body_hash);
        require(main.metadata_hash === other.metadata_hash);
        require(main.facet_revision === other.facet_revision);
        stage = "read local caches without dispatch";
        const calls = (await status()).core.provider_call_count;
        await requireSnapshot("main", { kind: "read-item" });
        await requireSnapshot(label, { kind: "read-body" });
        require((await status()).core.provider_call_count === calls);
        require(current.authorized_hydrate_requests === "0");
        stage = "retain one owner after the other detaches";
        await requireSnapshot("main", { kind: "detach" });
        await wait(async () =>
          (await status()).core.demand_lease_count === 1 ? true : null,
        );
        await requireSnapshot(label, { kind: "detach" });
        await wait(async () =>
          (await status()).core.demand_lease_count === 0 ? true : null,
        );
        stage = "detached stale Body admits no foreground call";
        await core("advance_refresh");
        require((await status()).core.provider_call_count === calls);
      } else if (scenario === "hints-and-catchup") {
        const label = await child();
        await mount("main", "primary");
        await mount(label, "primary");
        stage = "retain private editor during reordered hints";
        await requireSnapshot(label, {
          kind: "edit-draft",
          variant: "second-edit",
        });
        const dirty = await syntheticFingerprint(
          HARNESS_DRAFT_EDITS["second-edit"],
        );
        await wait(async () =>
          (await inspect(label)).editor_hash === dirty ? true : null,
        );
        const beforeHeld = keep(
          await wait(async () => {
            const snapshot = await inspect(label);
            return snapshot.body.status === "success" &&
              !snapshot.body.fetching &&
              snapshot.body.revision !== null &&
              snapshot.facet_revision !== null &&
              snapshot.editor_hash === dirty
              ? snapshot
              : null;
          }),
        );
        await control("hold_child_hints");
        await refresh("phase_one");
        await refresh("phase_two");
        await refresh("phase_not_modified");
        const finalRevision = current.core.committed_facet_revision;
        require(finalRevision);
        await wait(async () => {
          const latest = await status();
          return finalRevision &&
            latest.held_hint_revisions.length >= 2 &&
            latest.held_hint_revisions.includes(finalRevision)
            ? true
            : null;
        });
        stage = "verify actual child cache is unchanged while hints are held";
        const withheld = keep(await inspect(label));
        require(
          withheld.body.revision === beforeHeld.body.revision &&
            withheld.facet_revision === beforeHeld.facet_revision &&
            withheld.body_hash === beforeHeld.body_hash &&
            withheld.metadata_hash === beforeHeld.metadata_hash &&
            withheld.catchup.last_receipt === beforeHeld.catchup.last_receipt &&
            withheld.editor_hash === dirty,
        );
        stage = "deliver latest then older native hints";
        await control("deliver_child_hints_reverse");
        const final = await rendered(label, "two", finalRevision);
        require(final.facet_revision === finalRevision);
        require(final.editor_hash === dirty);
        await control("resume_hints");
        stage = "recover dropped hints after an explicit public SDK wake";
        const beforeDropped = keep(await inspect(label));
        await control("drop_child_hints");
        await refresh("phase_not_modified");
        const droppedFacet = current.core.committed_facet_revision;
        require(droppedFacet !== beforeDropped.facet_revision);
        stage =
          "verify actual child cache is unchanged while hints are dropped";
        const withoutHints = keep(await inspect(label));
        require(
          withoutHints.body.revision === beforeDropped.body.revision &&
            withoutHints.facet_revision === beforeDropped.facet_revision &&
            withoutHints.body_hash === beforeDropped.body_hash &&
            withoutHints.metadata_hash === beforeDropped.metadata_hash &&
            withoutHints.catchup.last_receipt ===
              beforeDropped.catchup.last_receipt &&
            withoutHints.editor_hash === dirty,
        );
        stage = "recover dropped hints after an explicit public SDK wake";
        await requireSnapshot(label, { kind: "wake" });
        await wait(async () => {
          const snapshot = await inspect(label);
          return snapshot.facet_revision === droppedFacet
            ? keep(snapshot)
            : null;
        });
        await control("resume_hints");
        stage = "edit main private draft after the current phase binds";
        const old = await inspect(label);
        await requireSnapshot("main", {
          kind: "edit-draft",
          variant: "first-edit",
        });
        stage = "observe main authored text before saving native CAS";
        const mainEditHash = await syntheticFingerprint(
          HARNESS_DRAFT_EDITS["first-edit"],
        );
        keep(
          await wait(async () => {
            const snapshot = await inspect("main");
            return snapshot.save_enabled &&
              snapshot.editor_hash === mainEditHash
              ? snapshot
              : null;
          }),
        );
        stage = "save main private draft through native CAS";
        await requireSnapshot("main", { kind: "save-draft" });
        stage = "observe child CAS conflict while retaining dirty text";
        const conflict = keep(
          await wait(async () => {
            const snapshot = await inspect(label);
            return snapshot.conflict_visible &&
              !snapshot.save_enabled &&
              snapshot.editor_hash === dirty &&
              snapshot.draft_generation !== old.draft_generation
              ? snapshot
              : null;
          }),
        );
        stage = "drain real 256-row pages despite irrelevant actor writes";
        await control("drop_child_hints");
        const paging = await inspect(label);
        await core("fill_catchup");
        await requireSnapshot(label, { kind: "wake" });
        keep(
          await wait(async () => {
            const snapshot = await inspect(label);
            return snapshot.catchup.pages_with_more >
              paging.catchup.pages_with_more &&
              snapshot.catchup.last_receipt === current.core.revision &&
              snapshot.editor_hash === dirty
              ? snapshot
              : null;
          }),
        );
        stage = "prepare real retention overflow before capturing a local read";
        const reset = await inspect(label);
        const filledRevision = (await core("fill_retention")).status.core
          .revision;
        function requirePreResetCursor(snapshot: HarnessProbeSnapshot) {
          require(
            snapshot.catchup.resets === reset.catchup.resets &&
              snapshot.catchup.last_receipt === reset.catchup.last_receipt &&
              snapshot.catchup.last_receipt !== null &&
              BigInt(snapshot.catchup.last_receipt) + 4096n <
                BigInt(filledRevision),
          );
        }
        // The real writes can take longer than a bounded held return. Prepare
        // them first with child hints dropped; its actual bridge cursor must
        // still precede the production retention floor before capturing a read.
        requirePreResetCursor(keep(await inspect(label)));
        stage = "capture an actual local read under the pre-reset SDK fence";
        const oldReadGate = (await control("arm_body_read")).gate_id;
        require(oldReadGate);
        if (!oldReadGate)
          throw new Error("The native retained read gate is missing");
        const oldRead = requestWhenBound(label, { kind: "read-body" }).then(
          (receipt) => receipt.outcome,
          () => "request_failed" as const,
        );
        await wait(async () =>
          (await status()).local_reads.some(
            (gate) => gate.gate_id === oldReadGate && gate.state === "held",
          )
            ? true
            : null,
        );
        requirePreResetCursor(await inspect(label));
        // The native snapshot may already carry the filled data revision. Its
        // SDK generation was captured before the actual ResetRequired below.
        stage =
          "apply real retention ResetRequired without discarding authored text";
        await requireSnapshot(label, { kind: "wake" });
        const after = keep(
          await wait(async () => {
            const snapshot = await inspect(label);
            return snapshot.catchup.resets > reset.catchup.resets &&
              snapshot.body.revision !== null &&
              BigInt(snapshot.body.revision) >= BigInt(filledRevision) &&
              snapshot.body_hash === reset.body_hash &&
              snapshot.editor_hash === dirty &&
              snapshot.draft_generation === conflict.draft_generation &&
              snapshot.conflict_visible
              ? snapshot
              : null;
          }),
        );
        require(after.saved_draft_hash !== after.editor_hash);
        stage = "fence an actual old local snapshot after retention reset";
        await control("release_local_read", null, oldReadGate);
        const oldReadOutcome = await oldRead;
        obsoleteReads.retention_reset = oldReadOutcome;
        require(
          oldReadOutcome === "stale_view" || oldReadOutcome === "cancelled",
        );
        const fenced = keep(await inspect(label));
        require(fenced.body.revision === after.body.revision);
        require(fenced.body_hash === after.body_hash);
        require(fenced.editor_hash === dirty);
        require(current.authorized_hydrate_requests === "0");
        await control("resume_hints");
      } else if (scenario === "reload-and-expiry") {
        const label = await child();
        const before = await mount(label, "primary");
        stage = "reload an actual child document without JS cleanup";
        const priorOwner = await collaborationInspectDemandOwner({
          ownerLabel: label,
        });
        await control("reload_concurrent_child");
        await wait(async () => {
          const receipt = await request(label, { kind: "inspect" }).catch(
            () => null,
          );
          return receipt?.snapshot &&
            receipt.snapshot.document_nonce !== before.document_nonce
            ? receipt.snapshot
            : null;
        });
        const nextOwner = await activate(label);
        require(nextOwner.generation !== priorOwner.generation);
        const reloaded = await mount(label, "primary");
        require(reloaded.document_nonce !== before.document_nonce);
        require(reloaded.saved_draft_hash === before.saved_draft_hash);
        stage = "native close fences owner after lost JavaScript cleanup";
        // Deliberately do not call the probe's detach/stop before native close.
        // The native close itself disposes the owner. The following advance
        // qualifies that this disposal remains fenced; it is not standalone
        // TTL-expiry evidence. Real lease expiry is covered by the core lane.
        await control("close_concurrent_child");
        await core("advance_lease_expiry");
        await wait(async () =>
          (await status()).core.demand_lease_count === 0 ? true : null,
        );
        const calls = current.core.provider_call_count;
        await core("advance_refresh");
        require((await status()).core.provider_call_count === calls);
        stage = "acquire independent new interest after actual recreation";
        const replacement = await child();
        require(replacement !== label);
        const fresh = await mount(replacement, "primary");
        require(fresh.document_nonce !== before.document_nonce);
        require(fresh.saved_draft_hash === before.saved_draft_hash);
      } else if (scenario === "normal-tab-lifecycle") {
        stage = "close actual concurrent secondary window";
        if (current.child_label) await control("close_concurrent_child");
        stage = "detach main fixture before ordinary native tab host";
        await requireSnapshot("main", { kind: "detach" });
        stage =
          "observe actual main document visibility before ordinary host startup";
        await visibleDocument("main");
        stage = "create ordinary first and second tab records";
        const first = useAppStore.getState().createTab({
          routePath: "/app/git",
          repositoryId: null,
          title: "RURU-103 first fixture tab",
        });
        const second = useAppStore.getState().createTab({
          routePath: "/app/git",
          repositoryId: null,
          title: "RURU-103 second fixture tab",
        });
        normalTabIds.add(first.id);
        normalTabIds.add(second.id);
        stage = "select the ordinary first tab record";
        useAppStore.getState().activateTab(first.id);
        stage = "navigate main into the ordinary native tab host";
        await router.navigate({ to: "/app", search: {}, replace: true });
        normalHostMounted = true;
        async function normalLabel(
          tabId: string,
          slot: "first" | "second" | "replacement",
        ) {
          const expected = sanitizeTabWebviewLabel(tabId);
          stage = {
            first: "observe actual first tab native surface",
            second: "observe actual second tab native surface",
            replacement: "observe actual replacement tab native surface",
          }[slot];
          const issued = await wait(async () => {
            const actual = (await Webview.getAll()).find(
              (candidate) => candidate.label === expected,
            );
            return actual?.label ?? null;
          });
          // Each actual document still proves its exact local native caller
          // through the generated own-view manifest. This inventory only
          // bounds the renderer's diagnostic destinations.
          normalLabels.add(issued);
          stage = {
            first: "handshake with actual first tab document",
            second: "handshake with actual second tab document",
            replacement: "handshake with actual replacement tab document",
          }[slot];
          await wait(async () => {
            const receipt = await request(issued, { kind: "inspect" }).catch(
              () => null,
            );
            return receipt?.outcome === "accepted" ? true : null;
          });
          return issued;
        }
        const firstLabel = await normalLabel(first.id, "first");
        const secondLabel = await normalLabel(second.id, "second");
        stage = "ordinary selected child deactivates main demand";
        await wait(async () => {
          const main = await collaborationInspectDemandOwner({
            ownerLabel: "main",
          });
          const selected = await collaborationInspectDemandOwner({
            ownerLabel: firstLabel,
          });
          return !main.active && selected.active ? true : null;
        });
        stage = "mount fixture in the actual selected first tab";
        const original = await mount(firstLabel, "primary");
        stage = "ordinary tab switch hides and fences prior owner";
        useAppStore.getState().activateTab(second.id);
        await wait(async () => {
          const previous = await collaborationInspectDemandOwner({
            ownerLabel: firstLabel,
          });
          const selected = await collaborationInspectDemandOwner({
            ownerLabel: secondLabel,
          });
          return !previous.active && selected.active ? true : null;
        });
        stage = "mount fixture in the actual selected second tab";
        await mount(secondLabel, "primary");
        stage = "host Accounts modal suspends native selected interest";
        await requestAccountSettings();
        await wait(async () => {
          const popup = document.querySelector(
            '[data-slot="dialog-popup"][data-open]:not([data-closed])',
          );
          const owner = await collaborationInspectDemandOwner({
            ownerLabel: secondLabel,
          });
          return popup && !owner.active ? true : null;
        });
        const popup = document.querySelector(
          '[data-slot="dialog-popup"][data-open]:not([data-closed])',
        );
        const close = Array.from(
          popup?.querySelectorAll<HTMLButtonElement>("button") ?? [],
        ).find(
          (button) => button.querySelector(".sr-only")?.textContent === "Close",
        );
        require(close);
        stage = "close host Accounts modal and restore selected owner";
        close?.click();
        await wait(async () =>
          (await collaborationInspectDemandOwner({ ownerLabel: secondLabel }))
            .active
            ? true
            : null,
        );
        stage = "inspect ordinary first owner before host disposal";
        const prior = await collaborationInspectDemandOwner({
          ownerLabel: firstLabel,
        });
        stage = "navigate out of the ordinary native tab host";
        await router.navigate({
          to: "/app/git",
          search: { embedded: 1 },
          replace: true,
        });
        normalHostMounted = false;
        stage = "observe native disposal of both ordinary tab surfaces";
        await wait(async () =>
          (await Webview.getAll()).every(
            (candidate) => !normalLabels.has(candidate.label),
          )
            ? true
            : null,
        );
        normalLabels.clear();
        stage = "select the first tab record for host recreation";
        useAppStore.getState().activateTab(first.id);
        stage = "navigate main into the recreated ordinary native host";
        await router.navigate({ to: "/app", search: {}, replace: true });
        normalHostMounted = true;
        const replacement = await normalLabel(first.id, "replacement");
        require(replacement === firstLabel);
        stage = "observe active replacement native owner";
        await wait(async () =>
          (await collaborationInspectDemandOwner({ ownerLabel: replacement }))
            .active
            ? true
            : null,
        );
        const owner = await collaborationInspectDemandOwner({
          ownerLabel: replacement,
        });
        stage = "require a fresh native generation for the replacement";
        require(owner.generation !== prior.generation);
        stage = "mount recreated document and preserve its saved private draft";
        const recreated = await mount(replacement, "primary");
        require(recreated.document_nonce !== original.document_nonce);
        require(recreated.saved_draft_hash === original.saved_draft_hash);
        stage = "public tab disposal removes its real native surface";
        useAppStore.getState().closeTab(first.id);
        await wait(async () =>
          (await Webview.getAll()).every(
            (candidate) => candidate.label !== firstLabel,
          )
            ? true
            : null,
        );
        normalLabels.delete(firstLabel);
      } else if (scenario === "authority") {
        const label = await child();
        stage = "mount actual main Body before authority freshness warmup";
        await mount("main", "primary");
        const fixture = current.core.actors.find(
          (actor) => actor.slot === "primary",
        );
        require(fixture);
        if (!fixture) throw new Error("Native primary actor is missing");
        stage = "observe real main Body demand before freshness warmup";
        await wait(async () =>
          (await status()).core.demand_lease_count === 1 ? true : null,
        );
        const prior = await status();
        const completedBodyCalls = (receipt: typeof current) =>
          receipt.core.calls.filter(
            (call) =>
              call.scenario_generation === prior.core.scenario_generation &&
              call.slot === "primary" &&
              call.authorization_epoch === fixture.authorization_epoch &&
              call.facet === "body" &&
              call.state === "completed",
          ).length;
        const priorCompleted = completedBodyCalls(prior);
        stage =
          "commit a fresh current-generation Body before peer counter baseline";
        await core("advance_refresh");
        const fresh = await wait(async () => {
          const receipt = await status();
          // The fake provider records Completed before SQLite applies the
          // detail. Both the new call and changed committed Body facet are
          // needed; cached-known/rendered state alone does not prove freshness.
          return receipt.core.committed_facet_revision !== null &&
            receipt.core.committed_facet_revision !==
              prior.core.committed_facet_revision &&
            completedBodyCalls(receipt) > priorCompleted
            ? receipt
            : null;
        });
        require(
          fresh.core.committed_phase === "one" ||
            fresh.core.committed_phase === "two",
        );
        stage =
          "render the exact freshly committed Body before detaching authority interest";
        await rendered(
          "main",
          fresh.core.committed_phase === "one" ? "one" : "two",
          fresh.core.committed_facet_revision,
        );
        stage =
          "detach normal SDK interest before acquiring the authority test lease";
        await requireSnapshot("main", { kind: "detach" });
        await wait(async () =>
          (await status()).core.demand_lease_count === 0 ? true : null,
        );
        manualLeaseBaseline = current.core.demand_lease_count;
        stage = "prepare one actual main lease for the fresh manifest Body";
        const mainOwner = await collaborationInspectDemandOwner({
          ownerLabel: "main",
        });
        require(mainOwner.active);
        manualLease = await collaborationAcquireDemand({
          request: {
            account_id: fixture.account_id,
            authorization_epoch: fixture.authorization_epoch,
            owner_generation: mainOwner.generation,
            target: {
              kind: "detail",
              repository_id: null,
              subject_id: fixture.subject_id,
              facet: "body",
            },
          },
        });
        const peer = HarnessPeerLeaseSchema.parse({
          lease_id: manualLease.lease_id,
          owner_label: "main",
          owner_generation: manualLease.owner_generation,
          account_id: fixture.account_id,
          authorization_epoch: fixture.authorization_epoch,
        });
        require(peer.owner_generation === mainOwner.generation);
        await wait(async () =>
          (await status()).core.demand_lease_count === manualLeaseBaseline + 1
            ? true
            : null,
        );
        stage = "deny foreign lease, credential and peer host authority";
        const accountsBefore = await collaboration.accounts();
        const before = await status();
        const receipt = await request(label, {
          kind: "check-peer-authority",
          peer,
        });
        require(receipt.outcome === "accepted" && receipt.authority);
        if (!receipt.authority)
          throw new Error("The native authority check receipt is missing");
        authority = {
          scope: "native-secondary-window",
          target: "manifest-primary-body",
          checks: receipt.authority,
          main_renewed: false,
          main_released: false,
          lease_count_restored: false,
          account_snapshot_unchanged: false,
          native_revision_unchanged: false,
          native_owner_unchanged: false,
          provider_calls_unchanged: false,
          vault_access_unchanged: false,
        };
        const renewal = await collaborationRenewDemand({
          request: {
            owner_generation: peer.owner_generation,
            leases: [
              {
                lease_id: peer.lease_id,
                account_id: peer.account_id,
                authorization_epoch: peer.authorization_epoch,
              },
            ],
          },
        });
        authority.main_renewed =
          renewal.leases.length === 1 &&
          renewal.leases[0].lease_id === peer.lease_id &&
          renewal.leases[0].owner_generation === peer.owner_generation;
        const accountsAfter = await collaboration.accounts();
        const mainAfter = await collaborationInspectDemandOwner({
          ownerLabel: "main",
        });
        const after = await status();
        authority.account_snapshot_unchanged =
          JSON.stringify(accountsBefore) === JSON.stringify(accountsAfter);
        authority.native_revision_unchanged =
          before.core.revision === after.core.revision;
        authority.native_owner_unchanged =
          mainAfter.generation === mainOwner.generation &&
          mainAfter.active === mainOwner.active;
        authority.provider_calls_unchanged =
          before.core.provider_call_count === after.core.provider_call_count;
        authority.vault_access_unchanged =
          before.core.vault_load_count === after.core.vault_load_count &&
          before.core.vault_store_count === after.core.vault_store_count &&
          before.core.vault_delete_count === after.core.vault_delete_count;
        require(
          Object.values(authority.checks).every(
            (outcome) => outcome === "permission_denied",
          ),
        );
        require(authority.main_renewed);
        require(authority.account_snapshot_unchanged);
        require(authority.native_revision_unchanged);
        require(authority.native_owner_unchanged);
        require(authority.provider_calls_unchanged);
        require(authority.vault_access_unchanged);
        require(
          before.core.scenario_generation === after.core.scenario_generation,
        );
        stage = "reject wrong run and stale scenario before side effects";
        for (const request of [
          {
            run_nonce: "ruru103-wrong-run",
            expected_generation: after.core.scenario_generation,
          },
          { run_nonce: runNonce, expected_generation: "0" },
        ]) {
          let rejected = false;
          try {
            await collaborationHarnessControl({
              request: {
                ...request,
                action: "core",
                core_action: "cancel_gates",
                gate_id: null,
              },
            });
          } catch {
            rejected = true;
          }
          require(rejected);
        }
        require((await status()).core.revision === before.core.revision);
      } else if (scenario === "disconnect") {
        const label = await child();
        await mount("main", "primary");
        const initial = await mount(label, "primary");
        stage =
          "detach both SDK interests before disconnect phase and provider gate";
        await requireSnapshot("main", { kind: "detach" });
        await requireSnapshot(label, { kind: "detach" });
        await wait(async () =>
          (await status()).core.demand_lease_count === 0 ? true : null,
        );
        stage = "advance disconnect phase without any live Body lease";
        await core("phase_two");
        stage = "arm old provider response before restoring SDK interest";
        const provider = (await core("arm_provider_gate")).gate_id;
        require(provider);
        if (!provider) throw new Error("The native provider gate is missing");
        stage = "remount both real owners behind the armed provider gate";
        await mount("main", "primary");
        await mount(label, "primary");
        stage = "observe both actual SDK leases behind the armed provider gate";
        await wait(async () =>
          (await status()).core.demand_lease_count === 2 ? true : null,
        );
        stage = "capture actual old provider response after due refresh";
        await core("advance_refresh");
        await providerGateHeld(provider);
        stage = "capture an authorized old local SDK return";
        const local = (await control("arm_body_read")).gate_id;
        require(local);
        if (!local) throw new Error("The native read gate is missing");
        const delayed = requestWhenBound(label, { kind: "read-body" });
        // Install the handler before any rejection so no old-epoch result is
        // left as an unhandled promise while the other native operations run.
        const settled = delayed.then(
          (receipt) => receipt.outcome,
          () => "request_failed" as const,
        );
        await wait(async () =>
          (await status()).local_reads.some(
            (gate) => gate.gate_id === local && gate.state === "held",
          )
            ? true
            : null,
        );
        stage = "disconnect through production SDK account fence";
        const actor = current.core.actors.find(
          (candidate) => candidate.slot === "primary",
        );
        require(actor);
        if (!actor) throw new Error("The native primary actor is missing");
        await collaboration.disconnect(actor.account_id);
        await requireSnapshot(label, { kind: "wake" });
        stage = "release obsolete responses and retain private draft";
        await core("release_provider_gate", provider);
        await control("release_local_read", null, local);
        const obsoleteOutcome = await settled;
        obsoleteReads.disconnect = obsoleteOutcome;
        require(
          obsoleteOutcome === "stale_view" || obsoleteOutcome === "cancelled",
        );
        for (const target of ["main", label]) {
          keep(
            await wait(async () => {
              const snapshot = await inspect(target);
              return !snapshot.provider_visible &&
                snapshot.body_hash === null &&
                snapshot.draft_generation === initial.draft_generation &&
                snapshot.saved_draft_hash === initial.saved_draft_hash
                ? snapshot
                : null;
            }),
          );
        }
        stage = "same canonical subject remains partitioned by actor";
        const other = await mount(label, "alternate");
        require(other.account_id !== initial.account_id);
        require(other.subject_id === initial.subject_id);
        require(other.saved_draft_hash !== initial.saved_draft_hash);
      } else if (
        scenario === "crash-before-commit" ||
        scenario === "crash-after-commit"
      ) {
        stage = "prepare genuine crash-held native response";
        require(current.core.committed_phase === null);
        const label = await child();
        if (scenario === "crash-after-commit")
          await control("hold_child_hints");
        const gate = (await core("arm_provider_gate")).gate_id;
        require(gate);
        if (!gate) throw new Error("The native provider gate is missing");
        await mount(label, "primary");
        await providerGateHeld(gate);
        if (scenario === "crash-before-commit") {
          stage = "publish run-bound checkpoint before commit";
          await control("checkpoint_before_commit", null, gate);
        } else {
          stage = "commit real response while targeted hint is held";
          await core("release_provider_gate", gate);
          await committed("one");
          await wait(async () => {
            const latest = await status();
            return latest.core.committed_facet_revision &&
              latest.held_hint_revisions.includes(
                latest.core.committed_facet_revision,
              )
              ? true
              : null;
          });
          stage = "publish exact pull commits before the retained checkpoint";
          const fixture = current.core.actors.find(
            (candidate) => candidate.slot === "primary",
          );
          require(fixture?.account_id === PULL_COMMITS.accountId);
          require(fixture?.subject_id === PULL_COMMITS.subjectId);
          if (!fixture) throw new HarnessScenarioError({ kind: "assertion" });
          const accounts = await collaboration.accounts();
          const account = accounts.accounts.find(
            (candidate) => candidate.id === fixture.account_id,
          );
          require(account?.state === "active");
          if (!account) throw new HarnessScenarioError({ kind: "assertion" });
          const before = await status();
          await collaboration.forAccount(account).hydrateDetail({
            subject_id: fixture.subject_id,
            facet: "commits",
          });
          const snapshot = await wait(async () => {
            const candidate = await collaboration
              .forAccount(account)
              .pullCommits({
                subject_id: fixture.subject_id,
                cursor: null,
                limit: 50,
              });
            return candidate.completeness.state === "complete"
              ? candidate
              : null;
          });
          const after = await status();
          pullCommits = pullCommitEvidence(
            account.id,
            snapshot,
            {
              provider: before.core.provider_call_count,
              vault: before.core.vault_load_count,
            },
            {
              provider: after.core.provider_call_count,
              vault: after.core.vault_load_count,
            },
            false,
          );
          require(
            pullCommits.context.metadata_facet_revision ===
              after.core.committed_facet_revision,
          );
          stage = "persist local inbox intents before the retained checkpoint";
          const inboxBefore = await status();
          const accountInbox = collaboration.forAccount(account);
          const initialInbox = await accountInbox.inbox({
            remote_state: null,
            local_state: "all",
            search: null,
            cursor: null,
            limit: 100,
          });
          const initialById = new Map(
            initialInbox.entries.map(
              (entry) => [entry.item.id, entry] as const,
            ),
          );
          const done = initialById.get(LOCAL_INBOX.doneId);
          const snoozed = initialById.get(LOCAL_INBOX.snoozedId);
          const bookmarked = initialById.get(LOCAL_INBOX.bookmarkedId);
          require(
            done?.local.generation === "0" &&
              snoozed?.local.generation === "0" &&
              bookmarked?.local.generation === "0",
          );
          if (!done || !snoozed || !bookmarked)
            throw new HarnessScenarioError({ kind: "assertion" });
          await accountInbox.setLocalInboxState({
            notification_id: done.item.id,
            expected_activity_updated_at: done.item.updated_at,
            mutation: "disposition",
            disposition: "done",
            bookmarked: null,
            snoozed_until: null,
            expected_generation: done.local.generation,
          });
          await accountInbox.setLocalInboxState({
            notification_id: snoozed.item.id,
            expected_activity_updated_at: snoozed.item.updated_at,
            mutation: "disposition",
            disposition: "inbox",
            bookmarked: null,
            snoozed_until: new Date(
              Date.now() + 7 * 24 * 60 * 60 * 1_000,
            ).toISOString(),
            expected_generation: snoozed.local.generation,
          });
          await accountInbox.setLocalInboxState({
            notification_id: bookmarked.item.id,
            expected_activity_updated_at: bookmarked.item.updated_at,
            mutation: "bookmark",
            disposition: null,
            bookmarked: true,
            snoozed_until: null,
            expected_generation: bookmarked.local.generation,
          });
          const persistedInbox = await accountInbox.inbox({
            remote_state: null,
            local_state: "all",
            search: null,
            cursor: null,
            limit: 100,
          });
          const inboxAfter = await status();
          localInbox = localInboxEvidence(
            persistedInbox,
            {
              provider: inboxBefore.core.provider_call_count,
              vault: inboxBefore.core.vault_load_count,
            },
            {
              provider: inboxAfter.core.provider_call_count,
              vault: inboxAfter.core.vault_load_count,
            },
            "write",
          );
          await control("checkpoint_committed_before_hint");
        }
        require(current.checkpoint);
        checkpoint = true;
      } else {
        stage = "read retained local state before acquiring fresh interest";
        require(
          current.checkpoint && current.checkpoint.session_id !== sessionId,
        );
        require(current.core.calls.length === 0);
        require(current.core.provider_call_count === "0");
        require(current.core.vault_load_count === "0");
        require(current.core.demand_lease_count === 0);
        require(current.core.durable_detail_requests === 0);
        const cacheOnlyBefore = {
          provider: current.core.provider_call_count,
          vault: current.core.vault_load_count,
        };
        const accounts = await collaboration.accounts();
        let retainedInbox: InboxPage | null = null;
        for (const fixture of current.core.actors) {
          const account = accounts.accounts.find(
            (candidate) => candidate.id === fixture.account_id,
          );
          require(account?.state === "active");
          if (!account) throw new Error("The retained actor is missing");
          const draft = await collaboration
            .forAccount(account)
            .draft(fixture.subject_id);
          require(draft?.generation === "1");
          const body = await collaboration.forAccount(account).detail({
            subject_id: fixture.subject_id,
            facet: "body",
            cursor: null,
            limit: 50,
          });
          if (fixture.slot === "primary") {
            if (current.checkpoint?.kind === "committed_before_hint") {
              require(current.core.committed_phase === "one");
              require(body.body.text === BODY.one);
              require(
                body.evidence.facet_revision ===
                  current.checkpoint.committed_facet_revision,
              );
              const snapshot = await collaboration
                .forAccount(account)
                .pullCommits({
                  subject_id: fixture.subject_id,
                  cursor: null,
                  limit: 50,
                });
              const after = await status();
              pullCommits = pullCommitEvidence(
                account.id,
                snapshot,
                cacheOnlyBefore,
                {
                  provider: after.core.provider_call_count,
                  vault: after.core.vault_load_count,
                },
                true,
              );
              require(
                pullCommits.context.metadata_facet_revision ===
                  current.checkpoint.committed_facet_revision,
              );
              retainedInbox = await collaboration.forAccount(account).inbox({
                remote_state: null,
                local_state: "all",
                search: null,
                cursor: null,
                limit: 100,
              });
            } else {
              require(current.checkpoint?.kind === "before_commit");
              require(current.core.committed_phase === null);
              require(body.body.state !== "known");
            }
          }
        }
        const cacheOnlyAfter = await status();
        require(cacheOnlyAfter.core.provider_call_count === "0");
        if (current.checkpoint?.kind === "committed_before_hint") {
          require(retainedInbox);
          if (!retainedInbox)
            throw new HarnessScenarioError({ kind: "assertion" });
          localInbox = localInboxEvidence(
            retainedInbox,
            cacheOnlyBefore,
            {
              provider: cacheOnlyAfter.core.provider_call_count,
              vault: cacheOnlyAfter.core.vault_load_count,
            },
            "restart_read",
          );
        }
        stage = "fresh real interest alone acquires a new Body";
        await mount("main", "primary");
        await rendered("main", "one");
      }
    } catch (error) {
      failed = true;
      failure = classifyHarnessFailure(error);
      failureContext = await captureFailureContext();
    } finally {
      if (manualLease) {
        try {
          // Always release the exact native-issued test handle, including a
          // failed peer check. This is separate from normal SDK interest.
          await collaborationReleaseDemand({
            request: { lease_id: manualLease.lease_id },
          });
          if (authority) authority.main_released = true;
          await wait(
            async () =>
              (await status()).core.demand_lease_count === manualLeaseBaseline
                ? true
                : null,
            true,
          );
          if (authority) authority.lease_count_restored = true;
        } catch (error) {
          cleanupFailure = classifyHarnessFailure(error);
          cleanupStage = "release_native_lease";
          if (!failed) {
            stage = "release native-issued authority lease";
            failure = cleanupFailure;
            failureContext = await captureFailureContext();
          }
          failed = true;
        }
      }
      // The dedicated performance driver samples the live process tree after
      // this receipt, so its two real views remain retained until the owned
      // WDIO launcher tears down the process. No later scenario shares it.
      if (!checkpoint && !performanceScenario) {
        try {
          if (normalHostMounted) {
            await router.navigate({
              to: "/app/git",
              search: { embedded: 1 },
              replace: true,
            });
            await wait(
              async () =>
                (await Webview.getAll()).every(
                  (candidate) => !normalLabels.has(candidate.label),
                )
                  ? true
                  : null,
              true,
            );
          }
          normalLabels.clear();
          for (const tabId of normalTabIds)
            useAppStore.getState().closeTab(tabId);
          await status();
          if (current.child_label)
            await request(current.child_label, { kind: "detach" }).catch(
              () => undefined,
            );
          await request("main", { kind: "detach" }).catch(() => undefined);
          await control("cancel_local_reads");
          await core("cancel_gates");
          await control("resume_hints");
          if (current.child_label) await control("close_concurrent_child");
        } catch (error) {
          cleanupFailure ??= classifyHarnessFailure(error);
          cleanupStage ??= "restore_native_scenario";
          if (!failed) {
            stage = "native scenario cleanup";
            failure = cleanupFailure;
            failureContext = await captureFailureContext();
          }
          failed = true;
        }
      }
      running = false;
    }
    return {
      scenario,
      outcome: failed ? "failed" : checkpoint ? "checkpoint" : "passed",
      stage,
      status: await status().catch(() => null),
      observations,
      authority,
      pull_commits: pullCommits,
      local_inbox: localInbox,
      performance: performanceEvidence,
      obsolete_reads: obsoleteReads,
      failure,
      cleanup_failure: cleanupFailure,
      cleanup_stage: cleanupStage,
      failure_context: failureContext,
    } satisfies HarnessScenarioResult;
  }

  const api = { runScenario };
  window.__GITRU_COLLABORATION_HARNESS__ = api;
  function stop() {
    if (!alive) return;
    alive = false;
    requester.stop();
    if (window.__GITRU_COLLABORATION_HARNESS__ === api)
      delete window.__GITRU_COLLABORATION_HARNESS__;
  }
  if (import.meta.hot) import.meta.hot.dispose(stop);
  return { runScenario, stop };
}

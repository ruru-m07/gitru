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
} from "@gitru/commands";
import { getCurrentWebview, Webview } from "@tauri-apps/api/webview";
import {
  createHarnessRequester,
  HARNESS_DRAFT_EDITS,
  HARNESS_REQUEST_EVENT,
  HARNESS_RESULT_EVENT,
  type HarnessAction,
  type HarnessAuthorityEvidence,
  type HarnessObsoleteReads,
  HarnessPeerLeaseSchema,
  type HarnessProbeSnapshot,
  type HarnessScenario,
  type HarnessScenarioResult,
  HarnessScenarioSchema,
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

  async function activate(label: string) {
    // These are production native owner receipts. No generation/visibility
    // proof is manufactured by the renderer. Native creation has shown the
    // secondary window before the executor can reach this operation.
    const owner = await collaborationInspectDemandOwner({ ownerLabel: label });
    return collaborationSetDemandOwnerActivity({
      ownerLabel: label,
      expectedGeneration: owner.generation,
      active: true,
    });
  }

  async function runScenario(input: HarnessScenario) {
    const scenario = HarnessScenarioSchema.parse(input);
    if (!alive || running)
      throw new Error("The retained scenario executor is unavailable");
    running = true;
    const deadline = Date.now() + SCENARIO_TIMEOUT_MS;
    const observations: HarnessProbeSnapshot[] = [];
    const obsoleteReads: HarnessObsoleteReads = {
      retention_reset: null,
      disconnect: null,
    };
    let authority: HarnessAuthorityEvidence | null = null;
    let manualLease: DemandLeaseReceipt | null = null;
    let manualLeaseBaseline = 0;
    let stage = "read native fixture";
    let checkpoint = false;
    let failed = false;
    const normalTabIds = new Set<string>();
    let normalHostMounted = false;

    function require(condition: unknown) {
      if (!condition) throw new Error("The finite scenario assertion failed");
    }
    function keep(snapshot: HarnessProbeSnapshot) {
      if (observations.length < 32) observations.push(snapshot);
      return snapshot;
    }
    async function wait<T>(read: () => Promise<T | null>, cleanup = false) {
      const end = cleanup
        ? Date.now() + STEP_TIMEOUT_MS
        : Math.min(Date.now() + STEP_TIMEOUT_MS, deadline);
      while (alive && Date.now() < end) {
        const value = await read();
        if (value !== null) return value;
        await new Promise((resolve) => window.setTimeout(resolve, 100));
      }
      throw new Error("The fixed retained state did not settle");
    }
    async function inspect(label: string) {
      return requireSnapshot(label, { kind: "inspect" });
    }
    async function requestWhenBound(label: string, action: BoundAction) {
      return wait(async () => {
        const receipt = await request(label, action);
        // A phase can publish its manifest before React commits the matching
        // real account binding. NotReady precedes a DOM mutation/local IPC,
        // so retry only that outcome. Accepted edits/saves run once; timeouts
        // and failed/stale/denied receipts never count as a successful action.
        return receipt.outcome === "not_ready" ? null : receipt;
      });
    }
    async function requireSnapshot(label: string, action: HarnessAction) {
      const receipt = requiresHarnessBinding(action)
        ? await requestWhenBound(label, action)
        : await request(label, action);
      if (receipt.outcome !== "accepted" || !receipt.snapshot)
        throw new Error("The fixed document action was rejected");
      return receipt.snapshot;
    }
    async function mount(label: string, actor: "primary" | "alternate") {
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
      if (scenario !== "restart" && !current.core.prepared)
        await core("prepare_primary");
      require(current.core.prepared);
      await collaboration.wake();
      await activate("main");

      if (scenario === "concurrent-demand") {
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
        stage = "deliver latest then older native hints";
        await control("deliver_child_hints_reverse");
        const final = await rendered(label, "two", finalRevision);
        require(final.facet_revision === finalRevision);
        require(final.editor_hash === dirty);
        await control("resume_hints");
        stage = "recover dropped hints after an explicit public SDK wake";
        await control("drop_child_hints");
        const beforeDropped = await inspect(label);
        await refresh("phase_not_modified");
        const droppedFacet = current.core.committed_facet_revision;
        require(droppedFacet !== beforeDropped.facet_revision);
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
        stage =
          "apply real retention ResetRequired without discarding authored text";
        const reset = await inspect(label);
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
        await core("fill_retention");
        await requireSnapshot(label, { kind: "wake" });
        const after = keep(
          await wait(async () => {
            const snapshot = await inspect(label);
            return snapshot.catchup.resets > reset.catchup.resets &&
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
        stage = "close concurrent fixture before ordinary native tab host";
        if (current.child_label) await control("close_concurrent_child");
        await requireSnapshot("main", { kind: "detach" });
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
        useAppStore.getState().activateTab(first.id);
        await router.navigate({ to: "/app", search: {}, replace: true });
        normalHostMounted = true;
        async function normalLabel(tabId: string) {
          const expected = sanitizeTabWebviewLabel(tabId);
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
          await wait(async () => {
            const receipt = await request(issued, { kind: "inspect" }).catch(
              () => null,
            );
            return receipt?.outcome === "accepted" ? true : null;
          });
          return issued;
        }
        const firstLabel = await normalLabel(first.id);
        const secondLabel = await normalLabel(second.id);
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
        close?.click();
        await wait(async () =>
          (await collaborationInspectDemandOwner({ ownerLabel: secondLabel }))
            .active
            ? true
            : null,
        );
        stage = "ordinary host disposal recreates the same native label";
        const prior = await collaborationInspectDemandOwner({
          ownerLabel: firstLabel,
        });
        await router.navigate({
          to: "/app/git",
          search: { embedded: 1 },
          replace: true,
        });
        normalHostMounted = false;
        await wait(async () =>
          (await Webview.getAll()).every(
            (candidate) => !normalLabels.has(candidate.label),
          )
            ? true
            : null,
        );
        normalLabels.clear();
        useAppStore.getState().activateTab(first.id);
        await router.navigate({ to: "/app", search: {}, replace: true });
        normalHostMounted = true;
        const replacement = await normalLabel(first.id);
        require(replacement === firstLabel);
        await wait(async () =>
          (await collaborationInspectDemandOwner({ ownerLabel: replacement }))
            .active
            ? true
            : null,
        );
        const owner = await collaborationInspectDemandOwner({
          ownerLabel: replacement,
        });
        require(owner.generation !== prior.generation);
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
        stage = "prepare one actual main lease for the manifest Body";
        await mount("main", "primary");
        await wait(async () => {
          const snapshot = await inspect("main");
          return snapshot.body_value_state === "known" &&
            !snapshot.body.fetching &&
            snapshot.provider_visible
            ? true
            : null;
        });
        const fixture = current.core.actors.find(
          (actor) => actor.slot === "primary",
        );
        require(fixture);
        if (!fixture) throw new Error("Native primary actor is missing");
        await requireSnapshot("main", { kind: "detach" });
        await wait(async () =>
          (await status()).core.demand_lease_count === 0 ? true : null,
        );
        manualLeaseBaseline = current.core.demand_lease_count;
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
        stage = "capture old provider and authorized local return";
        await core("phase_two");
        const provider = (await core("arm_provider_gate")).gate_id;
        require(provider);
        if (!provider) throw new Error("The native provider gate is missing");
        await core("advance_refresh");
        await providerGateHeld(provider);
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
        require(current.core.demand_lease_count === 0);
        require(current.core.durable_detail_requests === 0);
        const accounts = await collaboration.accounts();
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
            } else {
              require(current.checkpoint?.kind === "before_commit");
              require(current.core.committed_phase === null);
              require(body.body.state !== "known");
            }
          }
        }
        require((await status()).core.provider_call_count === "0");
        stage = "fresh real interest alone acquires a new Body";
        await mount("main", "primary");
        await rendered("main", "one");
      }
    } catch {
      failed = true;
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
        } catch {
          stage = "release native-issued authority lease";
          failed = true;
        }
      }
      if (!checkpoint) {
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
        } catch {
          stage = failed ? `${stage}; cleanup` : "native scenario cleanup";
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
      obsolete_reads: obsoleteReads,
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

/** Imported/activated only by the separately compiled retained native harness. */
import {
  collaboration,
  StaleAuthorizationError,
} from "@gitru/collaboration-client";
import {
  detailQueryOptions,
  itemQueryOptions,
  useCollaborationAccounts,
} from "@gitru/collaboration-client/react";
import type { HarnessViewManifest, RemoteAccount } from "@gitru/commands";
import {
  collaborationConnectGithub,
  collaborationDemandActivity,
  collaborationDisconnect,
  collaborationDisposeDemandOwner,
  collaborationHarnessControl,
  collaborationInspectDemandOwner,
  collaborationReleaseDemand,
  collaborationRenewDemand,
  collaborationSetDemandOwnerActivity,
} from "@gitru/commands";
import { isCancelledError, QueryClientProvider } from "@tanstack/react-query";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { StrictMode, useLayoutEffect, useSyncExternalStore } from "react";
import { createRoot } from "react-dom/client";
import {
  HARNESS_DRAFT_EDITS,
  HARNESS_MAX_PENDING,
  HARNESS_REQUEST_EVENT,
  HARNESS_RESULT_EVENT,
  type HarnessAction,
  type HarnessActor,
  type HarnessAuthorityChecks,
  type HarnessRequest,
  HarnessRequestSchema,
  type HarnessResult,
  HarnessResultSchema,
  matchesHarnessPeerLease,
} from "../../e2e/protocol/collaboration-harness";
import { SavedItemDetail } from "../features/collaboration/saved-item-detail";
import { queryClient } from "../state/core/state-manager";
import {
  installHarnessCatchupObservation,
  observeHarnessDocument,
} from "./e2e-collaboration-harness-observation";

function outcome(error: unknown): HarnessResult["outcome"] {
  if (error instanceof StaleAuthorizationError) return "stale_view";
  if (
    isCancelledError(error) ||
    (error instanceof DOMException && error.name === "AbortError")
  )
    return "cancelled";
  if (error && typeof error === "object" && "code" in error) {
    if (
      ["not_ready", "stale_view", "permission_denied", "busy"].includes(
        String(error.code),
      )
    )
      return error.code as HarnessResult["outcome"];
  }
  return "failed";
}

/** Root injects the generated own-view getter, never a fabricated manifest. */
export async function installCollaborationHarnessProbe(
  readManifest: () => Promise<HarnessViewManifest>,
) {
  const view = getCurrentWebview();
  const documentNonce = crypto.randomUUID();
  let manifest = await readManifest();
  if (manifest.webview_label !== view.label)
    throw new Error("Harness manifest did not bind the actual native view");
  const runNonce = manifest.run_nonce;
  const sessionId = manifest.session_id;
  const catchup = installHarnessCatchupObservation();
  let alive = true;
  let selection: { manifest: HarnessViewManifest; actor: HarnessActor | null } =
    {
      manifest,
      actor: null,
    };
  let binding: {
    actor: HarnessActor;
    scenarioGeneration: string;
    account: RemoteAccount;
    subjectId: string;
  } | null = null;
  const listeners = new Set<() => void>();
  const inFlight = new Set<string>();
  const container = document.createElement("section");
  container.setAttribute("aria-label", "Retained collaboration fixture");
  container.className =
    "fixed inset-4 z-70 overflow-y-auto rounded-lg border bg-background shadow-xl";
  container.hidden = true;
  document.body.append(container);
  const root = createRoot(container);

  function notify() {
    for (const listener of listeners) listener();
  }
  function choose(actor: HarnessActor | null) {
    selection = { manifest, actor };
    container.hidden = actor === null;
    notify();
  }
  function HarnessPanel() {
    const current = useSyncExternalStore(
      (listener) => {
        listeners.add(listener);
        return () => listeners.delete(listener);
      },
      () => selection,
    );
    const accounts = useCollaborationAccounts();
    const subject = current.manifest.actors.find(
      (candidate) => candidate.slot === current.actor,
    );
    const account = accounts.data?.accounts.find(
      (candidate) => candidate.id === subject?.account_id,
    );
    useLayoutEffect(() => {
      binding =
        account && subject && current.actor
          ? {
              actor: current.actor,
              scenarioGeneration: current.manifest.scenario_generation,
              account,
              subjectId: subject.subject_id,
            }
          : null;
      return () => {
        binding = null;
      };
    }, [account, subject, current.actor, current.manifest.scenario_generation]);
    return account && subject ? (
      <SavedItemDetail
        key={`${account.id}:${account.actor_id}:${subject.subject_id}`}
        account={account}
        itemId={subject.subject_id}
        kind="pull_request"
        instanceId={subject.instance_id}
        providerEnabled={
          account.state === "active" &&
          account.authorization_epoch === subject.authorization_epoch
        }
      />
    ) : (
      <p role="status" className="p-5 text-sm">
        {current.actor
          ? "Reading the saved fixture account…"
          : "Fixture detached"}
      </p>
    );
  }
  root.render(
    <StrictMode>
      <QueryClientProvider client={queryClient}>
        <HarnessPanel />
      </QueryClientProvider>
    </StrictMode>,
  );

  const inspect = () =>
    observeHarnessDocument({
      client: queryClient,
      container,
      documentNonce,
      actor: binding?.actor ?? selection.actor,
      account: binding?.account ?? null,
      subjectId: binding?.subjectId ?? null,
      catchup: catchup.snapshot(),
    });

  async function perform(action: HarnessAction) {
    const captured = binding;
    let authority: HarnessAuthorityChecks | undefined;
    if (
      ["edit-draft", "save-draft", "read-item", "read-body"].includes(
        action.kind,
      ) &&
      (!captured ||
        captured.actor !== selection.actor ||
        captured.scenarioGeneration !== manifest.scenario_generation)
    )
      throw { code: "not_ready" };
    if (action.kind === "mount") choose(action.actor);
    else if (action.kind === "detach") choose(null);
    else if (action.kind === "wake") await collaboration.wake();
    else if (action.kind === "check-control-authority") {
      // Exercise the same generated native controller from the real child.
      // Native caller verification must reject it before CancelGates runs.
      await collaborationHarnessControl({
        request: {
          run_nonce: manifest.run_nonce,
          expected_generation: manifest.scenario_generation,
          action: "core",
          core_action: "cancel_gates",
          gate_id: null,
        },
      });
    } else if (action.kind === "check-peer-authority") {
      // Scope the only opaque handle packet to the actual synthetic primary
      // actor and the fixed peer main. It supplies no credential or target.
      if (!matchesHarnessPeerLease(action.peer, manifest))
        throw { code: "permission_denied" };
      const peer = action.peer;
      const own = await collaborationDemandActivity({});
      if (!own.active) throw { code: "not_ready" };
      async function attempt(call: () => Promise<unknown>) {
        try {
          await call();
          return "accepted" as const;
        } catch (error) {
          return outcome(error);
        }
      }
      authority = {
        controller: await attempt(() =>
          collaborationHarnessControl({
            request: {
              run_nonce: manifest.run_nonce,
              expected_generation: manifest.scenario_generation,
              action: "core",
              core_action: "cancel_gates",
              gate_id: null,
            },
          }),
        ),
        renew_lease: await attempt(() =>
          collaborationRenewDemand({
            request: {
              owner_generation: own.generation,
              leases: [
                {
                  lease_id: peer.lease_id,
                  account_id: peer.account_id,
                  authorization_epoch: peer.authorization_epoch,
                },
              ],
            },
          }),
        ),
        release_lease: await attempt(() =>
          collaborationReleaseDemand({ request: { lease_id: peer.lease_id } }),
        ),
        disconnect: await attempt(() =>
          collaborationDisconnect({ accountId: peer.account_id }),
        ),
        inspect_owner: await attempt(() =>
          collaborationInspectDemandOwner({ ownerLabel: peer.owner_label }),
        ),
        set_owner: await attempt(() =>
          collaborationSetDemandOwnerActivity({
            ownerLabel: peer.owner_label,
            expectedGeneration: peer.owner_generation,
            active: false,
          }),
        ),
        dispose_owner: await attempt(() =>
          collaborationDisposeDemandOwner({
            ownerLabel: peer.owner_label,
            expectedGeneration: peer.owner_generation,
          }),
        ),
        connect_synthetic_pat: await attempt(() =>
          // Native authorizes this actual child before runtime/credential
          // access. The feature runtime has no real provider fallback.
          collaborationConnectGithub({
            token: "ruru103-synthetic-not-a-personal-token",
          }),
        ),
      };
    } else if (action.kind === "edit-draft") {
      const editor = container.querySelector<HTMLTextAreaElement>(
        'textarea[name="private-draft"]',
      );
      if (!editor || editor.disabled) throw { code: "not_ready" };
      const setter = Object.getOwnPropertyDescriptor(
        HTMLTextAreaElement.prototype,
        "value",
      )?.set;
      if (!setter) throw { code: "not_ready" };
      editor.focus();
      setter.call(editor, HARNESS_DRAFT_EDITS[action.variant]);
      editor.dispatchEvent(new Event("input", { bubbles: true }));
    } else if (action.kind === "save-draft") {
      const save = Array.from(
        container.querySelectorAll<HTMLButtonElement>('button[type="submit"]'),
      ).find((button) => button.textContent?.trim() === "Save draft");
      if (!save || save.disabled || !save.form) throw { code: "not_ready" };
      save.form.requestSubmit(save);
    } else if (action.kind === "read-item" || action.kind === "read-body") {
      if (!captured) throw { code: "not_ready" };
      // Force one actual local query; query options retain the real SDK fence.
      // No hydration/demand intent is added by these explicit probe reads.
      if (action.kind === "read-item")
        await queryClient.fetchQuery({
          ...itemQueryOptions(captured.account, captured.subjectId),
          staleTime: 0,
        });
      else
        await queryClient.fetchQuery({
          ...detailQueryOptions(captured.account, {
            subject_id: captured.subjectId,
            facet: "body",
            cursor: null,
            limit: 50,
          }),
          staleTime: 0,
        });
    }
    // React may commit after the action receipt. The driver waits on actual
    // read-only inspect receipts rather than forcing fake cache/UI state.
    return { snapshot: await inspect(), authority };
  }

  async function execute(request: HarnessRequest): Promise<HarnessResult> {
    request = HarnessRequestSchema.parse(request);
    const envelope = {
      run_nonce: request.run_nonce,
      scenario_generation: request.scenario_generation,
      request_id: request.request_id,
      label: view.label,
    };
    if (request.label !== view.label || request.run_nonce !== runNonce)
      return { ...envelope, outcome: "stale_view", snapshot: null };
    if (!alive || inFlight.size >= HARNESS_MAX_PENDING)
      return { ...envelope, outcome: "busy", snapshot: null };
    if (inFlight.has(request.request_id))
      return { ...envelope, outcome: "busy", snapshot: null };
    inFlight.add(request.request_id);
    try {
      const fresh = await readManifest();
      if (
        !alive ||
        fresh.webview_label !== view.label ||
        fresh.session_id !== sessionId ||
        fresh.run_nonce !== runNonce ||
        fresh.run_nonce !== request.run_nonce ||
        fresh.scenario_generation !== request.scenario_generation
      )
        return { ...envelope, outcome: "stale_view", snapshot: null };
      if (JSON.stringify(manifest) !== JSON.stringify(fresh)) {
        manifest = fresh;
        // Finite provider phases advance the native scenario fence, but do
        // not replace the authored editor for the same account and subject.
        choose(selection.actor);
      }
      const { snapshot, authority } = await perform(request.action);
      const after = await readManifest();
      if (
        !alive ||
        after.webview_label !== view.label ||
        after.session_id !== sessionId ||
        after.run_nonce !== runNonce ||
        after.scenario_generation !== request.scenario_generation
      )
        return { ...envelope, outcome: "stale_view", snapshot: null };
      if (
        snapshot.account_id &&
        !after.actors.some(
          (actor) =>
            actor.account_id === snapshot.account_id &&
            actor.authorization_epoch === snapshot.authorization_epoch,
        )
      )
        return { ...envelope, outcome: "stale_view", snapshot: null };
      return HarnessResultSchema.parse({
        ...envelope,
        outcome: "accepted",
        snapshot,
        ...(authority ? { authority } : {}),
      });
    } catch (error) {
      return { ...envelope, outcome: outcome(error), snapshot: null };
    } finally {
      inFlight.delete(request.request_id);
    }
  }

  let stopListener: (() => void) | undefined;
  const stop = () => {
    if (!alive) return;
    alive = false;
    stopListener?.();
    root.unmount();
    catchup.stop();
    listeners.clear();
    container.remove();
  };
  try {
    stopListener = await view.listen<unknown>(
      HARNESS_REQUEST_EVENT,
      ({ payload }) => {
        const parsed = HarnessRequestSchema.safeParse(payload);
        if (!parsed.success || parsed.data.label !== view.label || !alive)
          return;
        void execute(parsed.data).then((result) => {
          if (alive)
            void view
              .emitTo(
                { kind: "Webview", label: "main" },
                HARNESS_RESULT_EVENT,
                result,
              )
              .catch(() => undefined);
        });
      },
    );
  } catch (error) {
    stop();
    throw error;
  }
  if (import.meta.hot) import.meta.hot.dispose(stop);
  return { execute, inspect, stop };
}

/** Bounded synthetic diagnostics; this module is never imported by production. */
import { collaboration } from "@gitru/collaboration-client";
import {
  detailQueryOptions,
  draftQueryOptions,
  itemQueryOptions,
} from "@gitru/collaboration-client/react";
import type {
  DetailSnapshot,
  ItemSnapshot,
  LocalDraft,
  RemoteAccount,
} from "@gitru/commands";
import type { QueryClient, QueryState } from "@tanstack/react-query";
import {
  type HarnessActor,
  type HarnessCatchupObservation,
  type HarnessProbeSnapshot,
  HarnessProbeSnapshotSchema,
} from "../../e2e/protocol/collaboration-harness";

function emptyCatchup(): HarnessCatchupObservation {
  return {
    reads: 0,
    pages_with_more: 0,
    resets: 0,
    last_request: null,
    last_receipt: null,
  };
}

/** Decorates the actual generated transport without changing input or output. */
export function installHarnessCatchupObservation() {
  const transport = collaboration.transport;
  const original = transport.changesSince;
  let alive = true;
  let current = emptyCatchup();
  const observed: typeof original = async (revision) => {
    const receipt = await original.call(transport, revision);
    if (alive) {
      current = {
        reads: Math.min(current.reads + 1, 10_000),
        pages_with_more: Math.min(
          current.pages_with_more + Number(receipt.has_more),
          10_000,
        ),
        resets: Math.min(current.resets + Number(receipt.reset_required), 1000),
        last_request: revision,
        last_receipt: receipt.revision,
      };
    }
    return receipt;
  };
  transport.changesSince = observed;
  return {
    snapshot: () => ({ ...current }),
    stop() {
      alive = false;
      if (transport.changesSince === observed)
        transport.changesSince = original;
    },
  };
}

function queryReceipt(
  state: QueryState<ItemSnapshot | DetailSnapshot> | undefined,
  epoch: string | undefined,
): HarnessProbeSnapshot["item"] {
  return {
    status: state?.status ?? "absent",
    fetching: state?.fetchStatus === "fetching",
    revision: state?.data?.revision ?? null,
    authorization_epoch: state?.data ? (epoch ?? null) : null,
  };
}

export async function syntheticFingerprint(text: string | null) {
  if (text === null) return null;
  const encoded = new TextEncoder().encode(text);
  if (encoded.byteLength > 1_048_576)
    throw new Error("Synthetic diagnostic exceeds the bounded text limit");
  const digest = await crypto.subtle.digest("SHA-256", encoded);
  return Array.from(new Uint8Array(digest), (byte) =>
    byte.toString(16).padStart(2, "0"),
  ).join("");
}

/** Reads existing typed local caches and rendered UI, never fetches a snapshot. */
export async function observeHarnessDocument({
  client,
  container,
  documentNonce,
  actor,
  account,
  subjectId,
  catchup = emptyCatchup(),
}: {
  client: QueryClient;
  container: HTMLElement;
  documentNonce: string;
  actor: HarnessActor | null;
  account: RemoteAccount | null;
  subjectId: string | null;
  catchup?: HarnessCatchupObservation;
}): Promise<HarnessProbeSnapshot> {
  const item =
    account && subjectId
      ? client.getQueryState<ItemSnapshot>(
          itemQueryOptions(account, subjectId).queryKey,
        )
      : undefined;
  const body =
    account && subjectId
      ? client.getQueryState<DetailSnapshot>(
          detailQueryOptions(account, {
            subject_id: subjectId,
            facet: "body",
            cursor: null,
            limit: 50,
          }).queryKey,
        )
      : undefined;
  const draft =
    account && subjectId
      ? client.getQueryData<LocalDraft | null>(
          draftQueryOptions(account, subjectId).queryKey,
        )
      : undefined;
  const editor = container.querySelector<HTMLTextAreaElement>(
    'textarea[name="private-draft"]',
  );
  const save = Array.from(
    container.querySelectorAll<HTMLButtonElement>('button[type="submit"]'),
  ).find((button) => button.textContent?.trim() === "Save draft");
  const [bodyHash, metadataHash, savedDraftHash, editorHash] =
    await Promise.all([
      syntheticFingerprint(
        body?.data?.body.state === "known" ? body.data.body.text : null,
      ),
      syntheticFingerprint(
        body?.data?.metadata ? JSON.stringify(body.data.metadata.values) : null,
      ),
      syntheticFingerprint(draft?.body ?? null),
      syntheticFingerprint(editor?.value ?? null),
    ]);
  return HarnessProbeSnapshotSchema.parse({
    document_nonce: documentNonce,
    actor,
    account_id: account?.id ?? null,
    actor_id: account?.actor_id ?? null,
    subject_id: subjectId,
    authorization_epoch: account?.authorization_epoch ?? null,
    mounted: Boolean(
      container.querySelector('[aria-label="Saved item detail"]'),
    ),
    provider_visible: Boolean(
      container.querySelector('[aria-label="Selected resource metadata"]'),
    ),
    item: queryReceipt(item, account?.authorization_epoch),
    body: queryReceipt(body, account?.authorization_epoch),
    facet_revision: body?.data?.evidence.facet_revision ?? null,
    body_value_state: body?.data?.body.state ?? null,
    body_hash: bodyHash,
    metadata_hash: metadataHash,
    draft_generation: draft?.generation ?? null,
    saved_draft_hash: savedDraftHash,
    editor_hash: editorHash,
    save_enabled: Boolean(save && !save.disabled),
    conflict_visible: Boolean(
      container.textContent?.includes("This draft changed in another tab."),
    ),
    catchup,
    collaboration_query_count: client
      .getQueryCache()
      .findAll({ predicate: (query) => query.queryKey[0] === "collaboration" })
      .length,
  });
}

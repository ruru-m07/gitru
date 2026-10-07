import {
  collaborationErrorMessage,
  type RemoteAccount,
  type RemoteItemKind,
} from "@gitru/collaboration-client";
import {
  useCollaborationDetail,
  useCollaborationItem,
  useContextualCapabilities,
  useVisibleDemand,
} from "@gitru/collaboration-client/react";
import { Button } from "@gitru/ui/components/button";
import { ArrowLeft } from "lucide-react";
import { CapabilityBoundary, ReadOnlyCapability } from "./capability-boundary";
import {
  canMaintainDemand,
  canReadSaved,
  facetPolicy,
  feedFacet,
  resourceCapabilityTarget,
} from "./capability-policy";
import { SavedDraftEditor } from "./private-draft";
import { PullRequestCheckoutButton } from "./pull-checkout-dialog";
import { ResourceCapabilityPanels } from "./resource-capability-panels";
import { SelectedResourceHeader } from "./resource-metadata";

export function SavedItemDetail({
  account,
  itemId,
  close,
  kind,
  instanceId,
  providerEnabled = true,
}: {
  account: RemoteAccount;
  itemId: string;
  kind: RemoteItemKind;
  instanceId: string;
  close?: () => void;
  providerEnabled?: boolean;
}) {
  const context = useContextualCapabilities(
    account,
    resourceCapabilityTarget(instanceId, itemId, kind),
  );
  const policy = facetPolicy(context.data, feedFacet[kind]);
  const query = useCollaborationItem(
    account,
    itemId,
    providerEnabled && canReadSaved(policy),
  );
  const item = query.data?.item;
  const bodyPolicy = facetPolicy(
    context.data,
    kind === "pull_request" ? "pull_details" : "issue_details",
  );
  const body = useCollaborationDetail(
    account,
    { subject_id: itemId, facet: "body", cursor: null, limit: 50 },
    providerEnabled &&
      kind !== "notification" &&
      canReadSaved(policy) &&
      canReadSaved(bodyPolicy),
  );
  const bodyData =
    providerEnabled &&
    canReadSaved(bodyPolicy) &&
    body.data?.evidence.availability !== "unavailable"
      ? body.data
      : undefined;
  const foregroundDemandError = useVisibleDemand({
    account,
    target: {
      kind: "detail",
      repository_id: null,
      subject_id: itemId,
      facet: "body",
    },
    enabled:
      account.state === "active" &&
      providerEnabled &&
      kind !== "notification" &&
      !!item &&
      canReadSaved(policy) &&
      canMaintainDemand(bodyPolicy) &&
      body.data?.evidence.availability !== "unavailable",
  });
  const bodyAccessDenied =
    kind !== "notification" &&
    (bodyPolicy?.saved_read.state === "unavailable" ||
      body.data?.evidence.availability === "unavailable");
  return (
    <article
      className="min-w-0 overflow-y-auto border-l p-5"
      aria-label="Saved item detail"
    >
      {close ? (
        <Button variant="ghost" size="sm" onClick={close} className="mb-4">
          <ArrowLeft aria-hidden="true" />
          Back to list
        </Button>
      ) : null}
      {!providerEnabled ? (
        <p className="text-sm text-muted-foreground">
          Provider content is unavailable in the current notification view. Your
          private draft remains on this device.
        </p>
      ) : !canReadSaved(policy) ? (
        <CapabilityBoundary
          policy={policy}
          pending={context.isPending}
          error={
            context.isError
              ? collaborationErrorMessage(context.error)
              : undefined
          }
        >
          {null}
        </CapabilityBoundary>
      ) : bodyAccessDenied ? (
        <CapabilityBoundary policy={bodyPolicy}>{null}</CapabilityBoundary>
      ) : query.isPending ? (
        <p role="status" className="text-sm text-muted-foreground">
          Loading saved detail…
        </p>
      ) : query.isError ? (
        <p role="alert" className="text-sm text-destructive-foreground">
          {collaborationErrorMessage(query.error)}
        </p>
      ) : item ? (
        <>
          <SelectedResourceHeader
            item={item}
            metadata={bodyData?.metadata ?? null}
          />
          <PullRequestCheckoutButton
            account={account}
            item={item}
            instanceId={instanceId}
            metadata={bodyData?.metadata ?? null}
          />
          {kind === "notification" || !canReadSaved(bodyPolicy) ? (
            <div className="mt-4 whitespace-pre-wrap break-words text-sm leading-relaxed">
              {item.body ??
                (item.body_omitted
                  ? "This description is not saved on this device. Open the provider to read it."
                  : "This saved item has no description.")}
            </div>
          ) : null}
          <ReadOnlyCapability policy={policy} />
        </>
      ) : (
        <p className="text-sm text-muted-foreground">
          This item is no longer available in your saved view.
        </p>
      )}
      {foregroundDemandError ? (
        <p role="alert" className="mt-3 text-xs text-destructive-foreground">
          {collaborationErrorMessage(foregroundDemandError)}
        </p>
      ) : null}
      <ResourceCapabilityPanels
        account={account}
        subjectId={itemId}
        kind={kind}
        snapshot={providerEnabled ? context.data : undefined}
        instanceId={instanceId}
        repositoryId={item?.repository_id ?? null}
      />
      <SavedDraftEditor account={account} subjectId={itemId} />
    </article>
  );
}

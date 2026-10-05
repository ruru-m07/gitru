import {
  collaborationAccounts,
  collaborationAcquireDemand,
  collaborationCapabilities,
  collaborationChangesSince,
  collaborationConfirmLocalLink,
  collaborationConnectBitbucketCloud,
  collaborationConnectGithub,
  collaborationConnectGithubCli,
  collaborationConnectGitlab,
  collaborationContextualCapabilities,
  collaborationDemandActivity,
  collaborationDetail,
  collaborationDisconnect,
  collaborationDiscoverGithubCli,
  collaborationDiscoverNotificationSubject,
  collaborationDraft,
  collaborationHydrateDetail,
  collaborationItem,
  collaborationItems,
  collaborationLocalClones,
  collaborationLocalLinks,
  collaborationNotificationSubject,
  collaborationRefresh,
  collaborationReleaseDemand,
  collaborationRemoveLocalLink,
  collaborationRemoveTransportBinding,
  collaborationRenewDemand,
  collaborationRepositories,
  collaborationResolveResource,
  collaborationSaveDraft,
  collaborationSaveTransportBinding,
  collaborationSelectRepository,
  collaborationValidateLocalNavigation,
} from "@gitru/commands";
import type { QueryClient } from "@tanstack/react-query";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { CollaborationClient } from "./client";

export type {
  AcquireDemandRequest,
  CanonicalResource,
  CapabilitySnapshot,
  CapabilityTarget,
  ContextCapabilityAccess,
  ContextCapabilityRequest,
  ContextFacetCapability,
  ContextualCapabilitySnapshot,
  DemandLeaseReceipt,
  DemandOwnerActivity,
  DemandRenewalReceipt,
  DemandTarget,
  DetailActor,
  DetailBranch,
  DetailEntry,
  DetailEvidence,
  DetailFieldValidation,
  DetailLabel,
  DetailMilestone,
  DetailQuery,
  DetailSnapshot,
  DetailValue,
  DiscoverNotificationSubjectRequest,
  GithubCliAccount,
  GithubCliDiscovery,
  HydrateDetailRequest,
  ItemQuery,
  LocalCloneRecord,
  LocalCloneSnapshot,
  LocalLinkCandidate,
  LocalLinkInspection,
  LocalLinkVersion,
  LocalNavigationReceipt,
  LocalNavigationRequest,
  LocalRepositoryLink,
  LocalTransportBinding,
  MetadataFieldEvidence,
  NativeDetailPayload,
  NotificationSubjectQuery,
  NotificationSubjectSnapshot,
  ParticipantUser,
  ParticipantV1,
  ReleaseDemandRequest,
  RemoteAccount,
  RemoteItem,
  RemoteRepository,
  RenewDemandRequest,
  ResourceLocator,
  ResourceMetadataSnapshot,
  ResourceMetadataValues,
  ResourceResolution,
  TaskActor,
  TaskV1,
  TransportBindingRequest,
} from "@gitru/commands";
export { StaleAuthorizationError } from "./authorization-fence";
export * from "./client";
export type { DemandAccount, DemandHandle } from "./demand-coordinator";
export type LocalLinkState =
  import("@gitru/commands").LocalRepositoryLink["state"];
export type RemoteItemKind = import("@gitru/commands").ItemQuery["kind"];
export type DetailFacet = import("@gitru/commands").DetailQuery["facet"];
export type DetailField =
  import("@gitru/commands").DetailEntry["field_mask"][number];
export type CapabilityObservation =
  import("@gitru/commands").ContextFacetCapability["observation"];
export type ContextCapabilityReason = NonNullable<
  import("@gitru/commands").ContextCapabilityAccess["reason"]
>;
export type InboxSemantics =
  import("@gitru/commands").CapabilitySnapshot["inbox_semantics"];
export type ResourceFacet =
  import("@gitru/commands").ContextFacetCapability["facet"];
export type ResourceKind = import("@gitru/commands").ResourceLocator["kind"];

export const collaboration = new CollaborationClient({
  demandActivity: () => collaborationDemandActivity({}),
  acquireDemand: (request) => collaborationAcquireDemand({ request }),
  renewDemand: (request) => collaborationRenewDemand({ request }),
  releaseDemand: (request) => collaborationReleaseDemand({ request }),
  listenDemandActivity: (onActivity) => {
    const ownerLabel = getCurrentWebview().label;
    return listen<{
      owner_label: string;
      activity: import("@gitru/commands").DemandOwnerActivity;
    }>("collaboration:owner-activity", ({ payload }) => {
      if (payload.owner_label === ownerLabel) onActivity(payload.activity);
    });
  },
  localLinks: (localRepositoryId) =>
    collaborationLocalLinks({ localRepositoryId }),
  confirmLocalLink: (request) => collaborationConfirmLocalLink({ request }),
  removeLocalLink: ({ id, generation }) =>
    collaborationRemoveLocalLink({ id, generation }),
  saveTransportBinding: (request) =>
    collaborationSaveTransportBinding({ request }),
  removeTransportBinding: ({ id, generation }, expectedBindingsGeneration) =>
    collaborationRemoveTransportBinding({
      id,
      generation,
      expectedBindingsGeneration,
    }),
  localClones: (request) => collaborationLocalClones({ request }),
  validateLocalNavigation: (request) =>
    collaborationValidateLocalNavigation({ request }),
  listenLocalChanges: (onWake) => listen("gitru://repository-changed", onWake),
  accounts: () => collaborationAccounts({}),
  connectGithub: (token) => collaborationConnectGithub({ token }),
  connectGitlab: (token) => collaborationConnectGitlab({ token }),
  connectBitbucketCloud: (token) =>
    collaborationConnectBitbucketCloud({ token }),
  discoverGithubCli: () => collaborationDiscoverGithubCli({}),
  connectGithubCli: (candidateId) =>
    collaborationConnectGithubCli({ candidateId }),
  disconnect: (accountId) => collaborationDisconnect({ accountId }),
  repositories: (accountId) => collaborationRepositories({ accountId }),
  selectRepository: (accountId, repositoryId, selected) =>
    collaborationSelectRepository({ accountId, repositoryId, selected }),
  items: (query) => collaborationItems({ query }),
  item: (accountId, itemId) => collaborationItem({ accountId, itemId }),
  refresh: (request) => collaborationRefresh({ request }),
  changesSince: (afterRevision) => collaborationChangesSince({ afterRevision }),
  saveDraft: (draft) => collaborationSaveDraft({ draft }),
  draft: (accountId, subjectId) => collaborationDraft({ accountId, subjectId }),
  capabilities: (accountId) => collaborationCapabilities({ accountId }),
  contextualCapabilities: (request) =>
    collaborationContextualCapabilities({ request }),
  resolveResource: (accountId, locator) =>
    collaborationResolveResource({ accountId, locator }),
  detail: (query) => collaborationDetail({ query }),
  hydrateDetail: (request) => collaborationHydrateDetail({ request }),
  notificationSubject: (query) => collaborationNotificationSubject({ query }),
  discoverNotificationSubject: (request) =>
    collaborationDiscoverNotificationSubject({ request }),
  listen: (onWake) =>
    getCurrentWebview().listen<{ revision: string }>(
      "gitru:collaboration-change",
      onWake,
    ),
});

export function installCollaborationBridge(
  queryClient: QueryClient,
): () => void {
  const stop = collaboration.installBridge(queryClient);
  const wake = () => {
    void collaboration.wake();
  };
  const visible = () => {
    if (document.visibilityState === "visible") wake();
  };
  window.addEventListener("focus", wake);
  window.addEventListener("online", wake);
  document.addEventListener("visibilitychange", visible);
  return () => {
    window.removeEventListener("focus", wake);
    window.removeEventListener("online", wake);
    document.removeEventListener("visibilitychange", visible);
    stop();
  };
}

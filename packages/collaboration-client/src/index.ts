import {
  collaborationAccounts,
  collaborationCapabilities,
  collaborationChangesSince,
  collaborationConnectGithub,
  collaborationConnectGithubCli,
  collaborationContextualCapabilities,
  collaborationDetail,
  collaborationDisconnect,
  collaborationDiscoverGithubCli,
  collaborationDraft,
  collaborationHydrateDetail,
  collaborationItem,
  collaborationItems,
  collaborationRefresh,
  collaborationRepositories,
  collaborationResolveResource,
  collaborationSaveDraft,
  collaborationSelectRepository,
} from "@gitru/commands";
import type { QueryClient } from "@tanstack/react-query";
import { listen } from "@tauri-apps/api/event";
import { CollaborationClient } from "./client";

export type {
  CanonicalResource,
  CapabilitySnapshot,
  CapabilityTarget,
  ContextCapabilityAccess,
  ContextCapabilityRequest,
  ContextFacetCapability,
  ContextualCapabilitySnapshot,
  DetailEntry,
  DetailEvidence,
  DetailQuery,
  DetailSnapshot,
  DetailValue,
  GithubCliAccount,
  GithubCliDiscovery,
  HydrateDetailRequest,
  ItemQuery,
  RemoteAccount,
  RemoteItem,
  RemoteRepository,
  ResourceLocator,
  ResourceResolution,
} from "@gitru/commands";
export { StaleAuthorizationError } from "./authorization-fence";
export * from "./client";
export type RemoteItemKind = import("@gitru/commands").ItemQuery["kind"];
export type DetailFacet = import("@gitru/commands").DetailQuery["facet"];
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
  accounts: () => collaborationAccounts({}),
  connectGithub: (token) => collaborationConnectGithub({ token }),
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
  listen: (onWake) =>
    listen<{ revision: string }>("gitru:collaboration-change", onWake),
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

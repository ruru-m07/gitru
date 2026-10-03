import {
  collaborationAccounts,
  collaborationCapabilities,
  collaborationChangesSince,
  collaborationConnectGithub,
  collaborationConnectGithubCli,
  collaborationDisconnect,
  collaborationDiscoverGithubCli,
  collaborationDraft,
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
  GithubCliAccount,
  GithubCliDiscovery,
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
  resolveResource: (accountId, locator) =>
    collaborationResolveResource({ accountId, locator }),
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

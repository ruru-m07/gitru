import type {
  CapabilityTarget,
  ContextFacetCapability,
  ContextualCapabilitySnapshot,
  InboxSemantics,
  RemoteItemKind,
  ResourceFacet,
  ResourceKind,
} from "@gitru/collaboration-client";

export const accountCapabilityTarget: CapabilityTarget = {
  kind: "account",
  instance_id: null,
  repository_id: null,
  resource_id: null,
  resource_kind: null,
};

export function repositoryCapabilityTarget(
  instanceId: string,
  repositoryId: string,
): CapabilityTarget {
  return {
    kind: "repository",
    instance_id: instanceId,
    repository_id: repositoryId,
    resource_id: null,
    resource_kind: null,
  };
}

export function resourceCapabilityTarget(
  instanceId: string,
  resourceId: string,
  kind: Exclude<ResourceKind, "repository">,
): CapabilityTarget {
  return {
    kind: "resource",
    instance_id: instanceId,
    repository_id: null,
    resource_id: resourceId,
    resource_kind: kind,
  };
}

export const feedFacet: Record<RemoteItemKind, ResourceFacet> = {
  pull_request: "pull_requests",
  issue: "issues",
  notification: "inbox",
};

export function facetPolicy(
  snapshot: ContextualCapabilitySnapshot | undefined,
  facet: ResourceFacet,
): ContextFacetCapability | undefined {
  return snapshot?.facets.find((candidate) => candidate.facet === facet);
}

export function canReadSaved(policy: ContextFacetCapability | undefined) {
  return policy?.saved_read.state === "supported";
}

export function canSynchronize(policy: ContextFacetCapability | undefined) {
  return policy?.synchronize.state === "supported";
}

/** Visible interest survives a temporary network/quota pause; Rust owns due work. */
export function canMaintainDemand(policy: ContextFacetCapability | undefined) {
  return (
    canReadSaved(policy) &&
    (canSynchronize(policy) ||
      (policy?.synchronize.state === "unavailable" &&
        policy.synchronize.reason === "temporarily_unavailable"))
  );
}

/** Keep the handler guarded too: a disabled control is only presentation. */
export async function dispatchCapabilityIntent(
  policy: ContextFacetCapability | undefined,
  intent: "synchronize" | "remote_write" | "recheck_access",
  dispatch: () => Promise<unknown>,
): Promise<boolean> {
  const allowed =
    intent === "recheck_access"
      ? policy?.can_recheck_access === true &&
        policy.saved_read.state !== "unsupported" &&
        policy.synchronize.state === "unavailable" &&
        policy.synchronize.reason === "permission_denied"
      : policy?.[intent].state === "supported";
  if (!allowed) return false;
  await dispatch();
  return true;
}

export function inboxPresentation(semantics: InboxSemantics) {
  switch (semantics) {
    case "native_notifications":
      return {
        title: "Inbox",
        initialState: "unread",
        badgeState: "unread",
        badgeMeaning: "unread notifications",
        filters: [
          { label: "Provider unread", value: "unread" },
          { label: "Provider read", value: "read" },
          { label: "All provider states", value: "all" },
        ],
      };
    case "todos":
      return {
        title: "To-dos",
        initialState: "pending",
        badgeState: "pending",
        badgeMeaning: "pending to-dos",
        filters: [
          { label: "Provider pending", value: "pending" },
          { label: "Provider done", value: "done" },
          { label: "All provider states", value: "all" },
        ],
      };
    case "none":
      return {
        title: "Inbox",
        initialState: null,
        badgeState: null,
        badgeMeaning: "saved inbox items",
        filters: [],
      };
  }
}

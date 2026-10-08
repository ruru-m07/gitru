import type { ContextFacetCapability } from "@gitru/collaboration-client";
import { Badge } from "@gitru/ui/components/badge";
import type { ReactNode } from "react";
import { canReadSaved, canSynchronize } from "./capability-policy";
import { CollaborationStatePanel } from "./state-panel";

export function capabilityExplanation(
  policy: ContextFacetCapability | undefined,
) {
  const access = policy?.saved_read;
  if (!access) {
    return {
      title: "Checking available features",
      description:
        "Gitru is checking the local policy for this account and resource.",
    };
  }
  if (access.state === "unsupported") {
    return {
      title:
        access.reason === "not_applicable"
          ? "Not available for this resource"
          : "Feature not supported",
      description:
        access.reason === "not_applicable"
          ? "This feature does not apply to this resource."
          : access.reason === "provider_semantics"
            ? "This provider uses a different model for this feature."
            : "This feature is not implemented for the connected provider.",
    };
  }
  switch (access.reason) {
    case "authentication_required":
      return {
        title: "Reconnect your account",
        description:
          "Open Accounts to restore access to provider data. Your private drafts remain local.",
      };
    case "missing_scope":
      return {
        title: "Permission required",
        description:
          "This connection does not have the provider access needed for this feature. Open Accounts to update the connection.",
      };
    case "permission_denied":
      return {
        title: "Access denied",
        description:
          "The provider denied access to this resource. Saved provider content is hidden; your private draft remains available.",
      };
    case "repository_not_selected":
      return {
        title: "Choose repositories to sync",
        description:
          "Select a repository to make its saved activity available.",
      };
    case "adapter_unavailable":
      return {
        title: "Provider connection unavailable",
        description:
          "There is no installed adapter for this provider installation.",
      };
    case "temporarily_unavailable":
      return {
        title: "Provider temporarily unavailable",
        description:
          "Try again when the connection or provider limit permits. Saved authorized content stays available.",
      };
    default:
      return {
        title: "Availability not known yet",
        description:
          "Gitru has not observed enough local evidence to enable this feature.",
      };
  }
}

export function CapabilityBoundary({
  policy,
  children,
  pending = false,
  error,
  recheck,
  busy = false,
}: {
  policy: ContextFacetCapability | undefined;
  children: ReactNode;
  pending?: boolean;
  error?: string;
  recheck?: () => void;
  busy?: boolean;
}) {
  if (canReadSaved(policy)) return children;
  const explanation = capabilityExplanation(policy);
  return (
    <CollaborationStatePanel
      title={error ? "Could not check available features" : explanation.title}
      action={
        !pending && policy?.can_recheck_access && recheck
          ? "Recheck access"
          : undefined
      }
      onAction={recheck}
      busy={busy}
    >
      {error ?? explanation.description}
    </CollaborationStatePanel>
  );
}

export function ReadOnlyCapability({
  policy,
}: {
  policy: ContextFacetCapability | undefined;
}) {
  return canReadSaved(policy) && policy?.remote_write.state !== "supported" ? (
    <Badge
      variant="outline"
      size="sm"
      title="Remote changes are not implemented. Private drafts can still be saved locally."
    >
      Read-only
    </Badge>
  ) : null;
}

/** Current eligibility comes from policy; old failure status does not strand retries. */
export function SynchronizationAvailability({
  policy,
}: {
  policy: ContextFacetCapability | undefined;
}) {
  if (!canReadSaved(policy)) return null;
  let description: string | null = null;
  if (policy?.synchronize.reason === "temporarily_unavailable")
    description = policy.sync.next_retry_at
      ? `Sync is paused until ${new Date(policy.sync.next_retry_at).toLocaleString()}. Saved data is available.`
      : "Sync is temporarily unavailable. Saved data is available.";
  else if (
    canSynchronize(policy) &&
    policy &&
    (policy.sync.state === "offline" ||
      policy.sync.state === "rate_limited" ||
      policy.sync.error)
  )
    description =
      "The previous sync did not finish. You can retry now; saved data is available.";
  return description ? (
    <p role="status" className="text-xs text-muted-foreground">
      {description}
    </p>
  ) : null;
}

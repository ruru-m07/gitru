import { Badge } from "@gitru/ui/components/badge";
import { CloudCheck, RefreshCw, WifiOff } from "lucide-react";

export function SyncIndicator({
  state,
  validatedAt,
  partial,
}: {
  state: string;
  validatedAt?: string | null;
  partial?: boolean;
}) {
  const syncing = state === "syncing" || state === "queued";
  const offline = state === "offline";
  const label = syncing
    ? "Syncing"
    : offline
      ? "Offline · saved data"
      : state === "rate_limited" || state === "rate-limited"
        ? "Waiting for provider"
        : state === "auth_required" || state === "auth-required"
          ? "Reconnect account"
          : state === "error"
            ? "Sync needs attention"
            : validatedAt
              ? "Saved on this device"
              : "Waiting for first sync";
  const Icon = syncing ? RefreshCw : offline ? WifiOff : CloudCheck;
  return (
    <div
      className="flex min-w-0 flex-wrap items-center gap-2 text-xs text-muted-foreground"
      role="status"
    >
      <Icon
        className={`size-3.5 shrink-0 ${syncing ? "motion-safe:animate-spin" : ""}`}
        aria-hidden="true"
      />
      <span title={validatedAt ? `Last checked ${validatedAt}` : undefined}>
        {label}
      </span>
      {partial ? (
        <Badge variant="outline" size="sm">
          Partial history
        </Badge>
      ) : null}
    </div>
  );
}

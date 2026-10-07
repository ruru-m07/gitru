import {
  type AccountSyncDiagnostics,
  collaboration,
  collaborationErrorMessage,
  type RemoteAccount,
  type SyncRecoveryCategory,
} from "@gitru/collaboration-client";
import {
  useCollaborationAccounts,
  useCollaborationDiagnostics,
} from "@gitru/collaboration-client/react";
import { Badge } from "@gitru/ui/components/badge";
import { Button } from "@gitru/ui/components/button";
import {
  Activity,
  Database,
  Download,
  RefreshCw,
  TimerReset,
} from "lucide-react";
import { type ReactNode, useState } from "react";

export function SyncHealthPanel() {
  const diagnostics = useCollaborationDiagnostics();
  const accounts = useCollaborationAccounts();
  const [retrying, setRetrying] = useState<string | null>(null);
  const [exporting, setExporting] = useState(false);
  const [message, setMessage] = useState<{
    kind: "error" | "status";
    text: string;
  } | null>(null);
  const accountById = new Map(
    accounts.data?.accounts.map((account) => [account.id, account]) ?? [],
  );

  async function retry(
    diagnostic: AccountSyncDiagnostics,
    account: RemoteAccount,
  ) {
    if (!diagnostic.recovery?.explicit_retry_eligible || retrying) return;
    setRetrying(account.id);
    setMessage(null);
    try {
      await collaboration
        .forAccount(account)
        .refresh({ repository_id: null, kind: null });
      setMessage({ kind: "status", text: "Sync retry queued." });
      await diagnostics.refetch();
    } catch (error) {
      setMessage({ kind: "error", text: collaborationErrorMessage(error) });
    } finally {
      setRetrying(null);
    }
  }

  async function exportReport() {
    if (exporting) return;
    setExporting(true);
    setMessage(null);
    try {
      const receipt = await collaboration.exportDiagnostics();
      if (receipt.exported)
        setMessage({ kind: "status", text: "Sync diagnostics exported." });
    } catch (error) {
      setMessage({ kind: "error", text: collaborationErrorMessage(error) });
    } finally {
      setExporting(false);
    }
  }

  const snapshot = diagnostics.data;
  return (
    <section
      className="mt-5 space-y-3 border-t pt-4"
      aria-labelledby="sync-health-title"
    >
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div className="space-y-1">
          <h3 id="sync-health-title" className="text-sm font-medium">
            Sync health
          </h3>
          <p className="text-xs leading-relaxed text-muted-foreground">
            Local status only. Opening this panel never contacts a provider.
          </p>
        </div>
        <Button
          type="button"
          size="sm"
          variant="outline"
          disabled={exporting || diagnostics.isPending || diagnostics.isError}
          onClick={() => void exportReport()}
        >
          <Download aria-hidden="true" />
          {exporting ? "Exporting…" : "Export report"}
        </Button>
      </div>

      {diagnostics.isPending ? (
        <p className="text-xs text-muted-foreground" role="status">
          Reading saved sync health…
        </p>
      ) : null}
      {diagnostics.isError ? (
        <p className="text-xs text-destructive-foreground" role="alert">
          {collaborationErrorMessage(diagnostics.error)}
        </p>
      ) : null}
      {message ? (
        <p
          className={
            message.kind === "error"
              ? "text-xs text-destructive-foreground"
              : "text-xs text-muted-foreground"
          }
          role={message.kind === "error" ? "alert" : "status"}
        >
          {message.text}
        </p>
      ) : null}

      {snapshot ? (
        <>
          <div className="grid gap-2 sm:grid-cols-3" aria-label="Sync overview">
            <Metric
              icon={<Activity aria-hidden="true" />}
              label="Queued work"
              value={`${snapshot.ready_jobs} ready · ${snapshot.deferred_jobs} waiting`}
              detail={
                snapshot.oldest_job_age_seconds === null
                  ? "No queued work"
                  : `Oldest ${formatDuration(snapshot.oldest_job_age_seconds)}`
              }
            />
            <Metric
              icon={<TimerReset aria-hidden="true" />}
              label="Recent sync speed"
              value={
                snapshot.latency.p95_upper_bound_milliseconds === null
                  ? "No samples yet"
                  : `p95 ≤ ${formatMilliseconds(snapshot.latency.p95_upper_bound_milliseconds)}`
              }
              detail={`${snapshot.latency.sample_count} completed native ${snapshot.latency.sample_count === 1 ? "sample" : "samples"}`}
            />
            <Metric
              icon={<Database aria-hidden="true" />}
              label="Local storage"
              value={formatBytes(snapshot.storage.database_bytes)}
              detail={`WAL ${formatBytes(snapshot.storage.wal_bytes)}`}
            />
          </div>

          <div className="space-y-2" aria-label="Account sync health">
            {snapshot.accounts.length === 0 ? (
              <p className="rounded-lg border p-3 text-xs text-muted-foreground">
                Connect an account to start local sync diagnostics.
              </p>
            ) : null}
            {snapshot.accounts.map((diagnostic) => {
              const account = accountById.get(diagnostic.account_id);
              if (!account) return null;
              return (
                <AccountHealth
                  key={diagnostic.account_id}
                  diagnostic={diagnostic}
                  account={account}
                  busy={retrying === account.id}
                  onRetry={() => void retry(diagnostic, account)}
                />
              );
            })}
          </div>
        </>
      ) : null}
    </section>
  );
}

function Metric({
  icon,
  label,
  value,
  detail,
}: {
  icon: ReactNode;
  label: string;
  value: string;
  detail: string;
}) {
  return (
    <div className="min-w-0 rounded-lg border bg-muted/20 p-3">
      <div className="flex items-center gap-1.5 text-xs text-muted-foreground [&_svg]:size-3.5">
        {icon}
        <span>{label}</span>
      </div>
      <p className="mt-1 truncate text-sm font-medium">{value}</p>
      <p className="mt-0.5 truncate text-[11px] text-muted-foreground">
        {detail}
      </p>
    </div>
  );
}

function AccountHealth({
  diagnostic,
  account,
  busy,
  onRetry,
}: {
  diagnostic: AccountSyncDiagnostics;
  account: RemoteAccount;
  busy: boolean;
  onRetry(): void;
}) {
  const recovery = diagnostic.recovery;
  const coverage = diagnostic.coverage;
  const total =
    coverage.complete_scopes +
    coverage.partial_scopes +
    coverage.missing_scopes;
  return (
    <article className="rounded-lg border p-3">
      <div className="flex min-w-0 flex-wrap items-center gap-2">
        <p className="min-w-0 flex-1 truncate text-sm font-medium">
          {account.display_name ?? `@${account.login}`}
        </p>
        <Badge variant={recovery ? "warning" : "success"} size="sm">
          {recovery ? recoveryLabel(recovery.category) : "Healthy"}
        </Badge>
      </div>
      <p className="mt-1 text-xs text-muted-foreground">
        {total === 0
          ? "Waiting for first saved scope"
          : `${coverage.complete_scopes} complete · ${coverage.partial_scopes} partial · ${coverage.missing_scopes} missing`}
      </p>
      <p className="mt-1 text-xs text-muted-foreground">
        {diagnostic.ready_jobs} ready · {diagnostic.deferred_jobs} waiting
        {diagnostic.cooldown_remaining_seconds === null
          ? ""
          : ` · resumes in ${formatDuration(diagnostic.cooldown_remaining_seconds)}`}
      </p>
      {recovery ? (
        <div className="mt-2 flex flex-wrap items-center justify-between gap-2">
          <p className="text-xs text-muted-foreground">
            {recoveryGuidance(recovery.category, recovery.retry_after_seconds)}
          </p>
          {recovery.explicit_retry_eligible ? (
            <Button
              type="button"
              size="sm"
              variant="outline"
              disabled={busy}
              onClick={onRetry}
            >
              <RefreshCw
                className={busy ? "motion-safe:animate-spin" : undefined}
                aria-hidden="true"
              />
              {busy ? "Queuing…" : "Try sync now"}
            </Button>
          ) : null}
        </div>
      ) : null}
    </article>
  );
}

function recoveryLabel(category: SyncRecoveryCategory) {
  switch (category) {
    case "authentication":
      return "Reconnect";
    case "permission":
      return "Permissions";
    case "rate_limit":
      return "Waiting";
    case "offline":
      return "Offline";
    case "unavailable":
      return "Unavailable";
    case "permanent":
      return "Needs attention";
  }
}

function recoveryGuidance(
  category: SyncRecoveryCategory,
  retryAfterSeconds: number | null,
) {
  switch (category) {
    case "authentication":
      return "Reconnect this account to resume syncing.";
    case "permission":
      return "Use a token with access to the requested provider data.";
    case "rate_limit":
      return retryAfterSeconds === null
        ? "The provider wait has ended. You can retry explicitly."
        : `The provider asked Gitru to wait ${formatDuration(retryAfterSeconds)}.`;
    case "offline":
      return retryAfterSeconds === null
        ? "Connectivity may be available again. Retry when ready."
        : `Saved data remains available. Retry in ${formatDuration(retryAfterSeconds)}.`;
    case "unavailable":
      return retryAfterSeconds === null
        ? "The temporary wait has ended. You can retry explicitly."
        : `Sync can be retried in ${formatDuration(retryAfterSeconds)}.`;
    case "permanent":
      return "Review the account and provider setup before trying again.";
  }
}

function formatDuration(seconds: number) {
  if (seconds < 60) return `${seconds}s`;
  if (seconds < 3_600) return `${Math.ceil(seconds / 60)}m`;
  return `${Math.ceil(seconds / 3_600)}h`;
}

function formatMilliseconds(milliseconds: number) {
  return milliseconds < 1_000
    ? `${milliseconds} ms`
    : `${(milliseconds / 1_000).toFixed(1)} s`;
}

function formatBytes(bytes: number | null) {
  if (bytes === null) return "Unavailable";
  if (bytes < 1_024) return `${bytes} B`;
  const units = ["KiB", "MiB", "GiB"];
  let value = bytes / 1_024;
  let index = 0;
  while (value >= 1_024 && index < units.length - 1) {
    value /= 1_024;
    index += 1;
  }
  return `${value.toFixed(value >= 10 ? 0 : 1)} ${units[index]}`;
}

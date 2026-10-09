import type {
  AccountSnapshot,
  SyncDiagnosticsSnapshot,
  SyncRecoveryState,
} from "@gitru/commands";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it } from "vitest";
import { fixtureAccount } from "../../../tests/fixtures/collaboration";
import {
  mockTauriCommand,
  mockTauriCommandResult,
} from "../../../tests/mocks/tauri";
import { SyncHealthPanel } from "./sync-health-panel";

const categories = [
  "authentication",
  "permission",
  "rate_limit",
  "offline",
  "unavailable",
  "permanent",
] as const;

const accounts: AccountSnapshot = {
  accounts: categories.map((category, index) => ({
    ...fixtureAccount,
    id: `account-${category}`,
    actor_id: String(index + 1),
    login: `${category}-user`,
    display_name: category,
  })),
  revision: "10",
  authorization_view: "1",
};

function recovery(category: (typeof categories)[number]): SyncRecoveryState {
  const retryable = category === "offline";
  const waiting = category === "rate_limit" || category === "unavailable";
  return {
    category,
    affected_scopes: 1,
    next_retry_at: waiting ? "2026-10-08T01:00:00.000Z" : null,
    retry_after_seconds: waiting ? 42 : null,
    explicit_retry_eligible: retryable,
  };
}

const diagnostics: SyncDiagnosticsSnapshot = {
  generated_at: "2026-10-08T00:00:00.000Z",
  revision: "10",
  accounts: accounts.accounts.map((account, index) => ({
    account_id: account.id,
    provider: account.provider,
    coverage: {
      complete_scopes: index + 1,
      partial_scopes: index === 1 ? 1 : 0,
      missing_scopes: index === 2 ? 1 : 0,
    },
    ready_jobs: index === 3 ? 1 : 0,
    deferred_jobs: index === 2 ? 1 : 0,
    oldest_job_age_seconds: index === 3 ? 12 : null,
    cooldown_remaining_seconds: index === 2 ? 42 : null,
    recovery: recovery(categories[index]),
  })),
  ready_jobs: 1,
  deferred_jobs: 1,
  oldest_job_age_seconds: 12,
  accounts_in_cooldown: 1,
  latency: {
    sample_count: 5,
    total_milliseconds: 211,
    maximum_milliseconds: 101,
    p50_upper_bound_milliseconds: 30,
    p95_upper_bound_milliseconds: 250,
    p99_upper_bound_milliseconds: 250,
  },
  storage: {
    cache_usage_available: true,
    logical_bytes: 2_048,
    indexed_logical_bytes: 1_024,
    database_bytes: 4_096,
    wal_bytes: 512,
    wal_observation_supported: true,
    wal_busy: false,
    wal_log_frames: 3,
    wal_checkpointed_frames: 3,
  },
};

const caches: QueryClient[] = [];
afterEach(() => {
  for (const cache of caches.splice(0)) cache.clear();
});

function mount() {
  const cache = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  caches.push(cache);
  return render(
    <QueryClientProvider client={cache}>
      <SyncHealthPanel />
    </QueryClientProvider>,
  );
}

describe("sync health panel", () => {
  it("renders saved native observations without starting provider work", async () => {
    mockTauriCommandResult("collaboration_accounts", accounts);
    const read = mockTauriCommandResult(
      "collaboration_diagnostics",
      diagnostics,
    );
    const refresh = mockTauriCommandResult("collaboration_refresh", {
      job_id: "explicit-only",
    });
    const exportReport = mockTauriCommandResult(
      "collaboration_export_diagnostics",
      { exported: true },
    );
    mount();

    expect(await screen.findByText("p95 ≤ 250 ms")).toBeVisible();
    expect(screen.getByText("4.0 KiB")).toBeVisible();
    expect(screen.getByText("WAL 512 B")).toBeVisible();
    for (const guidance of [
      "Reconnect this account to resume syncing.",
      "Use a token with access to the requested provider data.",
      "The provider asked Gitru to wait 42s.",
      "Connectivity may be available again. Retry when ready.",
      "Sync can be retried in 42s.",
      "Review the account and provider setup before trying again.",
    ])
      expect(screen.getByText(guidance)).toBeVisible();
    expect(read).toHaveBeenCalledTimes(1);
    expect(refresh).not.toHaveBeenCalled();
    expect(exportReport).not.toHaveBeenCalled();
  });

  it("admits one retry and one export only after their explicit buttons", async () => {
    mockTauriCommandResult("collaboration_accounts", accounts);
    mockTauriCommandResult("collaboration_diagnostics", diagnostics);
    const refresh = mockTauriCommandResult("collaboration_refresh", {
      job_id: "explicit-retry",
    });
    const exportReport = mockTauriCommand(
      "collaboration_export_diagnostics",
      (payload) => {
        expect(payload).toEqual({});
        return { exported: true };
      },
    );
    mount();
    const user = userEvent.setup();

    await user.click(
      await screen.findByRole("button", { name: "Try sync now" }),
    );
    await waitFor(() =>
      expect(refresh).toHaveBeenCalledExactlyOnceWith({
        request: {
          account_id: "account-offline",
          repository_id: null,
          kind: null,
        },
      }),
    );
    expect(screen.getByText("Sync retry queued.")).toBeVisible();

    await user.click(screen.getByRole("button", { name: "Export report" }));
    await waitFor(() => expect(exportReport).toHaveBeenCalledTimes(1));
    expect(screen.getByText("Sync diagnostics exported.")).toBeVisible();
  });
});

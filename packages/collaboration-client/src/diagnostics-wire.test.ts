import {
  collaborationDiagnostics,
  collaborationExportDiagnostics,
  SyncDiagnosticsExportReceiptSchema,
  SyncDiagnosticsSnapshotSchema,
  SyncRecoveryCategorySchema,
} from "@gitru/commands";
import { afterEach, describe, expect, it, vi } from "vitest";

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
afterEach(() => invoke.mockReset());

const snapshot = {
  generated_at: "2026-10-08T00:00:00.000Z",
  revision: "9007199254740993",
  accounts: [
    {
      account_id: "account-a",
      provider: "github" as const,
      coverage: {
        complete_scopes: 4,
        partial_scopes: 1,
        missing_scopes: 2,
      },
      ready_jobs: 1,
      deferred_jobs: 2,
      oldest_job_age_seconds: 8,
      cooldown_remaining_seconds: null,
      recovery: {
        category: "permission" as const,
        affected_scopes: 2,
        next_retry_at: null,
        retry_after_seconds: null,
        explicit_retry_eligible: false,
      },
    },
  ],
  ready_jobs: 1,
  deferred_jobs: 2,
  oldest_job_age_seconds: 8,
  accounts_in_cooldown: 0,
  latency: {
    sample_count: 3,
    total_milliseconds: 72,
    maximum_milliseconds: 40,
    p50_upper_bound_milliseconds: 30,
    p95_upper_bound_milliseconds: 100,
    p99_upper_bound_milliseconds: 100,
  },
  storage: {
    cache_usage_available: true,
    logical_bytes: 1024,
    indexed_logical_bytes: 768,
    database_bytes: 4096,
    wal_bytes: 512,
    wal_observation_supported: true,
    wal_busy: false,
    wal_log_frames: 3,
    wal_checkpointed_frames: 3,
  },
};

describe("generated sync diagnostics IPC wire", () => {
  it("keeps categories snake-cased and every unavailable observation explicitly null", async () => {
    const parsed = SyncDiagnosticsSnapshotSchema.parse(snapshot);
    expect(parsed.accounts[0].recovery?.category).toBe("permission");
    expect(parsed.accounts[0].cooldown_remaining_seconds).toBeNull();
    expect(parsed.accounts[0].recovery?.next_retry_at).toBeNull();
    invoke.mockResolvedValue(parsed);
    await expect(collaborationDiagnostics({})).resolves.toEqual(parsed);
    expect(invoke).toHaveBeenCalledExactlyOnceWith(
      "collaboration_diagnostics",
      {},
    );
  });

  it("accepts only the six bounded recovery categories", () => {
    expect(
      [
        "authentication",
        "permission",
        "rate_limit",
        "offline",
        "unavailable",
        "permanent",
      ].map((category) => SyncRecoveryCategorySchema.parse(category)),
    ).toHaveLength(6);
    expect(SyncRecoveryCategorySchema.safeParse("unknown").success).toBe(false);
    expect(
      SyncDiagnosticsSnapshotSchema.safeParse({
        ...snapshot,
        accounts: [
          {
            ...snapshot.accounts[0],
            recovery: { ...snapshot.accounts[0].recovery, category: "unknown" },
          },
        ],
      }).success,
    ).toBe(false);
  });

  it("exports through a pathless native command and returns only its receipt", async () => {
    const receipt = SyncDiagnosticsExportReceiptSchema.parse({
      exported: false,
    });
    invoke.mockResolvedValue(receipt);
    await expect(collaborationExportDiagnostics({})).resolves.toEqual(receipt);
    expect(invoke).toHaveBeenCalledExactlyOnceWith(
      "collaboration_export_diagnostics",
      {},
    );
  });
});

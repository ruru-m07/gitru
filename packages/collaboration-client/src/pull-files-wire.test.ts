import {
  PullFileArtifactSnapshotSchema,
  PullFileSnapshotSchema,
} from "@gitru/commands";
import { QueryClient } from "@tanstack/react-query";
import { afterEach, describe, expect, it, vi } from "vitest";
import {
  collaboration,
  collaborationKeys,
  type PullFileArtifactSnapshot,
  type PullFileDiffRequest,
  type PullFileSnapshot,
  type RemoteAccount,
} from "./index";
import { pullFileArtifactQueryOptions, pullFilesQueryOptions } from "./react";

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
afterEach(() => invoke.mockReset());

const account: RemoteAccount = {
  id: "account-a",
  actor_id: "actor-a",
  provider: "github",
  host: "https://github.com",
  authorization_epoch: "9007199254740993",
  login: "fixture",
  state: "active",
  display_name: null,
  notifications_supported: true,
};
const context = {
  base_oid: "a".repeat(40),
  head_oid: "b".repeat(40),
  merge_base_oid: null,
  base_repository_provider_id: "target-repository",
  source_repository_provider_id: "source-repository",
  body_metadata_facet_revision: "9007199254740994",
};
const identity = { old_path: "src/old.ts", new_path: "src/new.ts" };
const file = {
  file_key: "opaque-file-key",
  context,
  provider_position: 0,
  file: {
    identity,
    provider_file_id: null,
    change_kind: "renamed" as const,
    provider_change_kind: "renamed",
    additions: { state: "known" as const, value: "12" },
    deletions: { state: "unknown" as const },
    total_changes: { state: "known" as const, value: "12" },
    old_mode: null,
    new_mode: "100644",
    mode_changed: { state: "known" as const, value: false },
    binary: { state: "unknown" as const },
    generated: { state: "known" as const, value: false },
    provider_collapsed: { state: "unknown" as const },
    provider_too_large: { state: "known" as const, value: true },
    diff_hint: "oversized" as const,
  },
};
const snapshot: PullFileSnapshot = PullFileSnapshotSchema.parse({
  subject_id: "pull-42",
  context,
  files: [file],
  next_cursor: null,
  completeness: {
    state: "capped",
    cap: {
      provenance: "provider",
      reason: "provider_file_limit",
      remote_has_more: { state: "known", value: true },
    },
  },
  coverage: {
    state: "partial",
    validated_at: "2026-10-08T00:00:00Z",
    remote_has_more: true,
  },
  sync: {
    state: "offline",
    last_success_at: "2026-10-08T00:00:00Z",
    next_retry_at: null,
    error: null,
  },
  freshness: "stale",
  facet_revision: "9007199254740995",
  revision: "9007199254740996",
  authorization_view: "3",
});
const request: PullFileDiffRequest = {
  account_id: account.id,
  authorization_epoch: account.authorization_epoch,
  subject_id: snapshot.subject_id,
  file_facet_revision: snapshot.facet_revision ?? "",
  context,
  file_key: file.file_key,
};
const membership = {
  account_id: account.id,
  authorization_epoch: account.authorization_epoch,
  authorization_view: "3",
  subject_id: snapshot.subject_id,
  generation: "123e4567-e89b-12d3-a456-426614174000",
  file_facet_revision: request.file_facet_revision,
  context,
  file_key: file.file_key,
  identity,
};
const artifactSnapshot: PullFileArtifactSnapshot =
  PullFileArtifactSnapshotSchema.parse({
    request,
    membership,
    artifact: null,
    revision: "9007199254740996",
    authorization_view: "3",
    freshness: "stale",
  });

describe("cached pull file wire", () => {
  it("preserves nullable range facts, mixed count/flag evidence and exact cap provenance", () => {
    expect(snapshot.context?.merge_base_oid).toBeNull();
    expect(snapshot.files[0]?.file.additions).toEqual({
      state: "known",
      value: "12",
    });
    expect(snapshot.files[0]?.file.deletions).toEqual({ state: "unknown" });
    expect(snapshot.files[0]?.file.mode_changed).toEqual({
      state: "known",
      value: false,
    });
    expect(snapshot.files[0]?.file.binary).toEqual({ state: "unknown" });
    expect(snapshot.completeness).toEqual({
      state: "capped",
      cap: {
        provenance: "provider",
        reason: "provider_file_limit",
        remote_has_more: { state: "known", value: true },
      },
    });
    expect(snapshot.next_cursor).toBeNull();
    expect(artifactSnapshot.artifact).toBeNull();
  });

  it.each([
    {
      kind: "provider" as const,
      provider_validated_at: "2026-10-08T00:00:00Z",
    },
    {
      kind: "local_exact_range" as const,
      local_validated_at: "2026-10-08T00:00:00Z",
      resolved_merge_base_oid: "c".repeat(40),
    },
  ])("keeps $kind validation evidence source-specific", (validation) => {
    const parsed = PullFileArtifactSnapshotSchema.parse({
      ...artifactSnapshot,
      artifact: {
        ...membership,
        source: {
          strategy:
            validation.kind === "provider"
              ? "github_pull_files"
              : "local_exact_range",
          adapter_version: 1,
        },
        validation,
        content_state: "text",
        unified_text: "diff --git a/src/old.ts b/src/new.ts\n",
        blob_references: { old: null, new: null },
        old_blob_oid: null,
        new_blob_oid: null,
        content_type: "text/x-diff",
        binary_hint: { state: "known", value: false },
        image_hint: { state: "unknown" },
        last_access_revision: request.file_facet_revision,
        logical_bytes: "43",
        on_disk_bytes: "43",
      },
    });
    expect(parsed.artifact?.validation).toEqual(validation);
  });

  it("keeps local reads separate from explicit provider and clone hydration", async () => {
    invoke
      .mockResolvedValueOnce(snapshot)
      .mockResolvedValueOnce(artifactSnapshot)
      .mockResolvedValueOnce({ job_id: "bounded-provider-job" })
      .mockResolvedValueOnce(artifactSnapshot);
    const files = collaboration.forAccount(account);
    const localQuery = {
      subject_id: snapshot.subject_id,
      cursor: null,
      limit: 100,
    };
    const selected = {
      subject_id: request.subject_id,
      file_facet_revision: request.file_facet_revision,
      context: request.context,
      file_key: request.file_key,
    };

    await expect(files.pullFiles(localQuery)).resolves.toEqual(snapshot);
    await expect(files.pullFileArtifact(selected)).resolves.toEqual(
      artifactSnapshot,
    );
    await expect(files.hydratePullFile(selected)).resolves.toEqual({
      job_id: "bounded-provider-job",
    });
    await expect(
      files.loadLocalPullFile({
        ...selected,
        local_repository_id: "11111111-1111-4111-8111-111111111111",
        link_id: "link-a",
        link_generation: "7",
      }),
    ).resolves.toEqual(artifactSnapshot);

    expect(invoke.mock.calls).toEqual([
      [
        "collaboration_pull_files",
        { query: { ...localQuery, account_id: account.id } },
      ],
      ["collaboration_pull_file_artifact", { request }],
      ["collaboration_hydrate_pull_file", { request }],
      [
        "collaboration_load_local_pull_file",
        {
          request: {
            ...request,
            local_repository_id: "11111111-1111-4111-8111-111111111111",
            link_id: "link-a",
            link_generation: "7",
          },
        },
      ],
    ]);
  });

  it("keys list and selected artifacts by epoch and every exact range identity", () => {
    const listKey = pullFilesQueryOptions(account, {
      subject_id: request.subject_id,
      cursor: null,
      limit: 100,
    }).queryKey;
    const artifactKey = pullFileArtifactQueryOptions(account, {
      subject_id: request.subject_id,
      file_facet_revision: request.file_facet_revision,
      context,
      file_key: request.file_key,
    }).queryKey;
    expect(
      pullFileArtifactQueryOptions(account, {
        subject_id: request.subject_id,
        file_facet_revision: request.file_facet_revision,
        context,
        file_key: request.file_key,
      }).gcTime,
    ).toBe(0);
    expect(listKey).toEqual(
      collaborationKeys.pullFiles(account, {
        account_id: account.id,
        subject_id: request.subject_id,
        cursor: null,
        limit: 100,
      }),
    );
    expect(artifactKey).not.toEqual(
      pullFileArtifactQueryOptions(account, {
        subject_id: request.subject_id,
        file_facet_revision: request.file_facet_revision,
        context: { ...context, head_oid: "d".repeat(40) },
        file_key: request.file_key,
      }).queryKey,
    );
    expect(artifactKey).not.toEqual(
      pullFileArtifactQueryOptions(
        { ...account, authorization_epoch: "9007199254740999" },
        {
          subject_id: request.subject_id,
          file_facet_revision: request.file_facet_revision,
          context,
          file_key: request.file_key,
        },
      ).queryKey,
    );
  });

  it("query options perform only the two SQLite reads", async () => {
    invoke
      .mockResolvedValueOnce(snapshot)
      .mockResolvedValueOnce(artifactSnapshot);
    const cache = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    });
    await cache.fetchQuery(
      pullFilesQueryOptions(account, {
        subject_id: request.subject_id,
        cursor: null,
        limit: 100,
      }),
    );
    await cache.fetchQuery(
      pullFileArtifactQueryOptions(account, {
        subject_id: request.subject_id,
        file_facet_revision: request.file_facet_revision,
        context,
        file_key: request.file_key,
      }),
    );
    expect(invoke.mock.calls.map(([command]) => command)).toEqual([
      "collaboration_pull_files",
      "collaboration_pull_file_artifact",
    ]);
    cache.clear();
  });
});

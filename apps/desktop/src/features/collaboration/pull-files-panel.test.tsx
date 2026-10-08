import {
  type ContextFacetCapability,
  collaboration,
  collaborationKeys,
  type PullFile,
  type PullFileArtifact,
  type PullFileArtifactSnapshot,
  type PullFileDiffRequest,
  type PullFileSnapshot,
} from "@gitru/collaboration-client";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { fixtureAccount } from "../../../tests/fixtures/collaboration";
import { PullFilesPanel } from "./pull-files-panel";
import { ReviewAuthoringProvider } from "./review-submission";

vi.mock("@pierre/diffs/react", () => ({
  PatchDiff: ({ patch }: { patch: string }) => (
    <pre data-testid="safe-patch-diff">{patch}</pre>
  ),
}));
vi.mock("next-themes", () => ({ useTheme: () => ({ theme: "light" }) }));
vi.mock("@tanstack/react-virtual", () => ({
  useVirtualizer: ({
    count,
    getItemKey,
  }: {
    count: number;
    getItemKey: (index: number) => string;
  }) => ({
    getTotalSize: () => count * 64,
    getVirtualItems: () =>
      Array.from({ length: count }, (_, index) => ({
        index,
        key: getItemKey(index),
        size: 64,
        start: index * 64,
      })),
    scrollToIndex: vi.fn(),
  }),
}));

const subjectId = "github:pull:repository:67";
const instanceId = "github:https://github.com/";
const repositoryId = "github:repository:1";
const context = {
  base_oid: "a".repeat(40),
  head_oid: "b".repeat(40),
  merge_base_oid: null,
  base_repository_provider_id: "target-repository",
  source_repository_provider_id: "source-repository",
  body_metadata_facet_revision: "10",
};
const policy: ContextFacetCapability = {
  facet: "pull_files",
  saved_read: { state: "supported", reason: null },
  synchronize: { state: "supported", reason: null },
  remote_write: { state: "unsupported", reason: "not_implemented" },
  observation: "complete",
  sync: {
    state: "offline",
    last_success_at: "2026-10-08T00:00:00Z",
    next_retry_at: null,
    error: null,
  },
  can_recheck_access: false,
};

function savedFile(key: string, path: string, position: number): PullFile {
  return {
    file_key: key,
    context,
    provider_position: position,
    file: {
      identity: { old_path: path, new_path: path },
      provider_file_id: null,
      change_kind: "modified",
      provider_change_kind: "modified",
      additions: { state: "known", value: String(position + 1) },
      deletions: { state: "unknown" },
      total_changes: { state: "unknown" },
      old_mode: null,
      new_mode: null,
      mode_changed: { state: "unknown" },
      binary: { state: "unknown" },
      generated: { state: "unknown" },
      provider_collapsed: { state: "unknown" },
      provider_too_large: { state: "unknown" },
      diff_hint: "candidate",
    },
  };
}

const files = [
  savedFile("file-a", "src/a.ts", 0),
  savedFile("file-b", "src/b.ts", 1),
  savedFile("file-c", "src/c.ts", 2),
];
const snapshot: PullFileSnapshot = {
  subject_id: subjectId,
  context,
  files,
  next_cursor: null,
  completeness: { state: "complete", cap: null },
  coverage: {
    state: "complete",
    validated_at: "2026-10-08T00:00:00Z",
    remote_has_more: false,
  },
  sync: policy.sync,
  freshness: "stale",
  facet_revision: "11",
  revision: "12",
  authorization_view: "3",
};

function artifactSnapshot(
  request: PullFileDiffRequest,
  artifact: PullFileArtifact | null = null,
): PullFileArtifactSnapshot {
  const selected =
    files.find((file) => file.file_key === request.file_key) ?? files[0];
  return {
    request,
    membership: {
      account_id: request.account_id,
      authorization_epoch: request.authorization_epoch,
      authorization_view: "3",
      subject_id: request.subject_id,
      generation: "123e4567-e89b-12d3-a456-426614174000",
      file_facet_revision: request.file_facet_revision,
      context: request.context,
      file_key: request.file_key,
      identity: selected.file.identity,
    },
    artifact,
    revision: "12",
    authorization_view: "3",
    freshness: "stale",
  };
}

function savedArtifact(
  request: PullFileDiffRequest,
  state: PullFileArtifact["content_state"],
  text: string | null = null,
): PullFileArtifact {
  const selected =
    files.find((file) => file.file_key === request.file_key) ?? files[0];
  return {
    account_id: request.account_id,
    authorization_epoch: request.authorization_epoch,
    authorization_view: "3",
    subject_id: request.subject_id,
    generation: "123e4567-e89b-12d3-a456-426614174000",
    file_key: request.file_key,
    identity: selected.file.identity,
    context: request.context,
    source: { strategy: "github_pull_files", adapter_version: 1 },
    validation: {
      kind: "provider",
      provider_validated_at: "2026-10-08T00:00:00Z",
    },
    content_state: state,
    unified_text: text,
    blob_references:
      state === "binary" || state === "image"
        ? { old: "old-blob", new: "new-blob" }
        : { old: null, new: null },
    old_blob_oid: null,
    new_blob_oid: null,
    content_type: state === "text" ? "text/x-diff" : null,
    binary_hint:
      state === "binary"
        ? { state: "known", value: true }
        : { state: "unknown" },
    image_hint:
      state === "image"
        ? { state: "known", value: true }
        : { state: "unknown" },
    last_access_revision: "11",
    logical_bytes: String(text?.length ?? 0),
    on_disk_bytes: String(text?.length ?? 0),
  };
}

let cache: QueryClient;
beforeEach(() => {
  cache = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  vi.spyOn(collaboration.transport, "pullFiles").mockResolvedValue(snapshot);
  vi.spyOn(collaboration.transport, "pullFileArtifact").mockImplementation(
    async (request) => artifactSnapshot(request),
  );
  vi.spyOn(collaboration.transport, "hydrateDetail").mockRejectedValue(
    new Error("unexpected list hydration"),
  );
  vi.spyOn(collaboration.transport, "hydratePullFile").mockResolvedValue({
    job_id: "selected-file-job",
  });
  vi.spyOn(collaboration.transport, "localClones").mockResolvedValue({
    clones: [],
  });
  vi.spyOn(collaboration.transport, "loadLocalPullFile").mockRejectedValue(
    new Error("unexpected local read"),
  );
});
afterEach(() => {
  cache.clear();
  vi.restoreAllMocks();
});

function mount() {
  render(
    <QueryClientProvider client={cache}>
      <PullFilesPanel
        account={fixtureAccount}
        subjectId={subjectId}
        instanceId={instanceId}
        repositoryId={repositoryId}
        policy={policy}
      />
    </QueryClientProvider>,
  );
}

async function openAndSelect(path = "src/a.ts") {
  const user = userEvent.setup();
  mount();
  await user.click(screen.getByRole("button", { name: "Files" }));
  const file = await screen.findByRole("button", { name: new RegExp(path) });
  await user.click(file);
  return { user, file };
}

describe("cached pull request files", () => {
  it("opens the saved list and selected missing artifact without provider hydration", async () => {
    await openAndSelect();

    expect(screen.getByText("Saved list may be stale")).toBeVisible();
    expect(
      await screen.findByText(
        "This file diff is not saved on this device yet.",
      ),
    ).toBeVisible();
    expect(collaboration.transport.pullFiles).toHaveBeenCalledExactlyOnceWith({
      account_id: fixtureAccount.id,
      subject_id: subjectId,
      cursor: null,
      limit: 100,
    });
    expect(
      collaboration.transport.pullFileArtifact,
    ).toHaveBeenCalledExactlyOnceWith({
      account_id: fixtureAccount.id,
      authorization_epoch: fixtureAccount.authorization_epoch,
      subject_id: subjectId,
      file_facet_revision: "11",
      context,
      file_key: "file-a",
    });
    expect(collaboration.transport.hydrateDetail).not.toHaveBeenCalled();
    expect(collaboration.transport.hydratePullFile).not.toHaveBeenCalled();
  });

  it("supports Arrow, Home, End and Enter with focus on the selected row", async () => {
    const { user, file } = await openAndSelect();
    file.focus();

    await user.keyboard("{ArrowDown}");
    expect(screen.getByRole("button", { name: /src\/b\.ts/ })).toHaveFocus();
    await user.keyboard("{End}");
    expect(screen.getByRole("button", { name: /src\/c\.ts/ })).toHaveFocus();
    await user.keyboard("{Home}");
    const first = screen.getByRole("button", { name: /src\/a\.ts/ });
    expect(first).toHaveFocus();
    await user.keyboard("{Enter}");
    expect(first).toHaveAttribute("aria-pressed", "true");
    expect(collaboration.transport.pullFileArtifact).toHaveBeenCalledWith(
      expect.objectContaining({ file_key: "file-c" }),
    );
  });

  it("virtualizes long saved pages while preserving keyboard selection", async () => {
    const longFiles = Array.from({ length: 45 }, (_, index) =>
      savedFile(`file-${index}`, `src/file-${index}.ts`, index),
    );
    vi.mocked(collaboration.transport.pullFiles).mockResolvedValue({
      ...snapshot,
      files: longFiles,
    });
    const user = userEvent.setup();
    mount();
    await user.click(screen.getByRole("button", { name: "Files" }));
    const list = await screen.findByRole("region", {
      name: "Saved pull request files",
    });
    expect(list).toHaveClass("overflow-auto");
    const first = screen.getByRole("button", { name: /src\/file-0\.ts/ });
    await user.click(first);
    first.focus();
    await user.keyboard("{End}");
    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: /src\/file-44\.ts/ }),
      ).toHaveFocus(),
    );
    expect(collaboration.transport.pullFileArtifact).toHaveBeenCalledWith(
      expect.objectContaining({ file_key: "file-44" }),
    );
  });

  it("starts selected provider hydration only after the explicit action", async () => {
    const { user } = await openAndSelect();
    expect(collaboration.transport.hydratePullFile).not.toHaveBeenCalled();

    await user.click(
      screen.getByRole("button", { name: "Load from provider" }),
    );

    expect(
      collaboration.transport.hydratePullFile,
    ).toHaveBeenCalledExactlyOnceWith({
      account_id: fixtureAccount.id,
      authorization_epoch: fixtureAccount.authorization_epoch,
      subject_id: subjectId,
      file_facet_revision: "11",
      context,
      file_key: "file-a",
    });
    expect(
      await screen.findByText(/Diff hydration was requested/),
    ).toBeVisible();
  });

  it("refuses to render an artifact returned for a different comparison", async () => {
    vi.mocked(collaboration.transport.pullFileArtifact).mockImplementation(
      async (request) =>
        artifactSnapshot({
          ...request,
          context: { ...request.context, head_oid: "d".repeat(40) },
        }),
    );
    await openAndSelect();
    expect(
      await screen.findByText(
        "This saved file selection changed. Choose the file again.",
      ),
    ).toBeVisible();
  });

  it("follows only native saved cursors and keeps each page bounded to 100 rows", async () => {
    vi.mocked(collaboration.transport.pullFiles).mockImplementation(
      async (query) =>
        query.cursor === null
          ? { ...snapshot, next_cursor: "cursor-2" }
          : { ...snapshot, files: [files[1]], next_cursor: null },
    );
    const user = userEvent.setup();
    mount();
    await user.click(screen.getByRole("button", { name: "Files" }));
    const next = await screen.findByRole("button", {
      name: "Next saved files",
    });
    await waitFor(() => expect(next).toBeEnabled());
    await user.click(next);

    expect(await screen.findByText("Saved page 2")).toBeVisible();
    expect(collaboration.transport.pullFiles).toHaveBeenLastCalledWith({
      account_id: fixtureAccount.id,
      subject_id: subjectId,
      cursor: "cursor-2",
      limit: 100,
    });
  });

  it("returns directly to page one when a saved continuation becomes stale", async () => {
    vi.mocked(collaboration.transport.pullFiles).mockImplementation(
      async (query) => {
        if (query.cursor !== null) throw { code: "stale_view" };
        return { ...snapshot, next_cursor: "stale-cursor" };
      },
    );
    const user = userEvent.setup();
    mount();
    await user.click(screen.getByRole("button", { name: "Files" }));
    const next = await screen.findByRole("button", {
      name: "Next saved files",
    });
    await waitFor(() => expect(next).toBeEnabled());
    await user.click(next);
    expect(
      await screen.findByText(
        "This saved view changed. Reload it before continuing.",
      ),
    ).toBeVisible();

    await user.click(
      screen.getByRole("button", { name: "Return to first saved page" }),
    );
    expect(await screen.findByText("Saved page 1")).toBeVisible();
    expect(screen.getByRole("button", { name: /src\/a\.ts/ })).toBeVisible();
  });

  it("reads only the explicitly selected linked clone and refreshes the local artifact", async () => {
    vi.mocked(collaboration.transport.localClones).mockResolvedValue({
      clones: [
        {
          local_repository_id: "11111111-1111-4111-8111-111111111111",
          local_repository_name: "Local project",
          link_id: "link-a",
          generation: "9",
          state: "linked",
        },
      ],
    });
    vi.mocked(collaboration.transport.loadLocalPullFile).mockImplementation(
      async (request) =>
        artifactSnapshot(
          request,
          savedArtifact(
            request,
            "text",
            "diff --git a/src/a.ts b/src/a.ts\n@@ -1 +1 @@\n-old\n+new\n",
          ),
        ),
    );
    const { user } = await openAndSelect();
    expect(collaboration.transport.loadLocalPullFile).not.toHaveBeenCalled();

    await user.click(screen.getByRole("button", { name: "Read linked clone" }));
    await user.click(
      await screen.findByRole("button", { name: "Read this clone" }),
    );

    await waitFor(() =>
      expect(
        collaboration.transport.loadLocalPullFile,
      ).toHaveBeenCalledExactlyOnceWith({
        account_id: fixtureAccount.id,
        authorization_epoch: fixtureAccount.authorization_epoch,
        subject_id: subjectId,
        file_facet_revision: "11",
        context,
        file_key: "file-a",
        local_repository_id: "11111111-1111-4111-8111-111111111111",
        link_id: "link-a",
        link_generation: "9",
      }),
    );
    expect(collaboration.transport.hydratePullFile).not.toHaveBeenCalled();
  });

  it.each([
    ["binary", "Binary content is saved without a text diff."],
    ["image", "An image artifact is saved; image preview is unavailable here."],
    ["omitted", "The source omitted content for this file."],
    ["oversized", "This diff exceeds Gitru's local text limit."],
    ["unsupported", "This source cannot provide a safe diff for the file."],
    ["unavailable", "The current file diff is unavailable."],
    ["not_loaded", "This file diff has not been loaded."],
  ] as const)("renders %s as explicit non-text evidence", async (state, message) => {
    vi.mocked(collaboration.transport.pullFileArtifact).mockImplementation(
      async (request) =>
        artifactSnapshot(request, savedArtifact(request, state)),
    );
    await openAndSelect();
    expect(await screen.findByText(message)).toBeVisible();
  });

  it("renders provider patch text through the existing diff component as data", async () => {
    const patch = "@@ -1 +1 @@\n-safe\n+<img src=x onerror=alert(1)>\n";
    vi.mocked(collaboration.transport.pullFileArtifact).mockImplementation(
      async (request) =>
        artifactSnapshot(request, savedArtifact(request, "text", patch)),
    );
    await openAndSelect();
    expect(await screen.findByTestId("safe-patch-diff")).toHaveTextContent(
      "<img src=x onerror=alert(1)>",
    );
    expect(screen.getByTestId("safe-patch-diff")).toHaveTextContent(
      "diff --git a/src/a.ts b/src/a.ts",
    );
    expect(screen.queryByRole("img")).toBeNull();
  });

  it("keeps a saved empty text artifact distinct from a missing artifact", async () => {
    vi.mocked(collaboration.transport.pullFileArtifact).mockImplementation(
      async (request) =>
        artifactSnapshot(request, savedArtifact(request, "text", "")),
    );
    await openAndSelect();
    expect(
      await screen.findByText("The saved textual diff is empty."),
    ).toBeVisible();
    expect(
      screen.queryByText("This file diff is not saved on this device yet."),
    ).toBeNull();
  });

  it("labels a local binary observation without claiming blob content was saved", async () => {
    vi.mocked(collaboration.transport.pullFileArtifact).mockImplementation(
      async (request) => {
        const artifact = savedArtifact(request, "omitted");
        return artifactSnapshot(request, {
          ...artifact,
          source: { strategy: "local_exact_range", adapter_version: 1 },
          validation: {
            kind: "local_exact_range",
            local_validated_at: "2026-10-08T00:00:00Z",
            resolved_merge_base_oid: "c".repeat(40),
          },
          binary_hint: { state: "known", value: true },
        });
      },
    );
    await openAndSelect();
    expect(
      await screen.findByText(
        "The linked clone reported binary content; no blob bytes were saved.",
      ),
    ).toBeVisible();
    expect(
      screen.getByText("Read from the linked clone at the saved comparison"),
    ).toBeVisible();
  });

  it("clears a selected artifact when the exact head and facet revision change", async () => {
    await openAndSelect();
    expect(
      await screen.findByText(
        "This file diff is not saved on this device yet.",
      ),
    ).toBeVisible();
    const nextContext = { ...context, head_oid: "d".repeat(40) };
    const next = {
      ...snapshot,
      context: nextContext,
      facet_revision: "13",
      files: snapshot.files.map((file) => ({ ...file, context: nextContext })),
      revision: "14",
    };
    cache.setQueryData(
      collaborationKeys.pullFiles(fixtureAccount, {
        account_id: fixtureAccount.id,
        subject_id: subjectId,
        cursor: null,
        limit: 100,
      }),
      next,
    );
    expect(
      await screen.findByText("Choose a saved file to open its cached diff."),
    ).toBeVisible();
  });
});

describe("inline review selection from provider diffs", () => {
  function mountReviewFiles() {
    const account = { ...fixtureAccount, host: "github.com" };
    vi.spyOn(collaboration.transport, "reviewDraft").mockResolvedValue({
      key: { account_id: account.id, subject_id: subjectId },
      event: "comment",
      body: "Review",
      comments: [],
      generation: "1",
      context: null,
      availability: "unavailable",
      reason: "stale_context",
      submission: null,
      revision: "12",
      authorization_view: "3",
    });
    vi.spyOn(collaboration.transport, "submittedReviews").mockResolvedValue({
      account_id: account.id,
      subject_id: subjectId,
      reviews: [],
      next_cursor: null,
      revision: "12",
      authorization_view: "3",
    });
    const matches = Element.prototype.matches;
    vi.spyOn(Element.prototype, "matches").mockImplementation(function (
      this: Element,
      selector: string,
    ) {
      return [":modal", ":fullscreen", ":popover-open"].includes(selector)
        ? false
        : matches.call(this, selector);
    });
    render(
      <QueryClientProvider client={cache}>
        <ReviewAuthoringProvider account={account} subjectId={subjectId}>
          <PullFilesPanel
            account={account}
            subjectId={subjectId}
            instanceId={instanceId}
            repositoryId={repositoryId}
            policy={policy}
          />
        </ReviewAuthoringProvider>
      </QueryClientProvider>,
    );
  }
  it("opens a local review with the exact selected provider file and line without provider hydration", async () => {
    vi.mocked(collaboration.transport.pullFileArtifact).mockImplementation(
      async (request) => ({
        ...artifactSnapshot(
          request,
          savedArtifact(request, "text", "@@ -1 +1 @@\n-old\n+new\n"),
        ),
        freshness: "fresh",
      }),
    );
    mountReviewFiles();
    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: "Files" }));
    await user.click(await screen.findByRole("button", { name: /src\/a.ts/ }));
    await user.type(await screen.findByLabelText("Line"), "1");
    await user.click(
      screen.getByRole("button", { name: "Add inline review comment" }),
    );
    expect(await screen.findByLabelText("Comment 1")).toHaveValue("");
    expect(screen.getByText("src/a.ts, right line 1")).toBeInTheDocument();
    expect(collaboration.transport.hydrateDetail).not.toHaveBeenCalled();
    expect(collaboration.transport.hydratePullFile).not.toHaveBeenCalled();
  });
  it.each([
    "stale",
    "local",
    "binary",
  ])("does not offer inline review authority for %s artifacts", async (kind) => {
    vi.mocked(collaboration.transport.pullFileArtifact).mockImplementation(
      async (request) => {
        const artifact = savedArtifact(
          request,
          kind === "binary" ? "binary" : "text",
          "@@ -1 +1 @@\n-old\n+new\n",
        );
        if (kind === "local")
          artifact.validation = {
            kind: "local_exact_range",
            local_validated_at: "2026-10-08T00:00:00Z",
            resolved_merge_base_oid: "a".repeat(40),
          };
        return {
          ...artifactSnapshot(request, artifact),
          freshness: kind === "stale" ? "stale" : "fresh",
        };
      },
    );
    mountReviewFiles();
    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: "Files" }));
    await user.click(await screen.findByRole("button", { name: /src\/a.ts/ }));
    await waitFor(() =>
      expect(collaboration.transport.pullFileArtifact).toHaveBeenCalled(),
    );
    expect(
      screen.queryByRole("button", { name: "Add inline review comment" }),
    ).not.toBeInTheDocument();
  });

  it.each([
    "renamed",
    "copied",
    "unknown",
    "unequal_paths",
  ] as const)("keeps the saved diff readable without assuming %s review anchors", async (kind) => {
    const file = structuredClone(files[0]);
    file.file.change_kind = kind === "unequal_paths" ? "modified" : kind;
    if (kind !== "unknown") file.file.identity.old_path = "old/a.ts";
    vi.mocked(collaboration.transport.pullFiles).mockResolvedValue({
      ...snapshot,
      files: [file],
    });
    vi.mocked(collaboration.transport.pullFileArtifact).mockImplementation(
      async (request) => {
        const artifact = savedArtifact(
          request,
          "text",
          "@@ -1 +1 @@\n-old\n+new\n",
        );
        artifact.identity = file.file.identity;
        const result = artifactSnapshot(request, artifact);
        result.membership.identity = file.file.identity;
        return { ...result, freshness: "fresh" };
      },
    );
    mountReviewFiles();
    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: "Files" }));
    await user.click(await screen.findByRole("button", { name: /src\/a.ts/ }));
    expect(await screen.findByTestId("safe-patch-diff")).toBeVisible();
    expect(
      screen.queryByRole("button", { name: "Add inline review comment" }),
    ).not.toBeInTheDocument();
    expect(collaboration.transport.hydratePullFile).not.toHaveBeenCalled();
  });
});

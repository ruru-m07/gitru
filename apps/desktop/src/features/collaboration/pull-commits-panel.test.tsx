import {
  type ContextFacetCapability,
  collaboration,
  type PullCommitSnapshot,
} from "@gitru/collaboration-client";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useAppStore } from "@/store/use-app-store";
import { fixtureAccount } from "../../../tests/fixtures/collaboration";
import { PullCommitsPanel } from "./pull-commits-panel";

vi.mock("@tauri-apps/plugin-store", () => ({
  Store: {
    load: async () => ({
      get: async () => null,
      set: async () => {},
      save: async () => {},
    }),
  },
}));

const subjectId = "github:pull:repository:67";
const instanceId = "github:https://github.com/";
const repositoryId = "github:repository:1";
const oid = "a".repeat(40);
const policy: ContextFacetCapability = {
  facet: "pull_commits",
  saved_read: { state: "supported", reason: null },
  synchronize: { state: "unsupported", reason: "not_implemented" },
  remote_write: { state: "unsupported", reason: "not_implemented" },
  observation: "complete",
  sync: {
    state: "offline",
    last_success_at: "2026-10-07T00:00:00Z",
    next_retry_at: null,
    error: null,
  },
  can_recheck_access: false,
};
const snapshot: PullCommitSnapshot = {
  subject_id: subjectId,
  context: {
    base_oid: "b".repeat(40),
    head_oid: oid,
    source_repository_provider_id: "source-1",
    metadata_facet_revision: "10",
  },
  commits: [
    {
      oid,
      position: 0,
      summary: "Keep the cached range exact",
      message: { state: "known", text: "Keep the cached range exact" },
      author: { name: "Example Author", provider: null },
      committer: { name: "Example Committer", provider: null },
      authored_at: "2026-10-06T00:00:00Z",
      committed_at: "2026-10-07T00:00:00Z",
      parent_oids: ["b".repeat(40)],
      web_url:
        "https://github.com/example/project/commit/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    },
  ],
  next_cursor: null,
  completeness: { state: "complete", reason: null },
  coverage: {
    state: "complete",
    validated_at: "2026-10-07T00:00:00Z",
    remote_has_more: false,
  },
  sync: policy.sync,
  freshness: "stale",
  facet_revision: "11",
  revision: "12",
  authorization_view: "3",
};

let cache: QueryClient;
beforeEach(() => {
  cache = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  vi.spyOn(collaboration.transport, "pullCommits").mockResolvedValue(snapshot);
  vi.spyOn(collaboration.transport, "hydrateDetail").mockRejectedValue(
    new Error("unexpected hydration"),
  );
});
afterEach(() => {
  cache.clear();
  vi.restoreAllMocks();
  useAppStore.setState({ repositories: [] });
});

function mount() {
  render(
    <QueryClientProvider client={cache}>
      <PullCommitsPanel
        account={fixtureAccount}
        subjectId={subjectId}
        instanceId={instanceId}
        repositoryId={repositoryId}
        policy={policy}
      />
    </QueryClientProvider>,
  );
}

describe("cached pull request commits", () => {
  it("reveals the saved ordered range offline without hydrating the provider", async () => {
    const user = userEvent.setup();
    mount();

    expect(screen.queryByText("Keep the cached range exact")).toBeNull();
    await user.click(screen.getByRole("button", { name: "Commits" }));

    expect(
      await screen.findByText("Keep the cached range exact"),
    ).toBeVisible();
    expect(
      within(
        screen.getByRole("list", { name: "Saved pull request commits" }),
      ).getByText(oid.slice(0, 12)),
    ).toBeVisible();
    expect(screen.getByText("Example Author")).toBeVisible();
    expect(screen.getByText("Saved data may be stale")).toBeVisible();
    expect(collaboration.transport.pullCommits).toHaveBeenCalledExactlyOnceWith(
      {
        account_id: fixtureAccount.id,
        subject_id: subjectId,
        cursor: null,
        limit: 50,
      },
    );
    expect(collaboration.transport.hydrateDetail).not.toHaveBeenCalled();
  });

  it("shows the exact cached source and range context accessibly", async () => {
    const user = userEvent.setup();
    mount();
    await user.click(screen.getByRole("button", { name: "Commits" }));

    const context = await screen.findByRole("region", {
      name: "Saved commit context",
    });
    expect(within(context).getByText("source-1")).toBeVisible();
    expect(within(context).getByText("10")).toBeVisible();

    const baseOid = "b".repeat(40);
    const base = within(context).getByLabelText(`Base OID: ${baseOid}`);
    expect(base).toHaveTextContent(baseOid.slice(0, 12));
    expect(base).toHaveAttribute("title", baseOid);

    const head = within(context).getByLabelText(`Head OID: ${oid}`);
    expect(head).toHaveTextContent(oid.slice(0, 12));
    expect(head).toHaveAttribute("title", oid);
  });

  it("does not invent cached context when the snapshot has none", async () => {
    vi.mocked(collaboration.transport.pullCommits).mockResolvedValueOnce({
      ...snapshot,
      context: null,
    });
    const user = userEvent.setup();
    mount();
    await user.click(screen.getByRole("button", { name: "Commits" }));

    expect(
      await screen.findByText("Keep the cached range exact"),
    ).toBeVisible();
    expect(
      screen.queryByRole("region", { name: "Saved commit context" }),
    ).toBeNull();
  });

  it("binds local navigation to the saved membership and linked clone IDs", async () => {
    const localClones = vi
      .spyOn(collaboration.transport, "localClones")
      .mockResolvedValue({
        clones: [
          {
            local_repository_id: "11111111-1111-4111-8111-111111111111",
            local_repository_name: "Local project",
            link_id: "link-1",
            generation: "9",
            state: "linked",
          },
        ],
      });
    const open = vi
      .spyOn(collaboration.transport, "openLocalPullCommit")
      .mockRejectedValue({
        code: "not_found",
        message: "private native detail",
      });
    const user = userEvent.setup();
    mount();
    await user.click(screen.getByRole("button", { name: "Commits" }));
    await user.click(
      await screen.findByRole("button", { name: "Open locally" }),
    );
    await user.click(
      await screen.findByRole("button", { name: `Open ${oid.slice(0, 12)}` }),
    );

    expect(localClones).toHaveBeenCalledExactlyOnceWith({
      account_id: fixtureAccount.id,
      authorization_epoch: fixtureAccount.authorization_epoch,
      instance_id: instanceId,
      repository_id: repositoryId,
      source_repository_provider_id: "source-1",
    });
    await waitFor(() =>
      expect(open).toHaveBeenCalledExactlyOnceWith({
        account_id: fixtureAccount.id,
        authorization_epoch: fixtureAccount.authorization_epoch,
        subject_id: subjectId,
        commit_oid: oid,
        facet_revision: "11",
        local_repository_id: "11111111-1111-4111-8111-111111111111",
        link_id: "link-1",
        link_generation: "9",
      }),
    );
    expect(screen.queryByText(/private native detail/)).toBeNull();
    expect(
      await screen.findByText(/not available in this clone/),
    ).toBeVisible();
  });
});

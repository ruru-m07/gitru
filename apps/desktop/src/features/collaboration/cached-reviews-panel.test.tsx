import { collaboration } from "@gitru/collaboration-client";
import type {
  ContextFacetCapability,
  DetailEntry,
  DetailSnapshot,
} from "@gitru/commands";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { fixtureAccount } from "../../../tests/fixtures/collaboration";
import { fixtureBody } from "../../../tests/fixtures/resource-detail";
import { mockForegroundDemand } from "../../../tests/mocks/collaboration-demand";
import {
  CachedReviewsPanel,
  type PullReviewContext,
} from "./cached-reviews-panel";

const baseOid = "b".repeat(40);
const headOid = "a".repeat(40);
const oldOid = "c".repeat(40);
const subjectId = fixtureBody().subject_id;
const authorizationView = fixtureBody().authorization_view;
const context: PullReviewContext = {
  baseOid,
  headOid,
  baseRepositoryProviderId: "target-1",
  sourceRepositoryProviderId: "source-1",
  metadataFacetRevision: "10",
  facetRevision: "10",
};
const nativeContext = {
  base_oid: baseOid,
  head_oid: headOid,
  base_repository_provider_id: "target-1",
  source_repository_provider_id: "source-1",
  metadata_facet_revision: "10",
};
const policy: ContextFacetCapability = {
  facet: "reviews",
  saved_read: { state: "supported", reason: null },
  synchronize: { state: "supported", reason: null },
  remote_write: { state: "unsupported", reason: "not_implemented" },
  observation: "partial",
  sync: {
    state: "idle",
    last_success_at: null,
    next_retry_at: null,
    error: null,
  },
  can_recheck_access: false,
};

function review(id: string, reviewedCommit: string | null): DetailEntry {
  return {
    id: `review-${id}`,
    provider_id: id,
    author: "alice",
    title: null,
    state: "APPROVED",
    body: { state: "known", text: "<img src=x onerror=alert(1)>" },
    observed_body_state: "known",
    updated_at: "2026-10-08T00:00:00Z",
    head_oid: headOid,
    native: {
      kind: "review.v1",
      value: {
        context: nativeContext,
        reviewer: {
          provider_id: "8",
          login: "alice",
          display_name: "Alice",
        },
        decision: "approved",
        provider_state: "APPROVED",
        reviewed_commit_oid: reviewedCommit,
        submitted_at: "2026-10-08T00:00:00Z",
      },
    },
    field_mask: ["body", "author", "state", "updated_at", "head_oid", "review"],
    field_validations: [],
  };
}

function unknownReview(id: string): DetailEntry {
  const entry = review(id, null);
  if (entry.native?.kind !== "review.v1") throw new Error("fixture");
  entry.native.value.decision = "unknown";
  entry.native.value.provider_state = "FUTURE_PROVIDER_DECISION";
  return entry;
}

function thread(id: string, commit: string | null = headOid): DetailEntry {
  return {
    id: `thread-${id}`,
    provider_id: id,
    author: "bob",
    title: null,
    state: null,
    body: { state: "known", text: `Comment ${id}` },
    observed_body_state: "known",
    updated_at: "2026-10-08T01:00:00Z",
    head_oid: headOid,
    native: {
      kind: "review_thread.v1",
      value: {
        context: nativeContext,
        thread_id: id,
        root_comment_id: id,
        comment_id: id,
        parent_comment_id: null,
        review_id: null,
        author: { provider_id: "9", login: "bob", display_name: null },
        created_at: "2026-10-08T00:30:00Z",
        updated_at: "2026-10-08T01:00:00Z",
        anchor: {
          path: "src/<literal>.ts",
          // A retained legacy/provider payload can lack this evidence even
          // though the current native contract requires a canonical OID.
          commit_oid: commit as string,
          original_commit_oid: baseOid,
          subject: "line",
          start_line: null,
          line: 7,
          start_side: null,
          side: "right",
        },
        provider_outdated: null,
        provider_resolved: null,
      },
    },
    field_mask: ["body", "author", "updated_at", "head_oid", "review_thread"],
    field_validations: [],
  };
}

function snapshot(
  facet: "review_summaries" | "review_threads",
  entries: DetailEntry[],
  nextCursor: string | null = null,
): DetailSnapshot {
  return fixtureBody({
    body: { state: "not_loaded", text: null },
    metadata: null,
    entries,
    next_cursor: nextCursor,
    evidence: {
      ...fixtureBody().evidence,
      facet,
      availability: "ready",
      freshness: "fresh",
      facet_revision: facet === "review_summaries" ? "21" : "31",
      coverage: {
        state: nextCursor ? "partial" : "complete",
        validated_at: "2026-10-08T01:00:00Z",
        remote_has_more: nextCursor !== null,
      },
    },
  });
}

function mount(withBridge = false) {
  const cache = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  let stopBridge = () => {};
  if (withBridge) {
    vi.spyOn(collaboration.transport, "listen").mockResolvedValue(() => {});
    vi.spyOn(collaboration.transport, "changesSince").mockResolvedValue({
      revision: "10",
      authorization_view: "1",
      changes: [],
      has_more: false,
      reset_required: false,
    });
    stopBridge = collaboration.installBridge(cache);
  }
  const user = userEvent.setup();
  const view = render(
    <QueryClientProvider client={cache}>
      <CachedReviewsPanel
        account={fixtureAccount}
        subjectId={subjectId}
        authorizationView={authorizationView}
        policy={policy}
        bodyContext={context}
      />
    </QueryClientProvider>,
  );
  return { cache, stopBridge, user, view };
}

afterEach(() => vi.restoreAllMocks());

describe("cached review disclosure", () => {
  it("does no work while closed, reads both local facets when opened, and syncs only on explicit action", async () => {
    const detail = vi
      .spyOn(collaboration.transport, "detail")
      .mockImplementation(async (query) =>
        query.facet === "review_summaries"
          ? snapshot("review_summaries", [
              review("1", headOid),
              unknownReview("2"),
            ])
          : snapshot("review_threads", [
              thread("10"),
              thread("11", oldOid),
              thread("12", null),
            ]),
      );
    const hydrate = vi
      .spyOn(collaboration.transport, "hydrateDetail")
      .mockResolvedValue({ job_id: "review-job" });
    const demand = mockForegroundDemand();
    const { cache, stopBridge, user, view } = mount(true);

    expect(detail).not.toHaveBeenCalled();
    expect(hydrate).not.toHaveBeenCalled();
    expect(demand.acquire).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "Reviews" }));

    expect(await screen.findByText("Current commit")).toBeVisible();
    expect(screen.getByText("Commit unknown")).toBeVisible();
    expect(
      screen.getByText("Provider state: FUTURE_PROVIDER_DECISION"),
    ).toBeVisible();
    expect(screen.getByText("Current anchor")).toBeVisible();
    expect(screen.getByText("Historical anchor")).toBeVisible();
    expect(screen.getByText("Anchor commit unknown")).toBeVisible();
    expect(
      screen
        .getAllByText("<img src=x onerror=alert(1)>")
        .every((node) => node.isConnected),
    ).toBe(true);
    expect(document.querySelector("img[src='x']")).toBeNull();
    await waitFor(() => expect(demand.acquire).toHaveBeenCalledTimes(2));
    expect(hydrate).not.toHaveBeenCalled();

    await user.click(screen.getByRole("button", { name: "Sync reviews" }));
    expect(hydrate).toHaveBeenCalledTimes(2);
    expect(hydrate).toHaveBeenCalledWith({
      account_id: fixtureAccount.id,
      authorization_epoch: fixtureAccount.authorization_epoch,
      subject_id: subjectId,
      facet: "review_summaries",
    });
    expect(hydrate).toHaveBeenCalledWith({
      account_id: fixtureAccount.id,
      authorization_epoch: fixtureAccount.authorization_epoch,
      subject_id: subjectId,
      facet: "review_threads",
    });

    await user.click(screen.getByRole("button", { name: "Reviews" }));
    await waitFor(() => expect(demand.release).toHaveBeenCalledTimes(2));
    view.unmount();
    stopBridge();
    cache.clear();
  });

  it("pages summaries and threads independently and fences a foreign saved page", async () => {
    const calls: Array<{ facet: string; cursor: string | null }> = [];
    vi.spyOn(collaboration.transport, "detail").mockImplementation(
      async (query) => {
        calls.push({ facet: query.facet, cursor: query.cursor });
        if (query.facet === "review_summaries") {
          if (query.cursor === "summary-2") {
            const foreign = snapshot("review_summaries", [review("3", oldOid)]);
            return { ...foreign, authorization_view: "foreign-view" };
          }
          return snapshot(
            "review_summaries",
            [review("1", headOid)],
            "summary-2",
          );
        }
        if (query.cursor === "thread-2")
          return snapshot("review_threads", [thread("12")]);
        return snapshot("review_threads", [thread("10")], "thread-2");
      },
    );
    vi.spyOn(collaboration.transport, "hydrateDetail").mockResolvedValue({
      job_id: "unused",
    });
    mockForegroundDemand();
    const { cache, user, view } = mount();
    await user.click(screen.getByRole("button", { name: "Reviews" }));
    const decisions = await screen.findByRole("region", {
      name: "Review decisions",
    });
    const discussions = screen.getByRole("region", {
      name: "Inline discussion",
    });
    await user.click(within(discussions).getByRole("button", { name: "Next" }));
    expect(await within(discussions).findByText("Comment 12")).toBeVisible();
    expect(within(decisions).queryByText("Comment 12")).not.toBeInTheDocument();
    await user.click(within(decisions).getByRole("button", { name: "Next" }));
    expect(
      await within(decisions).findByText(
        "Account access changed. Reload this view.",
      ),
    ).toBeVisible();
    expect(calls).toContainEqual({
      facet: "review_summaries",
      cursor: "summary-2",
    });
    expect(calls).toContainEqual({
      facet: "review_threads",
      cursor: "thread-2",
    });
    view.unmount();
    cache.clear();
  });
});

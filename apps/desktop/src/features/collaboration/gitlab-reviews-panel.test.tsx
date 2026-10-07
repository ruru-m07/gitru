import { collaboration } from "@gitru/collaboration-client";
import type {
  ContextFacetCapability,
  DetailEntry,
  RemoteAccount,
} from "@gitru/commands";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, expect, it, vi } from "vitest";
import { fixtureAccount } from "../../../tests/fixtures/collaboration";
import { fixtureBody } from "../../../tests/fixtures/resource-detail";
import { mockForegroundDemand } from "../../../tests/mocks/collaboration-demand";
import { CachedReviewsPanel } from "./cached-reviews-panel";

const head = "a".repeat(40);
const context = {
  base_oid: "b".repeat(40),
  head_oid: head,
  base_repository_provider_id: "123",
  source_repository_provider_id: "456",
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
const approval: DetailEntry = {
  id: "gitlab-approval:42",
  provider_id: "approval:42",
  author: "approver",
  title: null,
  state: "approved",
  body: { state: "omitted", text: null },
  observed_body_state: "omitted",
  updated_at: null,
  head_oid: head,
  native: {
    kind: "review.v1",
    value: {
      context,
      reviewer: { provider_id: "42", login: "approver", display_name: null },
      decision: "approved",
      provider_state: "approved",
      reviewed_commit_oid: null,
      submitted_at: null,
    },
  },
  field_mask: ["body", "author", "state", "updated_at", "head_oid", "review"],
  field_validations: [],
};
const thread: DetailEntry = {
  id: "gitlab-discussion:thread:9",
  provider_id: "9",
  author: "reviewer",
  title: null,
  state: null,
  body: { state: "known", text: "A saved GitLab note" },
  observed_body_state: "known",
  updated_at: "2026-10-08T00:00:00Z",
  head_oid: head,
  native: {
    kind: "review_thread.v1",
    value: {
      context,
      thread_id: "native-thread",
      root_comment_id: null,
      comment_id: "9",
      parent_comment_id: null,
      review_id: null,
      author: { provider_id: "43", login: "reviewer", display_name: null },
      created_at: "2026-10-08T00:00:00Z",
      updated_at: "2026-10-08T00:00:00Z",
      anchor: null,
      provider_outdated: null,
      provider_resolved: true,
      native: {
        provider: "gitlab",
        value: {
          note_type: "DiffNote",
          system: false,
          individual_note: false,
          resolvable: true,
          resolved_at: null,
          resolved_by: null,
          observed_note_count: 51,
          retained_note_count: 50,
          position: {
            position_type: "text",
            base_oid: "d".repeat(40),
            start_oid: "b".repeat(40),
            head_oid: "c".repeat(40),
            old_path: "before.rs",
            new_path: "after.rs",
            old_line: null,
            new_line: 4,
            line_range: null,
            width: null,
            height: null,
            x: null,
            y: null,
          },
        },
      },
    },
  },
  field_mask: ["body", "author", "updated_at", "head_oid", "review_thread"],
  field_validations: [],
};
afterEach(() => vi.restoreAllMocks());
it("reads GitLab approvers and native discussions locally with unknown approval commits and partial coverage", async () => {
  const account: RemoteAccount = {
    ...fixtureAccount,
    provider: "gitlab",
    host: "gitlab.com",
    actor_id: "42",
  };
  const body = fixtureBody();
  const detail = vi
    .spyOn(collaboration.transport, "detail")
    .mockImplementation(async (query) =>
      fixtureBody({
        body: { state: "not_loaded", text: null },
        metadata: null,
        entries: query.facet === "review_summaries" ? [approval] : [thread],
        evidence: {
          ...body.evidence,
          facet: query.facet,
          availability: query.facet === "review_threads" ? "partial" : "ready",
          freshness: "stale",
          facet_revision: query.facet === "review_summaries" ? "21" : "31",
          coverage: {
            state: query.facet === "review_threads" ? "partial" : "complete",
            validated_at: null,
            remote_has_more: false,
          },
        },
      }),
    );
  const hydrate = vi
    .spyOn(collaboration.transport, "hydrateDetail")
    .mockResolvedValue({ job_id: "manual-only" });
  const demand = mockForegroundDemand();
  const cache = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  vi.spyOn(collaboration.transport, "listen").mockResolvedValue(() => {});
  vi.spyOn(collaboration.transport, "changesSince").mockResolvedValue({
    revision: "10",
    authorization_view: "1",
    changes: [],
    has_more: false,
    reset_required: false,
  });
  const stopBridge = collaboration.installBridge(cache);
  const user = userEvent.setup();
  const view = render(
    <QueryClientProvider client={cache}>
      <CachedReviewsPanel
        account={account}
        subjectId={body.subject_id}
        authorizationView={body.authorization_view}
        policy={policy}
        bodyContext={{
          baseOid: context.base_oid,
          headOid: head,
          baseRepositoryProviderId: "123",
          sourceRepositoryProviderId: "456",
          metadataFacetRevision: "10",
          facetRevision: "10",
        }}
      />
    </QueryClientProvider>,
  );
  expect(detail).not.toHaveBeenCalled();
  await user.click(screen.getByRole("button", { name: "Reviews" }));
  expect(await screen.findByText("Commit unknown")).toBeVisible();
  expect(screen.getByText("GitLab diff position")).toBeVisible();
  expect(screen.getByText("Provider resolved")).toBeVisible();
  expect(screen.getByText("Partial provider history")).toBeVisible();
  expect(screen.getByText(/Saved 50 of 51 observed notes/)).toBeVisible();
  expect(screen.getByTitle("c".repeat(40))).toHaveTextContent("c".repeat(12));
  expect(screen.queryByText("Current commit")).toBeNull();
  expect(screen.queryByText("Current anchor")).toBeNull();
  expect(
    screen.queryByRole("button", { name: /approve|resolve discussion/i }),
  ).toBeNull();
  expect(hydrate).not.toHaveBeenCalled();
  expect(detail).toHaveBeenCalledTimes(2);
  await waitFor(() => expect(demand.acquire).toHaveBeenCalledTimes(2));
  view.unmount();
  stopBridge();
  cache.clear();
  await waitFor(() => expect(demand.release).toHaveBeenCalledTimes(2));
});

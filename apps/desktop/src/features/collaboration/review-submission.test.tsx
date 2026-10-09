import {
  collaboration,
  type ReviewDraftSnapshot,
  type SaveReviewDraftRequest,
  type SubmitReviewRequest,
} from "@gitru/collaboration-client";
import { reviewDraftQueryOptions } from "@gitru/collaboration-client/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { fixtureAccount as account } from "../../../tests/fixtures/collaboration";
import { mockTauriCommand } from "../../../tests/mocks/tauri";
import { DraftRecovery } from "./draft-recovery";
import { ResourceCapabilityPanels } from "./resource-capability-panels";
import {
  ReviewAuthoringProvider,
  ReviewComposerTrigger,
  useInlineReviewAuthoring,
} from "./review-submission";

const subjectId = "review-pull";
const key = { account_id: account.id, subject_id: subjectId };
const context = {
  ...key,
  authorization_epoch: account.authorization_epoch,
  authorization_view: "1",
  review_token: "c".repeat(64),
  review_context: {
    base_oid: "a".repeat(40),
    head_oid: "b".repeat(40),
    base_repository_provider_id: "77",
    source_repository_provider_id: "77",
    metadata_facet_revision: "8",
  },
};
const anchor = {
  file_facet_revision: "7",
  context: {
    base_oid: "a".repeat(40),
    head_oid: "b".repeat(40),
    merge_base_oid: null,
    base_repository_provider_id: "77",
    source_repository_provider_id: "77",
    body_metadata_facet_revision: "8",
  },
  file_key: "opaque-file-key",
  line: 17,
  side: "right" as const,
  start_line: null,
  start_side: null,
};
let snapshot: ReviewDraftSnapshot;
const caches: QueryClient[] = [];
const stops: Array<() => void> = [];
beforeEach(() => {
  const matches = Element.prototype.matches;
  vi.spyOn(Element.prototype, "matches").mockImplementation(function (
    this: Element,
    selector: string,
  ) {
    return [":modal", ":fullscreen", ":popover-open"].includes(selector)
      ? false
      : matches.call(this, selector);
  });
  snapshot = {
    key,
    event: "comment",
    body: "Saved review",
    comments: [],
    generation: "1",
    context,
    availability: "available",
    reason: null,
    submission: null,
    revision: "10",
    authorization_view: "1",
  };
  mockTauriCommand("collaboration_review_draft", () => snapshot);
  mockTauriCommand("collaboration_review_drafts", () => ({
    account_id: account.id,
    drafts: [],
    next_cursor: null,
    revision: "10",
    authorization_view: "1",
  }));
  mockTauriCommand("collaboration_submitted_reviews", () => ({
    ...key,
    reviews: [],
    next_cursor: null,
    revision: "10",
    authorization_view: "1",
  }));
});
afterEach(() => {
  for (const stop of stops.splice(0)) stop();
  for (const cache of caches.splice(0)) cache.clear();
  vi.restoreAllMocks();
});
function InlineTrigger() {
  const authoring = useInlineReviewAuthoring();
  return (
    <button
      type="button"
      onClick={() =>
        authoring?.add({ anchor, location: "src/private.ts, right line 17" })
      }
    >
      Fixture inline selection
    </button>
  );
}
function setup(element?: React.ReactNode) {
  const cache = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  caches.push(cache);
  const view = render(
    <QueryClientProvider client={cache}>
      {element ?? (
        <ReviewAuthoringProvider account={account} subjectId={subjectId}>
          <ReviewComposerTrigger />
          <InlineTrigger />
        </ReviewAuthoringProvider>
      )}
    </QueryClientProvider>,
  );
  return { cache, ...view, user: userEvent.setup() };
}
async function open(user: ReturnType<typeof userEvent.setup>) {
  await user.click(screen.getByRole("button", { name: "Write a review" }));
  await screen.findByLabelText("Review summary");
}
async function consent(user: ReturnType<typeof userEvent.setup>) {
  await user.click(
    screen.getByRole("checkbox", { name: /Queue this saved review/ }),
  );
  await user.click(
    screen.getByRole("checkbox", { name: /force-push can race/ }),
  );
}
function saveHandler() {
  return mockTauriCommand("collaboration_save_review_draft", (payload) => {
    const request = (payload as { request: SaveReviewDraftRequest }).request;
    snapshot = {
      ...snapshot,
      event: request.event,
      body: request.body,
      generation: String(Number(snapshot.generation) + 1),
      revision: "11",
      comments: request.comments.map((comment) => ({
        comment_id: comment.comment_id,
        body: comment.body,
        anchor: {
          provider: "github",
          anchor: { ...comment.anchor, path: "src/private.ts" },
        },
      })),
    };
    return snapshot;
  });
}

it("opens and saves locally and retains unsaved review text after close", async () => {
  const save = saveHandler();
  const submit = mockTauriCommand("collaboration_submit_review", () => {
    throw Error("must not submit");
  });
  const hydrate = mockTauriCommand("collaboration_hydrate_detail", () => {
    throw Error("must not hydrate");
  });
  const { user } = setup();
  await open(user);
  await user.type(screen.getByLabelText("Review summary"), " edited");
  await user.click(screen.getByRole("button", { name: "Close" }));
  await waitFor(() =>
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument(),
  );
  await open(user);
  expect(screen.getByLabelText("Review summary")).toHaveValue(
    "Saved review edited",
  );
  await user.click(screen.getByRole("button", { name: "Save review draft" }));
  await screen.findByText("Review draft saved on this device.");
  expect(save).toHaveBeenCalledOnce();
  expect(submit).not.toHaveBeenCalled();
  expect(hydrate).not.toHaveBeenCalled();
});
it("requires both consents and recovers the exact UUID after a lost local receipt", async () => {
  const requests: SubmitReviewRequest[] = [];
  const submit = mockTauriCommand("collaboration_submit_review", (payload) => {
    const request = (payload as { request: SubmitReviewRequest }).request;
    requests.push(request);
    if (requests.length === 1)
      throw { code: "storage_unavailable", message: "lost local receipt" };
    return {
      account_id: account.id,
      command_id: request.command_id,
      admitted_revision: "11",
      duplicate: true,
    };
  });
  const { user } = setup();
  await open(user);
  expect(
    screen.getByRole("button", { name: "Submit saved review" }),
  ).toBeDisabled();
  await user.click(
    screen.getByRole("checkbox", { name: /Queue this saved review/ }),
  );
  expect(
    screen.getByRole("button", { name: "Submit saved review" }),
  ).toBeDisabled();
  await user.click(
    screen.getByRole("checkbox", { name: /force-push can race/ }),
  );
  await user.click(screen.getByRole("button", { name: "Submit saved review" }));
  await user.click(
    await screen.findByRole("button", { name: "Recover review receipt" }),
  );
  await screen.findByText(
    "This exact review request is already recorded locally.",
  );
  expect(submit).toHaveBeenCalledTimes(2);
  expect(requests[1]).toEqual(requests[0]);
  expect(requests[0].context.review_context.head_oid).toBe(
    context.review_context.head_oid,
  );
  expect(
    screen.getByRole("button", { name: "Submit saved review" }),
  ).toBeDisabled();
});
it("a changed head retires consent without replacing authored text", async () => {
  const { cache, user } = setup();
  await open(user);
  await consent(user);
  expect(
    screen.getByRole("button", { name: "Submit saved review" }),
  ).toBeEnabled();
  act(() =>
    cache.setQueryData(reviewDraftQueryOptions(account, subjectId).queryKey, {
      ...snapshot,
      context: {
        ...context,
        review_context: { ...context.review_context, head_oid: "d".repeat(40) },
      },
    }),
  );
  await waitFor(() =>
    expect(
      screen.getByRole("button", { name: "Submit saved review" }),
    ).toBeDisabled(),
  );
  expect(screen.getByLabelText("Review summary")).toHaveValue(snapshot.body);
});
it("cross-window CAS keeps unsaved text and requires explicit latest load", async () => {
  const { cache, user } = setup();
  await open(user);
  await user.type(screen.getByLabelText("Review summary"), " mine");
  act(() =>
    cache.setQueryData(reviewDraftQueryOptions(account, subjectId).queryKey, {
      ...snapshot,
      generation: "2",
      body: "Other window",
    }),
  );
  await user.click(
    await screen.findByRole("button", { name: "Load latest saved review" }),
  );
  expect(screen.getByLabelText("Review summary")).toHaveValue("Other window");
  expect(screen.getByText("Saved review mine")).toBeInTheDocument();
});
it("adds a selected inline comment but sends no trusted path through save IPC", async () => {
  const save = saveHandler();
  const { user } = setup();
  await user.click(
    screen.getByRole("button", { name: "Fixture inline selection" }),
  );
  await user.type(
    await screen.findByLabelText("Comment 1"),
    "Explain this change",
  );
  await user.click(screen.getByRole("button", { name: "Save review draft" }));
  await screen.findByText("Review draft saved on this device.");
  const request = (save.mock.calls[0][0] as { request: SaveReviewDraftRequest })
    .request;
  expect(request.comments[0].anchor).toEqual(anchor);
  expect(request.comments[0].anchor).not.toHaveProperty("path");
});
it("retains disconnected inline text but offers no path or submission authority", async () => {
  const disconnected = { ...account, state: "disconnected" as const };
  snapshot = {
    ...snapshot,
    context: null,
    availability: "unavailable",
    reason: "account_unavailable",
    comments: [
      {
        comment_id: "123e4567-e89b-42d3-a456-426614174000",
        body: "Keep inline text",
        anchor: null,
      },
    ],
  };
  const { user } = setup(
    <ReviewAuthoringProvider account={disconnected} subjectId={subjectId}>
      <ReviewComposerTrigger />
    </ReviewAuthoringProvider>,
  );
  await open(user);
  expect(screen.getByLabelText("Comment 1")).toHaveValue("Keep inline text");
  expect(
    screen.getByRole("button", { name: "Submit saved review" }),
  ).toBeDisabled();
  expect(screen.getByText(/Diff selection unavailable/)).toBeInTheDocument();
});
it("shows accepted and confirmed receipts separately from provider review coverage", async () => {
  mockTauriCommand("collaboration_submitted_reviews", () => ({
    ...key,
    reviews: [false, true].map((confirmed, index) => ({
      command_id: String(index),
      draft_generation: "1",
      provider_id: String(index + 1),
      url: `https://github.com/o/r/pull/1#pullrequestreview-${index + 1}`,
      event: "approve",
      body: "",
      provider_state: "APPROVED",
      reviewed_commit_oid: "a".repeat(40),
      submitted_at: "2026-10-08T00:00:00Z",
      observed_at: "2026-10-08T00:00:01Z",
      inline_comment_count: 0,
      confirmed,
    })),
    next_cursor: null,
    revision: "10",
    authorization_view: "1",
  }));
  const { user } = setup();
  await open(user);
  expect(
    await screen.findByText(/Accepted; verification pending/),
  ).toBeInTheDocument();
  expect(screen.getByText(/^Confirmed ·/)).toBeInTheDocument();
});
it("finds disconnected reviews through the paginated authored recovery index", async () => {
  snapshot = {
    ...snapshot,
    context: null,
    availability: "unavailable",
    reason: "account_unavailable",
  };
  const list = mockTauriCommand("collaboration_review_drafts", (payload) => {
    const cursor = (payload as { query: { cursor: string | null } }).query
      .cursor;
    return {
      account_id: account.id,
      drafts: cursor
        ? [
            {
              subject_id: subjectId,
              event: "comment",
              preview: snapshot.body,
              inline_comment_count: 0,
              generation: "1",
              submission: null,
            },
          ]
        : [],
      next_cursor: cursor ? null : "next-review",
      revision: "10",
      authorization_view: "1",
    };
  });
  const { user } = setup(
    <DraftRecovery accounts={[{ ...account, state: "disconnected" }]} />,
  );
  await user.click(screen.getByRole("button", { name: "Review drafts" }));
  await user.click(
    await screen.findByRole("button", { name: "Next review drafts" }),
  );
  await user.click(
    await screen.findByRole("button", {
      name: "Open review draft Saved review",
    }),
  );
  expect(await screen.findByLabelText("Review summary")).toHaveValue(
    snapshot.body,
  );
  await user.type(screen.getByLabelText("Review summary"), " offline");
  expect(
    screen.getByRole("button", { name: "Save review draft" }),
  ).toBeEnabled();
  expect(list).toHaveBeenLastCalledWith({
    query: { account_id: account.id, cursor: "next-review", limit: 50 },
  });
});

it("runtime retirement disables exact receipt retry immediately and preserves the summary", async () => {
  let reset: (() => void) | undefined;
  vi.spyOn(collaboration.transport, "listen").mockResolvedValue(() => {});
  vi.spyOn(collaboration.transport, "listenLocalChanges").mockResolvedValue(
    () => {},
  );
  vi.spyOn(collaboration.transport, "listenRuntimeReset").mockImplementation(
    async (callback) => {
      reset = callback;
      return () => {};
    },
  );
  mockTauriCommand("collaboration_changes_since", () => ({
    revision: "10",
    authorization_view: "1",
    changes: [],
    has_more: false,
    reset_required: false,
  }));
  const submit = mockTauriCommand("collaboration_submit_review", () => {
    throw { code: "not_ready" };
  });
  const { cache, user } = setup();
  stops.push(collaboration.installBridge(cache));
  await act(async () => {
    await collaboration.wake();
  });
  await open(user);
  await consent(user);
  await user.click(screen.getByRole("button", { name: "Submit saved review" }));
  const retry = await screen.findByRole("button", {
    name: "Recover review receipt",
  });
  await waitFor(() => expect(retry).toBeEnabled());
  act(() => reset?.());
  expect(retry).toBeDisabled();
  fireEvent.click(retry);
  expect(submit).toHaveBeenCalledOnce();
  expect(screen.getByLabelText("Review summary")).toHaveValue("Saved review");
  await act(async () => {
    await collaboration.wake();
  });
});
it("a local query error retires inline anchor display and consent while preserving unsaved bodies", async () => {
  snapshot = {
    ...snapshot,
    comments: [
      {
        comment_id: "123e4567-e89b-42d3-a456-426614174000",
        body: "My inline text",
        anchor: {
          provider: "github",
          anchor: { ...anchor, path: "private-path.ts" },
        },
      },
    ],
  };
  const { cache, user } = setup();
  await open(user);
  await user.type(screen.getByLabelText("Review summary"), " unsaved");
  expect(screen.getByText(/private-path.ts/)).toBeInTheDocument();
  mockTauriCommand("collaboration_review_draft", () => {
    throw { code: "storage_unavailable" };
  });
  await act(async () => {
    await cache.invalidateQueries({
      queryKey: reviewDraftQueryOptions(account, subjectId).queryKey,
    });
  });
  await waitFor(() =>
    expect(screen.queryByText(/private-path.ts/)).not.toBeInTheDocument(),
  );
  expect(screen.getByLabelText("Comment 1")).toHaveValue("My inline text");
  expect(screen.getByLabelText("Review summary")).toHaveValue(
    "Saved review unsaved",
  );
  expect(
    screen.getByRole("button", { name: "Submit saved review" }),
  ).toBeDisabled();
});
it("edit then revert still requires new consent", async () => {
  const { user } = setup();
  await open(user);
  await consent(user);
  fireEvent.change(screen.getByLabelText("Review summary"), {
    target: { value: "Changed" },
  });
  fireEvent.change(screen.getByLabelText("Review summary"), {
    target: { value: "Saved review" },
  });
  expect(
    screen.getByRole("button", { name: "Submit saved review" }),
  ).toBeDisabled();
  expect(
    screen.getByRole("checkbox", { name: /Queue this saved review/ }),
  ).not.toBeChecked();
});

it("the real parent preserves unsaved review text across account epoch changes", async () => {
  snapshot = {
    ...snapshot,
    comments: [
      {
        comment_id: "123e4567-e89b-42d3-a456-426614174000",
        body: "Private inline body",
        anchor: {
          provider: "github",
          anchor: { ...anchor, path: "private-parent-path.ts" },
        },
      },
    ],
  };
  const props = {
    account,
    subjectId,
    kind: "pull_request" as const,
    snapshot: undefined,
    instanceId: "github-public",
    repositoryId: "77",
    bodyContext: null,
  };
  const { cache, user, rerender } = setup(
    <ResourceCapabilityPanels {...props} />,
  );
  await open(user);
  await user.type(screen.getByLabelText("Review summary"), " unsaved");
  await user.type(screen.getByLabelText("Comment 1"), " unsaved inline");
  rerender(
    <QueryClientProvider client={cache}>
      <ResourceCapabilityPanels
        {...props}
        account={{
          ...account,
          authorization_epoch: String(Number(account.authorization_epoch) + 1),
        }}
      />
    </QueryClientProvider>,
  );
  expect(screen.getByLabelText("Review summary")).toHaveValue(
    "Saved review unsaved",
  );
  expect(screen.getByLabelText("Comment 1")).toHaveValue(
    "Private inline body unsaved inline",
  );
  expect(screen.queryByText(/private-parent-path.ts/)).not.toBeInTheDocument();
  expect(screen.queryByText(/Inspected commit:/)).not.toBeInTheDocument();
  expect(
    screen.getByRole("button", { name: "Submit saved review" }),
  ).toBeDisabled();
});
it("opening an already errored cached query never reinstalls provider anchors", async () => {
  snapshot = {
    ...snapshot,
    comments: [
      {
        comment_id: "123e4567-e89b-42d3-a456-426614174000",
        body: "Preserve old authored comment",
        anchor: {
          provider: "github",
          anchor: { ...anchor, path: "errored-private-path.ts" },
        },
      },
    ],
  };
  const cache = new QueryClient({
    defaultOptions: { queries: { retry: false, retryOnMount: false } },
  });
  caches.push(cache);
  const queryKey = reviewDraftQueryOptions(account, subjectId).queryKey;
  cache.setQueryData(queryKey, snapshot);
  cache
    .getQueryCache()
    .find({ queryKey })
    ?.setState({
      status: "error",
      error: Object.assign(new Error("Synthetic local read failure"), {
        code: "storage_unavailable",
      }),
    });
  render(
    <QueryClientProvider client={cache}>
      <ReviewAuthoringProvider account={account} subjectId={subjectId}>
        <ReviewComposerTrigger />
      </ReviewAuthoringProvider>
    </QueryClientProvider>,
  );
  const user = userEvent.setup();
  await open(user);
  expect(screen.getByLabelText("Comment 1")).toHaveValue(
    "Preserve old authored comment",
  );
  expect(screen.queryByText(/errored-private-path.ts/)).not.toBeInTheDocument();
  expect(
    screen.getByRole("button", { name: "Submit saved review" }),
  ).toBeDisabled();
});

it("definitive invalid-input admission leaves the draft editable instead of retrying the rejected envelope", async () => {
  const submit = mockTauriCommand("collaboration_submit_review", () => {
    throw { code: "invalid_input" };
  });
  const { user } = setup();
  await open(user);
  await consent(user);
  await user.click(screen.getByRole("button", { name: "Submit saved review" }));
  await screen.findByText(/try shorter review messages/);
  expect(
    screen.queryByRole("button", { name: "Recover review receipt" }),
  ).not.toBeInTheDocument();
  expect(screen.getByLabelText("Review summary")).toBeEnabled();
  await user.type(screen.getByLabelText("Review summary"), " revise");
  expect(
    screen.getByRole("button", { name: "Save review draft" }),
  ).toBeEnabled();
  expect(submit).toHaveBeenCalledOnce();
});

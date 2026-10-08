import {
  collaboration,
  collaborationKeys,
  type DetailField,
} from "@gitru/collaboration-client";
import type {
  AcquireDemandRequest,
  CollaborationChange,
  ContextCapabilityRequest,
  DetailEntry,
  DetailQuery,
  DetailSnapshot,
  LocalDraft,
  RemoteAccount,
} from "@gitru/commands";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import {
  fixtureAccount,
  fixtureContextualCapabilities,
  fixtureItem,
  fixturePage,
  fixtureRepositories,
} from "../../../tests/fixtures/collaboration";
import { fixtureBody } from "../../../tests/fixtures/resource-detail";
import { mockForegroundDemand } from "../../../tests/mocks/collaboration-demand";
import {
  mockTauriCommand,
  mockTauriCommandResult,
} from "../../../tests/mocks/tauri";
import { CollaborationWorkspace } from "./workspace";

const account: RemoteAccount = {
  ...fixtureAccount,
  id: "github-comments-one",
  host: "github.com",
  actor_id: "101",
  login: "first-user",
  authorization_epoch: "3",
  notifications_supported: false,
};
const peer: RemoteAccount = {
  ...account,
  id: "github-comments-two",
  actor_id: "202",
  login: "second-user",
  authorization_epoch: "7",
};
const bitbucket: RemoteAccount = {
  ...account,
  id: "bitbucket-comments-one",
  provider: "bitbucket_cloud",
  host: "bitbucket.org",
  actor_id: "11111111-1111-4111-8111-111111111111",
  login: "bitbucket-user",
};
const bitbucketRepository = "33333333-3333-4333-8333-333333333333";
const gitlab: RemoteAccount = {
  ...account,
  id: "gitlab-comments-one",
  provider: "gitlab",
  host: "gitlab.com",
  actor_id: "303",
  login: "gitlab-user",
};
const observedAt = "2026-10-05T12:00:00Z";
const earlierAt = "2026-10-04T12:00:00Z";
const caches: QueryClient[] = [];
const stops: Array<() => void> = [];
afterEach(() => {
  for (const stop of stops.splice(0)) stop();
  for (const cache of caches.splice(0)) cache.clear();
});

function comment(id = "1", overrides: Partial<DetailEntry> = {}): DetailEntry {
  const fields: DetailField[] = ["body", "author", "updated_at"];
  return {
    id: `github-comment:${id.padStart(20, "0")}`,
    provider_id: id,
    author: "saved-author",
    title: null,
    state: null,
    body: { state: "known", text: `Saved comment ${id}` },
    observed_body_state: "known",
    updated_at: observedAt,
    head_oid: null,
    native: null,
    field_mask: fields,
    field_validations: fields.map((field) => ({
      field,
      validated_at: observedAt,
      source: "github/comments/2026-03-10",
      adapter_version: 1,
    })),
    ...overrides,
  };
}

function saved(
  actor = account,
  entries = [comment()],
  kind: "pull_request" | "issue" = "pull_request",
) {
  const isBitbucket = actor.provider === "bitbucket_cloud";
  const isGitlab = actor.provider === "gitlab";
  const repositoryProviderId = isBitbucket ? bitbucketRepository : "345";
  const repositoryId = `${actor.provider}:repository:${repositoryProviderId}`;
  const subjectId = isBitbucket
    ? `bitbucket_cloud:${kind === "pull_request" ? "pull" : "issue"}:${repositoryProviderId}:67`
    : isGitlab
      ? `gitlab:${kind === "pull_request" ? "pull" : "issue"}:${kind === "pull_request" ? "801" : "802"}`
      : kind === "pull_request"
        ? "github:pull:801"
        : "github:issue:802";
  const repository = {
    ...fixtureRepositories.repositories[0],
    id: repositoryId,
    account_id: actor.id,
    provider_id: repositoryProviderId,
    full_name: isBitbucket
      ? "workspace/engine"
      : isGitlab
        ? "example-group/engine"
        : "example-org/engine",
    name: "engine",
    web_url: isBitbucket
      ? "https://bitbucket.org/workspace/engine"
      : isGitlab
        ? "https://gitlab.com/example-group/engine"
        : "https://github.com/example-org/engine",
  };
  const summary = {
    ...fixtureItem,
    id: subjectId,
    account_id: actor.id,
    repository_id: repository.id,
    provider_id: isBitbucket
      ? `${repositoryProviderId}:67`
      : kind === "pull_request"
        ? "801"
        : "802",
    title: `${actor.login} ${kind}67`,
    kind,
    number: "67",
    body: null,
  };
  const body = fixtureBody({
    subject_id: subjectId,
    body: { state: "known", text: `Saved Body ${actor.login}` },
  });
  body.metadata!.kind = kind;
  body.evidence.authorization_epoch = actor.authorization_epoch;
  const comments = fixtureBody({
    pending_intent: null,

    subject_id: subjectId,
    body: { state: "not_loaded", text: null },
    metadata: null,
    entries,
    evidence: {
      ...body.evidence,
      facet: "comments",
      freshness: "stale",
      sync: { ...fixturePage.sync, state: "offline" },
      source: {
        source: "github/comments/2026-03-10",
        adapter_version: 1,
        field_mask: ["body", "author", "updated_at"],
        provider_updated_at: null,
        observed_at: observedAt,
      },
    },
  });
  const draft: LocalDraft = {
    account_id: actor.id,
    subject_id: subjectId,
    body: `Private ${actor.login}`,
    generation: "4",
  };
  return { account: actor, repository, summary, body, comments, draft };
}
type Saved = ReturnType<typeof saved>;
type Cursor = { offset: number; facet_revision: string | null };
function cursor(resource: Saved, offset: number) {
  return JSON.stringify({
    offset,
    facet_revision: resource.comments.evidence.facet_revision,
  });
}

function boundary(
  resources: Saved[],
  initialPolicy:
    | "supported"
    | "unsupported"
    | "not_applicable"
    | "denied" = "supported",
) {
  const accounts = resources.map((resource) => resource.account);
  let view = "1";
  let revision = "10";
  let reset = false;
  let policy = initialPolicy;
  const changes: CollaborationChange[] = [];
  const demand = mockForegroundDemand();
  vi.spyOn(collaboration.transport, "listen").mockResolvedValue(() => {});
  vi.spyOn(collaboration.transport, "listenLocalChanges").mockResolvedValue(
    () => {},
  );
  mockTauriCommand("collaboration_changes_since", (payload) => {
    const after = Number((payload as { afterRevision: string }).afterRevision);
    const receipt = {
      revision,
      authorization_view: view,
      changes: changes.filter((change) => Number(change.revision) > after),
      has_more: false,
      reset_required: reset,
    };
    reset = false;
    return receipt;
  });
  mockTauriCommand("collaboration_accounts", () => ({
    accounts,
    revision,
    authorization_view: view,
  }));
  mockTauriCommand("collaboration_contextual_capabilities", (payload) => {
    const { request } = payload as { request: ContextCapabilityRequest };
    const actor = accounts.find((value) => value.id === request.account_id)!;
    const snapshot = fixtureContextualCapabilities(
      actor,
      request.target,
      "none",
    );
    const supported = { state: "supported", reason: null } as const;
    return {
      ...snapshot,
      revision,
      authorization_view: view,
      facets: snapshot.facets.map((facet) =>
        facet.facet === "comments"
          ? {
              ...facet,
              saved_read:
                policy === "supported"
                  ? supported
                  : policy === "denied"
                    ? { state: "unavailable", reason: "permission_denied" }
                    : {
                        state: "unsupported",
                        reason:
                          policy === "not_applicable"
                            ? "not_applicable"
                            : "not_implemented",
                      },
              synchronize:
                policy === "supported"
                  ? supported
                  : policy === "denied"
                    ? { state: "unavailable", reason: "permission_denied" }
                    : {
                        state: "unsupported",
                        reason:
                          policy === "not_applicable"
                            ? "not_applicable"
                            : "not_implemented",
                      },
              observation: policy === "supported" ? "complete" : "unknown",
              can_recheck_access: policy === "denied",
            }
          : [
                "repositories",
                "pull_requests",
                "issues",
                "pull_details",
                "issue_details",
              ].includes(facet.facet)
            ? { ...facet, saved_read: supported, synchronize: supported }
            : facet,
      ),
    };
  });
  const lookup = (id: string) =>
    resources.find((resource) => resource.account.id === id)!;
  mockTauriCommand("collaboration_repositories", (payload) => ({
    ...fixtureRepositories,
    repositories: [
      lookup((payload as { accountId: string }).accountId).repository,
    ],
    revision,
    authorization_view: view,
  }));
  mockTauriCommand("collaboration_items", (payload) => ({
    ...fixturePage,
    items: [
      lookup((payload as { query: { account_id: string } }).query.account_id)
        .summary,
    ],
    revision,
    authorization_view: view,
  }));
  mockTauriCommand("collaboration_item", (payload) => ({
    pending_intent: null,

    item: lookup((payload as { accountId: string }).accountId).summary,
    revision,
    authorization_view: view,
  }));
  const detail = mockTauriCommand("collaboration_detail", (payload) => {
    const { query } = payload as { query: DetailQuery };
    const resource = lookup(query.account_id);
    expect(query.subject_id).toBe(resource.summary.id);
    if (query.facet === "comments") {
      expect(query.limit).toBe(50);
      const parsed: Cursor | null = query.cursor
        ? JSON.parse(query.cursor)
        : null;
      if (
        parsed &&
        parsed.facet_revision !== resource.comments.evidence.facet_revision
      )
        return Promise.reject({
          code: "stale_view",
          message: "obsolete cursor",
          retry_after_seconds: null,
        });
      const offset = parsed?.offset ?? 0;
      return {
        ...resource.comments,
        entries: resource.comments.entries.slice(offset, offset + 50),
        next_cursor:
          offset + 50 < resource.comments.entries.length
            ? cursor(resource, offset + 50)
            : null,
        revision,
        authorization_view: view,
      };
    }
    expect(query.facet).toBe("body");
    return { ...resource.body, revision, authorization_view: view };
  });
  mockTauriCommand(
    "collaboration_draft",
    (payload) => lookup((payload as { accountId: string }).accountId).draft,
  );
  const save = mockTauriCommand("collaboration_save_draft", (payload) => {
    const draft = (payload as { draft: LocalDraft }).draft;
    return { ...draft, generation: "5" };
  });
  const commentDraft = mockTauriCommand(
    "collaboration_comment_draft",
    (payload) => {
      const { accountId, subjectId } = payload as {
        accountId: string;
        subjectId: string;
      };
      const resource = lookup(accountId);
      expect(subjectId).toBe(resource.summary.id);
      return {
        account_id: accountId,
        subject_id: subjectId,
        body: "",
        generation: "0",
        context: null,
        availability: "unavailable",
        reason: "empty_draft",
        submission: null,
        revision,
        authorization_view: view,
      };
    },
  );
  mockTauriCommand("collaboration_created_comments", (payload) => {
    const { query } = payload as {
      query: { account_id: string; subject_id: string };
    };
    return {
      account_id: query.account_id,
      subject_id: query.subject_id,
      comments: [],
      next_cursor: null,
      revision,
      authorization_view: view,
    };
  });
  const hydrate = mockTauriCommandResult("collaboration_hydrate_detail", {
    job_id: "explicit-comments-refresh",
  });
  return {
    demand,
    detail,
    commentDraft,
    save,
    hydrate,
    change: (resource: Saved, scope = "comments") => {
      revision = String(Number(revision) + 1);
      if (scope === "comments")
        resource.comments.evidence.facet_revision = revision;
      changes.push({
        revision,
        account_id: resource.account.id,
        scope: `detail:${resource.summary.id}:${scope}`,
        reset: false,
      });
    },
    cut: () => {
      policy = "denied";
      view = "2";
      revision = String(Number(revision) + 1);
      reset = true;
    },
    epoch: (resource: Saved) => {
      const next = {
        ...resource.account,
        authorization_epoch: String(
          Number(resource.account.authorization_epoch) + 1,
        ),
      };
      accounts.splice(
        accounts.findIndex((actor) => actor.id === next.id),
        1,
        next,
      );
      resource.account = next;
      resource.body = {
        ...resource.body,
        evidence: {
          ...resource.body.evidence,
          authorization_epoch: next.authorization_epoch,
        },
      };
      resource.comments = {
        ...resource.comments,
        evidence: {
          ...resource.comments.evidence,
          authorization_epoch: next.authorization_epoch,
        },
      };
      view = "2";
      revision = String(Number(revision) + 1);
      reset = true;
    },
  };
}

async function mount(kind: "pull_request" | "issue" = "pull_request") {
  const cache = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  caches.push(cache);
  stops.push(collaboration.installBridge(cache));
  render(
    <QueryClientProvider client={cache}>
      <CollaborationWorkspace kind={kind} />
    </QueryClientProvider>,
  );
  await act(async () => collaboration.wake());
  return { cache, user: userEvent.setup() };
}
async function select(
  user: ReturnType<typeof userEvent.setup>,
  resource: Saved,
) {
  await user.click(
    await screen.findByRole("button", {
      name: new RegExp(resource.summary.title),
    }),
  );
  const article = within(
    await screen.findByRole("article", { name: "Saved item detail" }),
  );
  expect(await article.findByText(resource.body.body.text!)).toBeVisible();
  expect(await article.findByLabelText("Private draft")).toHaveValue(
    resource.draft.body,
  );
  return article;
}
function panel() {
  return within(screen.getByRole("region", { name: "Comments" }));
}
function queries(reads: ReturnType<typeof boundary>) {
  return reads.detail.mock.calls.filter(
    ([payload]) =>
      (payload as { query: DetailQuery }).query.facet === "comments",
  );
}
function interests(reads: ReturnType<typeof boundary>) {
  return reads.demand.acquire.mock.calls
    .map(([payload]) => (payload as { request: AcquireDemandRequest }).request)
    .filter((request) => request.target.facet === "comments");
}
function queryKey(resource: Saved, value: string | null = null) {
  return collaborationKeys.detail(resource.account, {
    account_id: resource.account.id,
    subject_id: resource.summary.id,
    facet: "comments",
    cursor: value,
    limit: 50,
  });
}

describe("cached conversation comments through the ordinary workspace", () => {
  it("starts closed, browses 51 local rows with one demand and evicts inactive pages without affecting the draft CAS", async () => {
    const resource = saved(
      account,
      Array.from({ length: 51 }, (_, index) => comment(String(index + 1))),
    );
    const reads = boundary([resource]);
    const { cache, user } = await mount();
    const article = await select(user, resource);
    expect(panel().getByRole("button", { name: "Comments" })).toHaveAttribute(
      "aria-expanded",
      "false",
    );
    expect(queries(reads)).toHaveLength(0);
    expect(interests(reads)).toHaveLength(0);
    expect(reads.commentDraft).not.toHaveBeenCalled();
    expect(reads.hydrate).not.toHaveBeenCalled();
    await user.type(
      article.getByLabelText("Private draft"),
      " unsaved conversation note",
    );
    await user.click(panel().getByRole("button", { name: "Comments" }));
    const commentEditor = await panel().findByRole("textbox", {
      name: "Comment",
    });
    expect(commentEditor).toHaveValue("");
    await user.type(commentEditor, "Unsaved provider comment");
    expect(reads.commentDraft).toHaveBeenCalledTimes(1);
    expect(article.getByLabelText("Private draft")).toHaveValue(
      "Private first-user unsaved conversation note",
    );
    const list = await panel().findByRole("list", {
      name: "Saved conversation comments",
    });
    expect(within(list).getAllByRole("listitem")).toHaveLength(50);
    await waitFor(() => expect(interests(reads)).toHaveLength(1));
    await user.click(
      panel().getByRole("button", { name: "Next saved comments" }),
    );
    expect(await panel().findByText("Saved comment 51")).toBeVisible();
    expect(
      within(panel().getByRole("list")).getAllByRole("listitem"),
    ).toHaveLength(1);
    expect(panel().queryByText("Saved comment 1")).not.toBeInTheDocument();
    expect(
      cache.getQueryData(queryKey(resource, cursor(resource, 50))),
    ).toBeDefined();
    await user.click(
      panel().getByRole("button", { name: "Previous saved comments" }),
    );
    expect(await panel().findByText("Saved comment 1")).toBeVisible();
    await waitFor(() =>
      expect(
        cache.getQueryData(queryKey(resource, cursor(resource, 50))),
      ).toBeUndefined(),
    );
    expect(queries(reads)).toHaveLength(2);
    expect(interests(reads)).toHaveLength(1);
    const call = reads.demand.acquire.mock.calls.findIndex(
      ([payload]) =>
        (payload as { request: AcquireDemandRequest }).request.target.facet ===
        "comments",
    );
    const lease = await reads.demand.acquire.mock.results[call].value;
    await user.click(panel().getByRole("button", { name: "Comments" }));
    await waitFor(() =>
      expect(reads.demand.release).toHaveBeenCalledWith({
        request: { lease_id: lease.lease_id },
      }),
    );
    expect(panel().queryByRole("list")).not.toBeInTheDocument();
    expect(article.getByLabelText("Private draft")).toHaveValue(
      "Private first-user unsaved conversation note",
    );
    await user.click(article.getByRole("button", { name: "Save draft" }));
    expect(reads.save).toHaveBeenCalledExactlyOnceWith({
      draft: {
        ...resource.draft,
        body: "Private first-user unsaved conversation note",
      },
    });
    expect(reads.hydrate).not.toHaveBeenCalled();
    expect(
      article.getByRole("button", { name: "Merge pull request unavailable" }),
    ).toBeDisabled();
    await user.click(panel().getByRole("button", { name: "Comments" }));
    expect(
      await panel().findByRole("textbox", { name: "Comment" }),
    ).toHaveValue("Unsaved provider comment");
  });

  it("renders safe raw text and distinct known-empty, nullable author, omitted/oversized retained clocks", async () => {
    const retained = comment("1", {
      body: {
        state: "known",
        text: "<script>private-provider-text</script>\nSaved retained text",
      },
      observed_body_state: "omitted",
      field_validations: comment().field_validations.map((field) =>
        field.field === "body" ? { ...field, validated_at: earlierAt } : field,
      ),
    });
    const oversized = comment("2", { observed_body_state: "oversized" });
    const empty = comment("3", {
      body: { state: "known", text: "" },
      author: null,
    });
    const unknown = comment("4", {
      body: { state: "omitted", text: null },
      observed_body_state: "omitted",
      field_validations: [],
    });
    const resource = saved(account, [retained, oversized, empty, unknown]);
    resource.comments.evidence.coverage.state = "partial";
    resource.comments.evidence.coverage.remote_has_more = true;
    resource.comments.evidence.sync.error = {
      code: "provider",
      message: "secret-native-echo",
      retry_after_seconds: null,
    };
    resource.comments.evidence.sync.next_retry_at = "2099-10-05T12:00:00Z";
    boundary([resource]);
    const { user } = await mount();
    await select(user, resource);
    await user.click(panel().getByRole("button", { name: "Comments" }));
    const list = await panel().findByRole("list", {
      name: "Saved conversation comments",
    });
    const rows = within(list).getAllByRole("listitem");
    expect(
      within(rows[0]).getByText(/<script>private-provider-text<\/script>/),
    ).toBeVisible();
    expect(list.querySelector("script")).toBeNull();
    expect(
      within(rows[0]).getByText("Latest comment text was omitted"),
    ).toBeVisible();
    const retainedTime = within(rows[0])
      .getByText(/Saved text retained from an earlier observation/)
      .querySelector("time");
    expect(retainedTime).toHaveAttribute("datetime", earlierAt);
    expect(
      within(rows[0])
        .getByText(/^Updated/)
        .querySelector("time"),
    ).toHaveAttribute("datetime", observedAt);
    expect(
      within(rows[1]).getByText("Latest comment text exceeds the text limit"),
    ).toBeVisible();
    expect(within(rows[1]).getByText("Saved comment 2")).toBeVisible();
    expect(within(rows[2]).getByText("This comment is empty.")).toBeVisible();
    expect(within(rows[2]).getByText("Author unavailable")).toBeVisible();
    expect(within(rows[3]).getByText("Unknown author")).toBeVisible();
    expect(
      within(rows[3]).getByText(
        "The provider omitted this comment; no text is saved.",
      ),
    ).toBeVisible();
    expect(panel().getByText("Partial conversation history")).toBeVisible();
    expect(panel().getByText(/Sync can resume after/)).toBeVisible();
    expect(panel().getByRole("alert")).not.toHaveTextContent(
      "secret-native-echo",
    );
  });

  it("renders a tombstone only from validated deleted state evidence", async () => {
    const validated = comment("1", {
      author: null,
      state: "deleted",
      body: { state: "known", text: null },
      field_mask: ["body", "author", "updated_at", "state"],
      field_validations: ["body", "author", "updated_at", "state"].map(
        (field) => ({
          field: field as DetailField,
          validated_at: observedAt,
          source: "bitbucket.comments.v1",
          adapter_version: 1,
        }),
      ),
    });
    const unvalidated = comment("2", {
      author: null,
      state: "deleted",
      body: { state: "known", text: null },
    });
    const resource = saved(bitbucket, [validated, unvalidated]);
    boundary([resource]);
    const { user } = await mount();
    await select(user, resource);
    await user.click(panel().getByRole("button", { name: "Comments" }));
    const rows = within(
      await panel().findByRole("list", {
        name: "Saved conversation comments",
      }),
    ).getAllByRole("listitem");
    expect(within(rows[0]).getByText("Comment deleted")).toBeVisible();
    expect(within(rows[0]).queryByText("Author unavailable")).toBeNull();
    expect(within(rows[0]).queryByText("No comment text is saved.")).toBeNull();
    expect(within(rows[1]).queryByText("Comment deleted")).toBeNull();
    expect(within(rows[1]).getByText("Author unavailable")).toBeVisible();
    expect(
      within(rows[1]).getByText("No comment text is saved."),
    ).toBeVisible();
  });

  it.each([
    "complete",
    "partial",
    "missing",
  ] as const)("keeps saved %s emptiness separate from unknown history", async (state) => {
    const resource = saved(account, []);
    resource.comments.evidence.coverage.state =
      state === "partial" ? "partial" : "complete";
    resource.comments.evidence.availability =
      state === "missing" ? "missing" : "ready";
    const reads = boundary([resource]);
    const { user } = await mount();
    await select(user, resource);
    await user.click(panel().getByRole("button", { name: "Comments" }));
    const copy =
      state === "missing"
        ? "Comments have not been saved on this device yet."
        : state === "partial"
          ? "No comments are saved in this partial view."
          : "No conversation comments were returned in the saved observation.";
    expect(await panel().findByText(copy)).toBeVisible();
    expect(reads.hydrate).not.toHaveBeenCalled();
  });

  it("mounts no query, demand, or hydration for unsupported policy", async () => {
    const resource = saved(account);
    const reads = boundary([resource], "unsupported");
    const { user } = await mount();
    await select(user, resource);
    await user.click(panel().getByRole("button", { name: "Comments" }));
    expect(await panel().findByText("Feature not supported")).toBeVisible();
    expect(queries(reads)).toHaveLength(0);
    expect(interests(reads)).toHaveLength(0);
    expect(reads.hydrate).not.toHaveBeenCalled();
  });

  it.each([
    {
      kind: "pull_request" as const,
      policy: "supported" as const,
      expected: "Saved comment 1",
      reads: 1,
    },
    {
      kind: "issue" as const,
      policy: "not_applicable" as const,
      expected: "Not available for this resource",
      reads: 0,
    },
  ])("uses Bitbucket Comments only for $kind resources", async ({
    kind,
    policy,
    expected,
    reads: expectedReads,
  }) => {
    const resource = saved(bitbucket, [comment()], kind);
    const reads = boundary([resource], policy);
    const { user } = await mount(kind);
    await select(user, resource);
    await user.click(panel().getByRole("button", { name: "Comments" }));
    expect(await panel().findByText(expected)).toBeVisible();
    if (kind === "issue")
      expect(
        panel().getByText("This feature does not apply to this resource."),
      ).toBeVisible();
    else
      expect(
        panel().getByText(
          "Top-level conversation comments are saved here. Inline comments, replies, and pending comments are not included.",
        ),
      ).toBeVisible();
    expect(queries(reads)).toHaveLength(expectedReads);
    if (kind === "pull_request")
      await waitFor(() => expect(interests(reads)).toHaveLength(1));
    else expect(interests(reads)).toHaveLength(0);
    expect(reads.hydrate).not.toHaveBeenCalled();
  });

  it.each([
    "pull_request",
    "issue",
  ] as const)("uses the common saved Comments panel for GitLab %s", async (kind) => {
    const entry = comment("1", {
      id: "gitlab-note:00000000000000000001",
      field_validations: comment().field_validations.map((field) => ({
        ...field,
        source: "gitlab/conversation-notes/v4",
      })),
    });
    const resource = saved(gitlab, [entry], kind);
    resource.comments.evidence.source = {
      ...resource.comments.evidence.source!,
      source: "gitlab/conversation-notes/v4",
    };
    const reads = boundary([resource]);
    const { user } = await mount(kind);
    await select(user, resource);
    await user.click(panel().getByRole("button", { name: "Comments" }));
    expect(await panel().findByText("Saved comment 1")).toBeVisible();
    expect(
      panel().getByText(
        "Top-level conversation comments are saved here. System activity, inline discussions, and resolvable notes are not included.",
      ),
    ).toBeVisible();
    expect(queries(reads)).toHaveLength(1);
    await waitFor(() => expect(interests(reads)).toHaveLength(1));
    expect(reads.hydrate).not.toHaveBeenCalled();
  });

  it("uses the same saved conversation disclosure on ordinary issues", async () => {
    const resource = saved(account, [comment()], "issue");
    const reads = boundary([resource]);
    const { user } = await mount("issue");
    await select(user, resource);
    await user.click(panel().getByRole("button", { name: "Comments" }));
    expect(await panel().findByText("Saved comment 1")).toBeVisible();
    expect(interests(reads)[0]).toMatchObject({
      target: { subject_id: resource.summary.id, facet: "comments" },
    });
    expect(reads.hydrate).not.toHaveBeenCalled();
  });

  it("reads cached comments while physically inactive and only admits a newer actual activation", async () => {
    const resource = saved();
    const reads = boundary([resource]);
    reads.demand.emit({ generation: "2", active: false });
    const { user } = await mount();
    await select(user, resource);
    await user.click(panel().getByRole("button", { name: "Comments" }));
    expect(await panel().findByText("Saved comment 1")).toBeVisible();
    expect(interests(reads)).toHaveLength(0);
    await act(async () => reads.demand.emit({ generation: "3", active: true }));
    await waitFor(() => expect(interests(reads)).toHaveLength(1));
    expect(interests(reads)[0].owner_generation).toBe("3");
    expect(reads.hydrate).not.toHaveBeenCalled();
  });

  it("drops a held page2 after a real Comments revision and restores page1 without another demand or losing dirty text", async () => {
    const resource = saved(
      account,
      Array.from({ length: 51 }, (_, index) => comment(String(index + 1))),
    );
    const reads = boundary([resource]);
    const ordinaryRead = reads.detail.getMockImplementation()!;
    let finish!: (snapshot: DetailSnapshot) => void;
    reads.detail.mockImplementation((payload) => {
      const { query } = payload as { query: DetailQuery };
      return query.facet === "comments" && query.cursor !== null
        ? new Promise<DetailSnapshot>((resolve) => {
            finish = resolve;
          })
        : ordinaryRead(payload);
    });
    const old = {
      ...resource.comments,
      entries: resource.comments.entries.slice(50),
      evidence: { ...resource.comments.evidence },
    };
    const oldCursor = cursor(resource, 50);
    const { cache, user } = await mount();
    const article = await select(user, resource);
    await user.type(
      article.getByLabelText("Private draft"),
      " keep dirty text",
    );
    await user.click(panel().getByRole("button", { name: "Comments" }));
    await panel().findByText("Saved comment 1");
    await user.click(
      panel().getByRole("button", { name: "Next saved comments" }),
    );
    await waitFor(() => expect(finish).toBeDefined());
    resource.comments.entries = [
      comment("1", {
        body: { state: "known", text: "Changed current comment" },
      }),
    ];
    reads.change(resource);
    await act(async () => collaboration.wake());
    expect(await panel().findByText("Changed current comment")).toBeVisible();
    expect(panel().getByText("Saved page 1")).toBeVisible();
    await act(async () => finish(old));
    expect(panel().queryByText("Saved comment 51")).not.toBeInTheDocument();
    await waitFor(() =>
      expect(cache.getQueryData(queryKey(resource, oldCursor))).toBeUndefined(),
    );
    expect(interests(reads)).toHaveLength(1);
    expect(article.getByText("Saved Body first-user")).toBeVisible();
    expect(article.getByLabelText("Private draft")).toHaveValue(
      "Private first-user keep dirty text",
    );
    expect(reads.save).not.toHaveBeenCalled();
    expect(reads.hydrate).not.toHaveBeenCalled();
  });

  it("keeps a current later page on an unrelated Body revision", async () => {
    const resource = saved(
      account,
      Array.from({ length: 51 }, (_, index) => comment(String(index + 1))),
    );
    const reads = boundary([resource]);
    const { user } = await mount();
    await select(user, resource);
    await user.click(panel().getByRole("button", { name: "Comments" }));
    await panel().findByText("Saved comment 1");
    await user.click(
      panel().getByRole("button", { name: "Next saved comments" }),
    );
    await panel().findByText("Saved comment 51");
    reads.change(resource, "body");
    await act(async () => collaboration.wake());
    expect(panel().getByText("Saved page 2")).toBeVisible();
    expect(panel().getByText("Saved comment 51")).toBeVisible();
    expect(queries(reads)).toHaveLength(2);
    expect(interests(reads)).toHaveLength(1);
  });

  it("recovers an obsolete local cursor through explicit first-page refresh without provider hydration", async () => {
    const resource = saved(
      account,
      Array.from({ length: 51 }, (_, index) => comment(String(index + 1))),
    );
    const reads = boundary([resource]);
    const { user } = await mount();
    await select(user, resource);
    await user.click(panel().getByRole("button", { name: "Comments" }));
    await panel().findByText("Saved comment 1");
    resource.comments.evidence.facet_revision = "11";
    resource.comments.entries = [comment("2")];
    await user.click(
      panel().getByRole("button", { name: "Next saved comments" }),
    );
    expect(await panel().findByRole("alert")).not.toHaveTextContent(
      "obsolete cursor",
    );
    expect(panel().queryByText("Saved comment 51")).not.toBeInTheDocument();
    await user.click(
      panel().getByRole("button", { name: "Restart saved conversation" }),
    );
    expect(await panel().findByText("Saved comment 2")).toBeVisible();
    expect(panel().getByText("Saved page 1")).toBeVisible();
    expect(interests(reads)).toHaveLength(1);
    expect(reads.hydrate).not.toHaveBeenCalled();
  });

  it("admits Sync/Recheck only on explicit intent and hides old held content on an authorized-view reset", async () => {
    const resource = saved(
      account,
      Array.from({ length: 51 }, (_, index) => comment(String(index + 1))),
    );
    const reads = boundary([resource]);
    const ordinaryRead = reads.detail.getMockImplementation()!;
    let finish!: (snapshot: DetailSnapshot) => void;
    reads.detail.mockImplementation((payload) => {
      const { query } = payload as { query: DetailQuery };
      return query.facet === "comments" && query.cursor !== null
        ? new Promise<DetailSnapshot>((resolve) => {
            finish = resolve;
          })
        : ordinaryRead(payload);
    });
    const { cache, user } = await mount();
    const article = await select(user, resource);
    await user.type(
      article.getByLabelText("Private draft"),
      " survive access cut",
    );
    await user.click(panel().getByRole("button", { name: "Comments" }));
    await panel().findByText("Saved comment 1");
    expect(reads.hydrate).not.toHaveBeenCalled();
    await user.click(panel().getByRole("button", { name: "Sync comments" }));
    expect(reads.hydrate).toHaveBeenCalledExactlyOnceWith({
      request: {
        account_id: account.id,
        authorization_epoch: "3",
        subject_id: resource.summary.id,
        facet: "comments",
      },
    });
    await user.click(
      panel().getByRole("button", { name: "Next saved comments" }),
    );
    await waitFor(() => expect(finish).toBeDefined());
    reads.cut();
    await act(async () => collaboration.wake());
    expect(await panel().findByText("Access denied")).toBeVisible();
    await act(async () =>
      finish({
        ...resource.comments,
        entries: resource.comments.entries.slice(50),
      }),
    );
    expect(panel().queryByText("Saved comment 51")).not.toBeInTheDocument();
    expect(
      cache.getQueryData(queryKey(resource, cursor(resource, 50))),
    ).toBeUndefined();
    expect(article.getByLabelText("Private draft")).toHaveValue(
      "Private first-user survive access cut",
    );
    expect(article.getByText("Saved Body first-user")).toBeVisible();
    await user.click(panel().getByRole("button", { name: "Recheck access" }));
    expect(reads.hydrate).toHaveBeenCalledTimes(2);
    expect(reads.hydrate.mock.calls[1]).toEqual(reads.hydrate.mock.calls[0]);
  });

  it("resets disclosure on actor cutover and fences the previous actor's held page", async () => {
    const matches = Element.prototype.matches;
    vi.spyOn(Element.prototype, "matches").mockImplementation(function (
      this: Element,
      selector,
    ) {
      return [":modal", ":fullscreen", ":popover-open"].includes(selector)
        ? false
        : matches.call(this, selector);
    });
    const first = saved(
      account,
      Array.from({ length: 51 }, (_, index) => comment(String(index + 1))),
    );
    const second = saved(peer, [
      comment("1", {
        body: { state: "known", text: "Peer saved conversation" },
      }),
    ]);
    const reads = boundary([first, second]);
    const ordinaryRead = reads.detail.getMockImplementation()!;
    let finish!: (snapshot: DetailSnapshot) => void;
    reads.detail.mockImplementation((payload) => {
      const { query } = payload as { query: DetailQuery };
      return query.account_id === account.id &&
        query.facet === "comments" &&
        query.cursor !== null
        ? new Promise<DetailSnapshot>((resolve) => {
            finish = resolve;
          })
        : ordinaryRead(payload);
    });
    const { cache, user } = await mount();
    await select(user, first);
    await user.click(panel().getByRole("button", { name: "Comments" }));
    await panel().findByText("Saved comment 1");
    await user.click(
      panel().getByRole("button", { name: "Next saved comments" }),
    );
    await waitFor(() => expect(finish).toBeDefined());
    await user.click(screen.getByLabelText("Provider account"));
    await user.click(
      await screen.findByRole("option", { name: "GitHub · @second-user" }),
    );
    const article = await select(user, second);
    expect(panel().getByRole("button", { name: "Comments" })).toHaveAttribute(
      "aria-expanded",
      "false",
    );
    await user.click(panel().getByRole("button", { name: "Comments" }));
    expect(await panel().findByText("Peer saved conversation")).toBeVisible();
    expect(panel().getByText("Saved page 1")).toBeVisible();
    await act(async () =>
      finish({ ...first.comments, entries: first.comments.entries.slice(50) }),
    );
    expect(panel().queryByText("Saved comment 51")).not.toBeInTheDocument();
    expect(
      cache.getQueryData(queryKey(first, cursor(first, 50))),
    ).toBeUndefined();
    expect(article.getByLabelText("Private draft")).toHaveValue(
      second.draft.body,
    );
    await waitFor(() =>
      expect(
        interests(reads).map((request) => [
          request.account_id,
          request.authorization_epoch,
        ]),
      ).toEqual([
        [account.id, "3"],
        [peer.id, "7"],
      ]),
    );
    expect(reads.hydrate).not.toHaveBeenCalled();
  });

  it.each([
    "success",
    "error",
  ] as const)("suppresses a displayed later page while the first-page refresh is held and then %s", async (outcome) => {
    const resource = saved(
      account,
      Array.from({ length: 51 }, (_, index) => comment(String(index + 1))),
    );
    const reads = boundary([resource]);
    const { user } = await mount();
    const article = await select(user, resource);
    await user.type(
      article.getByLabelText("Private draft"),
      " retained during refresh",
    );
    await user.click(panel().getByRole("button", { name: "Comments" }));
    await panel().findByText("Saved comment 1");
    await user.click(
      panel().getByRole("button", { name: "Next saved comments" }),
    );
    await panel().findByText("Saved comment 51");
    const ordinaryRead = reads.detail.getMockImplementation()!;
    let finish!: (snapshot: DetailSnapshot) => void;
    let fail!: (error: unknown) => void;
    reads.detail.mockImplementation((payload) => {
      const { query } = payload as { query: DetailQuery };
      return query.facet === "comments" && query.cursor === null
        ? new Promise<DetailSnapshot>((resolve, reject) => {
            finish = resolve;
            fail = reject;
          })
        : ordinaryRead(payload);
    });
    resource.comments.entries = [comment("2")];
    reads.change(resource);
    await act(async () => collaboration.wake());
    expect(
      await panel().findByText("Refreshing saved conversation…"),
    ).toBeVisible();
    expect(panel().queryByText("Saved comment 51")).not.toBeInTheDocument();
    expect(panel().queryByRole("list")).not.toBeInTheDocument();
    if (outcome === "success") {
      await act(async () => finish({ ...resource.comments, revision: "11" }));
      expect(await panel().findByText("Saved comment 2")).toBeVisible();
      expect(panel().getByText("Saved page 1")).toBeVisible();
    } else {
      await act(async () =>
        fail({
          code: "provider",
          message: "private-fetch-error",
          retry_after_seconds: null,
        }),
      );
      expect(await panel().findByRole("alert")).not.toHaveTextContent(
        "private-fetch-error",
      );
      expect(
        panel().getByRole("button", { name: "Restart saved conversation" }),
      ).toBeEnabled();
      expect(panel().queryByRole("list")).not.toBeInTheDocument();
    }
    expect(article.getByLabelText("Private draft")).toHaveValue(
      "Private first-user retained during refresh",
    );
    expect(interests(reads)).toHaveLength(1);
    expect(reads.hydrate).not.toHaveBeenCalled();
  });

  it.each([
    "subject",
    "facet",
    "epoch",
  ] as const)("refuses a first-page snapshot with a mismatched %s", async (mismatch) => {
    const resource = saved();
    const reads = boundary([resource]);
    const ordinaryRead = reads.detail.getMockImplementation()!;
    reads.detail.mockImplementation((payload) => {
      const { query } = payload as { query: DetailQuery };
      if (query.facet !== "comments") return ordinaryRead(payload);
      return {
        ...resource.comments,
        subject_id:
          mismatch === "subject" ? "github:issue:other" : resource.summary.id,
        evidence: {
          ...resource.comments.evidence,
          facet: mismatch === "facet" ? "reviews" : "comments",
          authorization_epoch:
            mismatch === "epoch" ? "999" : account.authorization_epoch,
        },
      };
    });
    const { user } = await mount();
    await select(user, resource);
    await user.click(panel().getByRole("button", { name: "Comments" }));
    expect(
      await panel().findByText(
        "The saved conversation changed. Read the current view again.",
      ),
    ).toBeVisible();
    expect(panel().queryByText("Saved comment 1")).not.toBeInTheDocument();
    expect(
      panel().getByRole("button", { name: "Restart saved conversation" }),
    ).toBeEnabled();
    expect(reads.hydrate).not.toHaveBeenCalled();
  });

  it.each([
    "subject",
    "facet",
    "epoch",
    "revision",
  ] as const)("refuses a later snapshot with a mismatched %s", async (mismatch) => {
    const resource = saved(
      account,
      Array.from({ length: 51 }, (_, index) => comment(String(index + 1))),
    );
    const reads = boundary([resource]);
    const ordinaryRead = reads.detail.getMockImplementation()!;
    reads.detail.mockImplementation((payload) => {
      const { query } = payload as { query: DetailQuery };
      if (query.facet !== "comments" || query.cursor === null)
        return ordinaryRead(payload);
      return {
        ...resource.comments,
        entries: resource.comments.entries.slice(50),
        subject_id:
          mismatch === "subject" ? "github:issue:other" : resource.summary.id,
        evidence: {
          ...resource.comments.evidence,
          facet: mismatch === "facet" ? "reviews" : "comments",
          authorization_epoch:
            mismatch === "epoch" ? "999" : account.authorization_epoch,
          facet_revision:
            mismatch === "revision"
              ? "999"
              : resource.comments.evidence.facet_revision,
        },
      };
    });
    const { user } = await mount();
    await select(user, resource);
    await user.click(panel().getByRole("button", { name: "Comments" }));
    await panel().findByText("Saved comment 1");
    await user.click(
      panel().getByRole("button", { name: "Next saved comments" }),
    );
    expect(
      await panel().findByText(
        "The saved conversation changed. Restart to read the current pages.",
      ),
    ).toBeVisible();
    expect(panel().queryByText("Saved comment 51")).not.toBeInTheDocument();
    expect(interests(reads)).toHaveLength(1);
    expect(reads.hydrate).not.toHaveBeenCalled();
  });

  it("closes only Comments on a same-actor epoch cut and preserves the dirty editor while rejecting the held old page", async () => {
    const resource = saved(
      { ...account },
      Array.from({ length: 51 }, (_, index) => comment(String(index + 1))),
    );
    const reads = boundary([resource]);
    const ordinaryRead = reads.detail.getMockImplementation()!;
    let finish!: (snapshot: DetailSnapshot) => void;
    let finishBody!: () => void;
    let holdReplacementBody = false;
    reads.detail.mockImplementation((payload) => {
      const { query } = payload as { query: DetailQuery };
      if (query.facet === "comments" && query.cursor !== null)
        return new Promise<DetailSnapshot>((resolve) => {
          finish = resolve;
        });
      if (query.facet === "body" && holdReplacementBody) {
        const snapshot = ordinaryRead(payload);
        return new Promise<DetailSnapshot>((resolve) => {
          finishBody = () => {
            void Promise.resolve(snapshot).then(resolve);
          };
        });
      }
      return ordinaryRead(payload);
    });
    const oldSnapshot = {
      ...resource.comments,
      entries: resource.comments.entries.slice(50),
    };
    const oldKey = queryKey(resource, cursor(resource, 50));
    const { cache, user } = await mount();
    const article = await select(user, resource);
    await user.type(
      article.getByLabelText("Private draft"),
      " keep after reconnect",
    );
    await user.click(panel().getByRole("button", { name: "Comments" }));
    await panel().findByText("Saved comment 1");
    await user.click(
      panel().getByRole("button", { name: "Next saved comments" }),
    );
    await waitFor(() => expect(finish).toBeDefined());
    holdReplacementBody = true;
    reads.epoch(resource);
    await act(async () => collaboration.wake());
    await waitFor(() =>
      expect(panel().getByRole("button", { name: "Comments" })).toHaveAttribute(
        "aria-expanded",
        "false",
      ),
    );
    await waitFor(() => expect(finishBody).toBeDefined());
    expect(
      article.queryByText("Saved Body first-user"),
    ).not.toBeInTheDocument();
    await act(async () => finish(oldSnapshot));
    expect(panel().queryByText("Saved comment 51")).not.toBeInTheDocument();
    expect(cache.getQueryData(oldKey)).toBeUndefined();
    expect(article.getByLabelText("Private draft")).toHaveValue(
      "Private first-user keep after reconnect",
    );
    expect(
      article.queryByText("Saved Body first-user"),
    ).not.toBeInTheDocument();
    await act(async () => finishBody());
    expect(await article.findByText("Saved Body first-user")).toBeVisible();
    expect(article.getByLabelText("Private draft")).toHaveValue(
      "Private first-user keep after reconnect",
    );
    await user.click(panel().getByRole("button", { name: "Comments" }));
    expect(await panel().findByText("Saved comment 1")).toBeVisible();
    await waitFor(() =>
      expect(
        interests(reads).map((request) => request.authorization_epoch),
      ).toEqual(["3", "4"]),
    );
    expect(reads.save).not.toHaveBeenCalled();
    expect(reads.hydrate).not.toHaveBeenCalled();
  });

  it("bounds cursor history at 100 pages and keeps only the first/current snapshots", async () => {
    const resource = saved();
    const reads = boundary([resource]);
    const ordinaryRead = reads.detail.getMockImplementation()!;
    reads.detail.mockImplementation((payload) => {
      const { query } = payload as { query: DetailQuery };
      if (query.facet !== "comments") return ordinaryRead(payload);
      expect(query.limit).toBe(50);
      const page = query.cursor === null ? 1 : Number(query.cursor);
      return {
        ...resource.comments,
        entries: [comment(String(page))],
        next_cursor: String(page + 1),
      };
    });
    const { cache, user } = await mount();
    await select(user, resource);
    await user.click(panel().getByRole("button", { name: "Comments" }));
    await panel().findByText("Saved comment 1");
    for (let page = 2; page <= 100; page++) {
      fireEvent.click(
        panel().getByRole("button", { name: "Next saved comments" }),
      );
      await panel().findByText(`Saved comment ${page}`);
    }
    expect(
      panel().getByRole("button", { name: "Next saved comments" }),
    ).toBeDisabled();
    expect(panel().getByText(/can browse up to 100 saved pages/)).toBeVisible();
    await waitFor(() =>
      expect(
        cache.getQueryCache().findAll({
          predicate: (query) =>
            (query.queryKey[5] as DetailQuery | undefined)?.facet ===
            "comments",
        }),
      ).toHaveLength(2),
    );
    expect(queries(reads)).toHaveLength(100);
    expect(interests(reads)).toHaveLength(1);
    expect(reads.hydrate).not.toHaveBeenCalled();
  }, 15_000);

  it("stops a repeated continuation and restarts local browsing without hydration", async () => {
    const resource = saved();
    const reads = boundary([resource]);
    const ordinaryRead = reads.detail.getMockImplementation()!;
    reads.detail.mockImplementation((payload) => {
      const { query } = payload as { query: DetailQuery };
      return query.facet === "comments"
        ? { ...resource.comments, next_cursor: "repeat" }
        : ordinaryRead(payload);
    });
    const { user } = await mount();
    await select(user, resource);
    await user.click(panel().getByRole("button", { name: "Comments" }));
    await panel().findByText("Saved comment 1");
    await user.click(
      panel().getByRole("button", { name: "Next saved comments" }),
    );
    expect(
      await panel().findByText(/saved continuation did not advance/),
    ).toBeVisible();
    expect(
      panel().getByRole("button", { name: "Next saved comments" }),
    ).toBeDisabled();
    await user.click(
      panel().getByRole("button", { name: "Restart saved conversation" }),
    );
    expect(await panel().findByText("Saved page 1")).toBeVisible();
    expect(reads.hydrate).not.toHaveBeenCalled();
  });
});

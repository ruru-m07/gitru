import {
  collaboration,
  collaborationKeys,
  type DetailField,
} from "@gitru/collaboration-client";
import type {
  AcquireDemandRequest,
  ContextCapabilityRequest,
  ContextFacetCapability,
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
  id: "bitbucket-tasks-one",
  provider: "bitbucket_cloud",
  host: "bitbucket.org",
  actor_id: "11111111-1111-4111-8111-111111111111",
  login: "first-user",
  authorization_epoch: "3",
  notifications_supported: false,
};
const peer: RemoteAccount = {
  ...account,
  id: "bitbucket-tasks-two",
  actor_id: "22222222-2222-4222-8222-222222222222",
  login: "second-user",
  authorization_epoch: "7",
};
const repositoryUuid = "33333333-3333-4333-8333-333333333333";
const taskUuid = "44444444-4444-4444-8444-444444444444";
const peerTaskUuid = "55555555-5555-4555-8555-555555555555";
const observedAt = "2026-10-04T00:00:00Z";
const earlierAt = "2026-10-03T00:00:00Z";
const subjectId = `bitbucket_cloud:pull:${repositoryUuid}:67`;
const caches: QueryClient[] = [];
const stops: Array<() => void> = [];
afterEach(() => {
  for (const stop of stops.splice(0)) stop();
  for (const cache of caches.splice(0)) cache.clear();
});

function task(id = "1", overrides: Partial<DetailEntry> = {}): DetailEntry {
  const fields: DetailField[] = [
    "task_content",
    "task_state",
    "task_created_at",
    "task_updated_at",
    "task_pending",
    "task_resolved_at",
    "task_resolver",
  ];
  return {
    id: `bitbucket_cloud:task:${repositoryUuid}:67:${id}`,
    provider_id: `${repositoryUuid}:67:${id}`,
    author: null,
    title: null,
    state: null,
    body: { state: "not_loaded", text: null },
    observed_body_state: "not_loaded",
    updated_at: null,
    head_oid: null,
    native: {
      kind: "task.v1",
      value: {
        content: { state: "known", text: `Saved task content ${id}` },
        observed_content_state: "omitted",
        creator: {
          provider_id: taskUuid,
          kind: "future_actor",
          login: null,
          display_name: null,
        },
        state: "<future-task-state>",
        created_at: earlierAt,
        updated_at: observedAt,
        pending: false,
        resolved_at: null,
        resolved_by: null,
        comment_id: null,
      },
    },
    field_mask: fields.filter((field) => field !== "task_content"),
    field_validations: fields.map((field) => ({
      field,
      validated_at: field === "task_content" ? earlierAt : observedAt,
      source: "bitbucket.tasks.v1",
      adapter_version: 1,
    })),
    ...overrides,
  };
}

function saved(actor = account, entries = [task()]) {
  const repository = {
    ...fixtureRepositories.repositories[0],
    id: `bitbucket_cloud:repository:${repositoryUuid}`,
    account_id: actor.id,
    provider_id: repositoryUuid,
    full_name: "workspace/engine",
    name: "engine",
    web_url: "https://bitbucket.org/workspace/engine",
  };
  const summary = {
    ...fixtureItem,
    id: subjectId,
    account_id: actor.id,
    repository_id: repository.id,
    provider_id: `${repositoryUuid}:67`,
    title: `${actor.login} PR67`,
    number: "67",
    body: null,
    web_url: `${repository.web_url}/pull-requests/67`,
  };
  const body = fixtureBody({
    subject_id: subjectId,
    body: { state: "known", text: `Saved Body ${actor.login}` },
  });
  body.evidence.authorization_epoch = actor.authorization_epoch;
  const tasks = fixtureBody({
    subject_id: subjectId,
    body: { state: "not_loaded", text: null },
    metadata: null,
    entries,
    evidence: {
      ...body.evidence,
      facet: "tasks",
      freshness: "stale",
      sync: { ...fixturePage.sync, state: "offline" },
      source: {
        source: "bitbucket.tasks.v1",
        adapter_version: 1,
        field_mask: [],
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
  return { account: actor, repository, summary, body, tasks, draft };
}
type Saved = ReturnType<typeof saved>;

function boundary(
  resources: Saved[],
  initialPolicy: "supported" | "unsupported" | "denied" = "supported",
) {
  const accounts = resources.map((resource) => resource.account);
  let view = "1";
  let revision = "10";
  let reset = false;
  let policy = initialPolicy;
  const demand = mockForegroundDemand();
  vi.spyOn(collaboration.transport, "listen").mockResolvedValue(() => {});
  vi.spyOn(collaboration.transport, "listenLocalChanges").mockResolvedValue(
    () => {},
  );
  mockTauriCommand("collaboration_changes_since", () => {
    const receipt = {
      revision,
      authorization_view: view,
      changes: [],
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
    const tasks: ContextFacetCapability = {
      facet: "tasks",
      saved_read:
        policy === "supported"
          ? supported
          : policy === "denied"
            ? { state: "unavailable", reason: "permission_denied" }
            : { state: "unsupported", reason: "not_implemented" },
      synchronize:
        policy === "supported"
          ? supported
          : policy === "denied"
            ? { state: "unavailable", reason: "permission_denied" }
            : { state: "unsupported", reason: "not_implemented" },
      remote_write: { state: "unsupported", reason: "not_implemented" },
      observation: policy === "supported" ? "complete" : "unknown",
      can_recheck_access: policy === "denied",
      sync: { ...fixturePage.sync, state: "offline" },
    };
    return {
      ...snapshot,
      revision,
      authorization_view: view,
      facets: [
        ...snapshot.facets.map((facet) =>
          [
            "repositories",
            "pull_requests",
            "pull_details",
            "issue_details",
          ].includes(facet.facet)
            ? { ...facet, saved_read: supported, synchronize: supported }
            : facet,
        ),
        tasks,
      ],
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
    item: lookup((payload as { accountId: string }).accountId).summary,
    revision,
    authorization_view: view,
  }));
  const detail = mockTauriCommand("collaboration_detail", (payload) => {
    const { query } = payload as { query: DetailQuery };
    expect(query.subject_id).toBe(subjectId);
    const resource = lookup(query.account_id);
    if (query.facet === "tasks") {
      expect(query.limit).toBe(50);
      const offset = query.cursor === null ? 0 : Number(query.cursor);
      expect(Number.isSafeInteger(offset)).toBe(true);
      return {
        ...resource.tasks,
        entries: resource.tasks.entries.slice(offset, offset + 50),
        next_cursor:
          offset + 50 < resource.tasks.entries.length
            ? String(offset + 50)
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
  const hydrate = mockTauriCommandResult("collaboration_hydrate_detail", {
    job_id: "explicit-task-refresh",
  });
  return {
    demand,
    detail,
    save,
    hydrate,
    cut: () => {
      policy = "denied";
      view = "2";
      revision = "11";
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
  return within(screen.getByRole("region", { name: "Tasks" }));
}
function queries(reads: ReturnType<typeof boundary>) {
  return reads.detail.mock.calls.filter(
    ([payload]) => (payload as { query: DetailQuery }).query.facet === "tasks",
  );
}
function interests(reads: ReturnType<typeof boundary>) {
  return reads.demand.acquire.mock.calls
    .map(([payload]) => (payload as { request: AcquireDemandRequest }).request)
    .filter((request) => request.target.facet === "tasks");
}
function taskKey(resource: Saved, cursor: string | null = null) {
  return collaborationKeys.detail(resource.account, {
    account_id: resource.account.id,
    subject_id: subjectId,
    facet: "tasks",
    cursor,
    limit: 50,
  });
}

function field(row: HTMLElement, label: string) {
  return within(within(row).getByText(label).nextElementSibling as HTMLElement);
}

describe("saved native task disclosure in the ordinary workspace", () => {
  it("browses beyond 50 local rows with one visible demand, exact retained field evidence and an independent authored draft", async () => {
    const consoleErrors = vi.spyOn(console, "error");
    const resource = saved(
      account,
      Array.from({ length: 51 }, (_, index) => task(String(index + 1))),
    );
    resource.tasks.evidence.sync.error = {
      code: "provider",
      message: "private-provider-echo",
      retry_after_seconds: null,
    };
    resource.tasks.evidence.sync.next_retry_at = "2099-10-04T00:00:00Z";
    const reads = boundary([resource]);
    const { cache, user } = await mount();
    const article = await select(user, resource);
    expect(panel().getByRole("button", { name: "Tasks" })).toHaveAttribute(
      "aria-expanded",
      "false",
    );
    expect(queries(reads)).toHaveLength(0);
    expect(interests(reads)).toHaveLength(0);
    expect(cache.getQueryData(taskKey(resource))).toBeUndefined();
    await user.type(
      article.getByLabelText("Private draft"),
      " unsaved task note",
    );
    await user.click(panel().getByRole("button", { name: "Tasks" }));
    const list = await panel().findByRole("list", { name: "Saved tasks" });
    expect(within(list).getAllByRole("listitem")).toHaveLength(50);
    const row = within(list).getAllByRole("listitem")[0];
    expect(
      field(row, "Pending flag").getByText("No", { exact: true }),
    ).toBeVisible();
    expect(
      field(row, "Resolver").getByText("None", { exact: true }),
    ).toBeVisible();
    expect(field(row, "Creator nickname").getByText("Unknown")).toBeVisible();
    expect(
      field(row, "Comment association").getByText("Unknown"),
    ).toBeVisible();
    expect(
      field(row, "Content").getByText(/Retained from an earlier observation/),
    ).toBeVisible();
    expect(
      field(row, "Content").getByText("Saved task content 1"),
    ).toBeVisible();
    expect(
      field(row, "Provider state").getByText("<future-task-state>"),
    ).toBeVisible();
    expect(
      within(row).getByText(`Creator: future_actor · ${taskUuid}`),
    ).toBeVisible();
    expect(list.querySelector("script")).toBeNull();
    expect(panel().getByRole("alert")).not.toHaveTextContent(
      "private-provider-echo",
    );
    expect(panel().getByText(/Sync can resume after/)).toBeVisible();
    expect(
      panel().getByText(
        /do not establish review approval of the current commit/,
      ),
    ).toBeVisible();
    await waitFor(() => expect(interests(reads)).toHaveLength(1));
    await user.click(panel().getByRole("button", { name: "Next saved tasks" }));
    expect(await panel().findByText("Saved task content 51")).toBeVisible();
    expect(
      within(panel().getByRole("list")).getAllByRole("listitem"),
    ).toHaveLength(1);
    expect(panel().queryByText("Saved task content 1")).not.toBeInTheDocument();
    expect(
      panel().getByRole("button", { name: "Next saved tasks" }),
    ).toBeDisabled();
    expect(cache.getQueryData(taskKey(resource, "50"))).toMatchObject({
      entries: [resource.tasks.entries[50]],
    });
    await user.click(
      panel().getByRole("button", { name: "Previous saved tasks" }),
    );
    expect(await panel().findByText("Saved task content 1")).toBeVisible();
    expect(queries(reads)).toHaveLength(2);
    expect(interests(reads)).toHaveLength(1);
    const call = reads.demand.acquire.mock.calls.findIndex(
      ([payload]) =>
        (payload as { request: AcquireDemandRequest }).request.target.facet ===
        "tasks",
    );
    const lease = await reads.demand.acquire.mock.results[call].value;
    await user.click(panel().getByRole("button", { name: "Tasks" }));
    await waitFor(() =>
      expect(reads.demand.release).toHaveBeenCalledWith({
        request: { lease_id: lease.lease_id },
      }),
    );
    expect(panel().queryByRole("list")).not.toBeInTheDocument();
    expect(article.getByLabelText("Private draft")).toHaveValue(
      "Private first-user unsaved task note",
    );
    await user.click(article.getByRole("button", { name: "Save draft" }));
    expect(reads.save).toHaveBeenCalledExactlyOnceWith({
      draft: {
        account_id: account.id,
        subject_id: subjectId,
        body: "Private first-user unsaved task note",
        generation: "4",
      },
    });
    expect(reads.hydrate).not.toHaveBeenCalled();
    expect(
      article.getByRole("button", { name: "Merge pull request unavailable" }),
    ).toBeDisabled();
    expect(consoleErrors).not.toHaveBeenCalled();
  });

  it("uses bounded saved content and validated actor presentation without exposing compound identity as the title", async () => {
    const named = task("1");
    const unknown = task("2");
    if (named.native?.kind !== "task.v1" || unknown.native?.kind !== "task.v1")
      throw new Error("Typed task fixtures");
    const fullContent = `\n\n${"🔧".repeat(130)}\nKeep the entire saved content here.`;
    named.native.value.content = { state: "known", text: fullContent };
    named.native.value.creator.display_name = "Known creator";
    named.native.value.resolved_by = {
      provider_id: peerTaskUuid,
      kind: "user",
      login: "resolver-login",
      display_name: "Known resolver",
    };
    for (const field of [
      "task_creator_display_name",
      "task_resolver_login",
      "task_resolver_display_name",
    ] as DetailField[]) {
      named.field_mask.push(field);
      named.field_validations.push({
        field,
        source: "bitbucket.tasks.v1",
        validated_at: observedAt,
        adapter_version: 1,
      });
    }
    unknown.native.value.content = { state: "known", text: "" };
    unknown.native.value.creator.display_name = "Unobserved name";
    unknown.native.value.creator.login = "Unobserved nickname";
    const resource = saved(account, [named, unknown]);
    boundary([resource]);
    const { user } = await mount();
    await select(user, resource);
    await user.click(panel().getByRole("button", { name: "Tasks" }));
    const rows = within(
      await panel().findByRole("list", { name: "Saved tasks" }),
    ).getAllByRole("listitem");
    expect(
      within(rows[0]).getByText(`Task: ${"🔧".repeat(120)}…`),
    ).toBeVisible();
    expect(
      field(rows[0], "Content").getByText(/Keep the entire saved content here/),
    ).toHaveTextContent("🔧".repeat(130));
    expect(within(rows[0]).getByText("Creator: Known creator")).toBeVisible();
    expect(
      field(rows[0], "Resolver").getByText("Known resolver"),
    ).toBeVisible();
    expect(within(rows[1]).getByText("Saved task")).toBeVisible();
    expect(
      within(rows[1]).getByText(`Creator: future_actor · ${taskUuid}`),
    ).toBeVisible();
    expect(
      panel().queryByText("Creator: Unobserved name"),
    ).not.toBeInTheDocument();
    expect(
      panel().queryByText(`Task ${named.provider_id}`),
    ).not.toBeInTheDocument();
  });

  it("distinguishes omitted/oversized unknown content, saved empty content and resolver identity without inventing presentations", async () => {
    const omitted = task("1", { field_validations: [], field_mask: [] });
    if (omitted.native?.kind === "task.v1")
      omitted.native.value.content = { state: "omitted", text: null };
    const oversized = task("2");
    if (oversized.native?.kind === "task.v1")
      oversized.native.value.observed_content_state = "oversized";
    const empty = task("3");
    if (empty.native?.kind === "task.v1") {
      empty.native.value.content = { state: "known", text: "" };
      empty.native.value.observed_content_state = "known";
      empty.native.value.resolved_by = {
        provider_id: peerTaskUuid,
        kind: "<future_actor>",
        login: null,
        display_name: null,
      };
      empty.native.value.comment_id = "9007199254740993";
    }
    empty.field_mask.push(
      "task_content",
      "task_resolver_display_name",
      "task_comment_id",
    );
    empty.field_validations.push(
      ...(
        ["task_resolver_display_name", "task_comment_id"] as DetailField[]
      ).map((name) => ({
        field: name,
        validated_at: observedAt,
        source: "bitbucket.tasks.v1",
        adapter_version: 1,
      })),
    );
    const resource = saved(account, [omitted, oversized, empty]);
    const reads = boundary([resource]);
    const { user } = await mount();
    await select(user, resource);
    await user.click(panel().getByRole("button", { name: "Tasks" }));
    const rows = within(
      await panel().findByRole("list", { name: "Saved tasks" }),
    ).getAllByRole("listitem");
    expect(field(rows[0], "Content").getByText("Unknown")).toBeVisible();
    expect(
      within(rows[0]).getByText("Latest task content was omitted"),
    ).toBeVisible();
    expect(
      within(rows[1]).getByText("Latest task content exceeds the text limit"),
    ).toBeVisible();
    expect(
      field(rows[1], "Content").getByText("Saved task content 2"),
    ).toBeVisible();
    expect(
      field(rows[1], "Content").getByText(
        /Retained from an earlier observation/,
      ),
    ).toBeVisible();
    expect(
      field(rows[2], "Content").getByText("This task’s content is empty."),
    ).toBeVisible();
    expect(
      field(rows[2], "Resolver").getByText(`<future_actor> · ${peerTaskUuid}`),
    ).toBeVisible();
    expect(
      field(rows[2], "Resolver nickname").getByText("Unknown"),
    ).toBeVisible();
    expect(
      field(rows[2], "Resolver display name").getByText("None"),
    ).toBeVisible();
    expect(
      field(rows[2], "Comment association").getByText("#9007199254740993"),
    ).toBeVisible();
    expect(reads.hydrate).not.toHaveBeenCalled();
  });

  it("does not query or admit unsupported tasks even after opening", async () => {
    const resource = saved();
    const reads = boundary([resource], "unsupported");
    const { user } = await mount();
    await select(user, resource);
    await user.click(panel().getByRole("button", { name: "Tasks" }));
    expect(await panel().findByText("Feature not supported")).toBeVisible();
    expect(queries(reads)).toHaveLength(0);
    expect(interests(reads)).toHaveLength(0);
    expect(reads.hydrate).not.toHaveBeenCalled();
  });

  it("does not mount task disclosure or reads on an ordinary issue", async () => {
    const resource = saved();
    resource.summary.kind = "issue";
    resource.body.metadata!.kind = "issue";
    const reads = boundary([resource]);
    const { user } = await mount("issue");
    await select(user, resource);
    expect(
      screen.queryByRole("region", { name: "Tasks" }),
    ).not.toBeInTheDocument();
    expect(queries(reads)).toHaveLength(0);
    expect(interests(reads)).toHaveLength(0);
    expect(reads.hydrate).not.toHaveBeenCalled();
  });

  it("reads offline saved tasks while native inactive and admits only actual new-generation activation", async () => {
    const resource = saved();
    const reads = boundary([resource]);
    reads.demand.emit({ generation: "2", active: false });
    const { user } = await mount();
    await select(user, resource);
    await user.click(panel().getByRole("button", { name: "Tasks" }));
    expect(await panel().findByText("Saved task content 1")).toBeVisible();
    expect(interests(reads)).toHaveLength(0);
    await act(async () => reads.demand.emit({ generation: "3", active: true }));
    await waitFor(() => expect(interests(reads)).toHaveLength(1));
    expect(interests(reads)[0].owner_generation).toBe("3");
    expect(reads.hydrate).not.toHaveBeenCalled();
  });

  it.each([
    "complete",
    "partial",
    "missing",
  ] as const)("keeps a saved %s observation distinct from an unknown task set", async (state) => {
    const resource = saved(account, []);
    resource.tasks.evidence.coverage.state =
      state === "partial" ? "partial" : "complete";
    resource.tasks.evidence.availability =
      state === "missing" ? "missing" : "ready";
    const reads = boundary([resource]);
    const { user } = await mount();
    await select(user, resource);
    await user.click(panel().getByRole("button", { name: "Tasks" }));
    const copy =
      state === "missing"
        ? "Tasks have not been saved on this device yet."
        : state === "partial"
          ? "No tasks are saved in this partial view."
          : "No tasks were returned in the saved observation.";
    expect(await panel().findByText(copy)).toBeVisible();
    expect(reads.hydrate).not.toHaveBeenCalled();
  });

  it("issues Sync/Recheck only on explicit intent and hides old paged content on a real grant cut while retaining dirty text", async () => {
    const resource = saved(
      account,
      Array.from({ length: 51 }, (_, index) => task(String(index + 1))),
    );
    const reads = boundary([resource]);
    const { user } = await mount();
    const article = await select(user, resource);
    await user.click(panel().getByRole("button", { name: "Tasks" }));
    await panel().findByText("Saved task content 1");
    await user.click(panel().getByRole("button", { name: "Next saved tasks" }));
    await panel().findByText("Saved task content 51");
    expect(reads.hydrate).not.toHaveBeenCalled();
    await user.click(panel().getByRole("button", { name: "Sync tasks" }));
    expect(reads.hydrate).toHaveBeenCalledExactlyOnceWith({
      request: {
        account_id: account.id,
        authorization_epoch: "3",
        subject_id: subjectId,
        facet: "tasks",
      },
    });
    await user.type(
      article.getByLabelText("Private draft"),
      " survive grant cut",
    );
    reads.cut();
    await act(async () => collaboration.wake());
    expect(await panel().findByText("Access denied")).toBeVisible();
    expect(
      panel().queryByText("Saved task content 51"),
    ).not.toBeInTheDocument();
    expect(article.getByLabelText("Private draft")).toHaveValue(
      "Private first-user survive grant cut",
    );
    expect(article.getByText("Saved Body first-user")).toBeVisible();
    await user.click(panel().getByRole("button", { name: "Recheck access" }));
    expect(reads.hydrate).toHaveBeenCalledTimes(2);
    expect(reads.hydrate.mock.calls[1]).toEqual(reads.hydrate.mock.calls[0]);
  });

  it("fences a held second local page across a real authorized-view reset without discarding the dirty draft", async () => {
    const resource = saved(
      account,
      Array.from({ length: 51 }, (_, index) => task(String(index + 1))),
    );
    const reads = boundary([resource]);
    const ordinaryRead = reads.detail.getMockImplementation()!;
    let finish!: (snapshot: DetailSnapshot) => void;
    reads.detail.mockImplementation((payload) => {
      const { query } = payload as { query: DetailQuery };
      return query.facet === "tasks" && query.cursor === "50"
        ? new Promise<DetailSnapshot>((resolve) => {
            finish = resolve;
          })
        : ordinaryRead(payload);
    });
    const { cache, user } = await mount();
    const article = await select(user, resource);
    await user.type(
      article.getByLabelText("Private draft"),
      " unsaved privacy text",
    );
    await user.click(panel().getByRole("button", { name: "Tasks" }));
    await panel().findByText("Saved task content 1");
    await user.click(panel().getByRole("button", { name: "Next saved tasks" }));
    await waitFor(() => expect(finish).toBeDefined());
    reads.cut();
    await act(async () => collaboration.wake());
    expect(await panel().findByText("Access denied")).toBeVisible();
    await act(async () =>
      finish({ ...resource.tasks, entries: resource.tasks.entries.slice(50) }),
    );
    expect(
      panel().queryByText("Saved task content 51"),
    ).not.toBeInTheDocument();
    expect(cache.getQueryData(taskKey(resource, "50"))).toBeUndefined();
    expect(article.getByLabelText("Private draft")).toHaveValue(
      "Private first-user unsaved privacy text",
    );
    expect(reads.save).not.toHaveBeenCalled();
    expect(reads.hydrate).not.toHaveBeenCalled();
  });

  it("resets only its local page/disclosure on actor cutover and rejects the previous actor's held page", async () => {
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
      Array.from({ length: 51 }, (_, index) => task(String(index + 1))),
    );
    const second = saved(peer, [task("1")]);
    if (second.tasks.entries[0].native?.kind === "task.v1")
      second.tasks.entries[0].native.value.content.text =
        "Peer actor saved task";
    const reads = boundary([first, second]);
    const ordinaryRead = reads.detail.getMockImplementation()!;
    let finish!: (snapshot: DetailSnapshot) => void;
    reads.detail.mockImplementation((payload) => {
      const { query } = payload as { query: DetailQuery };
      return query.account_id === account.id &&
        query.facet === "tasks" &&
        query.cursor === "50"
        ? new Promise<DetailSnapshot>((resolve) => {
            finish = resolve;
          })
        : ordinaryRead(payload);
    });
    const { cache, user } = await mount();
    await select(user, first);
    await user.click(panel().getByRole("button", { name: "Tasks" }));
    await panel().findByText("Saved task content 1");
    await user.click(panel().getByRole("button", { name: "Next saved tasks" }));
    await waitFor(() => expect(finish).toBeDefined());
    await user.click(screen.getByLabelText("Provider account"));
    await user.click(
      await screen.findByRole("option", {
        name: "Bitbucket Cloud · @second-user",
      }),
    );
    const article = await select(user, second);
    expect(panel().getByRole("button", { name: "Tasks" })).toHaveAttribute(
      "aria-expanded",
      "false",
    );
    await user.click(panel().getByRole("button", { name: "Tasks" }));
    expect(await panel().findByText("Peer actor saved task")).toBeVisible();
    expect(panel().getByText("Saved page 1")).toBeVisible();
    await act(async () =>
      finish({ ...first.tasks, entries: first.tasks.entries.slice(50) }),
    );
    expect(
      panel().queryByText("Saved task content 51"),
    ).not.toBeInTheDocument();
    expect(cache.getQueryData(taskKey(first, "50"))).toBeUndefined();
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

  it("bounds its saved cursor history to 100 positions without hydration or a second demand", async () => {
    const resource = saved();
    const reads = boundary([resource]);
    const ordinaryRead = reads.detail.getMockImplementation()!;
    reads.detail.mockImplementation((payload) => {
      const { query } = payload as { query: DetailQuery };
      if (query.facet !== "tasks") return ordinaryRead(payload);
      expect(query.limit).toBe(50);
      const page = query.cursor === null ? 1 : Number(query.cursor);
      return {
        ...resource.tasks,
        entries: [task(String(page))],
        next_cursor: String(page + 1),
      };
    });
    const { user } = await mount();
    await select(user, resource);
    await user.click(panel().getByRole("button", { name: "Tasks" }));
    await panel().findByText("Saved task content 1");
    for (let page = 2; page <= 100; page++) {
      fireEvent.click(
        panel().getByRole("button", { name: "Next saved tasks" }),
      );
      await panel().findByText(`Saved task content ${page}`);
    }
    expect(
      panel().getByRole("button", { name: "Next saved tasks" }),
    ).toBeDisabled();
    expect(panel().getByText(/can browse up to 100 saved pages/)).toBeVisible();
    expect(queries(reads)).toHaveLength(100);
    expect(interests(reads)).toHaveLength(1);
    expect(reads.hydrate).not.toHaveBeenCalled();
  }, 15_000);

  it("does not follow a repeated local continuation", async () => {
    const resource = saved();
    const reads = boundary([resource]);
    const ordinaryRead = reads.detail.getMockImplementation()!;
    reads.detail.mockImplementation((payload) => {
      const { query } = payload as { query: DetailQuery };
      return query.facet === "tasks"
        ? { ...resource.tasks, next_cursor: "repeat" }
        : ordinaryRead(payload);
    });
    const { user } = await mount();
    await select(user, resource);
    await user.click(panel().getByRole("button", { name: "Tasks" }));
    await panel().findByText("Saved task content 1");
    await user.click(panel().getByRole("button", { name: "Next saved tasks" }));
    expect(
      await panel().findByText(/saved continuation did not advance/),
    ).toBeVisible();
    expect(
      panel().getByRole("button", { name: "Next saved tasks" }),
    ).toBeDisabled();
    expect(queries(reads)).toHaveLength(2);
    expect(reads.hydrate).not.toHaveBeenCalled();
  });
});

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
import { act, render, screen, waitFor, within } from "@testing-library/react";
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
  id: "bitbucket-participants-one",
  provider: "bitbucket_cloud",
  host: "bitbucket.org",
  actor_id: "11111111-1111-4111-8111-111111111111",
  login: "first-user",
  authorization_epoch: "3",
  notifications_supported: false,
};
const peer: RemoteAccount = {
  ...account,
  id: "bitbucket-participants-two",
  actor_id: "22222222-2222-4222-8222-222222222222",
  login: "second-user",
  authorization_epoch: "7",
};
const repositoryUuid = "33333333-3333-4333-8333-333333333333";
const participantUuid = "44444444-4444-4444-8444-444444444444";
const peerParticipantUuid = "55555555-5555-4555-8555-555555555555";
const observedAt = "2026-10-04T00:00:00Z";
const earlierAt = "2026-10-03T00:00:00Z";
const subjectId = `bitbucket_cloud:pull:${repositoryUuid}:67`;
const caches: QueryClient[] = [];
const stops: Array<() => void> = [];
afterEach(() => {
  for (const stop of stops.splice(0)) stop();
  for (const cache of caches.splice(0)) cache.clear();
});

function participant(
  uuid = participantUuid,
  overrides: Partial<DetailEntry> = {},
): DetailEntry {
  const fields: DetailField[] = [
    "participant_login",
    "participant_role",
    "participant_approved",
    "participant_state",
  ];
  return {
    id: `bitbucket_cloud:participant:${repositoryUuid}:67:${uuid}`,
    provider_id: `${repositoryUuid}:67:${uuid}`,
    author: null,
    title: null,
    state: null,
    body: { state: "not_loaded", text: null },
    observed_body_state: "not_loaded",
    updated_at: null,
    head_oid: null,
    native: {
      kind: "participant.v1",
      value: {
        user: { provider_id: uuid, login: "same-nickname", display_name: null },
        role: "FUTURE_REVIEWER",
        approved: false,
        state: null,
        participated_at: null,
      },
    },
    // Login is retained, display name/date were never observed; state is known null.
    field_mask: fields.filter((field) => field !== "participant_login"),
    field_validations: fields.map((field) => ({
      field,
      validated_at: field === "participant_login" ? earlierAt : observedAt,
      source: "bitbucket.participants.v1",
      adapter_version: 1,
    })),
    ...overrides,
  };
}

function saved(actor = account, entries = [participant()]) {
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
  const participants = fixtureBody({
    pending_intent: null,

    subject_id: subjectId,
    body: { state: "not_loaded", text: null },
    metadata: null,
    entries,
    evidence: {
      ...body.evidence,
      facet: "participants",
      freshness: "stale",
      sync: { ...fixturePage.sync, state: "offline" },
      source: {
        source: "bitbucket.participants.v1",
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
  return { account: actor, repository, summary, body, participants, draft };
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
    const participants: ContextFacetCapability = {
      facet: "participants",
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
        participants,
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
    pending_intent: null,

    item: lookup((payload as { accountId: string }).accountId).summary,
    revision,
    authorization_view: view,
  }));
  const detail = mockTauriCommand("collaboration_detail", (payload) => {
    const { query } = payload as { query: DetailQuery };
    expect(query.subject_id).toBe(subjectId);
    const resource = lookup(query.account_id);
    if (query.facet === "participants") {
      expect(query.cursor).toBeNull();
      expect(query.limit).toBe(100);
      return { ...resource.participants, revision, authorization_view: view };
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
    job_id: "explicit-participant-refresh",
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
  return within(screen.getByRole("region", { name: "Participants" }));
}
function queries(reads: ReturnType<typeof boundary>) {
  return reads.detail.mock.calls.filter(
    ([payload]) =>
      (payload as { query: DetailQuery }).query.facet === "participants",
  );
}
function interests(reads: ReturnType<typeof boundary>) {
  return reads.demand.acquire.mock.calls
    .map(([payload]) => (payload as { request: AcquireDemandRequest }).request)
    .filter((request) => request.target.facet === "participants");
}
function participantKey(resource: Saved) {
  return collaborationKeys.detail(resource.account, {
    account_id: resource.account.id,
    subject_id: subjectId,
    facet: "participants",
    cursor: null,
    limit: 100,
  });
}

describe("saved native participant disclosure in the ordinary workspace", () => {
  it("reads only when opened, distinguishes false/null/unknown/retained fields and releases its sole interest without losing a dirty draft", async () => {
    const future = participant(peerParticipantUuid);
    future.native!.value.state = "<script>future-state</script>";
    const resource = saved(account, [participant(), future]);
    resource.participants.evidence.sync.error = {
      code: "provider",
      message: "private-provider-echo",
      retry_after_seconds: null,
    };
    resource.participants.evidence.sync.next_retry_at = "2099-10-04T00:00:00Z";
    const reads = boundary([resource]);
    const { cache, user } = await mount();
    const article = await select(user, resource);
    expect(
      panel().getByRole("button", { name: "Participants" }),
    ).toHaveAttribute("aria-expanded", "false");
    expect(queries(reads)).toHaveLength(0);
    expect(interests(reads)).toHaveLength(0);
    expect(cache.getQueryData(participantKey(resource))).toBeUndefined();
    await user.type(article.getByLabelText("Private draft"), " unsaved");
    await user.click(panel().getByRole("button", { name: "Participants" }));
    const list = await panel().findByRole("list", {
      name: "Saved participants",
    });
    const rows = within(list).getAllByRole("listitem");
    expect(rows).toHaveLength(2);
    expect(within(rows[0]).getByText(participantUuid)).toBeVisible();
    expect(within(rows[1]).getByText(peerParticipantUuid)).toBeVisible();
    expect(within(rows[0]).getByText("No", { exact: true })).toBeVisible();
    expect(within(rows[0]).getByText("None", { exact: true })).toBeVisible();
    expect(
      within(rows[0]).getAllByText("Unknown", { exact: true }),
    ).toHaveLength(2);
    expect(
      within(rows[0]).getByText(/Retained from an earlier observation/),
    ).toBeVisible();
    expect(
      within(rows[1]).getByText("<script>future-state</script>"),
    ).toBeVisible();
    expect(list.querySelector("script")).toBeNull();
    expect(
      panel().getByText(/do not establish approval of the current commit/),
    ).toBeVisible();
    expect(panel().getByRole("alert")).not.toHaveTextContent(
      "private-provider-echo",
    );
    expect(panel().getByText(/Sync can resume after/)).toBeVisible();
    await waitFor(() => expect(interests(reads)).toHaveLength(1));
    expect(interests(reads)[0]).toMatchObject({
      account_id: account.id,
      authorization_epoch: "3",
      target: { kind: "detail", subject_id: subjectId, facet: "participants" },
    });
    const call = reads.demand.acquire.mock.calls.findIndex(
      ([payload]) =>
        (payload as { request: AcquireDemandRequest }).request.target.facet ===
        "participants",
    );
    const lease = await reads.demand.acquire.mock.results[call].value;
    await user.click(panel().getByRole("button", { name: "Participants" }));
    await waitFor(() =>
      expect(reads.demand.release).toHaveBeenCalledWith({
        request: { lease_id: lease.lease_id },
      }),
    );
    expect(panel().queryByRole("list")).not.toBeInTheDocument();
    expect(article.getByLabelText("Private draft")).toHaveValue(
      "Private first-user unsaved",
    );
    expect(reads.save).not.toHaveBeenCalled();
    expect(reads.hydrate).not.toHaveBeenCalled();
    expect(
      article.getByRole("button", { name: "Merge pull request unavailable" }),
    ).toBeDisabled();
  });

  it("does not query or admit unsupported participants even after the user opens the disclosure", async () => {
    const resource = saved();
    const reads = boundary([resource], "unsupported");
    const { user } = await mount();
    await select(user, resource);
    await user.click(panel().getByRole("button", { name: "Participants" }));
    expect(await panel().findByText("Feature not supported")).toBeVisible();
    expect(queries(reads)).toHaveLength(0);
    expect(interests(reads)).toHaveLength(0);
    expect(reads.hydrate).not.toHaveBeenCalled();
  });

  it("allows offline saved reads while native activity is inactive and admits only after actual new-generation activation", async () => {
    const resource = saved();
    const reads = boundary([resource]);
    reads.demand.emit({ generation: "2", active: false });
    const { user } = await mount();
    await select(user, resource);
    await user.click(panel().getByRole("button", { name: "Participants" }));
    expect(await panel().findByText(participantUuid)).toBeVisible();
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
  ] as const)("keeps a saved %s observation distinct from an unknown participant set", async (state) => {
    const resource = saved(account, []);
    resource.participants.evidence.coverage.state =
      state === "partial" ? "partial" : "complete";
    resource.participants.evidence.availability =
      state === "missing" ? "missing" : "ready";
    const reads = boundary([resource]);
    const { user } = await mount();
    await select(user, resource);
    await user.click(panel().getByRole("button", { name: "Participants" }));
    const copy =
      state === "missing"
        ? "Participants have not been saved on this device yet."
        : state === "partial"
          ? "No participants are saved in this partial view."
          : "No participants were returned in the saved observation.";
    expect(await panel().findByText(copy)).toBeVisible();
    expect(reads.hydrate).not.toHaveBeenCalled();
  });

  it("uses explicit Sync and Recheck commands without implicit hydration or remote writes", async () => {
    const resource = saved();
    const reads = boundary([resource]);
    const { user } = await mount();
    const article = await select(user, resource);
    await user.click(panel().getByRole("button", { name: "Participants" }));
    await panel().findByText(participantUuid);
    expect(reads.hydrate).not.toHaveBeenCalled();
    await user.click(
      panel().getByRole("button", { name: "Sync participants" }),
    );
    expect(reads.hydrate).toHaveBeenCalledExactlyOnceWith({
      request: {
        account_id: account.id,
        authorization_epoch: "3",
        subject_id: subjectId,
        facet: "participants",
      },
    });
    await user.type(article.getByLabelText("Private draft"), " preserve me");
    reads.cut();
    await act(async () => collaboration.wake());
    expect(await panel().findByText("Access denied")).toBeVisible();
    expect(panel().queryByText(participantUuid)).not.toBeInTheDocument();
    expect(article.getByLabelText("Private draft")).toHaveValue(
      "Private first-user preserve me",
    );
    expect(article.getByText("Saved Body first-user")).toBeVisible();
    await user.click(panel().getByRole("button", { name: "Recheck access" }));
    expect(reads.hydrate).toHaveBeenCalledTimes(2);
    expect(reads.hydrate.mock.calls[1]).toEqual(reads.hydrate.mock.calls[0]);
  });

  it("fences a held participant snapshot on a real authorized-view reset and preserves the independent dirty draft", async () => {
    const resource = saved();
    const reads = boundary([resource]);
    let finish!: (snapshot: DetailSnapshot) => void;
    reads.detail.mockImplementation((payload) => {
      const { query } = payload as { query: DetailQuery };
      return query.facet === "participants"
        ? new Promise<DetailSnapshot>((resolve) => {
            finish = resolve;
          })
        : resource.body;
    });
    const { cache, user } = await mount();
    const article = await select(user, resource);
    await user.type(
      article.getByLabelText("Private draft"),
      " unsaved privacy text",
    );
    await user.click(panel().getByRole("button", { name: "Participants" }));
    await waitFor(() => expect(finish).toBeDefined());
    reads.cut();
    // Only the first old participant return stays held; authorized Body rereads use the current view.
    reads.detail.mockImplementation((payload) => {
      const { query } = payload as { query: DetailQuery };
      expect(query.facet).toBe("body");
      return { ...resource.body, revision: "11", authorization_view: "2" };
    });
    await act(async () => collaboration.wake());
    expect(await panel().findByText("Access denied")).toBeVisible();
    await act(async () => finish(resource.participants));
    expect(panel().queryByText(participantUuid)).not.toBeInTheDocument();
    expect(cache.getQueryData(participantKey(resource))).toBeUndefined();
    expect(article.getByLabelText("Private draft")).toHaveValue(
      "Private first-user unsaved privacy text",
    );
    expect(reads.save).not.toHaveBeenCalled();
    expect(reads.hydrate).not.toHaveBeenCalled();
  });

  it("partitions the same PR67 by actor/epoch when the ordinary account picker changes and rejects the old held read", async () => {
    const matches = Element.prototype.matches;
    vi.spyOn(Element.prototype, "matches").mockImplementation(function (
      this: Element,
      selector,
    ) {
      return [":modal", ":fullscreen", ":popover-open"].includes(selector)
        ? false
        : matches.call(this, selector);
    });
    const first = saved();
    const second = saved(peer, [participant(peerParticipantUuid)]);
    const reads = boundary([first, second]);
    let finish!: (snapshot: DetailSnapshot) => void;
    reads.detail.mockImplementation((payload) => {
      const { query } = payload as { query: DetailQuery };
      const resource = query.account_id === account.id ? first : second;
      return query.facet === "body"
        ? resource.body
        : query.account_id === account.id
          ? new Promise<DetailSnapshot>((resolve) => {
              finish = resolve;
            })
          : second.participants;
    });
    const { cache, user } = await mount();
    await select(user, first);
    await user.click(panel().getByRole("button", { name: "Participants" }));
    await waitFor(() => expect(finish).toBeDefined());
    await user.click(screen.getByLabelText("Provider account"));
    await user.click(
      await screen.findByRole("option", {
        name: "Bitbucket Cloud · @second-user",
      }),
    );
    const article = await select(user, second);
    expect(
      panel().getByRole("button", { name: "Participants" }),
    ).toHaveAttribute("aria-expanded", "false");
    await user.click(panel().getByRole("button", { name: "Participants" }));
    expect(await panel().findByText(peerParticipantUuid)).toBeVisible();
    await act(async () => finish(first.participants));
    expect(panel().queryByText(participantUuid)).not.toBeInTheDocument();
    expect(cache.getQueryData(participantKey(first))).toBeUndefined();
    expect(cache.getQueryData(participantKey(second))).toEqual(
      second.participants,
    );
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
});

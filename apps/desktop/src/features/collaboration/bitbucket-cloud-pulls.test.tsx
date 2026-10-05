import { collaboration, collaborationKeys } from "@gitru/collaboration-client";
import type {
  AcquireDemandRequest,
  ContextCapabilityRequest,
  DetailQuery,
  DetailSnapshot,
  ItemQuery,
  LocalDraft,
  RemoteAccount,
  RemoteItem,
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
import {
  fixtureBody,
  fixtureMetadata,
} from "../../../tests/fixtures/resource-detail";
import { mockForegroundDemand } from "../../../tests/mocks/collaboration-demand";
import {
  mockTauriCommand,
  mockTauriCommandResult,
} from "../../../tests/mocks/tauri";
import { CollaborationWorkspace } from "./workspace";

const actor: RemoteAccount = {
  ...fixtureAccount,
  id: "bitbucket-actor-one",
  provider: "bitbucket_cloud",
  host: "bitbucket.org",
  actor_id: "11111111-1111-4111-8111-111111111111",
  login: "first-user",
  authorization_epoch: "3",
  notifications_supported: false,
};
const peer: RemoteAccount = {
  ...actor,
  id: "bitbucket-actor-two",
  actor_id: "22222222-2222-4222-8222-222222222222",
  login: "second-user",
  authorization_epoch: "7",
};
const firstRepository = "33333333-3333-4333-8333-333333333333";
const secondRepository = "44444444-4444-4444-8444-444444444444";
const caches: QueryClient[] = [];
const stops: Array<() => void> = [];
afterEach(() => {
  for (const stop of stops.splice(0)) stop();
  for (const cache of caches.splice(0)) cache.clear();
});

function resource(
  account: RemoteAccount,
  repositoryUuid: string,
  name: string,
) {
  const repository = {
    ...fixtureRepositories.repositories[0],
    id: `bitbucket_cloud:repository:${repositoryUuid}`,
    account_id: account.id,
    provider_id: repositoryUuid,
    full_name: `workspace/${name}`,
    name,
    web_url: `https://bitbucket.org/workspace/${name}`,
  };
  const summary: RemoteItem = {
    ...fixtureItem,
    id: `bitbucket_cloud:pull:${repositoryUuid}:67`,
    account_id: account.id,
    repository_id: repository.id,
    provider_id: `${repositoryUuid}:67`,
    number: "67",
    title: `${account.login} ${name} list preview`,
    body: `Old list description for ${account.login}/${name}`,
    web_url: `${repository.web_url}/pull-requests/67`,
    head_oid: "a".repeat(40),
    is_draft: null,
  };
  const metadata = fixtureMetadata();
  const omitted = new Set([
    "labels",
    "assignees",
    "milestone",
    "is_draft",
    "merged_at",
  ]);
  metadata.values = {
    ...metadata.values,
    title: `${account.login} ${name} saved singleton header`,
    state: "closed",
    state_reason: "declined",
    author: {
      provider_id: account.actor_id,
      login: account.login,
      web_url: null,
    },
    web_url: summary.web_url,
    labels: [],
    assignees: [],
    milestone: null,
    is_draft: null,
    merged_at: null,
    head: { name: `feature/${name}`, oid: "b".repeat(40), repository: null },
    base: {
      name: "main",
      oid: "c".repeat(40),
      repository: {
        provider_id: repositoryUuid,
        full_name: repository.full_name,
        web_url: repository.web_url,
      },
    },
  };
  metadata.fields = metadata.fields.map((field) => ({
    ...field,
    ...(omitted.has(field.field)
      ? {
          saved_state: "omitted" as const,
          observed_state: "omitted" as const,
          validated_at: null,
          stale_at: null,
          source: null,
        }
      : {}),
  }));
  const body = fixtureBody({
    subject_id: summary.id,
    metadata,
    body: {
      state: "known",
      text: `Cached raw Markdown for ${account.login}/${name}\n\nIndependent of the list preview.`,
    },
  });
  body.evidence.authorization_epoch = account.authorization_epoch;
  body.evidence.sync = { ...fixturePage.sync, state: "offline" };
  const draft: LocalDraft = {
    account_id: account.id,
    subject_id: summary.id,
    body: `Private ${account.login}/${name} note`,
    generation: "4",
  };
  return { account, repository, summary, body, draft };
}
type SavedResource = ReturnType<typeof resource>;

function reads(
  resources: SavedResource[],
  denied?: "missing_scope" | "permission_denied",
) {
  const accounts = [
    ...new Map(resources.map((r) => [r.account.id, r.account])).values(),
  ];
  const demand = mockForegroundDemand();
  vi.spyOn(collaboration.transport, "listen").mockResolvedValue(() => {});
  vi.spyOn(collaboration.transport, "listenLocalChanges").mockResolvedValue(
    () => {},
  );
  mockTauriCommandResult("collaboration_changes_since", {
    revision: "10",
    authorization_view: "1",
    changes: [],
    has_more: false,
    reset_required: false,
  });
  mockTauriCommandResult("collaboration_accounts", {
    accounts,
    revision: "10",
    authorization_view: "1",
  });
  mockTauriCommand("collaboration_contextual_capabilities", (payload) => {
    const { request } = payload as { request: ContextCapabilityRequest };
    const account = accounts.find((a) => a.id === request.account_id)!;
    expect(account).toBeDefined();
    const snapshot = fixtureContextualCapabilities(
      account,
      request.target,
      "none",
    );
    return {
      ...snapshot,
      facets: snapshot.facets.map((facet) => {
        const pull = ["pull_requests", "pull_details"].includes(facet.facet);
        const supported = facet.facet === "repositories" || pull;
        const access = {
          state: supported
            ? denied && pull
              ? ("unavailable" as const)
              : ("supported" as const)
            : ("unsupported" as const),
          reason: supported
            ? pull && denied
              ? denied
              : null
            : facet.facet === "issues" || facet.facet === "inbox"
              ? ("provider_semantics" as const)
              : ("not_implemented" as const),
        };
        return {
          ...facet,
          saved_read: access,
          synchronize: access,
          observation: supported ? ("complete" as const) : ("unknown" as const),
          can_recheck_access: pull && denied === "permission_denied",
          sync: {
            ...facet.sync,
            error:
              denied && pull
                ? {
                    code: "permission_denied" as const,
                    message: "synthetic-private-provider-echo",
                    retry_after_seconds: null,
                  }
                : null,
          },
        };
      }),
    };
  });
  const repositories = mockTauriCommand(
    "collaboration_repositories",
    (payload) => ({
      ...fixtureRepositories,
      repositories: resources
        .filter(
          (r) => r.account.id === (payload as { accountId: string }).accountId,
        )
        .map((r) => r.repository),
      sync: { ...fixtureRepositories.sync, state: "offline" },
    }),
  );
  const items = mockTauriCommand("collaboration_items", (payload) => {
    const { query } = payload as { query: ItemQuery };
    expect(query.kind).toBe("pull_request");
    return {
      ...fixturePage,
      items: resources
        .filter(
          (r) =>
            r.account.id === query.account_id &&
            (!query.repository_id || r.repository.id === query.repository_id),
        )
        .map((r) => r.summary),
    };
  });
  const lookup = (accountId: string, subjectId: string) => {
    const saved = resources.find(
      (r) => r.account.id === accountId && r.summary.id === subjectId,
    );
    expect(saved).toBeDefined();
    return saved!;
  };
  const item = mockTauriCommand("collaboration_item", (payload) => {
    const { accountId, itemId } = payload as {
      accountId: string;
      itemId: string;
    };
    return {
      item: lookup(accountId, itemId).summary,
      revision: "10",
      authorization_view: "1",
    };
  });
  const detail = mockTauriCommand("collaboration_detail", (payload) => {
    const { query } = payload as { query: DetailQuery };
    expect(query.facet).toBe("body");
    return lookup(query.account_id, query.subject_id).body;
  });
  const draft = mockTauriCommand("collaboration_draft", (payload) => {
    const { accountId, subjectId } = payload as {
      accountId: string;
      subjectId: string;
    };
    return lookup(accountId, subjectId).draft;
  });
  const save = mockTauriCommand("collaboration_save_draft", (payload) => {
    const { draft } = payload as { draft: LocalDraft };
    const saved = lookup(draft.account_id, draft.subject_id);
    expect(draft.generation).toBe(saved.draft.generation);
    saved.draft = {
      ...draft,
      generation: String(Number(draft.generation) + 1),
    };
    return saved.draft;
  });
  const hydrate = mockTauriCommandResult("collaboration_hydrate_detail", {
    job_id: "unexpected",
  });
  const refresh = mockTauriCommandResult("collaboration_refresh", {
    job_id: "manual",
  });
  return {
    demand,
    repositories,
    items,
    item,
    detail,
    draft,
    save,
    hydrate,
    refresh,
  };
}

async function mount(
  kind: "pull_request" | "issue" | "notification" = "pull_request",
) {
  const cache = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  caches.push(cache);
  stops.push(collaboration.installBridge(cache));
  const view = render(
    <QueryClientProvider client={cache}>
      <CollaborationWorkspace kind={kind} />
    </QueryClientProvider>,
  );
  await act(async () => collaboration.wake());
  return { ...view, cache, user: userEvent.setup() };
}

async function select(
  user: ReturnType<typeof userEvent.setup>,
  saved: SavedResource,
) {
  await user.click(
    await screen.findByRole("button", {
      name: new RegExp(saved.summary.title),
    }),
  );
  const detail = within(
    await screen.findByRole("article", { name: "Saved item detail" }),
  );
  expect(await detail.findByText(savedText(saved))).toBeVisible();
  expect(await detail.findByLabelText("Private draft")).toHaveValue(
    saved.draft.body,
  );
  return detail;
}

function savedText(saved: SavedResource) {
  return saved.body.body.text!.replace(/\s+/g, " ");
}

function bodyDemands(boundary: ReturnType<typeof reads>) {
  return boundary.demand.acquire.mock.calls
    .map(([payload]) => (payload as { request: AcquireDemandRequest }).request)
    .filter((request) => request.target.kind === "detail");
}
function detailKey(saved: SavedResource) {
  return collaborationKeys.detail(saved.account, {
    account_id: saved.account.id,
    subject_id: saved.summary.id,
    facet: "body",
    cursor: null,
    limit: 50,
  });
}

describe("Bitbucket Cloud saved pull requests", () => {
  it("uses offline singleton Body and field evidence, one Body interest, and local drafts without unsupported remote actions", async () => {
    const saved = resource(actor, firstRepository, "engine");
    const boundary = reads([saved]);
    const { cache, user } = await mount();
    const detail = await select(user, saved);
    expect(
      detail.getByRole("heading", { name: saved.body.metadata!.values.title! }),
    ).toBeVisible();
    expect(detail.queryByText(saved.summary.body!)).not.toBeInTheDocument();
    const header = within(detail.getByLabelText("Selected resource metadata"));
    expect(header.getByText("#67")).toBeVisible();
    expect(header.getByText("declined")).toBeVisible();
    expect(header.getByText("b".repeat(40))).toBeVisible();
    expect(header.getByText("c".repeat(40))).toBeVisible();
    expect(header.getAllByText("No saved value").length).toBeGreaterThanOrEqual(
      4,
    );
    for (const unsupported of [
      "Ready for review",
      "No assignees",
      "No labels",
      "No milestone",
      "Not merged",
    ])
      expect(header.queryByText(unsupported)).not.toBeInTheDocument();
    expect(
      detail.queryByRole("button", { name: "Sync comments" }),
    ).not.toBeInTheDocument();
    await user.click(detail.getByRole("button", { name: "Comments" }));
    const comments = within(detail.getByRole("region", { name: "Comments" }));
    expect(comments.getByText("Feature not supported")).toBeVisible();
    expect(
      comments.queryByRole("button", { name: "Sync comments" }),
    ).not.toBeInTheDocument();
    for (const facet of ["reviews", "checks"]) {
      const button = detail.getByRole("button", { name: `Sync ${facet}` });
      expect(button).toBeDisabled();
      await user.click(button);
    }
    const merge = detail.getByRole("button", {
      name: "Merge pull request unavailable",
    });
    expect(merge).toBeDisabled();
    await user.click(merge);
    await waitFor(() => expect(bodyDemands(boundary)).toHaveLength(1));
    expect(bodyDemands(boundary)[0]).toMatchObject({
      account_id: actor.id,
      authorization_epoch: actor.authorization_epoch,
      target: { subject_id: saved.summary.id, facet: "body" },
    });
    expect(boundary.detail).toHaveBeenCalledOnce();
    expect(cache.getQueryData(detailKey(saved))).toEqual(saved.body);
    await user.type(
      detail.getByLabelText("Private draft"),
      " — offline addition",
    );
    await user.click(detail.getByRole("button", { name: "Save draft" }));
    await waitFor(() =>
      expect(boundary.save).toHaveBeenCalledExactlyOnceWith({
        draft: {
          account_id: actor.id,
          subject_id: saved.summary.id,
          body: "Private first-user/engine note — offline addition",
          generation: "4",
        },
      }),
    );
    expect(boundary.hydrate).not.toHaveBeenCalled();
    expect(boundary.refresh).not.toHaveBeenCalled();
  });

  it("isolates PR67 in two UUID repositories through ordinary selection, cache keys and private draft saves", async () => {
    const first = resource(actor, firstRepository, "engine");
    const second = resource(actor, secondRepository, "tools");
    const boundary = reads([first, second]);
    const { cache, user } = await mount();
    await select(user, first);
    const detail = await select(user, second);
    expect(detail.queryByText(savedText(first))).not.toBeInTheDocument();
    expect(
      detail.queryByRole("heading", {
        name: first.body.metadata!.values.title!,
      }),
    ).not.toBeInTheDocument();
    expect(first.summary.number).toBe(second.summary.number);
    expect(first.summary.provider_id).not.toBe(second.summary.provider_id);
    expect(cache.getQueryData(detailKey(first))).toEqual(first.body);
    expect(cache.getQueryData(detailKey(second))).toEqual(second.body);
    await user.type(
      detail.getByLabelText("Private draft"),
      " tools-only addition",
    );
    await user.click(detail.getByRole("button", { name: "Save draft" }));
    await waitFor(() =>
      expect(boundary.save).toHaveBeenCalledExactlyOnceWith({
        draft: {
          account_id: actor.id,
          subject_id: second.summary.id,
          body: "Private first-user/tools note tools-only addition",
          generation: "4",
        },
      }),
    );
    expect(
      cache.getQueryData(collaborationKeys.draft(actor, first.summary.id)),
    ).toEqual(first.draft);
    expect(
      cache.getQueryData(collaborationKeys.draft(actor, second.summary.id)),
    ).toEqual(second.draft);
    await waitFor(() =>
      expect(bodyDemands(boundary).map((r) => r.target.subject_id)).toEqual([
        first.summary.id,
        second.summary.id,
      ]),
    );
    expect(boundary.demand.release).toHaveBeenCalled();
    expect(boundary.hydrate).not.toHaveBeenCalled();
  });

  it("fences a held PR67 Body from another actor and epoch when the ordinary account picker switches", async () => {
    const matches = Element.prototype.matches;
    vi.spyOn(Element.prototype, "matches").mockImplementation(function (
      this: Element,
      selector,
    ) {
      // JSDOM lacks these native top-layer selectors used by Base UI Select.
      return [":modal", ":fullscreen", ":popover-open"].includes(selector)
        ? false
        : matches.call(this, selector);
    });
    const first = resource(actor, firstRepository, "engine");
    const second = resource(peer, firstRepository, "engine");
    const boundary = reads([first, second]);
    let finish!: (snapshot: DetailSnapshot) => void;
    const detailRead = mockTauriCommand("collaboration_detail", (payload) => {
      const { query } = payload as { query: DetailQuery };
      expect(query.subject_id).toBe(first.summary.id);
      expect(query.facet).toBe("body");
      return query.account_id === actor.id
        ? new Promise<DetailSnapshot>((resolve) => {
            finish = resolve;
          })
        : second.body;
    });
    const { cache, user } = await mount();
    await user.click(
      await screen.findByRole("button", {
        name: new RegExp(first.summary.title),
      }),
    );
    await waitFor(() => expect(finish).toBeDefined());
    await user.type(
      await screen.findByLabelText("Private draft"),
      " actor-one unsaved",
    );
    await user.click(screen.getByLabelText("Provider account"));
    await user.click(
      await screen.findByRole("option", {
        name: "Bitbucket Cloud · @second-user",
      }),
    );
    const detail = await select(user, second);
    await act(async () => finish(first.body));
    expect(detail.queryByText(savedText(first))).not.toBeInTheDocument();
    expect(
      detail.queryByRole("heading", {
        name: first.body.metadata!.values.title!,
      }),
    ).not.toBeInTheDocument();
    expect(detail.getByLabelText("Private draft")).toHaveValue(
      second.draft.body,
    );
    expect(cache.getQueryData(detailKey(second))).toEqual(second.body);
    expect(cache.getQueryData(detailKey(first))).toBeUndefined();
    await waitFor(() =>
      expect(
        bodyDemands(boundary).map((r) => [r.account_id, r.authorization_epoch]),
      ).toEqual([
        [actor.id, "3"],
        [peer.id, "7"],
      ]),
    );
    expect(detailRead).toHaveBeenCalledTimes(2);
    expect(boundary.save).not.toHaveBeenCalled();
    expect(boundary.hydrate).not.toHaveBeenCalled();
  });

  it.each([
    "missing_scope",
    "permission_denied",
  ] as const)("keeps repositories usable when PR access is %s without provider text or automatic PR admission", async (reason) => {
    const saved = resource(actor, firstRepository, "engine");
    const boundary = reads([saved], reason);
    const { user } = await mount();
    expect(
      await screen.findByText(
        reason === "missing_scope" ? "Permission required" : "Access denied",
      ),
    ).toBeVisible();
    expect(screen.getByRole("button", { name: "Refresh" })).toBeDisabled();
    expect(
      screen.queryByText(/synthetic-private-provider-echo/),
    ).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Repositories" }));
    expect(
      await screen.findByRole("checkbox", { name: saved.repository.full_name }),
    ).toBeEnabled();
    await user.click(
      screen.getByRole("button", { name: "Discover repositories" }),
    );
    expect(boundary.refresh).toHaveBeenCalledExactlyOnceWith({
      request: {
        account_id: actor.id,
        repository_id: null,
        kind: null,
      },
    });
    expect(boundary.repositories).toHaveBeenCalledWith({ accountId: actor.id });
    expect(boundary.items).not.toHaveBeenCalled();
    expect(boundary.item).not.toHaveBeenCalled();
    expect(boundary.detail).not.toHaveBeenCalled();
    expect(boundary.hydrate).not.toHaveBeenCalled();
    await waitFor(() => expect(boundary.demand.acquire).toHaveBeenCalled());
    for (const [payload] of boundary.demand.acquire.mock.calls)
      expect(
        (payload as { request: AcquireDemandRequest }).request.target.kind,
      ).toBe("repositories");
  });

  it.each([
    "issue",
    "notification",
  ] as const)("keeps %s unsupported even with the implemented PR/Body profile", async (kind) => {
    const boundary = reads([resource(actor, firstRepository, "engine")]);
    const { user } = await mount(kind);
    expect(await screen.findByText("Feature not supported")).toBeVisible();
    expect(
      screen.getByText(
        "This provider uses a different model for this feature.",
      ),
    ).toBeVisible();
    const refresh = screen.getByRole("button", { name: "Refresh" });
    expect(refresh).toBeDisabled();
    await user.click(refresh);
    expect(boundary.items).not.toHaveBeenCalled();
    expect(boundary.detail).not.toHaveBeenCalled();
    expect(boundary.refresh).not.toHaveBeenCalled();
    expect(boundary.hydrate).not.toHaveBeenCalled();
    await waitFor(() => expect(boundary.demand.acquire).toHaveBeenCalled());
    for (const [payload] of boundary.demand.acquire.mock.calls)
      expect(
        (payload as { request: AcquireDemandRequest }).request.target.kind,
      ).toBe("repositories");
  });
});

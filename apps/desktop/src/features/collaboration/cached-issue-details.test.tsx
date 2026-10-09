import { collaboration, type RemoteAccount } from "@gitru/collaboration-client";
import type {
  AcquireDemandRequest,
  ContextCapabilityRequest,
  DetailSnapshot,
} from "@gitru/commands";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { StrictMode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  fixtureAccount,
  fixtureAccounts,
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

const issue = {
  ...fixtureItem,
  kind: "issue" as const,
  title: "Older issue list title",
  head_oid: null,
  is_draft: null,
  web_url: "https://github.com/example-org/engine/issues/42",
};
let revision = "10";
let authorizationView = "1";
let accounts = fixtureAccounts;
let body: DetailSnapshot;
let denied = false;
let paused = false;
const stops: Array<() => void> = [];
const caches: QueryClient[] = [];

function issueBody(overrides: Partial<DetailSnapshot> = {}) {
  const metadata = fixtureMetadata("issue");
  metadata.values.web_url = issue.web_url;
  return fixtureBody({ metadata, ...overrides });
}

beforeEach(() => {
  revision = "10";
  authorizationView = "1";
  accounts = fixtureAccounts;
  body = issueBody();
  denied = false;
  paused = false;
  vi.spyOn(collaboration.transport, "listen").mockResolvedValue(() => {});
  mockTauriCommand("collaboration_accounts", () => ({
    ...accounts,
    revision,
    authorization_view: authorizationView,
  }));
  mockTauriCommand("collaboration_changes_since", (payload) => ({
    revision,
    authorization_view: authorizationView,
    changes:
      Number((payload as { afterRevision: string }).afterRevision) <
      Number(revision)
        ? [
            {
              revision,
              account_id: fixtureAccount.id,
              scope: `detail:${issue.id}:body`,
              reset: false,
            },
          ]
        : [],
    has_more: false,
    reset_required: false,
  }));
  mockTauriCommandResult("collaboration_repositories", fixtureRepositories);
  mockTauriCommand("collaboration_items", () => ({
    ...fixturePage,
    items: [issue],
    revision,
    authorization_view: authorizationView,
  }));
  mockTauriCommand("collaboration_item", (payload) => ({
    pending_intent: null,

    item: {
      ...issue,
      account_id: (payload as { accountId: string }).accountId,
    },
    revision,
    authorization_view: authorizationView,
  }));
  mockTauriCommand("collaboration_detail", () => ({
    ...body,
    revision,
    authorization_view: authorizationView,
  }));
  mockTauriCommandResult("collaboration_draft", null);
  mockTauriCommand("collaboration_contextual_capabilities", (payload) => {
    const { request } = payload as { request: ContextCapabilityRequest };
    const account =
      accounts.accounts.find((entry) => entry.id === request.account_id) ??
      fixtureAccount;
    const context = fixtureContextualCapabilities(account, request.target);
    return {
      ...context,
      revision,
      authorization_view: authorizationView,
      facets: context.facets.map((policy) =>
        policy.facet === "issue_details"
          ? {
              ...policy,
              saved_read: denied
                ? { state: "unavailable", reason: "permission_denied" }
                : { state: "supported", reason: null },
              synchronize: denied
                ? { state: "unavailable", reason: "permission_denied" }
                : paused
                  ? { state: "unavailable", reason: "temporarily_unavailable" }
                  : { state: "supported", reason: null },
              observation: "complete",
              can_recheck_access: denied,
              sync: paused
                ? {
                    state: "rate_limited",
                    next_retry_at: "2099-10-03T12:00:00Z",
                    last_success_at: null,
                    error: null,
                  }
                : policy.sync,
            }
          : policy,
      ),
    };
  });
});

afterEach(() => {
  for (const stop of stops.splice(0)) stop();
  for (const cache of caches.splice(0)) cache.clear();
});

async function open() {
  const cache = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  caches.push(cache);
  stops.push(collaboration.installBridge(cache));
  await act(async () => {
    await collaboration.wake();
  });
  render(
    <StrictMode>
      <QueryClientProvider client={cache}>
        <CollaborationWorkspace kind="issue" />
      </QueryClientProvider>
    </StrictMode>,
  );
  const user = userEvent.setup();
  await user.click(await screen.findByText(issue.title));
  return {
    user,
    detail: within(screen.getByRole("article", { name: "Saved item detail" })),
  };
}

async function change() {
  revision = String(Number(revision) + 1);
  await act(async () => {
    await collaboration.wake();
  });
}

describe("cached issue detail authority", () => {
  it.each([
    null,
    "",
  ])("keeps known %s description, absent reason and empty collections authoritative", async (text) => {
    body.body = { state: "known", text };
    Object.assign(body.metadata!.values, {
      title: "Issue with an absent reason",
      state: "closed",
      state_reason: null,
      labels: [],
      assignees: [],
      milestone: null,
    });
    const hydrate = mockTauriCommandResult("collaboration_hydrate_detail", {
      job_id: "must-not-run",
    });
    const { detail } = await open();
    expect(
      await detail.findByRole("heading", {
        name: "Issue with an absent reason",
      }),
    ).toBeVisible();
    expect(
      detail.getByText(
        text === null
          ? "This resource has no description."
          : "This description is empty.",
      ),
    ).toBeVisible();
    for (const value of [
      "No state reason",
      "No labels",
      "No assignees",
      "No milestone",
    ]) {
      expect(detail.getByText(value)).toBeVisible();
    }
    expect(detail.queryByText("completed")).not.toBeInTheDocument();
    expect(detail.queryByText("Head branch / SHA")).not.toBeInTheDocument();
    expect(detail.queryByRole("region", { name: "Reviews" })).toBeNull();
    expect(detail.queryByRole("region", { name: "Checks" })).toBeNull();
    expect(
      detail.queryByRole("button", { name: /Merge/ }),
    ).not.toBeInTheDocument();
    expect(hydrate).not.toHaveBeenCalled();
  });

  it("renders Unicode and additive issue reasons from saved data during an account cooldown", async () => {
    paused = true;
    body.body = { state: "known", text: "保存済みの説明 — مرحبا 🌱" };
    Object.assign(body.metadata!.values, {
      title: "修復済みの課題 🌱",
      state: "future-provider-state",
      state_reason: "future-reason — 完了",
      labels: [{ provider_id: null, name: "日本語ラベル", color: null }],
    });
    const hydrate = mockTauriCommandResult("collaboration_hydrate_detail", {
      job_id: "must-not-run",
    });
    const { detail } = await open();
    expect(
      await detail.findByRole("heading", { name: "修復済みの課題 🌱" }),
    ).toBeVisible();
    for (const value of [
      "保存済みの説明 — مرحبا 🌱",
      "future-provider-state",
      "future-reason — 完了",
      "日本語ラベル",
    ]) {
      expect(detail.getByText(value)).toBeVisible();
    }
    expect(
      detail.getByRole("button", { name: "Sync full description" }),
    ).toBeDisabled();
    expect(detail.getByText(/Sync is paused until/)).toBeVisible();
    expect(hydrate).not.toHaveBeenCalled();
  });

  it("retains an issue reason and Unicode text through omission and a later validation with one Body lease", async () => {
    body.body = { state: "known", text: "Issue history — 保存された本文" };
    body.evidence.freshness = "stale";
    const demands = mockForegroundDemand();
    const bodyAcquisitions = () =>
      demands.acquire.mock.calls.filter(
        ([payload]) =>
          (payload as { request: AcquireDemandRequest }).request.target.kind ===
          "detail",
      );
    const hydrate = mockTauriCommandResult("collaboration_hydrate_detail", {
      job_id: "one-selected-issue-intent",
    });
    const { detail } = await open();
    expect(await detail.findByText("completed")).toBeVisible();
    await waitFor(() => expect(bodyAcquisitions()).toHaveLength(1));
    expect(bodyAcquisitions()[0]?.[0]).toEqual({
      request: {
        account_id: fixtureAccount.id,
        authorization_epoch: fixtureAccount.authorization_epoch,
        owner_generation: "1",
        target: {
          kind: "detail",
          repository_id: null,
          subject_id: issue.id,
          facet: "body",
        },
      },
    });
    expect(hydrate).not.toHaveBeenCalled();
    body.metadata!.fields = body.metadata!.fields.map((field) =>
      field.field === "state_reason"
        ? {
            ...field,
            observed_state: "omitted",
            stale_at: "2020-01-01T00:00:00Z",
          }
        : field,
    );
    body.evidence.observed_state = "omitted";
    await change();
    expect(
      await detail.findByText("Latest provider value omitted"),
    ).toBeVisible();
    expect(detail.getByText("completed")).toBeVisible();
    expect(detail.getByText("Issue history — 保存された本文")).toBeVisible();
    expect(detail.getByText("May be stale")).toBeVisible();
    expect(bodyAcquisitions()).toHaveLength(1);
    expect(hydrate).not.toHaveBeenCalled();
    body.evidence.freshness = "fresh";
    body.evidence.facet_revision = "12";
    await change();
    expect(await detail.findByText("completed")).toBeVisible();
    expect(detail.getByText("Latest provider value omitted")).toBeVisible();
    expect(bodyAcquisitions()).toHaveLength(1);
    expect(hydrate).not.toHaveBeenCalled();
  });

  it.each([
    "permission_denied",
    "inactive_membership",
  ])("suppresses issue data after %s while saving only the private editor's inspected generation", async (loss) => {
    const hydrate = mockTauriCommandResult("collaboration_hydrate_detail", {
      job_id: "must-remain-manual",
    });
    mockTauriCommandResult("collaboration_draft", {
      account_id: fixtureAccount.id,
      subject_id: issue.id,
      body: "Private saved issue draft",
      generation: "3",
    });
    const { user, detail } = await open();
    expect(await detail.findByText("completed")).toBeVisible();
    await user.type(await detail.findByLabelText("Private draft"), " — 編集");
    if (loss === "permission_denied") {
      denied = true;
      authorizationView = "2";
    } else {
      body = {
        ...body,
        body: { state: "not_loaded", text: null },
        metadata: null,
        evidence: {
          ...body.evidence,
          availability: "unavailable",
          access_reason: "permission_denied",
        },
      };
    }
    await change();
    await waitFor(() =>
      expect(detail.queryByText("completed")).not.toBeInTheDocument(),
    );
    expect(
      detail.queryByRole("heading", { name: "Authoritative detail title" }),
    ).not.toBeInTheDocument();
    expect(detail.queryByText(issue.title)).not.toBeInTheDocument();
    expect(
      detail.queryByText("Full cached resource description"),
    ).not.toBeInTheDocument();
    expect(detail.getByLabelText("Private draft")).toHaveValue(
      "Private saved issue draft — 編集",
    );
    const save = mockTauriCommand("collaboration_save_draft", (payload) => {
      const { draft } = payload as {
        draft: { body: string; generation: string };
      };
      expect(draft.generation).toBe("3");
      expect(draft.body).toBe("Private saved issue draft — 編集");
      return {
        ...draft,
        account_id: fixtureAccount.id,
        subject_id: issue.id,
        generation: "4",
      };
    });
    await user.click(detail.getByRole("button", { name: "Save draft" }));
    await waitFor(() => expect(save).toHaveBeenCalledTimes(1));
    expect(hydrate).not.toHaveBeenCalled();
  });

  it("isolates issue reasons and private drafts when another actor opens the same subject before an old read returns", async () => {
    const matches = Element.prototype.matches;
    vi.spyOn(Element.prototype, "matches").mockImplementation(function (
      this: Element,
      selector,
    ) {
      return [":modal", ":fullscreen", ":popover-open"].includes(selector)
        ? false
        : matches.call(this, selector);
    });
    const other: RemoteAccount = {
      ...fixtureAccount,
      id: "other-account",
      actor_id: "456",
      login: "other-user",
      authorization_epoch: "9",
    };
    accounts = { ...fixtureAccounts, accounts: [fixtureAccount, other] };
    let finishA!: (snapshot: DetailSnapshot) => void;
    mockTauriCommand("collaboration_detail", (payload) => {
      const { query } = payload as { query: { account_id: string } };
      if (query.account_id === fixtureAccount.id) {
        return new Promise<DetailSnapshot>((resolve) => {
          finishA = resolve;
        });
      }
      const result = issueBody({
        body: { state: "known", text: "Actor B issue — 日本語" },
      });
      result.evidence.authorization_epoch = "9";
      result.metadata!.values.title = "Actor B authoritative issue";
      result.metadata!.values.state_reason = "not_planned";
      return result;
    });
    mockTauriCommand("collaboration_draft", (payload) => ({
      account_id: (payload as { accountId: string }).accountId,
      subject_id: issue.id,
      body:
        (payload as { accountId: string }).accountId === other.id
          ? "Actor B private issue draft"
          : "Actor A private issue draft",
      generation: "1",
    }));
    const hydrate = mockTauriCommandResult("collaboration_hydrate_detail", {
      job_id: "must-not-run",
    });
    const { user } = await open();
    await user.type(await screen.findByLabelText("Private draft"), " A edits");
    await user.click(screen.getByLabelText("Provider account"));
    await user.click(
      await screen.findByRole("option", { name: "GitHub · @other-user" }),
    );
    await user.click(await screen.findByText(issue.title));
    expect(
      await screen.findByRole("heading", {
        name: "Actor B authoritative issue",
      }),
    ).toBeVisible();
    expect(screen.getByText("not_planned")).toBeVisible();
    expect(screen.getByLabelText("Private draft")).toHaveValue(
      "Actor B private issue draft",
    );
    await act(async () => {
      finishA(issueBody());
    });
    expect(screen.queryByText("completed")).not.toBeInTheDocument();
    expect(
      screen.queryByRole("heading", { name: "Authoritative detail title" }),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByText("Actor A private issue draft"),
    ).not.toBeInTheDocument();
    expect(screen.getByText("Actor B issue — 日本語")).toBeVisible();
    expect(screen.getByLabelText("Private draft")).toHaveValue(
      "Actor B private issue draft",
    );
    expect(hydrate).not.toHaveBeenCalled();
  });
});

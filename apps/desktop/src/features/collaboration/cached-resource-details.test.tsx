import {
  collaboration,
  type RemoteAccount,
  type RemoteItemKind,
} from "@gitru/collaboration-client";
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

let revision = "10";
let authorizationView = "1";
let accounts = fixtureAccounts;
let summary = fixtureItem;
let body: DetailSnapshot;
let denied = false;
let demands: ReturnType<typeof mockForegroundDemand>;
const stops: Array<() => void> = [];
const caches: QueryClient[] = [];
beforeEach(() => {
  revision = "10";
  authorizationView = "1";
  accounts = fixtureAccounts;
  summary = fixtureItem;
  body = fixtureBody();
  denied = false;
  demands = mockForegroundDemand();
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
        Number(revision) && revision !== "10"
        ? [
            {
              revision,
              account_id: fixtureAccount.id,
              scope: `detail:${summary.id}:body`,
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
    items: [summary],
    revision,
    authorization_view: authorizationView,
  }));
  mockTauriCommand("collaboration_item", () => ({
    pending_intent: null,

    item: summary,
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
      accounts.accounts.find(
        (candidate) => candidate.id === request.account_id,
      ) ?? fixtureAccount;
    const context = fixtureContextualCapabilities(account, request.target);
    return {
      ...context,
      revision,
      authorization_view: authorizationView,
      facets: context.facets.map((policy) =>
        ["pull_details", "issue_details"].includes(policy.facet)
          ? {
              ...policy,
              saved_read: denied
                ? { state: "unavailable", reason: "permission_denied" }
                : { state: "supported", reason: null },
              synchronize: denied
                ? { state: "unavailable", reason: "permission_denied" }
                : { state: "supported", reason: null },
              observation: "complete",
              can_recheck_access: denied,
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

async function open(kind: RemoteItemKind = "pull_request") {
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
        <CollaborationWorkspace kind={kind} />
      </QueryClientProvider>
    </StrictMode>,
  );
  const user = userEvent.setup();
  await user.click(await screen.findByText(summary.title));
  return {
    user,
    detail: within(screen.getByRole("article", { name: "Saved item detail" })),
    cache,
  };
}
async function change() {
  revision = String(Number(revision) + 1);
  await act(async () => {
    await collaboration.wake();
  });
}

function bodyAcquisitions() {
  return demands.acquire.mock.calls.filter(
    ([payload]) =>
      (payload as { request: AcquireDemandRequest }).request.target.kind ===
      "detail",
  );
}

describe("cached PR and issue detail views", () => {
  it("retains one Body interest after an upgrade even when the saved description is fresh", async () => {
    body.metadata = null;
    const hydrate = mockTauriCommandResult("collaboration_hydrate_detail", {
      job_id: "metadata-upgrade",
    });
    const { detail } = await open();
    expect(
      await detail.findByText("Full cached resource description"),
    ).toBeVisible();
    expect(
      detail.getByRole("heading", { name: fixtureItem.title }),
    ).toBeVisible();
    await waitFor(() => expect(bodyAcquisitions()).toHaveLength(1));
    body = fixtureBody();
    await change();
    expect(
      await detail.findByRole("heading", {
        name: "Authoritative detail title",
      }),
    ).toBeVisible();
    expect(bodyAcquisitions()).toHaveLength(1);
    expect(hydrate).not.toHaveBeenCalled();
  });

  it("retains missing partial description interest without durable admission or omitted-observation loops", async () => {
    body.body = { state: "omitted", text: null };
    body.evidence = {
      ...body.evidence,
      availability: "partial",
      freshness: "unknown",
      observed_state: "omitted",
    };
    const hydrate = mockTauriCommandResult("collaboration_hydrate_detail", {
      job_id: "partial-description",
    });
    const { detail } = await open();
    await waitFor(() => expect(bodyAcquisitions()).toHaveLength(1));
    expect(
      detail.getByText(/The provider omitted this description/),
    ).toBeVisible();
    await change();
    expect(
      await detail.findByText(/The provider omitted this description/),
    ).toBeVisible();
    expect(bodyAcquisitions()).toHaveLength(1);
    expect(hydrate).not.toHaveBeenCalled();
  });

  it("retains oversized description interest once and leaves retry eligibility to native policy", async () => {
    body.body = { state: "oversized", text: null };
    body.evidence = {
      ...body.evidence,
      availability: "partial",
      freshness: "unknown",
      observed_state: "oversized",
    };
    const hydrate = mockTauriCommandResult("collaboration_hydrate_detail", {
      job_id: "must-not-run",
    });
    const { detail } = await open();
    expect(
      await detail.findByText("This description exceeds the local text limit."),
    ).toBeVisible();
    await change();
    expect(bodyAcquisitions()).toHaveLength(1);
    expect(hydrate).not.toHaveBeenCalled();
  });

  it("renders cached body/metadata immediately while the separate Body lease remains pending", async () => {
    body.evidence.freshness = "stale";
    let finish!: (
      receipt: import("@gitru/commands").DemandLeaseReceipt,
    ) => void;
    mockTauriCommand("collaboration_acquire_demand", (payload) => {
      const { request } = payload as { request: AcquireDemandRequest };
      return request.target.kind === "detail"
        ? new Promise<import("@gitru/commands").DemandLeaseReceipt>(
            (resolve) => {
              finish = resolve;
            },
          )
        : {
            lease_id: `other-${request.target.kind}`,
            owner_generation: request.owner_generation,
            expires_in_seconds: 45,
            renew_after_seconds: 15,
          };
    });
    const hydrate = mockTauriCommandResult("collaboration_hydrate_detail", {
      job_id: "must-not-run",
    });
    const { detail } = await open();
    expect(
      await detail.findByRole("heading", {
        name: "Authoritative detail title",
      }),
    ).toBeVisible();
    expect(detail.getByText("Full cached resource description")).toBeVisible();
    expect(detail.getByText("detail-label")).toBeVisible();
    await waitFor(() => expect(finish).toBeDefined());
    await act(async () => {
      finish({
        lease_id: "pending-body",
        owner_generation: "1",
        expires_in_seconds: 45,
        renew_after_seconds: 15,
      });
    });
    expect(hydrate).not.toHaveBeenCalled();
  });

  it.each([
    null,
    "",
  ])("renders issue metadata for authoritative description %s without hydration", async (text) => {
    summary = { ...fixtureItem, kind: "issue", is_draft: null };
    body = fixtureBody({
      body: { state: "known", text },
      metadata: fixtureMetadata("issue"),
    });
    const hydrate = mockTauriCommandResult("collaboration_hydrate_detail", {
      job_id: "must-not-run",
    });
    const { detail } = await open("issue");
    expect(
      await detail.findByRole("heading", {
        name: "Authoritative detail title",
      }),
    ).toBeVisible();
    expect(
      detail.getByText(
        text === null
          ? "This resource has no description."
          : "This description is empty.",
      ),
    ).toBeVisible();
    expect(detail.getByText("Detail milestone")).toBeVisible();
    expect(detail.queryByText("Head branch / SHA")).not.toBeInTheDocument();
    expect(hydrate).not.toHaveBeenCalled();
    expect(bodyAcquisitions()).toHaveLength(1);
  });

  it("reuses Body interest across 304 and retained omitted metadata revisions", async () => {
    body.evidence.freshness = "stale";
    const hydrate = mockTauriCommandResult("collaboration_hydrate_detail", {
      job_id: "queued",
    });
    const { detail } = await open();
    await waitFor(() => expect(bodyAcquisitions()).toHaveLength(1));
    body = {
      ...body,
      evidence: { ...body.evidence, freshness: "fresh", facet_revision: "11" },
    };
    await change();
    expect(
      await detail.findByText("Full cached resource description"),
    ).toBeVisible();
    body = {
      ...body,
      metadata: {
        ...fixtureMetadata(),
        fields: fixtureMetadata().fields.map((field) =>
          field.field === "labels"
            ? { ...field, observed_state: "omitted" }
            : field,
        ),
      },
      evidence: {
        ...body.evidence,
        freshness: "stale",
        facet_revision: "12",
        observed_state: "omitted",
      },
    };
    await change();
    expect(
      await detail.findByText("Latest provider value omitted"),
    ).toBeVisible();
    expect(detail.getByText("detail-label")).toBeVisible();
    expect(bodyAcquisitions()).toHaveLength(1);
    expect(hydrate).not.toHaveBeenCalled();
  });

  it("reuses the canonical Body lease when an accepted parent head changes", async () => {
    body.evidence.freshness = "stale";
    const hydrate = mockTauriCommandResult("collaboration_hydrate_detail", {
      job_id: "queued",
    });
    const { cache, detail } = await open();
    await waitFor(() => expect(bodyAcquisitions()).toHaveLength(1));
    summary = { ...summary, head_oid: "new-summary-head" };
    await act(async () => {
      await cache.invalidateQueries({
        predicate: (query) => query.queryKey[4] === "item",
      });
    });
    expect(bodyAcquisitions()).toHaveLength(1);
    body = fixtureBody();
    body.metadata!.values.head!.oid = "c".repeat(40);
    body.metadata!.values.title = "New head detail title";
    await change();
    expect(
      await detail.findByRole("heading", { name: "New head detail title" }),
    ).toBeVisible();
    expect(detail.getByText("c".repeat(40))).toBeVisible();
    expect(bodyAcquisitions()).toHaveLength(1);
    expect(hydrate).not.toHaveBeenCalled();
  });

  it("hides denied Body/header metadata while preserving private text and its inspected generation", async () => {
    const hydrate = mockTauriCommandResult("collaboration_hydrate_detail", {
      job_id: "manual-recheck",
    });
    const { user, detail } = await open();
    expect(
      await detail.findByRole("heading", {
        name: "Authoritative detail title",
      }),
    ).toBeVisible();
    const editor = await detail.findByLabelText("Private draft");
    await user.type(editor, "Private unsaved resource text");
    denied = true;
    authorizationView = "2";
    await change();
    await waitFor(() =>
      expect(
        detail.queryByRole("heading", { name: "Authoritative detail title" }),
      ).not.toBeInTheDocument(),
    );
    expect(detail.queryByText(fixtureItem.title)).not.toBeInTheDocument();
    expect(
      detail.queryByText("Full cached resource description"),
    ).not.toBeInTheDocument();
    expect(detail.getByLabelText("Private draft")).toHaveValue(
      "Private unsaved resource text",
    );
    expect(hydrate).not.toHaveBeenCalled();
    expect(bodyAcquisitions()).toHaveLength(1);
    expect(demands.release).toHaveBeenCalled();
    const save = mockTauriCommand("collaboration_save_draft", (payload) => {
      const draft = (payload as { draft: { body: string; generation: string } })
        .draft;
      expect(draft.generation).toBe("0");
      return {
        ...draft,
        account_id: fixtureAccount.id,
        subject_id: fixtureItem.id,
        generation: "1",
      };
    });
    await user.click(detail.getByRole("button", { name: "Save draft" }));
    await waitFor(() => expect(save).toHaveBeenCalledTimes(1));
  });

  it("copies current detail text and exports only its saved generation", async () => {
    mockTauriCommandResult("collaboration_draft", {
      account_id: fixtureAccount.id,
      subject_id: fixtureItem.id,
      body: "Saved detail draft",
      generation: "1",
    });
    const save = mockTauriCommand("collaboration_save_draft", (payload) => ({
      ...(
        payload as {
          draft: {
            account_id: string;
            subject_id: string;
            body: string;
            generation: string;
          };
        }
      ).draft,
      generation: "2",
    }));
    const exportDraft = mockTauriCommandResult(
      "collaboration_export_draft",
      true,
    );
    const { user, detail } = await open();
    const copy = vi.spyOn(navigator.clipboard, "writeText").mockResolvedValue();
    const editor = await detail.findByLabelText("Private draft");
    await user.type(editor, " with current edits");
    await user.click(detail.getByRole("button", { name: "Copy draft text" }));
    expect(copy).toHaveBeenCalledWith("Saved detail draft with current edits");
    expect(
      detail.getByRole("button", { name: "Export saved draft" }),
    ).toBeDisabled();
    expect(detail.getByText("Save changes before exporting.")).toBeVisible();
    await user.click(detail.getByRole("button", { name: "Save draft" }));
    await waitFor(() => expect(save).toHaveBeenCalledTimes(1));
    await waitFor(() =>
      expect(
        detail.getByRole("button", { name: "Export saved draft" }),
      ).toBeEnabled(),
    );
    await user.click(
      detail.getByRole("button", { name: "Export saved draft" }),
    );
    expect(exportDraft).toHaveBeenCalledExactlyOnceWith({
      accountId: fixtureAccount.id,
      subjectId: fixtureItem.id,
      generation: "2",
    });
    expect(await detail.findByText("Saved draft exported.")).toBeVisible();
  });

  it("uses one explicit durable intent only after the user requests Sync", async () => {
    const hydrate = mockTauriCommandResult("collaboration_hydrate_detail", {
      job_id: "manual-request",
    });
    const { user, detail } = await open();
    await waitFor(() => expect(bodyAcquisitions()).toHaveLength(1));
    expect(hydrate).not.toHaveBeenCalled();
    await user.click(
      detail.getByRole("button", { name: "Sync full description" }),
    );
    await waitFor(() =>
      expect(hydrate).toHaveBeenCalledExactlyOnceWith({
        request: {
          account_id: fixtureAccount.id,
          authorization_epoch: fixtureAccount.authorization_epoch,
          subject_id: fixtureItem.id,
          facet: "body",
        },
      }),
    );
    expect(bodyAcquisitions()).toHaveLength(1);
  });

  it("releases selected Body interest on Back while retaining the visible feed and repository leases", async () => {
    const { user, detail } = await open();
    await waitFor(() => expect(bodyAcquisitions()).toHaveLength(1));
    const bodyLease = `fixture-lease-${demands.acquire.mock.calls.indexOf(bodyAcquisitions()[0]) + 1}`;
    const initial = demands.acquire.mock.calls.length;
    await user.click(detail.getByRole("button", { name: "Back to list" }));
    await waitFor(() =>
      expect(demands.release).toHaveBeenCalledExactlyOnceWith({
        request: { lease_id: bodyLease },
      }),
    );
    expect(
      screen.queryByRole("article", { name: "Saved item detail" }),
    ).not.toBeInTheDocument();
    expect(demands.acquire).toHaveBeenCalledTimes(initial);
  });

  it("releases interest on native hide and restores only the current visible targets after activation", async () => {
    const hydrate = mockTauriCommandResult("collaboration_hydrate_detail", {
      job_id: "must-not-run",
    });
    const { detail } = await open();
    await waitFor(() => expect(bodyAcquisitions()).toHaveLength(1));
    const initial = demands.acquire.mock.calls.length;
    await act(async () => {
      demands.emit({ generation: "2", active: false });
    });
    await waitFor(() => expect(demands.release).toHaveBeenCalledTimes(initial));
    expect(detail.getByText("Full cached resource description")).toBeVisible();
    expect(demands.acquire).toHaveBeenCalledTimes(initial);
    await act(async () => {
      demands.emit({ generation: "3", active: true });
    });
    await waitFor(() =>
      expect(demands.acquire).toHaveBeenCalledTimes(initial * 2),
    );
    expect(bodyAcquisitions()).toHaveLength(2);
    for (const [payload] of demands.acquire.mock.calls.slice(initial))
      expect(
        (payload as { request: AcquireDemandRequest }).request.owner_generation,
      ).toBe("3");
    expect(hydrate).not.toHaveBeenCalled();
  });

  it("suppresses a late actor-A Body snapshot after switching the same subject to actor B", async () => {
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
    };
    accounts = { ...fixtureAccounts, accounts: [fixtureAccount, other] };
    let finishA!: (snapshot: DetailSnapshot) => void;
    mockTauriCommand("collaboration_detail", (payload) => {
      const { query } = payload as { query: { account_id: string } };
      return query.account_id === fixtureAccount.id
        ? new Promise<DetailSnapshot>((resolve) => {
            finishA = resolve;
          })
        : fixtureBody({
            metadata: {
              ...fixtureMetadata(),
              values: {
                ...fixtureMetadata().values,
                title: "Actor B metadata",
              },
            },
            body: { state: "known", text: "Actor B full description" },
          });
    });
    mockTauriCommand("collaboration_item", (payload) => ({
      pending_intent: null,

      item: {
        ...fixtureItem,
        account_id: (payload as { accountId: string }).accountId,
      },
      revision,
      authorization_view: authorizationView,
    }));
    mockTauriCommand("collaboration_draft", (payload) => ({
      account_id: (payload as { accountId: string }).accountId,
      subject_id: fixtureItem.id,
      body:
        (payload as { accountId: string }).accountId === other.id
          ? "Actor B private draft"
          : "Actor A private draft",
      generation: "1",
    }));
    const hydrate = mockTauriCommandResult("collaboration_hydrate_detail", {
      job_id: "must-not-run",
    });
    const { user } = await open();
    const editor = await screen.findByLabelText("Private draft");
    await user.type(editor, " A unsaved addition");
    await user.click(screen.getByLabelText("Provider account"));
    await user.click(
      await screen.findByRole("option", { name: "GitHub · @other-user" }),
    );
    await user.click(await screen.findByText(fixtureItem.title));
    expect(
      await screen.findByRole("heading", { name: "Actor B metadata" }),
    ).toBeVisible();
    expect(screen.getByLabelText("Private draft")).toHaveValue(
      "Actor B private draft",
    );
    await act(async () => {
      finishA(fixtureBody());
    });
    expect(
      screen.queryByRole("heading", { name: "Authoritative detail title" }),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByText("Full cached resource description"),
    ).not.toBeInTheDocument();
    expect(screen.getByText("Actor B full description")).toBeVisible();
    expect(
      bodyAcquisitions().map(
        ([payload]) =>
          (payload as { request: AcquireDemandRequest }).request.account_id,
      ),
    ).toEqual([fixtureAccount.id, other.id]);
    expect(demands.release).toHaveBeenCalled();
    expect(hydrate).not.toHaveBeenCalled();
  });
});

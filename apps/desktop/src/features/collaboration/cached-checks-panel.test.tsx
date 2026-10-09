import { collaboration } from "@gitru/collaboration-client";
import type {
  CheckV1,
  ContextFacetCapability,
  DetailEntry,
  DetailSnapshot,
} from "@gitru/commands";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { fixtureAccount } from "../../../tests/fixtures/collaboration";
import { fixtureBody } from "../../../tests/fixtures/resource-detail";
import { mockForegroundDemand } from "../../../tests/mocks/collaboration-demand";
import {
  CachedChecksPanel,
  summarizeCachedChecks,
} from "./cached-checks-panel";

const headA = "a".repeat(40);
const headB = "b".repeat(40);
const policy: ContextFacetCapability = {
  facet: "checks",
  saved_read: { state: "supported", reason: null },
  synchronize: { state: "supported", reason: null },
  remote_write: { state: "unsupported", reason: "not_implemented" },
  observation: "complete",
  sync: {
    state: "idle",
    last_success_at: null,
    next_retry_at: null,
    error: null,
  },
  can_recheck_access: false,
};

function entry(id: string, headOid: string, value: CheckV1): DetailEntry {
  return {
    id,
    provider_id: id,
    author: null,
    title: null,
    state: null,
    body: { state: "not_loaded", text: null },
    observed_body_state: "not_loaded",
    updated_at: null,
    head_oid: headOid,
    native: { kind: "check.v1", value },
    field_mask: ["check", "head_oid"],
    field_validations: [],
  };
}

function status(
  id: string,
  state: string,
  headOid = headA,
  allowFailure: boolean | null = null,
) {
  return entry(id, headOid, {
    kind: "commit_status",
    name: `Status ${id}`,
    state: { kind: "commit_status", state },
    description: { state: "known", text: `Description ${id}` },
    producer: "fixture-ci",
    started_at: null,
    completed_at: null,
    updated_at: "2026-10-07T00:00:00Z",
    allow_failure: allowFailure,
  });
}

function run(id: string, statusValue: string, conclusion: string | null) {
  return entry(id, headA, {
    kind: "check_run",
    name: `Run ${id}`,
    state: {
      kind: "check_run",
      status: statusValue,
      conclusion,
    },
    description: { state: "omitted", text: null },
    producer: null,
    started_at: null,
    completed_at: null,
    updated_at: null,
    allow_failure: null,
  });
}

function snapshot(
  entries: DetailEntry[],
  overrides: Partial<DetailSnapshot> = {},
) {
  const base = fixtureBody({
    metadata: null,
    body: { state: "not_loaded", text: null },
    entries,
    evidence: {
      ...fixtureBody().evidence,
      facet: "checks",
      availability: "ready",
      freshness: "fresh",
      coverage: {
        state: "complete",
        validated_at: "2026-10-07T00:00:00Z",
        remote_has_more: false,
      },
      observed_state: "not_loaded",
    },
  });
  return { ...base, ...overrides };
}

afterEach(() => {
  vi.restoreAllMocks();
});

describe("cached exact-head checks", () => {
  it("gates the common aggregate on complete fresh exact-head evidence", () => {
    expect(
      summarizeCachedChecks(
        snapshot([
          status("status", "success"),
          run("run", "completed", "neutral"),
        ]),
        headA,
      ),
    ).toEqual({ state: "passed", authoritative: true, total: 2 });

    const partial = snapshot([status("status", "success")]);
    partial.evidence.coverage.state = "partial";
    expect(summarizeCachedChecks(partial, headA)).toEqual({
      state: "partial",
      authoritative: false,
      total: 1,
    });

    const stale = snapshot([status("status", "success")]);
    stale.evidence.freshness = "stale";
    expect(summarizeCachedChecks(stale, headA).state).toBe("stale");
    expect(
      summarizeCachedChecks(
        snapshot([status("status", "success", headA)]),
        headB,
      ),
    ).toEqual({ state: "stale", authoritative: false, total: 1 });
    expect(
      summarizeCachedChecks(
        snapshot([status("allowed", "failure", headA, true)]),
        headA,
      ),
    ).toEqual({ state: "failed", authoritative: true, total: 1 });
    expect(
      summarizeCachedChecks(
        snapshot([run("future", "completed", "future")]),
        headA,
      ),
    ).toEqual({ state: "unknown", authoritative: false, total: 1 });
    expect(
      summarizeCachedChecks(snapshot([run("queued", "queued", null)]), headA),
    ).toEqual({ state: "pending", authoritative: false, total: 1 });
    for (const entries of [
      [status("failed", "failure"), run("pending", "queued", null)],
      [run("pending", "queued", null), status("failed", "failure")],
      [status("failed", "failure"), status("unknown", "future")],
      [status("unknown", "future"), status("failed", "failure")],
    ])
      expect(summarizeCachedChecks(snapshot(entries), headA)).toEqual({
        state: "failed",
        authoritative: false,
        total: 2,
      });
    expect(summarizeCachedChecks(snapshot([]), headA)).toEqual({
      state: "empty",
      authoritative: false,
      total: 0,
    });
    expect(
      summarizeCachedChecks(
        snapshot(
          Array.from({ length: 50 }, (_, index) =>
            status(String(index), "success"),
          ),
          { next_cursor: "after-50" },
        ),
        headA,
      ),
    ).toEqual({ state: "partial", authoritative: false, total: 50 });
  });

  it("waits for saved exact-head metadata before issuing a local check read", async () => {
    const detail = vi.spyOn(collaboration.transport, "detail");
    mockForegroundDemand();
    const cache = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    });
    const rendered = render(
      <QueryClientProvider client={cache}>
        <CachedChecksPanel
          account={fixtureAccount}
          subjectId={fixtureBody().subject_id}
          policy={policy}
          bodyContext={null}
        />
      </QueryClientProvider>,
    );
    expect(
      await screen.findByText(
        "Checks need saved current-head metadata before they can be read.",
      ),
    ).toBeVisible();
    expect(detail).not.toHaveBeenCalled();
    rendered.unmount();
    cache.clear();
  });

  it("reads locally, maintains visible demand, and syncs only after an explicit action", async () => {
    const saved = snapshot([status("lint", "success")]);
    const detail = vi
      .spyOn(collaboration.transport, "detail")
      .mockResolvedValue(saved);
    const hydrate = vi
      .spyOn(collaboration.transport, "hydrateDetail")
      .mockResolvedValue({ job_id: "checks-job" });
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
    const stop = collaboration.installBridge(cache);
    const user = userEvent.setup();
    const view = render(
      <QueryClientProvider client={cache}>
        <CachedChecksPanel
          account={fixtureAccount}
          subjectId={saved.subject_id}
          policy={policy}
          bodyContext={{ headOid: headA, facetRevision: "10" }}
        />
      </QueryClientProvider>,
    );
    expect(
      await screen.findByText(
        "All reported checks passed for this exact head.",
      ),
    ).toBeVisible();
    expect(screen.getByText("Status lint")).toBeVisible();
    expect(screen.getByText("success · fixture-ci")).toBeVisible();
    expect(detail).toHaveBeenCalledExactlyOnceWith({
      account_id: fixtureAccount.id,
      subject_id: saved.subject_id,
      facet: "checks",
      cursor: null,
      limit: 50,
    });
    await waitFor(() => expect(demand.acquire).toHaveBeenCalledTimes(1));
    expect(hydrate).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "Sync checks" }));
    expect(hydrate).toHaveBeenCalledExactlyOnceWith({
      account_id: fixtureAccount.id,
      authorization_epoch: fixtureAccount.authorization_epoch,
      subject_id: saved.subject_id,
      facet: "checks",
    });
    view.unmount();
    stop();
    cache.clear();
  });

  it("aggregates every saved local page while rendering only the first page", async () => {
    const first = snapshot(
      Array.from({ length: 50 }, (_, index) =>
        status(`visible-${index}`, "success"),
      ),
      { next_cursor: "after-50" },
    );
    const final = snapshot([status("hidden-failure", "failure")]);
    const detail = vi
      .spyOn(collaboration.transport, "detail")
      .mockResolvedValueOnce(first)
      .mockResolvedValueOnce(final);
    mockForegroundDemand();
    const cache = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    });
    const view = render(
      <QueryClientProvider client={cache}>
        <CachedChecksPanel
          account={fixtureAccount}
          subjectId={first.subject_id}
          policy={policy}
          bodyContext={{ headOid: headA, facetRevision: "10" }}
        />
      </QueryClientProvider>,
    );

    expect(
      await screen.findByText("One or more reported checks failed."),
    ).toBeVisible();
    expect(screen.getByText("Exact head")).toBeVisible();
    expect(screen.getByText("Status visible-0")).toBeVisible();
    expect(screen.queryByText("Status hidden-failure")).not.toBeInTheDocument();
    expect(
      screen.getByText(
        "More saved checks are available. This view shows the first 50.",
      ),
    ).toBeVisible();
    expect(detail).toHaveBeenNthCalledWith(1, {
      account_id: fixtureAccount.id,
      subject_id: first.subject_id,
      facet: "checks",
      cursor: null,
      limit: 50,
    });
    expect(detail).toHaveBeenNthCalledWith(2, {
      account_id: fixtureAccount.id,
      subject_id: first.subject_id,
      facet: "checks",
      cursor: "after-50",
      limit: 50,
    });
    view.unmount();
    cache.clear();
  });

  it("uses Body head and revision in the query identity before showing replacement data", async () => {
    const old = snapshot([status("old", "success", headA)]);
    const foreign = snapshot([status("old", "success", headA)]);
    let finish!: (value: DetailSnapshot) => void;
    vi.spyOn(collaboration.transport, "detail")
      .mockResolvedValueOnce(old)
      .mockImplementationOnce(
        () =>
          new Promise<DetailSnapshot>((resolve) => {
            finish = resolve;
          }),
      );
    vi.spyOn(collaboration.transport, "hydrateDetail").mockResolvedValue({
      job_id: "unused",
    });
    mockForegroundDemand();
    const cache = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    });
    const rendered = render(
      <QueryClientProvider client={cache}>
        <CachedChecksPanel
          account={fixtureAccount}
          subjectId={old.subject_id}
          policy={policy}
          bodyContext={{ headOid: headA, facetRevision: "10" }}
        />
      </QueryClientProvider>,
    );
    expect(
      await screen.findByText(
        "All reported checks passed for this exact head.",
      ),
    ).toBeVisible();
    rendered.rerender(
      <QueryClientProvider client={cache}>
        <CachedChecksPanel
          account={fixtureAccount}
          subjectId={old.subject_id}
          policy={policy}
          bodyContext={{ headOid: headB, facetRevision: "11" }}
        />
      </QueryClientProvider>,
    );
    expect(await screen.findByText("Loading saved checks…")).toBeVisible();
    expect(
      screen.queryByText("All reported checks passed for this exact head."),
    ).not.toBeInTheDocument();
    await waitFor(() => expect(finish).toBeDefined());
    await act(async () => finish(foreign));
    expect(
      await screen.findByText(
        "Saved checks are stale or belong to a different pull request head.",
      ),
    ).toBeVisible();
    expect(screen.getByText("Not authoritative")).toBeVisible();
    rendered.unmount();
    cache.clear();
  });
});

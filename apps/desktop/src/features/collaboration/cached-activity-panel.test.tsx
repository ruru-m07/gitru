import { type ActivityEvent, collaboration } from "@gitru/collaboration-client";
import type {
  ContextFacetCapability,
  DetailEntry,
  DetailSnapshot,
  RemoteAccount,
} from "@gitru/commands";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { fixtureAccount } from "../../../tests/fixtures/collaboration";
import { fixtureBody } from "../../../tests/fixtures/resource-detail";
import { mockForegroundDemand } from "../../../tests/mocks/collaboration-demand";
import { CachedActivityPanel } from "./cached-activity-panel";

const subjectId = fixtureBody().subject_id;
const authorizationView = fixtureBody().authorization_view;
const policy: ContextFacetCapability = {
  facet: "activity",
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

function activity(
  id: string,
  overrides: Partial<ActivityEvent> = {},
): DetailEntry {
  return {
    id: `activity-${id}`,
    provider_id: id,
    author: "octocat",
    title: "Changed the title",
    state: null,
    body: { state: "known", text: "<img src=x onerror=alert(1)>" },
    observed_body_state: "known",
    updated_at: "2026-10-08T00:00:00Z",
    head_oid: null,
    native: {
      kind: "activity.v1",
      value: {
        kind: "renamed",
        supported: true,
        occurred_at: "2026-10-08T00:00:00Z",
        description: "Title changed safely",
        ...overrides,
      },
    },
    field_mask: ["body", "author", "updated_at", "activity"],
    field_validations: [],
  };
}

function unknownActivity(id: string): DetailEntry {
  return activity(id, {
    kind: "future_provider_event",
    supported: false,
    occurred_at: null,
    description: null,
  });
}

function snapshot(
  entries: DetailEntry[],
  nextCursor: string | null = null,
  overrides: Partial<DetailSnapshot> = {},
): DetailSnapshot {
  return fixtureBody({
    body: { state: "not_loaded", text: null },
    metadata: null,
    entries,
    next_cursor: nextCursor,
    evidence: {
      ...fixtureBody().evidence,
      facet: "activity",
      availability: "ready",
      freshness: "fresh",
      facet_revision: "activity-1",
      coverage: {
        state: nextCursor ? "partial" : "complete",
        validated_at: "2026-10-08T00:00:00Z",
        remote_has_more: nextCursor !== null,
      },
    },
    ...overrides,
  });
}

function mount(account: RemoteAccount = fixtureAccount) {
  const cache = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  vi.spyOn(collaboration.transport, "listen").mockResolvedValue(() => {});
  vi.spyOn(collaboration.transport, "changesSince").mockResolvedValue({
    revision: "10",
    authorization_view: authorizationView,
    changes: [],
    has_more: false,
    reset_required: false,
  });
  const stopBridge = collaboration.installBridge(cache);
  const user = userEvent.setup();
  const view = render(
    <QueryClientProvider client={cache}>
      <label>
        Unsaved edit
        <textarea defaultValue="draft remains" />
      </label>
      <CachedActivityPanel
        account={account}
        subjectId={subjectId}
        authorizationView={authorizationView}
        policy={policy}
      />
    </QueryClientProvider>,
  );
  return { cache, stopBridge, user, view };
}

afterEach(() => vi.restoreAllMocks());

describe("cached activity disclosure", () => {
  it("explains partial GitLab history and renders system notes as safe text", async () => {
    const saved = snapshot([activity("note:1", { kind: "system_note" })]);
    saved.entries[0].title = null;
    saved.evidence.coverage.state = "partial";
    vi.spyOn(collaboration.transport, "detail").mockResolvedValue(saved);
    mockForegroundDemand();
    const { cache, stopBridge, user, view } = mount({
      ...fixtureAccount,
      provider: "gitlab",
      host: "gitlab.com",
    });
    await user.click(screen.getByRole("button", { name: "Activity" }));
    expect(await screen.findByText("System note")).toBeVisible();
    expect(screen.getByText("Partial activity history")).toBeVisible();
    expect(
      screen.getByText(
        "GitLab activity includes system notes, state changes, and label changes. Each sync reads up to 1,000 records; other history may be missing.",
      ),
    ).toBeVisible();
    expect(screen.getByText("<img src=x onerror=alert(1)>")).toBeVisible();
    expect(document.querySelector("img[src='x']")).toBeNull();
    expect(
      screen.getByRole("button", { name: "Next saved activity" }),
    ).toBeDisabled();
    expect(screen.getByLabelText("Unsaved edit")).toHaveValue("draft remains");
    view.unmount();
    stopBridge();
    cache.clear();
  });

  it("does no work while closed, renders inert saved evidence, and syncs explicitly", async () => {
    const detail = vi
      .spyOn(collaboration.transport, "detail")
      .mockResolvedValue(
        snapshot([activity("1"), unknownActivity("2")], "activity-page-2"),
      );
    const hydrate = vi
      .spyOn(collaboration.transport, "hydrateDetail")
      .mockResolvedValue({ job_id: "activity-job" });
    const demand = mockForegroundDemand();
    const { cache, stopBridge, user, view } = mount();

    expect(detail).not.toHaveBeenCalled();
    expect(hydrate).not.toHaveBeenCalled();
    expect(demand.acquire).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "Activity" }));

    expect(await screen.findByText("Changed the title")).toBeVisible();
    expect(
      screen.getByText("Unsupported activity · future_provider_event"),
    ).toBeVisible();
    expect(screen.getByText("Event details are not interpreted")).toBeVisible();
    expect(screen.getByText("Partial activity history")).toBeVisible();
    expect(
      screen
        .getAllByText("<img src=x onerror=alert(1)>")
        .every((node) => node.isConnected),
    ).toBe(true);
    expect(document.querySelector("img[src='x']")).toBeNull();
    await waitFor(() => expect(demand.acquire).toHaveBeenCalledTimes(1));
    expect(hydrate).not.toHaveBeenCalled();

    await user.click(screen.getByRole("button", { name: "Sync activity" }));
    expect(hydrate).toHaveBeenCalledWith({
      account_id: fixtureAccount.id,
      authorization_epoch: fixtureAccount.authorization_epoch,
      subject_id: subjectId,
      facet: "activity",
    });

    await user.click(screen.getByRole("button", { name: "Activity" }));
    expect(screen.getByLabelText("Unsaved edit")).toHaveValue("draft remains");
    await waitFor(() => expect(demand.release).toHaveBeenCalledTimes(1));
    view.unmount();
    stopBridge();
    cache.clear();
  });

  it("pages locally, preserves dirty sibling state, and fences a foreign continuation", async () => {
    const calls: Array<string | null> = [];
    vi.spyOn(collaboration.transport, "detail").mockImplementation(
      async (query) => {
        calls.push(query.cursor);
        if (query.cursor === "activity-page-2") {
          const foreign = snapshot([activity("2")]);
          return { ...foreign, authorization_view: "foreign-view" };
        }
        return snapshot([activity("1")], "activity-page-2");
      },
    );
    vi.spyOn(collaboration.transport, "hydrateDetail").mockResolvedValue({
      job_id: "unused",
    });
    mockForegroundDemand();
    const { cache, stopBridge, user, view } = mount();
    const editor = screen.getByLabelText("Unsaved edit");
    await user.clear(editor);
    await user.type(editor, "locally edited");
    await user.click(screen.getByRole("button", { name: "Activity" }));
    expect(await screen.findByText("Changed the title")).toBeVisible();

    await user.click(
      screen.getByRole("button", { name: "Next saved activity" }),
    );
    expect(
      await screen.findByText("Account access changed. Reload this view."),
    ).toBeVisible();
    expect(editor).toHaveValue("locally edited");
    expect(calls).toContain("activity-page-2");
    view.unmount();
    stopBridge();
    cache.clear();
  });

  it.each([
    ["complete", "No activity was returned in the saved observation."],
    ["partial", "No activity is saved in this partial view."],
  ] as const)("labels an empty %s observation truthfully", async (state, copy) => {
    const saved = snapshot([]);
    saved.evidence.coverage.state = state;
    saved.evidence.availability = state === "partial" ? "partial" : "ready";
    vi.spyOn(collaboration.transport, "detail").mockResolvedValue(saved);
    vi.spyOn(collaboration.transport, "hydrateDetail").mockResolvedValue({
      job_id: "unused",
    });
    mockForegroundDemand();
    const { cache, stopBridge, user, view } = mount();
    await user.click(screen.getByRole("button", { name: "Activity" }));
    expect(await screen.findByText(copy)).toBeVisible();
    view.unmount();
    stopBridge();
    cache.clear();
  });
});

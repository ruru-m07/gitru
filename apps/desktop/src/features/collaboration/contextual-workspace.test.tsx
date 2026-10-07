import {
  collaboration,
  type InboxSemantics,
  type RemoteAccount,
} from "@gitru/collaboration-client";
import type {
  AccountSnapshot,
  ContextCapabilityRequest,
  ContextualCapabilitySnapshot,
  DetailSnapshot,
} from "@gitru/commands";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  fixtureAccount,
  fixtureAccounts,
  fixtureContextualCapabilities,
  fixtureInboxPage,
  fixtureItem,
  fixtureLocalInboxState,
  fixturePage,
  fixtureRepositories,
} from "../../../tests/fixtures/collaboration";
import { fixtureMetadata } from "../../../tests/fixtures/resource-detail";
import { mockForegroundDemand } from "../../../tests/mocks/collaboration-demand";
import {
  mockTauriCommand,
  mockTauriCommandResult,
} from "../../../tests/mocks/tauri";
import { useSavedInboxBadge } from "./sidebar-accounts";
import { CollaborationWorkspace } from "./workspace";

const caches: QueryClient[] = [];
const stops: Array<() => void> = [];
let currentAccounts: AccountSnapshot;
let currentView = "1";
let currentRevision = "10";
beforeEach(() => {
  mockForegroundDemand();
  currentAccounts = fixtureAccounts;
  currentView = "1";
  currentRevision = "10";
  vi.spyOn(collaboration.transport, "listen").mockResolvedValue(() => {});
  mockTauriCommand("collaboration_accounts", () => currentAccounts);
  mockTauriCommand("collaboration_changes_since", () => ({
    revision: currentRevision,
    authorization_view: currentView,
    changes: [],
    has_more: false,
    reset_required: false,
  }));
  mockTauriCommandResult("collaboration_repositories", fixtureRepositories);
  mockTauriCommandResult("collaboration_draft", null);
  mockTauriCommandResult("collaboration_item", {
    pending_intent: null,

    item: fixtureItem,
    revision: "10",
    authorization_view: "1",
  });
  mockTauriCommandResult("collaboration_discover_github_cli", {
    status: "not_installed",
    accounts: [],
  });
});
afterEach(() => {
  for (const stop of stops.splice(0)) stop();
  for (const cache of caches.splice(0)) cache.clear();
});

async function mount(component: React.ReactNode) {
  const cache = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  caches.push(cache);
  stops.push(collaboration.installBridge(cache));
  await act(async () => {
    await collaboration.wake();
  });
  return {
    cache,
    ...render(
      <QueryClientProvider client={cache}>{component}</QueryClientProvider>,
    ),
  };
}
function contextMock(
  semantics: InboxSemantics = "native_notifications",
  transform: (
    snapshot: ContextualCapabilitySnapshot,
    request: ContextCapabilityRequest,
  ) => ContextualCapabilitySnapshot | Promise<ContextualCapabilitySnapshot> = (
    snapshot,
  ) => snapshot,
) {
  return mockTauriCommand(
    "collaboration_contextual_capabilities",
    (payload) => {
      const { request } = payload as { request: ContextCapabilityRequest };
      const account = currentAccounts.accounts.find(
        (candidate) => candidate.id === request.account_id,
      );
      if (
        !account ||
        account.authorization_epoch !== request.authorization_epoch
      )
        throw { code: "stale_view", message: "fixture old authorization" };
      const snapshot = {
        ...fixtureContextualCapabilities(account, request.target, semantics),
        revision: currentRevision,
        authorization_view: currentView,
      };
      return transform(snapshot, request);
    },
  );
}
function deny(
  snapshot: ContextualCapabilitySnapshot,
  facet: "issues" | "pull_requests" | "inbox",
  reason: "permission_denied" | "missing_scope" | "not_implemented",
  recheck = false,
) {
  return {
    ...snapshot,
    facets: snapshot.facets.map((policy) =>
      policy.facet === facet
        ? {
            ...policy,
            saved_read: {
              state:
                reason === "not_implemented"
                  ? ("unsupported" as const)
                  : ("unavailable" as const),
              reason,
            },
            synchronize: { state: "unavailable" as const, reason },
            observation: "unknown" as const,
            can_recheck_access: recheck,
          }
        : policy,
    ),
  };
}
function BadgeProbe() {
  const badge = useSavedInboxBadge();
  return <output aria-label="Saved inbox badge">{badge ?? "none"}</output>;
}

describe("ordinary collaboration workspace across provider policies", () => {
  it.each([
    "native_notifications",
    "todos",
  ] as const)("uses %s filters and badge without the legacy GitHub grant flag", async (semantics) => {
    const account: RemoteAccount = {
      ...fixtureAccount,
      provider: semantics === "todos" ? "gitlab" : "github",
      host: semantics === "todos" ? "gitlab.com" : "github.com",
      notifications_supported: false,
    };
    currentAccounts = { ...fixtureAccounts, accounts: [account] };
    contextMock(semantics);
    const state = semantics === "todos" ? "pending" : "unread";
    const items = mockTauriCommand("collaboration_inbox", (payload) => {
      const { query } = payload as {
        query: { remote_state: string; local_state: string };
      };
      expect(query.remote_state).toBe(state);
      expect(query.local_state).toBe("inbox");
      return fixtureInboxPage([
        {
          ...fixtureItem,
          kind: "notification",
          state,
          unread: semantics === "todos" ? null : true,
        },
      ]);
    });
    await mount(
      <>
        <CollaborationWorkspace kind="notification" />
        <BadgeProbe />
      </>,
    );
    expect(
      await screen.findByRole("heading", {
        name: semantics === "todos" ? "To-dos" : "Inbox",
      }),
    ).toBeVisible();
    expect(await screen.findByText(fixtureItem.title)).toBeVisible();
    await waitFor(() =>
      expect(screen.getByLabelText("Saved inbox badge")).toHaveTextContent(
        "1+",
      ),
    );
    expect(
      items.mock.calls.some(
        ([payload]) =>
          (payload as { query: { limit: number } }).query.limit === 100,
      ),
    ).toBe(true);
    expect(
      screen.getByRole("combobox", { name: "Provider state" }),
    ).toHaveTextContent(
      semantics === "todos" ? "Provider pending" : "Provider unread",
    );
  });

  it("does not query or refresh an unsupported issue feature", async () => {
    contextMock("none", (snapshot) =>
      deny(snapshot, "issues", "not_implemented"),
    );
    const items = mockTauriCommandResult(
      "collaboration_inbox",
      fixtureInboxPage(),
    );
    const refresh = mockTauriCommandResult("collaboration_refresh", {
      job_id: "must-not-run",
    });
    const hydrate = mockTauriCommandResult("collaboration_hydrate_detail", {
      job_id: "must-not-run",
    });
    await mount(<CollaborationWorkspace kind="issue" />);
    expect(await screen.findByText("Feature not supported")).toBeVisible();
    expect(screen.getByRole("button", { name: "Refresh" })).toBeDisabled();
    await userEvent
      .setup()
      .click(screen.getByRole("button", { name: "Refresh" }));
    expect(items).not.toHaveBeenCalled();
    expect(refresh).not.toHaveBeenCalled();
    expect(hydrate).not.toHaveBeenCalled();
  });

  it("keeps unsupported inboxes out of badge queries", async () => {
    contextMock("none");
    const items = mockTauriCommandResult("collaboration_items", fixturePage);
    await mount(
      <>
        <CollaborationWorkspace kind="notification" />
        <BadgeProbe />
      </>,
    );
    expect(await screen.findByText("Feature not supported")).toBeVisible();
    expect(screen.getByLabelText("Saved inbox badge")).toHaveTextContent(
      "none",
    );
    expect(items).not.toHaveBeenCalled();
  });

  it("writes bookmark and disposition as independent local inbox intents", async () => {
    contextMock("native_notifications");
    const notification = {
      ...fixtureItem,
      kind: "notification" as const,
      state: "unread",
      unread: true,
    };
    mockTauriCommandResult(
      "collaboration_inbox",
      fixtureInboxPage([notification]),
    );
    const write = mockTauriCommand(
      "collaboration_set_local_inbox_state",
      (payload) => {
        const { request } = payload as {
          request: import("@gitru/commands").SetLocalInboxStateRequest;
        };
        return {
          state: {
            ...fixtureLocalInboxState(notification),
            disposition: request.disposition ?? "inbox",
            bookmarked: request.bookmarked ?? false,
            generation: "1",
          },
          revision: "11",
          authorization_view: "1",
        };
      },
    );
    const refresh = mockTauriCommandResult("collaboration_refresh", {
      job_id: "must-not-run",
    });
    const user = userEvent.setup();
    await mount(<CollaborationWorkspace kind="notification" />);
    await user.click(
      await screen.findByRole("button", {
        name: `Bookmark ${notification.title}`,
      }),
    );
    await waitFor(() => expect(write).toHaveBeenCalledTimes(1));
    expect(write).toHaveBeenNthCalledWith(1, {
      request: {
        account_id: fixtureAccount.id,
        authorization_epoch: fixtureAccount.authorization_epoch,
        notification_id: notification.id,
        expected_activity_updated_at: notification.updated_at,
        mutation: "bookmark",
        disposition: null,
        bookmarked: true,
        snoozed_until: null,
        expected_generation: "0",
      },
    });
    await user.click(
      screen.getByRole("button", {
        name: `Mark ${notification.title} locally done`,
      }),
    );
    await waitFor(() => expect(write).toHaveBeenCalledTimes(2));
    expect(write).toHaveBeenNthCalledWith(2, {
      request: {
        account_id: fixtureAccount.id,
        authorization_epoch: fixtureAccount.authorization_epoch,
        notification_id: notification.id,
        expected_activity_updated_at: notification.updated_at,
        mutation: "disposition",
        disposition: "done",
        bookmarked: null,
        snoozed_until: null,
        expected_generation: "0",
      },
    });
    const snoozeStarted = Date.now();
    await user.click(
      screen.getByRole("button", {
        name: `Snooze ${notification.title} for one hour`,
      }),
    );
    await waitFor(() => expect(write).toHaveBeenCalledTimes(3));
    const third = write.mock.calls[2][0] as {
      request: import("@gitru/commands").SetLocalInboxStateRequest;
    };
    expect(third.request).toMatchObject({
      account_id: fixtureAccount.id,
      authorization_epoch: fixtureAccount.authorization_epoch,
      notification_id: notification.id,
      expected_activity_updated_at: notification.updated_at,
      mutation: "disposition",
      disposition: "inbox",
      bookmarked: null,
      expected_generation: "0",
    });
    expect(
      Date.parse(third.request.snoozed_until ?? ""),
    ).toBeGreaterThanOrEqual(snoozeStarted + 60 * 60_000);
    expect(Date.parse(third.request.snoozed_until ?? "")).toBeLessThanOrEqual(
      Date.now() + 60 * 60_000,
    );
    expect(refresh).not.toHaveBeenCalled();
  });

  it("reloads SQLite after a stale local inbox CAS without overwriting it", async () => {
    contextMock("native_notifications");
    const notification = {
      ...fixtureItem,
      kind: "notification" as const,
      state: "unread",
      unread: true,
    };
    const inbox = mockTauriCommandResult(
      "collaboration_inbox",
      fixtureInboxPage([notification]),
    );
    const write = mockTauriCommand(
      "collaboration_set_local_inbox_state",
      () => {
        throw { code: "stale_view", message: "private native diagnostics" };
      },
    );
    const user = userEvent.setup();
    await mount(<CollaborationWorkspace kind="notification" />);
    await user.click(
      await screen.findByRole("button", {
        name: `Mark ${notification.title} locally done`,
      }),
    );
    expect(
      await screen.findByText(
        "This saved view changed. Reload it before continuing.",
      ),
    ).toBeVisible();
    expect(write).toHaveBeenCalledOnce();
    await waitFor(() => expect(inbox.mock.calls.length).toBeGreaterThan(1));
    expect(
      screen.getByRole("button", {
        name: `Mark ${notification.title} locally done`,
      }),
    ).toBeEnabled();
  });

  it("moves an active local snooze directly back to the inbox", async () => {
    contextMock("native_notifications");
    const notification = {
      ...fixtureItem,
      kind: "notification" as const,
      state: "unread",
      unread: true,
    };
    mockTauriCommandResult("collaboration_inbox", {
      ...fixtureInboxPage(),
      entries: [
        {
          item: notification,
          local: {
            ...fixtureLocalInboxState(notification),
            effective_disposition: "snoozed",
            snoozed_until: "2099-10-07T12:00:00Z",
            generation: "2",
          },
        },
      ],
    });
    const write = mockTauriCommandResult(
      "collaboration_set_local_inbox_state",
      {
        state: fixtureLocalInboxState(notification),
        revision: "11",
        authorization_view: "1",
      },
    );
    const user = userEvent.setup();
    await mount(<CollaborationWorkspace kind="notification" />);
    await user.click(
      await screen.findByRole("button", {
        name: `Move ${notification.title} to local inbox`,
      }),
    );
    expect(write).toHaveBeenCalledWith({
      request: {
        account_id: fixtureAccount.id,
        authorization_epoch: fixtureAccount.authorization_epoch,
        notification_id: notification.id,
        expected_activity_updated_at: notification.updated_at,
        mutation: "disposition",
        disposition: "inbox",
        bookmarked: null,
        snoozed_until: null,
        expected_generation: "2",
      },
    });
  });

  it("returns external notification changes to page one and closes stale detail", async () => {
    contextMock("native_notifications");
    const notification = {
      ...fixtureItem,
      kind: "notification" as const,
      state: "unread",
      unread: true,
    };
    const cursors: Array<string | null> = [];
    mockTauriCommand("collaboration_inbox", (payload) => {
      const { query } = payload as { query: { cursor: string | null } };
      cursors.push(query.cursor);
      return {
        ...fixtureInboxPage([notification]),
        next_cursor: query.cursor ? null : "second-page",
      };
    });
    let onChange:
      | ((change: import("@gitru/commands").CollaborationChange) => void)
      | undefined;
    vi.spyOn(collaboration, "subscribeChanges").mockImplementation(
      (listener) => {
        onChange = listener;
        return () => {};
      },
    );
    const user = userEvent.setup();
    await mount(<CollaborationWorkspace kind="notification" />);
    await user.click(
      await screen.findByRole("button", { name: "Next saved page" }),
    );
    await waitFor(() => expect(cursors).toContain("second-page"));
    await user.click(screen.getByText(notification.title));
    expect(screen.getByLabelText("Notification subject")).toBeVisible();
    await act(async () => {
      onChange?.({
        account_id: fixtureAccount.id,
        revision: "11",
        scope: "notifications",
        reset: false,
      });
    });
    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: "Next saved page" }),
      ).toBeEnabled(),
    );
    expect(screen.queryByLabelText("Notification subject")).toBeNull();
  });

  it("returns paged inboxes to the SQLite root when a snooze deadline arrives", async () => {
    contextMock("native_notifications");
    const notification = {
      ...fixtureItem,
      kind: "notification" as const,
      state: "unread",
      unread: true,
    };
    const deadline = new Date(Date.now() + 1_000).toISOString();
    const cursors: Array<string | null> = [];
    mockTauriCommand("collaboration_inbox", (payload) => {
      const { query } = payload as { query: { cursor: string | null } };
      cursors.push(query.cursor);
      return {
        ...fixtureInboxPage([notification]),
        next_cursor: query.cursor ? null : "second-page",
        next_local_change_at: deadline,
      };
    });
    const user = userEvent.setup();
    await mount(<CollaborationWorkspace kind="notification" />);
    await user.click(
      await screen.findByRole("button", { name: "Next saved page" }),
    );
    await waitFor(() => expect(cursors).toContain("second-page"));
    await waitFor(
      () => {
        const secondPage = cursors.indexOf("second-page");
        expect(cursors.slice(secondPage + 1)).toContain(null);
      },
      { timeout: 4_000 },
    );
  });

  it("distinguishes missing permission from an explicit denied-scope recheck", async () => {
    contextMock("native_notifications", (snapshot) =>
      deny(snapshot, "issues", "missing_scope"),
    );
    const items = mockTauriCommandResult("collaboration_items", fixturePage);
    const refresh = mockTauriCommandResult("collaboration_refresh", {
      job_id: "explicit-recheck",
    });
    const view = await mount(<CollaborationWorkspace kind="issue" />);
    expect(await screen.findByText("Permission required")).toBeVisible();
    expect(
      screen.queryByRole("button", { name: "Recheck access" }),
    ).not.toBeInTheDocument();
    expect(items).not.toHaveBeenCalled();
    view.unmount();
    contextMock("native_notifications", (snapshot) =>
      deny(snapshot, "issues", "permission_denied", true),
    );
    await act(async () => {
      await view.cache.invalidateQueries({ queryKey: ["collaboration"] });
    });
    // Reuse the actual cache/client, rather than silently replacing the feature.
    render(
      <QueryClientProvider client={view.cache}>
        <CollaborationWorkspace kind="issue" />
      </QueryClientProvider>,
    );
    expect(await screen.findByText("Access denied")).toBeVisible();
    expect(refresh).not.toHaveBeenCalled();
    await userEvent
      .setup()
      .click(screen.getByRole("button", { name: "Recheck access" }));
    expect(refresh).toHaveBeenCalledOnce();
    expect(refresh).toHaveBeenCalledWith({
      request: {
        account_id: fixtureAccount.id,
        repository_id: null,
        kind: "issue",
      },
    });
    expect(items).not.toHaveBeenCalled();
  });

  it("reads saved summaries while remote sync is offline and never dispatches unsupported resource facets", async () => {
    contextMock("native_notifications", (snapshot) => ({
      ...snapshot,
      facets: snapshot.facets.map((facet) =>
        facet.saved_read.state === "supported"
          ? {
              ...facet,
              synchronize: {
                state: "unavailable",
                reason: "temporarily_unavailable",
              },
            }
          : facet,
      ),
    }));
    mockTauriCommandResult("collaboration_items", fixturePage);
    const detail = mockTauriCommandResult("collaboration_detail", {});
    const hydrate = mockTauriCommandResult("collaboration_hydrate_detail", {
      job_id: "must-not-run",
    });
    const refresh = mockTauriCommandResult("collaboration_refresh", {
      job_id: "must-not-run",
    });
    await mount(<CollaborationWorkspace kind="pull_request" />);
    const user = userEvent.setup();
    await user.click(await screen.findByText(fixtureItem.title));
    expect(
      await screen.findByText(/This description is saved locally/),
    ).toBeVisible();
    expect(screen.getByRole("button", { name: "Refresh" })).toBeDisabled();
    for (const name of [
      "Sync reviews",
      "Sync checks",
      "Merge pull request unavailable",
    ]) {
      const button = screen.getByRole("button", { name });
      expect(button).toBeDisabled();
      await user.click(button);
    }
    expect(detail).not.toHaveBeenCalled();
    expect(hydrate).not.toHaveBeenCalled();
    expect(refresh).not.toHaveBeenCalled();
    expect(await screen.findByLabelText("Private draft")).toBeEnabled();
  });

  it("keeps saved content readable during a successful-page cooldown with no error payload", async () => {
    const deadline = "2099-10-03T12:00:00Z";
    contextMock("native_notifications", (snapshot) => ({
      ...snapshot,
      facets: snapshot.facets.map((policy) =>
        policy.facet === "pull_requests"
          ? {
              ...policy,
              synchronize: {
                state: "unavailable",
                reason: "temporarily_unavailable",
              },
              sync: {
                state: "rate_limited",
                next_retry_at: deadline,
                error: null,
                last_success_at: "2026-10-03T12:00:00Z",
              },
            }
          : policy,
      ),
    }));
    mockTauriCommandResult("collaboration_items", {
      ...fixturePage,
      sync: {
        state: "rate_limited",
        next_retry_at: deadline,
        error: null,
        last_success_at: "2026-10-03T12:00:00Z",
      },
    });
    const refresh = mockTauriCommandResult("collaboration_refresh", {
      job_id: "must-not-run",
    });
    await mount(<CollaborationWorkspace kind="pull_request" />);
    expect(await screen.findByText(fixtureItem.title)).toBeVisible();
    expect(screen.getByText("Waiting for provider")).toBeVisible();
    expect(screen.getByRole("button", { name: "Refresh" })).toBeDisabled();
    await userEvent
      .setup()
      .click(screen.getByRole("button", { name: "Refresh" }));
    expect(refresh).not.toHaveBeenCalled();
  });

  it("explains account-wide cooldown while the saved feed itself is idle", async () => {
    contextMock("native_notifications", (snapshot) => ({
      ...snapshot,
      facets: snapshot.facets.map((policy) =>
        policy.facet === "pull_requests"
          ? {
              ...policy,
              synchronize: {
                state: "unavailable" as const,
                reason: "temporarily_unavailable" as const,
              },
              sync: {
                state: "rate_limited" as const,
                next_retry_at: "2099-10-03T12:00:00Z",
                last_success_at: null,
                error: null,
              },
            }
          : policy,
      ),
    }));
    mockTauriCommandResult("collaboration_items", {
      ...fixturePage,
      sync: { ...fixturePage.sync, state: "idle", error: null },
    });
    await mount(<CollaborationWorkspace kind="pull_request" />);
    expect(await screen.findByText(fixtureItem.title)).toBeVisible();
    expect(screen.getByText(/Sync is paused until/)).toBeVisible();
    expect(screen.getByText("Waiting for provider")).toBeVisible();
    expect(screen.getByRole("button", { name: "Refresh" })).toBeDisabled();
  });

  it.each([
    [null, "This resource has no description."],
    ["", "This description is empty."],
  ] as const)("renders authoritative saved description %s and empty reviews/checks without hydration", async (text, message) => {
    contextMock("native_notifications", (snapshot) => ({
      ...snapshot,
      facets: snapshot.facets.map((policy) =>
        ["pull_details", "comments", "reviews", "checks"].includes(policy.facet)
          ? {
              ...policy,
              saved_read: { state: "supported" as const, reason: null },
              synchronize: { state: "supported" as const, reason: null },
              observation: "empty" as const,
            }
          : policy,
      ),
    }));
    mockTauriCommandResult("collaboration_items", fixturePage);
    const detail = mockTauriCommand("collaboration_detail", (payload) => {
      const { query } = payload as {
        query: {
          subject_id: string;
          facet: DetailSnapshot["evidence"]["facet"];
        };
      };
      const value: DetailSnapshot = {
        pending_intent: null,

        subject_id: query.subject_id,
        metadata: query.facet === "body" ? fixtureMetadata() : null,
        body: {
          state: query.facet === "body" ? "known" : "not_loaded",
          text: query.facet === "body" ? text : null,
        },
        entries: [],
        next_cursor: null,
        revision: "10",
        authorization_view: "1",
        evidence: {
          facet: query.facet,
          availability: "ready",
          coverage: {
            state: "complete",
            validated_at: "2026-10-03T12:00:00Z",
            remote_has_more: false,
          },
          freshness: "fresh",
          stale_at: "2099-10-03T12:00:00Z",
          facet_revision: "10",
          authorization_epoch: "1",
          access_reason: null,
          source: null,
          value_source: null,
          saved_empty: true,
          observed_state: query.facet === "body" ? "known" : "not_loaded",
          sync: {
            state: "idle",
            last_success_at: null,
            next_retry_at: null,
            error: null,
          },
        },
      };
      return value;
    });
    const hydrate = mockTauriCommandResult("collaboration_hydrate_detail", {
      job_id: "must-not-run",
    });
    const user = userEvent.setup();
    await mount(<CollaborationWorkspace kind="pull_request" />);
    await user.click(await screen.findByText(fixtureItem.title));
    expect(await screen.findByText(message)).toBeVisible();
    expect(
      await screen.findByText("No reviews were returned by the provider."),
    ).toBeVisible();
    expect(
      await screen.findByText(
        "The provider returned no checks for this exact head.",
      ),
    ).toBeVisible();
    expect(detail).toHaveBeenCalledTimes(3);
    expect(
      detail.mock.calls.some(
        ([payload]) =>
          (payload as { query: { facet: string } }).query.facet === "comments",
      ),
    ).toBe(false);
    await user.click(screen.getByRole("button", { name: "Comments" }));
    expect(
      await screen.findByText(
        "No conversation comments were returned in the saved observation.",
      ),
    ).toBeVisible();
    expect(detail).toHaveBeenCalledTimes(4);
    expect(hydrate).not.toHaveBeenCalled();
    expect(
      screen.getByRole("button", { name: "Merge pull request unavailable" }),
    ).toBeDisabled();
  });

  it("preserves authored text and the inspected CAS generation through an actual authorization reset", async () => {
    contextMock("native_notifications", (snapshot) =>
      currentAccounts.accounts[0].state === "auth_required"
        ? {
            ...snapshot,
            facets: snapshot.facets.map((facet) => ({
              ...facet,
              saved_read: {
                state: "unavailable",
                reason: "authentication_required",
              },
              synchronize: {
                state: "unavailable",
                reason: "authentication_required",
              },
            })),
          }
        : snapshot,
    );
    mockTauriCommandResult("collaboration_items", fixturePage);
    let accountReload!: (snapshot: AccountSnapshot) => void;
    let draftReload!: (draft: {
      account_id: string;
      subject_id: string;
      body: string;
      generation: string;
    }) => void;
    let delaying = false;
    mockTauriCommand("collaboration_accounts", () =>
      delaying
        ? new Promise<AccountSnapshot>((resolve) => {
            accountReload = resolve;
          })
        : currentAccounts,
    );
    mockTauriCommand("collaboration_draft", () =>
      delaying
        ? new Promise((resolve) => {
            draftReload = resolve;
          })
        : null,
    );
    const item = mockTauriCommand("collaboration_item", () => {
      if (delaying)
        throw { code: "auth_required", message: "fixture content denied" };
      return {
        pending_intent: null,
        item: fixtureItem,
        revision: "10",
        authorization_view: "1",
      };
    });
    const user = userEvent.setup();
    await mount(<CollaborationWorkspace kind="pull_request" />);
    await user.click(await screen.findByText(fixtureItem.title));
    const draft = await screen.findByLabelText("Private draft");
    await user.type(draft, "Keep my unsaved private text");
    const itemReads = item.mock.calls.length;
    delaying = true;
    currentRevision = "11";
    currentView = "2";
    currentAccounts = {
      accounts: [
        { ...fixtureAccount, authorization_epoch: "2", state: "auth_required" },
      ],
      revision: "11",
      authorization_view: "2",
    };
    await act(async () => {
      await collaboration.wake();
    });
    await waitFor(() =>
      expect(screen.queryByText(fixtureItem.title)).not.toBeInTheDocument(),
    );
    expect(
      screen.queryByText(/This description is saved locally/),
    ).not.toBeInTheDocument();
    expect(screen.getByLabelText("Private draft")).toHaveValue(
      "Keep my unsaved private text",
    );
    expect(item).toHaveBeenCalledTimes(itemReads);
    await act(async () => {
      accountReload(currentAccounts);
    });
    expect(
      (await screen.findAllByText("Reconnect your account"))[0],
    ).toBeVisible();
    expect(screen.getByLabelText("Private draft")).toHaveValue(
      "Keep my unsaved private text",
    );
    await act(async () => {
      draftReload({
        account_id: fixtureAccount.id,
        subject_id: fixtureItem.id,
        body: "Other editor saved this",
        generation: "1",
      });
    });
    expect(
      await screen.findByText(/This draft changed in another tab/),
    ).toBeVisible();
    expect(screen.getByRole("button", { name: "Save draft" })).toBeDisabled();
    expect(screen.getByLabelText("Private draft")).toHaveValue(
      "Keep my unsaved private text",
    );
  });

  it("does not bridge a late resource-policy response or private draft across actor IDs", async () => {
    // jsdom/NWSAPI recurses on browser top-layer selectors used by Floating UI.
    // This document has no native top-layer elements; preserve every other
    // selector. The real WKWebView picker is covered by isolated native QA.
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
    currentAccounts = { ...fixtureAccounts, accounts: [fixtureAccount, other] };
    let finishOld!: (snapshot: ContextualCapabilitySnapshot) => void;
    let old!: ContextualCapabilitySnapshot;
    contextMock("native_notifications", (snapshot, request) => {
      if (
        request.account_id === fixtureAccount.id &&
        request.target.kind === "resource"
      ) {
        old = snapshot;
        return new Promise((resolve) => {
          finishOld = resolve;
        });
      }
      return snapshot;
    });
    mockTauriCommand("collaboration_items", (payload) => {
      const { query } = payload as { query: { account_id: string } };
      return {
        ...fixturePage,
        items: [
          {
            ...fixtureItem,
            account_id: query.account_id,
            title:
              query.account_id === other.id
                ? "Other actor item"
                : fixtureItem.title,
          },
        ],
      };
    });
    const item = mockTauriCommand("collaboration_item", (payload) => {
      const { accountId } = payload as { accountId: string };
      expect(accountId).toBe(other.id);
      return {
        pending_intent: null,

        item: {
          ...fixtureItem,
          account_id: other.id,
          title: "Other actor item",
          body: "Other actor provider body",
        },
        revision: "10",
        authorization_view: "1",
      };
    });
    mockTauriCommand("collaboration_draft", (payload) => {
      const { accountId, subjectId } = payload as {
        accountId: string;
        subjectId: string;
      };
      return {
        account_id: accountId,
        subject_id: subjectId,
        body:
          accountId === other.id
            ? "Other actor saved draft"
            : "First actor saved draft",
        generation: "1",
      };
    });
    const user = userEvent.setup();
    await mount(<CollaborationWorkspace kind="pull_request" />);
    await user.click(await screen.findByText(fixtureItem.title));
    const draft = await screen.findByLabelText("Private draft");
    await user.type(draft, " plus unsaved first-actor text");
    expect(item).not.toHaveBeenCalled();
    await user.click(screen.getByLabelText("Provider account"));
    await user.click(
      await screen.findByRole("option", { name: "GitHub · @other-user" }),
    );
    await user.click(await screen.findByText("Other actor item"));
    expect(await screen.findByText("Other actor provider body")).toBeVisible();
    expect(await screen.findByLabelText("Private draft")).toHaveValue(
      "Other actor saved draft",
    );
    await act(async () => {
      finishOld(old);
    });
    expect(screen.getByLabelText("Private draft")).toHaveValue(
      "Other actor saved draft",
    );
    expect(screen.queryByText(fixtureItem.title)).not.toBeInTheDocument();
    expect(
      item.mock.calls.every(
        ([payload]) =>
          (payload as { accountId: string }).accountId === other.id,
      ),
    ).toBe(true);
  });
});

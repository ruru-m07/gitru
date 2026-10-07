import { collaboration, type RemoteAccount } from "@gitru/collaboration-client";
import type {
  ContextCapabilityRequest,
  DetailSnapshot,
  DiscoverNotificationSubjectRequest,
  NotificationSubjectQuery,
  NotificationSubjectSnapshot,
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
  fixtureInboxPage,
  fixtureItem,
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
import { NotificationSubjectView } from "./notification-subject-view";
import { CollaborationWorkspace } from "./workspace";

const instanceId = "github:https://github.com/";
const otherAccount: RemoteAccount = {
  ...fixtureAccount,
  id: "other-account",
  actor_id: "9007199254740993",
  login: "other-user",
};
const thread = {
  ...fixtureItem,
  id: "thread-a",
  kind: "notification" as const,
  title: "Original notification title",
  body: null,
  reason: "review_requested",
  unread: true,
};
let revision = "10";
let view = "1";
let selector = "9007199254740995";
let subjectId = "canonical-pr";
let subjectKind: "pull_request" | "issue" = "pull_request";
let subjectState: NotificationSubjectSnapshot["state"] = "resolved";
let subjectReason: NotificationSubjectSnapshot["reason"] = null;
let admission = true;
let paused = false;
let supported = true;
let denied = false;
let body: DetailSnapshot;
let changes: Array<{
  account_id: string;
  revision: string;
  scope: string;
  reset: boolean;
}> = [];
const stops: Array<() => void> = [];
const caches: QueryClient[] = [];

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((done, fail) => {
    resolve = done;
    reject = fail;
  });
  return { promise, resolve, reject };
}
function savedSubject(account: RemoteAccount = fixtureAccount, id = subjectId) {
  return {
    ...fixtureItem,
    id,
    account_id: account.id,
    kind: subjectKind,
    title:
      account.id === otherAccount.id
        ? "Other actor subject"
        : "Canonical subject summary",
    body: null,
    reason: null,
    unread: null,
  };
}
function snapshot(
  account: RemoteAccount = fixtureAccount,
): NotificationSubjectSnapshot {
  return {
    revision,
    authorization_view: view,
    authorization_epoch: account.authorization_epoch,
    state: subjectState,
    reason: subjectReason,
    selector_generation: selector,
    subject:
      subjectState === "resolved"
        ? {
            account_id: account.id,
            instance_id: instanceId,
            id: subjectId,
            kind: subjectKind,
            provider_id: "9007199254740997",
          }
        : null,
    fallback_web_url: subjectState === "unavailable" ? null : thread.web_url,
    discovery: {
      support: supported ? "supported" : "unsupported",
      admission,
      paused,
      retry_at: paused ? "2099-10-03T12:00:00Z" : null,
      attempts: subjectReason === "attempts_exhausted" ? 3 : 0,
      sync: {
        state: paused ? "rate_limited" : "idle",
        next_retry_at: paused ? "2099-10-03T12:00:00Z" : null,
        last_success_at: null,
        error: null,
      },
    },
  };
}

beforeEach(() => {
  revision = "10";
  view = "1";
  selector = "9007199254740995";
  subjectId = "canonical-pr";
  subjectKind = "pull_request";
  subjectState = "resolved";
  subjectReason = null;
  admission = true;
  paused = false;
  supported = true;
  denied = false;
  changes = [];
  body = fixtureBody({ subject_id: subjectId });
  vi.spyOn(collaboration.transport, "listen").mockResolvedValue(() => {});
  mockTauriCommand("collaboration_accounts", () => ({
    ...fixtureAccounts,
    revision,
    authorization_view: view,
  }));
  mockTauriCommand("collaboration_changes_since", () => ({
    revision,
    authorization_view: view,
    changes,
    has_more: false,
    reset_required: false,
  }));
  mockTauriCommandResult("collaboration_repositories", {
    ...fixtureRepositories,
    repositories: fixtureRepositories.repositories.map((repository) => ({
      ...repository,
      selected: false,
    })),
  });
  mockTauriCommand("collaboration_inbox", () => ({
    ...fixtureInboxPage([thread]),
    revision,
    authorization_view: view,
  }));
  mockTauriCommand("collaboration_notification_subject", (payload) => {
    const { query } = payload as { query: NotificationSubjectQuery };
    return snapshot(
      query.account_id === otherAccount.id ? otherAccount : fixtureAccount,
    );
  });
  mockTauriCommand("collaboration_item", (payload) => {
    const { accountId, itemId } = payload as {
      accountId: string;
      itemId: string;
    };
    const account =
      accountId === otherAccount.id ? otherAccount : fixtureAccount;
    return {
      item: itemId.startsWith("thread-")
        ? { ...thread, id: itemId, account_id: accountId }
        : savedSubject(account, itemId),
      revision,
      authorization_view: view,
    };
  });
  mockTauriCommand("collaboration_detail", (payload) => {
    const { query } = payload as {
      query: { subject_id: string; account_id: string };
    };
    const currentBody =
      query.account_id === otherAccount.id
        ? "Other actor saved body 日本語"
        : body.body.text;
    return {
      ...body,
      subject_id: query.subject_id,
      body: { ...body.body, text: currentBody },
      revision,
      authorization_view: view,
    };
  });
  mockTauriCommandResult("collaboration_draft", null);
  mockTauriCommand("collaboration_contextual_capabilities", (payload) => {
    const { request } = payload as { request: ContextCapabilityRequest };
    const account =
      request.account_id === otherAccount.id ? otherAccount : fixtureAccount;
    const context = fixtureContextualCapabilities(account, request.target);
    return {
      ...context,
      revision,
      authorization_view: view,
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

async function open(workspace = false) {
  const cache = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  caches.push(cache);
  stops.push(collaboration.installBridge(cache));
  await act(async () => {
    await collaboration.wake();
  });
  const element = (
    account: RemoteAccount = fixtureAccount,
    notificationId = thread.id,
  ) => (
    <StrictMode>
      <QueryClientProvider client={cache}>
        {workspace ? (
          <CollaborationWorkspace kind="notification" />
        ) : (
          <NotificationSubjectView
            account={account}
            notificationId={notificationId}
            instanceId={instanceId}
            close={() => {}}
          />
        )}
      </QueryClientProvider>
    </StrictMode>
  );
  const rendered = render(element());
  const user = userEvent.setup();
  if (workspace) await user.click(await screen.findByText(thread.title));
  return {
    cache,
    user,
    rerender: (account: RemoteAccount, notificationId = thread.id) =>
      rendered.rerender(element(account, notificationId)),
  };
}
async function change(scope: string, withdrawal = false) {
  revision = (BigInt(revision) + 1n).toString();
  if (withdrawal) view = (BigInt(view) + 1n).toString();
  changes = [{ account_id: fixtureAccount.id, revision, scope, reset: false }];
  await act(async () => {
    await collaboration.wake();
  });
}

describe("cached notification canonical subjects", () => {
  it("opens cached canonical details from an unselected repository while lease admission is pending, preserving notification semantics", async () => {
    const pending = deferred<{
      lease_id: string;
      owner_generation: string;
      expires_in_seconds: number;
      renew_after_seconds: number;
    }>();
    const demand = mockForegroundDemand();
    demand.acquire.mockImplementation(() => pending.promise);
    const discovery = mockTauriCommandResult(
      "collaboration_discover_notification_subject",
      { job_id: "unexpected" },
    );
    const selection = mockTauriCommandResult(
      "collaboration_select_repository",
      "unexpected",
    );
    const refresh = mockTauriCommandResult("collaboration_refresh", {
      job_id: "unexpected",
    });
    const hydrate = mockTauriCommandResult("collaboration_hydrate_detail", {
      job_id: "unexpected",
    });
    await open(true);
    expect(
      await screen.findByText("Full cached resource description"),
    ).toBeVisible();
    expect(screen.getByText("Reason: review_requested")).toBeVisible();
    expect(screen.getByText("Unread notification")).toBeVisible();
    expect(
      screen.getByText(/does not mark the notification as read/),
    ).toBeVisible();
    expect(
      screen.getByRole("heading", { name: "Authoritative detail title" }),
    ).toBeVisible();
    expect(discovery).not.toHaveBeenCalled();
    expect(selection).not.toHaveBeenCalled();
    expect(refresh).not.toHaveBeenCalled();
    expect(hydrate).not.toHaveBeenCalled();
  });

  it.each([
    null,
    "",
  ])("shows an authoritative %s issue description and metadata without relabeling the thread", async (text) => {
    subjectKind = "issue";
    body = fixtureBody({
      subject_id: subjectId,
      body: { state: "known", text },
      metadata: fixtureMetadata("issue"),
    });
    body.evidence.saved_empty = true;
    mockForegroundDemand();
    await open();
    expect(
      await screen.findByText(
        text === null
          ? "This resource has no description."
          : "This description is empty.",
      ),
    ).toBeVisible();
    expect(screen.getByText("Reason: review_requested")).toBeVisible();
    expect(
      screen.getByRole("heading", { name: "Authoritative detail title" }),
    ).toBeVisible();
    expect(screen.queryByRole("region", { name: "Reviews" })).toBeNull();
  });

  it("keeps discovery explicit, captures selector generation, and accepts intent while native dispatch is paused", async () => {
    subjectState = "not_cached";
    subjectReason = "not_cached";
    paused = true;
    const pending = deferred<{ job_id: string }>();
    const discovery = mockTauriCommand(
      "collaboration_discover_notification_subject",
      () => pending.promise,
    );
    const item = mockTauriCommand("collaboration_item", () => ({
      item: thread,
      revision,
      authorization_view: view,
    }));
    const { user } = await open();
    const button = await screen.findByRole("button", {
      name: "Load notification subject",
    });
    expect(button).toBeEnabled();
    expect(discovery).not.toHaveBeenCalled();
    expect(
      screen.getByText(
        `Loading is paused until ${new Date("2099-10-03T12:00:00Z").toLocaleString()}. An accepted request waits until the provider allows another request.`,
      ),
    ).toBeVisible();
    await user.click(button);
    await waitFor(() => expect(discovery).toHaveBeenCalledTimes(1));
    expect(screen.queryByText(/Loading request accepted/)).toBeNull();
    expect(
      screen.getByRole("button", { name: "Requesting subject…" }),
    ).toBeDisabled();
    await act(async () => {
      pending.resolve({ job_id: "finite-native-intent" });
    });
    expect(await screen.findByRole("status")).toHaveTextContent(
      "Loading request accepted. The subject is not loaded yet.",
    );
    expect(discovery).toHaveBeenCalledWith({
      request: {
        account_id: fixtureAccount.id,
        authorization_epoch: fixtureAccount.authorization_epoch,
        notification_id: thread.id,
        selector_generation: selector,
      } satisfies DiscoverNotificationSubjectRequest,
    });
    expect(
      item.mock.calls.every(
        ([payload]) => (payload as { itemId: string }).itemId === thread.id,
      ),
    ).toBe(true);
    expect(
      screen.queryByRole("article", { name: "Saved item detail" }),
    ).toBeNull();
  });

  it.each([
    "unsupported",
    "ambiguous",
    "identity_unverified",
  ] as const)("renders %s as a fallback without guessed canonical queries or automatic discovery", async (state) => {
    subjectState = state;
    supported = false;
    admission = false;
    const discovery = mockTauriCommandResult(
      "collaboration_discover_notification_subject",
      { job_id: "unexpected" },
    );
    const detail = mockTauriCommandResult("collaboration_detail", body);
    await open();
    expect(
      await screen.findByText(/Original notification title/),
    ).toBeVisible();
    expect(
      screen.getByRole("button", { name: "Open on provider" }),
    ).toBeEnabled();
    expect(
      screen.queryByRole("button", { name: /Load notification subject/ }),
    ).toBeNull();
    expect(
      screen.queryByRole("article", { name: "Saved item detail" }),
    ).toBeNull();
    expect(detail).not.toHaveBeenCalled();
    expect(discovery).not.toHaveBeenCalled();
  });

  it("shows terminal attempts as an explicit retry without claiming that a missing subject was deleted", async () => {
    subjectState = "not_cached";
    subjectReason = "attempts_exhausted";
    const discovery = mockTauriCommandResult(
      "collaboration_discover_notification_subject",
      { job_id: "manual-new-generation" },
    );
    const { user } = await open();
    expect(
      await screen.findByText(/bounded loading attempts have stopped/),
    ).toBeVisible();
    expect(discovery).not.toHaveBeenCalled();
    await user.click(
      screen.getByRole("button", { name: "Try loading subject again" }),
    );
    await waitFor(() => expect(discovery).toHaveBeenCalledTimes(1));
    subjectReason = "not_found";
    await change(`notification_subject:${thread.id}`);
    expect(
      await screen.findByText(/does not establish that it was deleted/),
    ).toBeVisible();
    expect(discovery).toHaveBeenCalledTimes(1);
  });

  it("clears accepted feedback for a new request or failure without blocking another explicit retry", async () => {
    subjectState = "not_cached";
    const pending = deferred<{ job_id: string }>();
    const discovery = mockTauriCommandResult(
      "collaboration_discover_notification_subject",
      { job_id: "accepted-intent" },
    );
    const { user } = await open();
    await user.click(
      await screen.findByRole("button", { name: "Load notification subject" }),
    );
    expect(await screen.findByRole("status")).toHaveTextContent(
      "Loading request accepted",
    );
    discovery.mockImplementationOnce(() => pending.promise);
    await user.click(
      screen.getByRole("button", { name: "Load notification subject" }),
    );
    expect(screen.queryByText(/Loading request accepted/)).toBeNull();
    await act(async () => {
      pending.reject({ code: "network", message: "private provider payload" });
    });
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Could not reach the provider",
    );
    expect(screen.queryByText(/Loading request accepted/)).toBeNull();
    expect(
      screen.getByRole("button", { name: "Load notification subject" }),
    ).toBeEnabled();
    expect(discovery).toHaveBeenCalledTimes(2);
  });

  it("hides accepted feedback when a later native observation exhausts attempts, retaining explicit retry", async () => {
    subjectState = "not_cached";
    const discovery = mockTauriCommandResult(
      "collaboration_discover_notification_subject",
      { job_id: "accepted-intent" },
    );
    const { user } = await open();
    await user.click(
      await screen.findByRole("button", { name: "Load notification subject" }),
    );
    expect(await screen.findByRole("status")).toHaveTextContent(
      "Loading request accepted",
    );
    subjectReason = "attempts_exhausted";
    await change(`notification_subject:${thread.id}`);
    expect(
      await screen.findByRole("button", { name: "Try loading subject again" }),
    ).toBeEnabled();
    expect(screen.queryByText(/Loading request accepted/)).toBeNull();
    expect(discovery).toHaveBeenCalledTimes(1);
  });

  it.each([
    "thread",
    "actor",
    "selector",
  ] as const)("ignores a late explicit discovery receipt after the %s binding changes", async (binding) => {
    subjectState = "not_cached";
    const pending = deferred<{ job_id: string }>();
    const discovery = mockTauriCommand(
      "collaboration_discover_notification_subject",
      () => pending.promise,
    );
    const resolver = mockTauriCommand(
      "collaboration_notification_subject",
      (payload) =>
        snapshot(
          (payload as { query: NotificationSubjectQuery }).query.account_id ===
            otherAccount.id
            ? otherAccount
            : fixtureAccount,
        ),
    );
    const { user, rerender } = await open();
    await user.click(
      await screen.findByRole("button", { name: "Load notification subject" }),
    );
    expect(discovery).toHaveBeenCalledTimes(1);
    if (binding === "selector") {
      selector = "9007199254740996";
      await change(`notification_subject:${thread.id}`);
    } else {
      rerender(
        binding === "actor" ? otherAccount : fixtureAccount,
        binding === "thread" ? "thread-b" : thread.id,
      );
    }
    expect(
      await screen.findByRole("button", { name: "Load notification subject" }),
    ).toBeEnabled();
    const reads = resolver.mock.calls.length;
    await act(async () => {
      pending.resolve({ job_id: "retired-notification-receipt" });
    });
    expect(resolver).toHaveBeenCalledTimes(reads);
    expect(
      screen.queryByRole("article", { name: "Saved item detail" }),
    ).toBeNull();
    expect(
      screen.getByRole("button", { name: "Load notification subject" }),
    ).toBeEnabled();
    expect(screen.queryByText(/Loading request accepted/)).toBeNull();
  });

  it("hides withdrawn provider content through a real bridge reset and retains private text with its original CAS generation", async () => {
    mockForegroundDemand();
    let draftGeneration = "7";
    mockTauriCommand("collaboration_draft", (payload) =>
      (payload as { subjectId: string }).subjectId === thread.id
        ? null
        : {
            account_id: fixtureAccount.id,
            subject_id: (payload as { subjectId: string }).subjectId,
            body:
              draftGeneration === "7"
                ? "Original private subject text"
                : "Newer other-tab private text",
            generation: draftGeneration,
          },
    );
    const save = mockTauriCommandResult("collaboration_save_draft", {
      account_id: fixtureAccount.id,
      subject_id: subjectId,
      body: "unexpected",
      generation: "9",
    });
    const { user } = await open();
    expect(
      await screen.findByText("Full cached resource description"),
    ).toBeVisible();
    const editor = await screen.findByRole("textbox", {
      name: "Private draft",
    });
    await user.clear(editor);
    await user.type(editor, "Unsaved private subject text 日本語");
    subjectState = "unavailable";
    subjectReason = "inactive_membership";
    draftGeneration = "8";
    await change("notifications", true);
    expect(
      await screen.findByText(
        /Provider content is unavailable in the current notification view/,
      ),
    ).toBeVisible();
    expect(screen.queryByText("Full cached resource description")).toBeNull();
    expect(
      screen.queryByRole("heading", { name: "Authoritative detail title" }),
    ).toBeNull();
    expect(screen.queryByText("Reason: review_requested")).toBeNull();
    expect(
      screen.queryByRole("button", { name: "Open on provider" }),
    ).toBeNull();
    expect(screen.getByRole("textbox", { name: "Private draft" })).toHaveValue(
      "Unsaved private subject text 日本語",
    );
    expect(
      await screen.findByText(/This draft changed in another tab/),
    ).toBeVisible();
    const article = within(
      screen.getByRole("article", { name: "Saved item detail" }),
    );
    expect(article.getByRole("button", { name: "Save draft" })).toBeDisabled();
    expect(save).not.toHaveBeenCalled();
  });

  it("keeps notification-keyed drafts separate from the canonical private editor", async () => {
    mockForegroundDemand();
    mockTauriCommand("collaboration_draft", (payload) => ({
      account_id: fixtureAccount.id,
      subject_id: (payload as { subjectId: string }).subjectId,
      body:
        (payload as { subjectId: string }).subjectId === thread.id
          ? "Original thread-only private text"
          : "Canonical-only private text",
      generation: "4",
    }));
    const { user } = await open();
    expect(
      await screen.findByRole("textbox", { name: "Private draft" }),
    ).toHaveValue("Canonical-only private text");
    await user.click(screen.getByText("Saved draft for this notification"));
    expect(
      screen.getByRole("textbox", { name: "Private notification draft" }),
    ).toHaveValue("Original thread-only private text");
    expect(
      screen.getByText(/This thread draft is separate/),
    ).toBeInTheDocument();
  });

  it("reuses the known-subject lease through body observations and replaces private buffers when the selector names another canonical subject", async () => {
    const demand = mockForegroundDemand();
    await open();
    expect(
      await screen.findByText("Full cached resource description"),
    ).toBeVisible();
    await waitFor(() => expect(demand.acquire).toHaveBeenCalledTimes(1));
    const user = userEvent.setup();
    const editor = await screen.findByRole("textbox", {
      name: "Private draft",
    });
    await user.type(editor, "Unsaved old subject");
    body.evidence.observed_state = "omitted";
    await change(`detail:${subjectId}:body`);
    expect(
      await screen.findByText("Latest provider value was omitted"),
    ).toBeVisible();
    expect(screen.getByRole("textbox", { name: "Private draft" })).toHaveValue(
      "Unsaved old subject",
    );
    expect(demand.acquire).toHaveBeenCalledTimes(1);
    subjectId = "canonical-replacement";
    selector = "9007199254740996";
    body.body = { state: "known", text: "Replacement canonical body" };
    await change(`notification_subject:${thread.id}`, true);
    expect(await screen.findByText("Replacement canonical body")).toBeVisible();
    expect(screen.getByRole("textbox", { name: "Private draft" })).toHaveValue(
      "",
    );
    await waitFor(() => expect(demand.acquire).toHaveBeenCalledTimes(2));
    expect(demand.acquire.mock.calls.at(-1)?.[0]).toMatchObject({
      request: { target: { subject_id: subjectId } },
    });
  });

  it("discards old actor buffers and late canonical reads on account switch", async () => {
    mockForegroundDemand();
    const oldBody = deferred<DetailSnapshot>();
    mockTauriCommand("collaboration_detail", (payload) => {
      const { query } = payload as {
        query: { account_id: string; subject_id: string };
      };
      return query.account_id === fixtureAccount.id
        ? oldBody.promise
        : {
            ...body,
            subject_id: query.subject_id,
            body: {
              state: "known" as const,
              text: "Other actor saved body 日本語",
            },
          };
    });
    const { user, rerender } = await open();
    const editor = await screen.findByRole("textbox", {
      name: "Private draft",
    });
    await user.type(editor, "Old actor unsaved text");
    rerender(otherAccount);
    expect(
      await screen.findByText("Other actor saved body 日本語"),
    ).toBeVisible();
    expect(screen.getByRole("textbox", { name: "Private draft" })).toHaveValue(
      "",
    );
    await act(async () => {
      oldBody.resolve(
        fixtureBody({
          body: { state: "known", text: "Late old actor provider body" },
        }),
      );
    });
    expect(screen.queryByText("Late old actor provider body")).toBeNull();
    expect(screen.queryByDisplayValue("Old actor unsaved text")).toBeNull();
  });

  it("clears the prior notification editor even when another cached thread resolves the same canonical subject", async () => {
    mockForegroundDemand();
    const { user, rerender } = await open();
    const editor = await screen.findByRole("textbox", {
      name: "Private draft",
    });
    await user.type(editor, "Prior thread unsaved buffer");
    rerender(fixtureAccount, "thread-b");
    expect(
      await screen.findByText("Full cached resource description"),
    ).toBeVisible();
    await waitFor(() =>
      expect(
        screen.getByRole("textbox", { name: "Private draft" }),
      ).toHaveValue(""),
    );
    expect(
      screen.queryByDisplayValue("Prior thread unsaved buffer"),
    ).toBeNull();
  });

  it("does not admit a canonical item or lease from a snapshot naming a foreign account", async () => {
    const demand = mockForegroundDemand();
    mockTauriCommand("collaboration_notification_subject", () => ({
      ...snapshot(),
      subject: { ...snapshot().subject!, account_id: otherAccount.id },
    }));
    const detail = mockTauriCommandResult("collaboration_detail", body);
    const item = mockTauriCommand("collaboration_item", () => ({
      item: thread,
      revision,
      authorization_view: view,
    }));
    await open();
    expect(
      await screen.findByText(
        /saved subject does not match the current account/,
      ),
    ).toBeVisible();
    expect(
      screen.queryByRole("article", { name: "Saved item detail" }),
    ).toBeNull();
    expect(detail).not.toHaveBeenCalled();
    expect(demand.acquire).not.toHaveBeenCalled();
    expect(
      item.mock.calls.every(
        ([payload]) => (payload as { itemId: string }).itemId === thread.id,
      ),
    ).toBe(true);
  });

  it("does not expose denied body metadata while preserving a locally authored editor", async () => {
    denied = true;
    mockForegroundDemand();
    const detail = mockTauriCommandResult("collaboration_detail", body);
    await open();
    expect(
      await screen.findByRole("textbox", { name: "Private draft" }),
    ).toBeVisible();
    expect(
      screen.queryByRole("heading", { name: "Authoritative detail title" }),
    ).toBeNull();
    expect(screen.queryByText("Full cached resource description")).toBeNull();
    expect(detail).not.toHaveBeenCalled();
  });
});

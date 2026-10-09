import type {
  IssueDraftSnapshot,
  IssueDraftV2Snapshot,
  IssueMetadataPage,
  IssueMetadataSelection,
  RemoteAccount,
  RemoteRepository,
} from "@gitru/collaboration-client";
import { collaboration } from "@gitru/collaboration-client";
import { issueDraftV2QueryOptions } from "@gitru/collaboration-client/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { mockForegroundDemand } from "../../../tests/mocks/collaboration-demand";
import { mockTauriCommand } from "../../../tests/mocks/tauri";
import { NewIssueDialog } from "./issue-creation";

const account: RemoteAccount = {
  id: "issue-account",
  provider: "github",
  host: "github.com",
  actor_id: "44",
  login: "writer",
  display_name: null,
  authorization_epoch: "7",
  state: "active",
  notifications_supported: true,
};
const repository: RemoteRepository = {
  id: "repo-local",
  account_id: account.id,
  provider_id: "9007199254740999",
  full_name: "owner/project",
  name: "project",
  web_url: "https://github.com/owner/project",
  description: null,
  default_branch: "main",
  selected: true,
};
const context = {
  account_id: account.id,
  repository_id: repository.id,
  authorization_epoch: account.authorization_epoch,
  authorization_view: "11",
  review_token: "a".repeat(64),
};
let snapshot: IssueDraftSnapshot;
let metadata: IssueMetadataSelection;
let outcome: IssueDraftV2Snapshot["metadata_outcome"];
function versionTwo(draft = snapshot): IssueDraftV2Snapshot {
  return { draft, metadata, metadata_outcome: outcome };
}

function labelCatalog(
  overrides: Partial<IssueMetadataPage> = {},
): IssueMetadataPage {
  return {
    account_id: account.id,
    repository_id: repository.id,
    kind: "labels",
    options: [
      {
        reference: {
          kind: "label",
          value: { provider_id: "19", name: "bug", color: "ff0000" },
        },
        availability: "unknown",
        reason: "unobserved",
      },
    ],
    next_cursor: null,
    coverage: { state: "partial", validated_at: null, remote_has_more: true },
    freshness: "stale",
    sync: {
      state: "offline",
      last_success_at: null,
      next_retry_at: null,
      error: null,
    },
    revision: "20",
    authorization_view: "11",
    catalog_revision: "1",
    ...overrides,
  };
}
const caches: QueryClient[] = [];
const stops: Array<() => void> = [];

function localSnapshot(
  overrides: Partial<IssueDraftSnapshot> = {},
): IssueDraftSnapshot {
  return {
    account_id: account.id,
    draft_id: "11111111-1111-4111-8111-111111111111",
    repository_id: repository.id,
    title: "",
    body: "",
    generation: "0",
    context,
    availability: "unavailable",
    reason: "empty_title",
    submission: null,
    published: null,
    revision: "20",
    authorization_view: "11",
    ...overrides,
  };
}

beforeEach(() => {
  vi.spyOn(crypto, "randomUUID").mockReturnValue(
    "11111111-1111-4111-8111-111111111111",
  );
  snapshot = localSnapshot();
  metadata = { labels: [], assignees: [], milestone: null };
  outcome = null;
  mockTauriCommand("collaboration_issue_draft_v2", () => versionTwo());
  mockTauriCommand("collaboration_issue_drafts_v2", () => ({
    account_id: account.id,
    drafts: [],
    next_cursor: null,
    revision: snapshot.revision,
    authorization_view: snapshot.authorization_view,
  }));
});

afterEach(() => {
  for (const stop of stops.splice(0)) stop();
  vi.restoreAllMocks();
  for (const cache of caches.splice(0)) cache.clear();
});

function setup(onOpenCreated = vi.fn()) {
  const cache = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  caches.push(cache);
  const content = (currentAccount: RemoteAccount) => (
    <QueryClientProvider client={cache}>
      <NewIssueDialog
        account={currentAccount}
        repository={repository}
        onOpenCreated={onOpenCreated}
      />
    </QueryClientProvider>
  );
  const view = render(content(account));
  return {
    cache,
    user: userEvent.setup(),
    onOpenCreated,
    rerenderAccount: (next: RemoteAccount) => view.rerender(content(next)),
  };
}

it("retires consent when an edit is reverted to the same saved text", async () => {
  snapshot = localSnapshot({
    title: "Saved title",
    body: "Saved body",
    generation: "1",
    availability: "available",
    reason: null,
  });
  const { user } = setup();
  await user.click(screen.getByRole("button", { name: "New issue" }));
  const consent = await screen.findByRole("checkbox", {
    name: /exact saved issue may be submitted/i,
  });
  await user.click(consent);
  expect(
    screen.getByRole("button", { name: "Queue issue submission" }),
  ).toBeEnabled();
  await user.type(screen.getByLabelText("Title"), "x{Backspace}");
  expect(screen.getByLabelText("Title")).toHaveValue("Saved title");
  expect(consent).not.toBeChecked();
  expect(
    screen.getByRole("button", { name: "Queue issue submission" }),
  ).toBeDisabled();
});

it("preserves unsaved text while an account epoch change retires submission consent", async () => {
  snapshot = localSnapshot({
    title: "Saved title",
    body: "Saved body",
    generation: "1",
    availability: "available",
    reason: null,
  });
  const { user, rerenderAccount } = setup();
  await user.click(screen.getByRole("button", { name: "New issue" }));
  const consent = await screen.findByRole("checkbox", {
    name: /exact saved issue may be submitted/i,
  });
  await user.click(consent);
  rerenderAccount({ ...account, authorization_epoch: "8" });
  expect(screen.getByRole("dialog")).toBeVisible();
  expect(consent).not.toBeChecked();
  expect(
    screen.getByRole("button", { name: "Queue issue submission" }),
  ).toBeDisabled();
  await user.type(screen.getByLabelText("Description"), " unsaved");
  rerenderAccount({ ...account, authorization_epoch: "9" });
  expect(screen.getByLabelText("Description")).toHaveValue(
    "Saved body unsaved",
  );
});

it("gives a different actor a separate editor without the previous unsaved text", async () => {
  const { user, rerenderAccount } = setup();
  await user.click(screen.getByRole("button", { name: "New issue" }));
  await user.type(
    await screen.findByLabelText("Title"),
    "Private unsaved title",
  );
  rerenderAccount({ ...account, actor_id: "55" });
  expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  await user.click(screen.getByRole("button", { name: "New issue" }));
  expect(await screen.findByLabelText("Title")).toHaveValue("");
});

it("keeps opening local-only, preserves an unsaved close, then saves and explicitly queues", async () => {
  const read = mockTauriCommand("collaboration_issue_draft_v2", () =>
    versionTwo(),
  );
  const save = mockTauriCommand(
    "collaboration_save_issue_draft_v2",
    (payload) => {
      const request = (payload as { request: { title: string; body: string } })
        .request;
      snapshot = localSnapshot({
        title: request.title,
        body: request.body,
        generation: "1",
        availability: "available",
        reason: null,
        revision: "21",
      });
      return versionTwo();
    },
  );
  const submit = mockTauriCommand(
    "collaboration_submit_issue_v2",
    (payload) => ({
      account_id: account.id,
      command_id: (payload as { request: { command_id: string } }).request
        .command_id,
      admitted_revision: "22",
      duplicate: false,
    }),
  );
  const { user } = setup();
  expect(read).not.toHaveBeenCalled();
  await user.click(screen.getByRole("button", { name: "New issue" }));
  expect(await screen.findByRole("dialog")).toBeVisible();
  await waitFor(() => expect(read).toHaveBeenCalledTimes(1));
  await user.type(screen.getByLabelText("Title"), "Offline title");
  await user.type(screen.getByLabelText("Description"), "Retained body");
  await user.click(screen.getByRole("button", { name: "Close" }));
  await user.click(screen.getByRole("button", { name: "New issue" }));
  expect(await screen.findByLabelText("Title")).toHaveValue("Offline title");
  expect(screen.getByLabelText("Description")).toHaveValue("Retained body");
  await user.click(screen.getByRole("button", { name: "Save issue draft" }));
  await waitFor(() => expect(save).toHaveBeenCalledTimes(1));
  expect(
    screen.getByRole("button", { name: "Queue issue submission" }),
  ).toBeDisabled();
  await user.click(
    screen.getByRole("checkbox", {
      name: /exact saved issue may be submitted/i,
    }),
  );
  await user.click(
    screen.getByRole("button", { name: "Queue issue submission" }),
  );
  await waitFor(() => expect(submit).toHaveBeenCalledTimes(1));
  expect(
    (submit.mock.calls[0]?.[0] as { request: unknown }).request,
  ).toMatchObject({
    context,
    draft_id: snapshot.draft_id,
    draft_generation: "1",
    accept_background_delivery: true,
  });
});

it("retains the exact command UUID when admission succeeds but its IPC receipt is lost", async () => {
  snapshot = localSnapshot({
    title: "Saved title",
    body: "Saved body",
    generation: "1",
    availability: "available",
    reason: null,
  });
  let calls = 0;
  const submit = mockTauriCommand(
    "collaboration_submit_issue_v2",
    (payload) => {
      calls += 1;
      const request = (payload as { request: { command_id: string } }).request;
      if (calls === 1) {
        snapshot = localSnapshot({
          title: "Saved title",
          body: "Saved body",
          generation: "1",
          context: null,
          availability: "unavailable",
          reason: "pending_submission",
          submission: {
            command_id: request.command_id,
            draft_generation: "1",
            state: "queued",
            attempt_count: 0,
            quarantined: false,
            attention: null,
          },
          revision: "21",
        });
        throw { code: "network" };
      }
      return {
        account_id: account.id,
        command_id: request.command_id,
        admitted_revision: "21",
        duplicate: true,
      };
    },
  );
  const { cache, user } = setup();
  await user.click(screen.getByRole("button", { name: "New issue" }));
  await user.click(
    await screen.findByRole("checkbox", {
      name: /exact saved issue may be submitted/i,
    }),
  );
  await user.click(
    screen.getByRole("button", { name: "Queue issue submission" }),
  );
  expect(await screen.findByRole("alert")).toHaveTextContent(
    "exact request identity",
  );
  const key = issueDraftV2QueryOptions(account, {
    draft_id: snapshot.draft_id,
    repository_id: snapshot.repository_id,
  }).queryKey;
  await act(async () => {
    await cache.cancelQueries({ queryKey: key });
    cache.setQueryData(
      key,
      versionTwo({
        ...snapshot,
        availability: "unavailable",
        reason: "account_unavailable",
      }),
    );
  });
  await waitFor(() =>
    expect(
      screen.queryByRole("button", { name: "Retry exact issue submission" }),
    ).not.toBeInTheDocument(),
  );
  await act(async () => {
    cache.setQueryData(key, versionTwo());
  });
  await user.click(
    await screen.findByRole("button", {
      name: "Retry exact issue submission",
    }),
  );
  await waitFor(() => expect(submit).toHaveBeenCalledTimes(2));
  expect(submit.mock.calls[1]?.[0]).toEqual(submit.mock.calls[0]?.[0]);
});

it("does not offer a separate draft while a submission outcome is unresolved", async () => {
  snapshot = localSnapshot({
    title: "Saved title",
    body: "Saved body",
    generation: "1",
    context: null,
    availability: "unavailable",
    reason: "already_submitted",
    submission: {
      command_id: "22222222-2222-4222-8222-222222222222",
      draft_generation: "1",
      state: "unknown",
      attempt_count: 1,
      quarantined: false,
      attention: "The provider outcome is unknown.",
    },
  });
  const { user } = setup();
  await user.click(screen.getByRole("button", { name: "New issue" }));
  expect(await screen.findByLabelText("Title")).toHaveValue("Saved title");
  expect(
    screen.queryByRole("button", { name: "Start another draft" }),
  ).not.toBeInTheDocument();
});

it("reports native title boundaries before attempting a save", async () => {
  const save = mockTauriCommand("collaboration_save_issue_draft_v2", () =>
    versionTwo(),
  );
  const { user } = setup();
  await user.click(screen.getByRole("button", { name: "New issue" }));
  const title = await screen.findByLabelText("Title");
  await user.type(title, " padded title ");
  expect(screen.getByRole("alert")).toHaveTextContent(
    "Remove leading or trailing whitespace",
  );
  expect(
    screen.getByRole("button", { name: "Save issue draft" }),
  ).toBeDisabled();
  expect(save).not.toHaveBeenCalled();
});

it("navigates only from a validated cached canonical identity", async () => {
  snapshot = localSnapshot({
    title: "Published issue",
    generation: "1",
    context: null,
    availability: "unavailable",
    reason: "already_submitted",
    published: {
      subject_id: "github:issue:77",
      provider_id: "77",
      number: "12",
      url: "https://github.com/owner/project/issues/12",
      command_id: "22222222-2222-4222-8222-222222222222",
    },
  });
  const { user, onOpenCreated } = setup();
  await user.click(screen.getByRole("button", { name: "New issue" }));
  await user.click(
    await screen.findByRole("button", { name: "Open created issue" }),
  );
  expect(onOpenCreated).toHaveBeenCalledWith("github:issue:77");
});

it("keeps an open draft but immediately hides provider authority when disconnect refetch fails", async () => {
  const commandId = "22222222-2222-4222-8222-222222222222";
  snapshot = localSnapshot({
    title: "Published issue",
    body: "Authored body stays local",
    generation: "1",
    context: null,
    availability: "unavailable",
    reason: "already_submitted",
    submission: {
      command_id: commandId,
      draft_generation: "1",
      state: "confirmed",
      attempt_count: 1,
      quarantined: false,
      attention: null,
    },
    published: {
      subject_id: "github:issue:77",
      provider_id: "77",
      number: "12",
      url: "https://github.com/owner/project/issues/12",
      command_id: commandId,
    },
  });
  let reads = 0;
  const read = mockTauriCommand("collaboration_issue_draft_v2", () => {
    reads += 1;
    if (reads === 1) return versionTwo();
    throw { code: "storage", message: "fixture reset refetch failed" };
  });
  mockTauriCommand("collaboration_changes_since", () => ({
    revision: "20",
    authorization_view: "11",
    changes: [],
    has_more: false,
    reset_required: false,
  }));
  vi.spyOn(collaboration.transport, "listen").mockResolvedValue(() => {});
  vi.spyOn(collaboration.transport, "listenRuntimeReset").mockResolvedValue(
    () => {},
  );
  vi.spyOn(collaboration.transport, "listenLocalChanges").mockResolvedValue(
    () => {},
  );
  let finishDisconnect!: (revision: string) => void;
  mockTauriCommand(
    "collaboration_disconnect",
    () =>
      new Promise<string>((resolve) => {
        finishDisconnect = resolve;
      }),
  );
  const { cache, user } = setup();
  stops.push(collaboration.installBridge(cache));
  await act(async () => collaboration.wake());
  await user.click(screen.getByRole("button", { name: "New issue" }));
  expect(
    await screen.findByRole("button", { name: "Open created issue" }),
  ).toBeVisible();
  expect(
    screen.getByRole("button", { name: "Open on provider" }),
  ).toBeVisible();

  let disconnecting!: Promise<void>;
  act(() => {
    disconnecting = collaboration.disconnect(account.id);
  });
  await waitFor(() => expect(read).toHaveBeenCalledTimes(2));
  expect(screen.getByLabelText("Title")).toHaveValue("Published issue");
  expect(screen.getByLabelText("Description")).toHaveValue(
    "Authored body stays local",
  );
  expect(
    screen.queryByRole("button", { name: "Open created issue" }),
  ).not.toBeInTheDocument();
  expect(
    screen.queryByRole("button", { name: "Open on provider" }),
  ).not.toBeInTheDocument();
  expect(screen.getByText(/reconnect this account/i)).toBeVisible();
  expect(
    screen.getByRole("button", { name: "Queue issue submission" }),
  ).toBeDisabled();
  expect(screen.getByRole("alert")).toHaveTextContent(
    "disabled until this local draft reloads",
  );

  finishDisconnect("21");
  await act(async () => disconnecting);
});

it("a definite admission refusal allows shortening the saved issue without replaying it", async () => {
  snapshot = localSnapshot({
    title: "Saved title",
    body: "Saved body",
    generation: "1",
    availability: "available",
    reason: null,
  });
  const submit = mockTauriCommand("collaboration_submit_issue_v2", () => {
    throw { code: "invalid_input", message: "provider secret must not render" };
  });
  const save = mockTauriCommand(
    "collaboration_save_issue_draft_v2",
    (payload) => {
      const request = (payload as { request: { title: string; body: string } })
        .request;
      snapshot = localSnapshot({
        title: request.title,
        body: request.body,
        generation: "2",
        availability: "available",
        reason: null,
      });
      return versionTwo();
    },
  );
  const { user } = setup();
  await user.click(screen.getByRole("button", { name: "New issue" }));
  const consent = await screen.findByRole("checkbox", {
    name: /exact saved issue may be submitted/i,
  });
  await user.click(consent);
  await user.click(
    screen.getByRole("button", { name: "Queue issue submission" }),
  );
  expect(await screen.findByRole("alert")).toHaveTextContent(
    "Check the saved title, description and metadata",
  );
  expect(screen.getByRole("alert")).not.toHaveTextContent("provider secret");
  expect(
    screen.queryByRole("button", { name: "Retry exact issue submission" }),
  ).not.toBeInTheDocument();
  expect(consent).not.toBeChecked();
  const body = screen.getByLabelText("Description");
  expect(body).toHaveValue("Saved body");
  await user.clear(body);
  await user.type(body, "Short");
  await user.click(screen.getByRole("button", { name: "Save issue draft" }));
  await waitFor(() => expect(save).toHaveBeenCalledTimes(1));
  expect(submit).toHaveBeenCalledTimes(1);
  expect(
    screen.getByRole("button", { name: "Queue issue submission" }),
  ).toBeDisabled();
});

it("saves metadata atomically with text and requires separate best-effort consent", async () => {
  snapshot = localSnapshot({
    title: "Saved title",
    body: "Body",
    generation: "1",
    availability: "available",
    reason: null,
  });
  mockForegroundDemand();
  mockTauriCommand("collaboration_issue_metadata_options", () =>
    labelCatalog(),
  );
  const save = mockTauriCommand(
    "collaboration_save_issue_draft_v2",
    (payload) => {
      const request = (
        payload as {
          request: {
            title: string;
            body: string;
            metadata: IssueMetadataSelection;
          };
        }
      ).request;
      metadata = request.metadata;
      snapshot = {
        ...snapshot,
        title: request.title,
        body: request.body,
        generation: "2",
        revision: "21",
      };
      return versionTwo();
    },
  );
  const submit = mockTauriCommand(
    "collaboration_submit_issue_v2",
    (payload) => ({
      account_id: account.id,
      command_id: (payload as { request: { command_id: string } }).request
        .command_id,
      duplicate: false,
      admitted_revision: "22",
    }),
  );
  const { user } = setup();
  await user.click(screen.getByRole("button", { name: "New issue" }));
  await user.click(
    await screen.findByRole("button", { name: "Choose labels" }),
  );
  await user.click(
    await screen.findByRole("button", {
      name: /bug · Availability not yet verified/,
    }),
  );
  await user.click(screen.getByRole("button", { name: "Save issue draft" }));
  await waitFor(() => expect(save).toHaveBeenCalledOnce());
  expect(save.mock.calls[0]?.[0]).toMatchObject({
    request: {
      title: "Saved title",
      body: "Body",
      expected_generation: "1",
      metadata: {
        labels: [{ provider_id: "19", name: "bug" }],
        assignees: [],
        milestone: null,
      },
    },
  });
  await user.click(
    screen.getByRole("checkbox", {
      name: /exact saved issue may be submitted/i,
    }),
  );
  const queue = screen.getByRole("button", { name: "Queue issue submission" });
  expect(queue).toBeDisabled();
  await user.click(
    screen.getByRole("checkbox", {
      name: /issue may be created even if GitHub/i,
    }),
  );
  expect(queue).toBeEnabled();
  await user.click(queue);
  await waitFor(() => expect(submit).toHaveBeenCalledOnce());
  expect(submit.mock.calls[0]?.[0]).toMatchObject({
    request: {
      draft_generation: "2",
      accept_metadata_best_effort: true,
      accept_background_delivery: true,
    },
  });
});

it("retires both consents after metadata removal and reselection, and keeps saved names offline", async () => {
  snapshot = localSnapshot({
    title: "Saved title",
    generation: "1",
    availability: "available",
    reason: null,
  });
  metadata = {
    labels: [{ provider_id: "19", name: "bug", color: "ff0000" }],
    assignees: [],
    milestone: null,
  };
  mockForegroundDemand();
  mockTauriCommand("collaboration_issue_metadata_options", () =>
    labelCatalog(),
  );
  const { user, rerenderAccount } = setup();
  await user.click(screen.getByRole("button", { name: "New issue" }));
  const deliveryConsent = await screen.findByRole("checkbox", {
    name: /exact saved issue may be submitted/i,
  });
  await user.click(deliveryConsent);
  await user.click(
    screen.getByRole("checkbox", {
      name: /issue may be created even if GitHub/i,
    }),
  );
  await user.click(screen.getByRole("button", { name: "Remove bug" }));
  await user.click(screen.getByRole("button", { name: "Choose labels" }));
  await user.click(
    await screen.findByRole("button", {
      name: /bug · Availability not yet verified/,
    }),
  );
  expect(deliveryConsent).not.toBeChecked();
  expect(
    screen.getByRole("checkbox", {
      name: /issue may be created even if GitHub/i,
    }),
  ).not.toBeChecked();
  expect(screen.getByRole("button", { name: "Draft saved" })).toBeDisabled();
  rerenderAccount({ ...account, state: "disconnected" });
  expect(
    screen.queryByRole("button", { name: "Choose labels" }),
  ).not.toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Remove bug" })).toBeEnabled();
  await user.click(screen.getByRole("button", { name: "Remove bug" }));
  expect(
    screen.getByRole("button", { name: "Save issue draft" }),
  ).toBeEnabled();
});

it("bounds catalog pages and search, refuses unavailable options and releases demand on close", async () => {
  snapshot = localSnapshot({
    title: "Saved title",
    generation: "1",
    availability: "available",
    reason: null,
  });
  const demand = mockForegroundDemand();
  const read = mockTauriCommand(
    "collaboration_issue_metadata_options",
    (payload) => {
      const query = (
        payload as { query: { cursor: string | null; search: string } }
      ).query;
      if (query.cursor === "page-two")
        return labelCatalog({ options: [], next_cursor: null });
      return labelCatalog({
        next_cursor: "page-two",
        options: [
          {
            reference: {
              kind: "label",
              value: { provider_id: "20", name: "archived", color: null },
            },
            availability: "unavailable",
            reason: "archived",
          },
        ],
      });
    },
  );
  const refresh = mockTauriCommand(
    "collaboration_refresh_issue_metadata",
    () => ({ job_id: "metadata-refresh" }),
  );
  const { user, cache } = setup();
  mockTauriCommand("collaboration_changes_since", () => ({
    revision: "20",
    authorization_view: "11",
    changes: [],
    has_more: false,
    reset_required: false,
  }));
  vi.spyOn(collaboration.transport, "listen").mockResolvedValue(() => {});
  vi.spyOn(collaboration.transport, "listenRuntimeReset").mockResolvedValue(
    () => {},
  );
  vi.spyOn(collaboration.transport, "listenLocalChanges").mockResolvedValue(
    () => {},
  );
  stops.push(collaboration.installBridge(cache));
  await act(async () => collaboration.wake());
  await user.click(screen.getByRole("button", { name: "New issue" }));
  expect(read).not.toHaveBeenCalled();
  await user.click(
    await screen.findByRole("button", { name: "Choose labels" }),
  );
  expect(
    await screen.findByRole("button", { name: "archived · Unavailable" }),
  ).toBeDisabled();
  expect(screen.getByText(/Saved options cover part/)).toBeVisible();
  expect(refresh).not.toHaveBeenCalled();
  await waitFor(() => expect(demand.acquire).toHaveBeenCalledOnce());
  await user.click(screen.getByRole("button", { name: "Next saved options" }));
  await waitFor(() =>
    expect(read).toHaveBeenLastCalledWith({
      query: {
        account_id: account.id,
        repository_id: repository.id,
        kind: "labels",
        search: "",
        cursor: "page-two",
        limit: 50,
      },
    }),
  );
  await user.type(screen.getByLabelText("Search saved labels"), "bug");
  await waitFor(() =>
    expect(read).toHaveBeenLastCalledWith({
      query: {
        account_id: account.id,
        repository_id: repository.id,
        kind: "labels",
        search: "bug",
        cursor: null,
        limit: 50,
      },
    }),
  );
  await waitFor(() =>
    expect(
      cache
        .getQueryCache()
        .findAll({ predicate: (q) => q.queryKey.includes("issue-metadata") }),
    ).toHaveLength(1),
  );
  await user.click(screen.getByRole("button", { name: "Close" }));
  await waitFor(() => expect(demand.release).toHaveBeenCalledOnce());
});

it("preserves a conflicting local metadata edit when loading another window's draft", async () => {
  snapshot = localSnapshot({
    title: "Saved title",
    generation: "1",
    availability: "available",
    reason: null,
  });
  metadata = {
    labels: [{ provider_id: "19", name: "bug", color: null }],
    assignees: [],
    milestone: null,
  };
  const { user, cache } = setup();
  await user.click(screen.getByRole("button", { name: "New issue" }));
  await user.type(
    await screen.findByLabelText("Description"),
    "My unsaved description",
  );
  const queryKey = issueDraftV2QueryOptions(account, {
    draft_id: snapshot.draft_id,
    repository_id: repository.id,
  }).queryKey;
  await act(async () => {
    cache.setQueryData(queryKey, {
      draft: { ...snapshot, generation: "2", title: "Other window" },
      metadata: {
        labels: [],
        assignees: [],
        milestone: { provider_id: "88", number: "2", title: "Later" },
      },
      metadata_outcome: null,
    });
  });
  await waitFor(() =>
    expect(
      screen.getByRole("button", { name: "Save issue draft" }),
    ).toBeDisabled(),
  );
  await user.click(
    await screen.findByRole("button", { name: "Load latest issue draft" }),
  );
  expect(screen.getByLabelText("Title")).toHaveValue("Other window");
  expect(screen.getByRole("button", { name: "Remove Later" })).toBeVisible();
  await user.click(screen.getByText("Your previous draft"));
  expect(screen.getByText("My unsaved description")).toBeVisible();
  expect(screen.getByText("bug")).toBeVisible();
});

it("shows partial metadata success alongside the confirmed issue without a corrective submission", async () => {
  const commandId = "22222222-2222-4222-8222-222222222222";
  snapshot = localSnapshot({
    title: "Published",
    generation: "1",
    context: null,
    reason: "already_submitted",
    published: {
      command_id: commandId,
      subject_id: "issue-77",
      provider_id: "77",
      number: "12",
      url: "https://github.com/owner/project/issues/12",
    },
  });
  metadata = {
    labels: [{ provider_id: "19", name: "bug", color: null }],
    assignees: [{ provider_id: "4", login: "writer" }],
    milestone: null,
  };
  outcome = {
    command_id: commandId,
    needs_attention: true,
    fields: [
      { field: "labels", result: "different", reason: null },
      { field: "assignees", result: "unobserved", reason: "malformed" },
      { field: "milestone", result: "not_requested", reason: null },
    ],
  };
  const { user, onOpenCreated, rerenderAccount } = setup();
  await user.click(screen.getByRole("button", { name: "New issue" }));
  expect(
    await screen.findByText(/some requested metadata needs attention/),
  ).toBeVisible();
  expect(screen.getByText(/Labels: differed/)).toBeVisible();
  expect(screen.getByText(/Assignees: could not be verified/)).toBeVisible();
  await user.click(screen.getByRole("button", { name: "Open created issue" }));
  expect(onOpenCreated).toHaveBeenCalledWith("issue-77");
  await user.click(screen.getByRole("button", { name: "New issue" }));
  rerenderAccount({ ...account, state: "disconnected" });
  expect(
    screen.queryByText(/some requested metadata needs attention/),
  ).not.toBeInTheDocument();
  expect(
    screen.queryByRole("button", { name: "Open created issue" }),
  ).not.toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Remove bug" })).toBeVisible();
});

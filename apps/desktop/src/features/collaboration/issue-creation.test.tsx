import type {
  IssueDraftSnapshot,
  RemoteAccount,
  RemoteRepository,
} from "@gitru/collaboration-client";
import { collaboration } from "@gitru/collaboration-client";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
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
  mockTauriCommand("collaboration_issue_draft", () => snapshot);
  mockTauriCommand("collaboration_issue_drafts", () => ({
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
  render(
    <QueryClientProvider client={cache}>
      <NewIssueDialog
        account={account}
        repository={repository}
        onOpenCreated={onOpenCreated}
      />
    </QueryClientProvider>,
  );
  return { cache, user: userEvent.setup(), onOpenCreated };
}

it("keeps opening local-only, preserves an unsaved close, then saves and explicitly queues", async () => {
  const read = mockTauriCommand("collaboration_issue_draft", () => snapshot);
  const save = mockTauriCommand("collaboration_save_issue_draft", (payload) => {
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
    return snapshot;
  });
  const submit = mockTauriCommand("collaboration_submit_issue", (payload) => ({
    account_id: account.id,
    command_id: (payload as { request: { command_id: string } }).request
      .command_id,
    admitted_revision: "22",
    duplicate: false,
  }));
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
  const submit = mockTauriCommand("collaboration_submit_issue", (payload) => {
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
  });
  const { user } = setup();
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
  await user.click(
    screen.getByRole("button", { name: "Retry exact issue submission" }),
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
  const save = mockTauriCommand(
    "collaboration_save_issue_draft",
    () => snapshot,
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
  const read = mockTauriCommand("collaboration_issue_draft", () => {
    reads += 1;
    if (reads === 1) return snapshot;
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

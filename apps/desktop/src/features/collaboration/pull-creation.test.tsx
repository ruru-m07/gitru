import {
  collaboration,
  type PullCreationPreview,
  type PullDraftSnapshot,
  type SavePullDraftRequest,
  type SubmitPullRequest,
} from "@gitru/collaboration-client";
import { pullDraftQueryOptions } from "@gitru/collaboration-client/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { useAppStore } from "@/store/use-app-store";
import {
  fixtureAccount as account,
  fixtureRepositories,
} from "../../../tests/fixtures/collaboration";
import {
  fixtureInspection,
  fixtureLink,
} from "../../../tests/fixtures/local-links";
import { mockTauriCommand } from "../../../tests/mocks/tauri";
import { DraftRecovery } from "./draft-recovery";
import { NewPullDialog, RecoveredPullDraft } from "./pull-creation";

vi.mock("@tauri-apps/plugin-store", () => ({
  Store: {
    load: async () => ({
      get: async () => null,
      set: async () => {},
      save: async () => {},
    }),
  },
}));
const repository = fixtureRepositories.repositories[0]!;
const key = {
  account_id: account.id,
  draft_id: "123e4567-e89b-42d3-a456-426614174000",
  repository_id: repository.id,
};
const values = {
  title: "New feature",
  body: "Private draft",
  source_branch: "feature",
  base_branch: "main",
  local_repository_id: "registered-a",
  link_id: fixtureLink.id,
  link_generation: fixtureLink.generation,
  is_draft: true,
};
let snapshot: PullDraftSnapshot;
let onlinePreview: PullCreationPreview;
let inspection = fixtureInspection();
const caches: QueryClient[] = [];
const stops: Array<() => void> = [];
const initialStore = useAppStore.getState();
beforeEach(() => {
  const matches = Element.prototype.matches;
  vi.spyOn(Element.prototype, "matches").mockImplementation(function (
    this: Element,
    selector: string,
  ) {
    // jsdom does not implement these Base UI top-layer selectors.
    return [":modal", ":fullscreen", ":popover-open"].includes(selector)
      ? false
      : matches.call(this, selector);
  });

  vi.spyOn(crypto, "randomUUID").mockReturnValue(
    "123e4567-e89b-42d3-a456-426614174000",
  );
  snapshot = {
    key,
    values,
    generation: "1",
    can_preview: true,
    reason: null,
    submission: null,
    published: null,
    revision: "10",
    authorization_view: "1",
  };
  onlinePreview = {
    context: {
      key,
      draft_generation: "1",
      authorization_epoch: account.authorization_epoch,
      authorization_view: "1",
      grant_id: "223e4567-e89b-42d3-a456-426614174000",
      source_oid: "a".repeat(40),
      base_oid: "b".repeat(40),
    },
    reason: null,
    values,
    local_source_oid: "a".repeat(40),
    observed_source_oid: "a".repeat(40),
    observed_base_oid: "b".repeat(40),
    can_push: true,
    observed_at: "2026-10-08T00:00:00Z",
    expires_in_seconds: 60,
    authorization_view: "1",
  };
  inspection = fixtureInspection();
  useAppStore.setState({
    repositories: [
      {
        id: "registered-a",
        name: "Local clone",
        path: "/synthetic/private-clone",
        origin: null,
        current_branch: null,
        ahead_behind: null,
        has_uncommitted_changes: false,
        last_updated: 0,
      },
    ],
  });
  mockTauriCommand("collaboration_pull_draft", () => snapshot);
  mockTauriCommand("collaboration_pull_drafts", () => ({
    account_id: account.id,
    drafts: [],
    next_cursor: null,
    revision: "10",
    authorization_view: "1",
  }));
  mockTauriCommand("collaboration_local_links", () => inspection);
  vi.spyOn(collaboration.transport, "listen").mockResolvedValue(() => {});
  vi.spyOn(collaboration.transport, "listenLocalChanges").mockResolvedValue(
    () => {},
  );
});
afterEach(() => {
  for (const stop of stops.splice(0)) stop();
  for (const cache of caches.splice(0)) cache.clear();
  useAppStore.setState(initialStore);
  vi.restoreAllMocks();
});
function setup(element?: React.ReactNode) {
  const cache = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  caches.push(cache);
  render(
    <QueryClientProvider client={cache}>
      {element ?? (
        <NewPullDialog
          account={account}
          repository={repository}
          onOpenCreated={vi.fn()}
        />
      )}
    </QueryClientProvider>,
  );
  return { cache, user: userEvent.setup() };
}
async function open(user: ReturnType<typeof userEvent.setup>) {
  await user.click(screen.getByRole("button", { name: "New pull request" }));
  await screen.findByLabelText("Title");
}
async function check(user: ReturnType<typeof userEvent.setup>) {
  await waitFor(() =>
    expect(
      screen.getByRole("button", { name: "Check branches online" }),
    ).toBeEnabled(),
  );
  await user.click(
    screen.getByRole("button", { name: "Check branches online" }),
  );
  await screen.findByText(/GitHub currently reports push permission/);
}

it("opens and saves locally, preserving unsaved text across dialog close without preview or submission", async () => {
  const preview = mockTauriCommand(
    "collaboration_preview_pull_creation",
    () => onlinePreview,
  );
  const submit = mockTauriCommand("collaboration_submit_pull", () => {
    throw new Error("must not submit");
  });
  const save = mockTauriCommand("collaboration_save_pull_draft", (payload) => {
    const request = (payload as { request: SavePullDraftRequest }).request;
    snapshot = {
      ...snapshot,
      values: request.values,
      generation: "2",
      revision: "11",
    };
    return snapshot;
  });
  const { user } = setup();
  await open(user);
  await user.type(screen.getByLabelText("Title"), " changed");
  await user.click(screen.getByRole("button", { name: "Close" }));
  await waitFor(() =>
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument(),
  );
  await open(user);
  expect(screen.getByLabelText("Title")).toHaveValue("New feature changed");
  await user.click(
    screen.getByRole("button", { name: "Save pull request draft" }),
  );
  expect(
    await screen.findByText("Pull request draft saved on this device."),
  ).toBeVisible();
  expect(save).toHaveBeenCalledWith({
    request: {
      key,
      authorization_epoch: account.authorization_epoch,
      authorization_view: "1",
      expected_generation: "1",
      values: { ...values, title: "New feature changed" },
    },
  });
  expect(JSON.stringify(save.mock.calls)).not.toContain("/synthetic/");
  expect(preview).not.toHaveBeenCalled();
  expect(submit).not.toHaveBeenCalled();
});
it("requires an explicit branch check and current-branches consent, then retries only the same local receipt", async () => {
  const preview = mockTauriCommand(
    "collaboration_preview_pull_creation",
    () => onlinePreview,
  );
  const submit = mockTauriCommand("collaboration_submit_pull", () => {
    throw { code: "not_ready" };
  });
  const { user, cache } = setup();
  await open(user);
  await check(user);
  expect(preview).toHaveBeenCalledTimes(1);
  expect(
    screen.getByRole("button", { name: "Create pull request" }),
  ).toBeDisabled();
  await user.click(
    screen.getByRole("checkbox", {
      name: /I want to create this pull request/,
    }),
  );
  await user.click(screen.getByRole("button", { name: "Create pull request" }));
  const retry = await screen.findByRole("button", {
    name: "Recover exact local receipt",
  });
  const sent = (submit.mock.calls[0]![0] as { request: SubmitPullRequest })
    .request;
  snapshot = {
    ...snapshot,
    can_preview: false,
    reason: "pending_submission",
    submission: {
      command_id: sent.command_id,
      draft_generation: "1",
      state: "outcome_unknown",
      attempt_count: 1,
      quarantined: false,
      attention: null,
    },
  };
  act(() =>
    cache.setQueryData(pullDraftQueryOptions(account, key).queryKey, snapshot),
  );
  await user.click(retry);
  expect(submit).toHaveBeenCalledTimes(2);
  expect(submit.mock.calls[1]).toEqual(submit.mock.calls[0]);
  expect(
    await screen.findByText(/The creation outcome is uncertain/),
  ).toBeVisible();
  expect(screen.getByLabelText("Title")).toBeDisabled();
});
it("expires a preview without granting any authority from the renderer timer", async () => {
  onlinePreview = { ...onlinePreview, expires_in_seconds: 0 };
  mockTauriCommand("collaboration_preview_pull_creation", () => onlinePreview);
  const { user } = setup();
  await open(user);
  await check(user);
  expect(
    await screen.findByText("This branch check expired. Check online again."),
  ).toBeVisible();
  expect(
    screen.getByRole("button", { name: "Create pull request" }),
  ).toBeDisabled();
});
it("preserves edits when another window saves and requires an explicit latest-snapshot load", async () => {
  const { user, cache } = setup();
  await open(user);
  await user.type(screen.getByLabelText("Title"), " mine");
  snapshot = {
    ...snapshot,
    generation: "2",
    values: { ...values, title: "Other window" },
  };
  act(() =>
    cache.setQueryData(pullDraftQueryOptions(account, key).queryKey, snapshot),
  );
  expect(screen.getByLabelText("Title")).toHaveValue("New feature mine");
  await waitFor(() =>
    expect(
      screen.getByRole("button", { name: "Save pull request draft" }),
    ).toBeDisabled(),
  );
  await user.click(
    screen.getByRole("button", { name: "Load latest pull request draft" }),
  );
  expect(screen.getByLabelText("Title")).toHaveValue("Other window");
  expect(screen.getByText(/New feature mine/)).toBeInTheDocument();
});
it("retires an exact-receipt retry synchronously on native runtime replacement", async () => {
  let reset: (() => void) | undefined;
  vi.spyOn(collaboration.transport, "listenRuntimeReset").mockImplementation(
    async (callback) => {
      reset = callback;
      return () => {};
    },
  );
  mockTauriCommand("collaboration_changes_since", () => ({
    revision: "10",
    authorization_view: "1",
    changes: [],
    has_more: false,
    reset_required: false,
  }));
  mockTauriCommand("collaboration_preview_pull_creation", () => onlinePreview);
  const submit = mockTauriCommand("collaboration_submit_pull", () => {
    throw { code: "not_ready" };
  });
  const { user, cache } = setup();
  stops.push(collaboration.installBridge(cache));
  await act(async () => {
    await collaboration.wake();
  });
  await open(user);
  await check(user);
  await user.click(
    screen.getByRole("checkbox", {
      name: /I want to create this pull request/,
    }),
  );
  await user.click(screen.getByRole("button", { name: "Create pull request" }));
  const retry = await screen.findByRole("button", {
    name: "Recover exact local receipt",
  });
  await waitFor(() => expect(retry).toBeEnabled());
  act(() => reset?.());
  expect(retry).toBeDisabled();
  fireEvent.click(retry);
  expect(submit).toHaveBeenCalledTimes(1);
  expect(screen.getByLabelText("Title")).toHaveValue(values.title);
});
it("disables a preview when the current confirmed local link disappears", async () => {
  mockTauriCommand("collaboration_preview_pull_creation", () => onlinePreview);
  const { user, cache } = setup();
  await open(user);
  await check(user);
  await user.click(
    screen.getByRole("checkbox", {
      name: /I want to create this pull request/,
    }),
  );
  expect(
    screen.getByRole("button", { name: "Create pull request" }),
  ).toBeEnabled();
  inspection = {
    ...inspection,
    snapshot: { ...inspection.snapshot, links: [] },
  };
  await act(async () => {
    await cache.invalidateQueries({
      queryKey: ["collaboration", "local-links"],
    });
  });
  await waitFor(() =>
    expect(
      screen.getByRole("button", { name: "Create pull request" }),
    ).toBeDisabled(),
  );
});
it("retains the confirmed PR identity and displays inspected versus created branch drift", async () => {
  const onOpenCreated = vi.fn();
  snapshot = {
    ...snapshot,
    can_preview: false,
    reason: "already_submitted",
    published: {
      subject_id: "created-pr",
      provider_id: "100",
      number: "14",
      url: "https://github.com/example-org/engine/pull/14",
      command_id: "command",
      inspected_source_oid: "a".repeat(40),
      inspected_base_oid: "b".repeat(40),
      observed_source_oid: "c".repeat(40),
      observed_base_oid: "d".repeat(40),
      branches_changed: true,
    },
  };
  const { user } = setup(
    <RecoveredPullDraft
      account={account}
      draftId={key.draft_id}
      repositoryId={repository.id}
      onOpenCreated={onOpenCreated}
    />,
  );
  expect(
    await screen.findByText(/Pull request #14 was confirmed/),
  ).toBeVisible();
  expect(screen.getByText(/The branches moved after inspection/)).toBeVisible();
  await user.click(
    screen.getByRole("button", { name: "Open created pull request" }),
  );
  expect(onOpenCreated).toHaveBeenCalledWith("created-pr");
});
it("finds and edits saved PR drafts through the paginated recovery index while disconnected", async () => {
  const disconnected = { ...account, state: "disconnected" as const };
  snapshot = {
    ...snapshot,
    values: {
      ...values,
      local_repository_id: "",
      link_id: "",
      link_generation: "",
    },
    can_preview: false,
    reason: "account_unavailable",
  };
  const list = mockTauriCommand("collaboration_pull_drafts", (payload) => {
    const cursor = (payload as { query: { cursor: string | null } }).query
      .cursor;
    return {
      account_id: account.id,
      drafts: cursor
        ? [
            {
              draft_id: key.draft_id,
              repository_id: repository.id,
              title: values.title,
              preview: values.body,
              source_branch: values.source_branch,
              base_branch: values.base_branch,
              generation: "1",
              submission: null,
            },
          ]
        : [],
      next_cursor: cursor ? null : "next-draft",
      revision: "10",
      authorization_view: "1",
    };
  });
  const { user } = setup(<DraftRecovery accounts={[disconnected]} />);
  await user.click(screen.getByRole("button", { name: "Pull request drafts" }));
  await user.click(
    await screen.findByRole("button", { name: "Next pull request drafts" }),
  );
  await user.click(
    await screen.findByRole("button", {
      name: `Open pull request draft ${values.title}`,
    }),
  );
  expect(await screen.findByLabelText("Description")).toHaveValue(values.body);
  expect(
    screen.getByRole("button", { name: "Check branches online" }),
  ).toBeDisabled();
  await user.type(screen.getByLabelText("Title"), " offline");
  expect(
    screen.getByRole("button", { name: "Save pull request draft" }),
  ).toBeEnabled();
  expect(list).toHaveBeenLastCalledWith({
    query: { account_id: account.id, cursor: "next-draft", limit: 50 },
  });
});

it("requires explicit local clone and native link selection and keeps filesystem paths out of the draft", async () => {
  snapshot = {
    ...snapshot,
    values: {
      ...values,
      local_repository_id: "",
      link_id: "",
      link_generation: "",
    },
    can_preview: false,
    reason: "incomplete_draft",
  };
  const links = mockTauriCommand("collaboration_local_links", () => inspection);
  const save = mockTauriCommand("collaboration_save_pull_draft", (payload) => {
    const request = (payload as { request: SavePullDraftRequest }).request;
    snapshot = {
      ...snapshot,
      values: request.values,
      generation: "2",
      can_preview: true,
      reason: null,
    };
    return snapshot;
  });
  const { user } = setup();
  await open(user);
  expect(links).not.toHaveBeenCalled();
  await user.click(
    screen.getByRole("combobox", { name: "Local source repository" }),
  );
  await user.click(await screen.findByRole("option", { name: "Local clone" }));
  await waitFor(() =>
    expect(links).toHaveBeenCalledWith({ localRepositoryId: "registered-a" }),
  );
  await user.click(
    screen.getByRole("combobox", { name: "Source repository link" }),
  );
  await user.click(
    await screen.findByRole("option", { name: "origin · fetch" }),
  );
  await user.click(
    screen.getByRole("button", { name: "Save pull request draft" }),
  );
  await screen.findByText("Pull request draft saved on this device.");
  expect(save).toHaveBeenCalledWith({
    request: {
      key,
      authorization_epoch: account.authorization_epoch,
      authorization_view: "1",
      expected_generation: "1",
      values,
    },
  });
  expect(JSON.stringify(save.mock.calls)).not.toContain("/synthetic/");
});
it("retires preview consent on authored edits even if the user later restores the same text", async () => {
  mockTauriCommand("collaboration_preview_pull_creation", () => onlinePreview);
  const { user } = setup();
  await open(user);
  await check(user);
  await user.click(
    screen.getByRole("checkbox", {
      name: /I want to create this pull request/,
    }),
  );
  await user.type(screen.getByLabelText("Title"), "x");
  await user.type(screen.getByLabelText("Title"), "{backspace}");
  expect(screen.getByLabelText("Title")).toHaveValue(values.title);
  expect(
    screen.queryByRole("button", { name: "Create pull request" }),
  ).not.toBeInTheDocument();
  expect(
    screen.getByRole("button", { name: "Check branches online" }),
  ).toBeEnabled();
});

it("keeps an oversized encoded draft editable and explains a refused online preview", async () => {
  mockTauriCommand("collaboration_preview_pull_creation", () => {
    throw { code: "invalid_input", message: "not displayed" };
  });
  const submit = mockTauriCommand("collaboration_submit_pull", () => {
    throw new Error("must not submit");
  });
  const { user } = setup();
  await open(user);
  await waitFor(() =>
    expect(
      screen.getByRole("button", { name: "Check branches online" }),
    ).toBeEnabled(),
  );
  await user.click(
    screen.getByRole("button", { name: "Check branches online" }),
  );
  expect(
    await screen.findByText(
      /Check the branch names and try a shorter title or description/,
    ),
  ).toBeVisible();
  expect(screen.getByLabelText("Description")).toHaveValue(values.body);
  expect(screen.getByLabelText("Description")).toBeEnabled();
  expect(
    screen.queryByRole("button", { name: "Create pull request" }),
  ).not.toBeInTheDocument();
  expect(submit).not.toHaveBeenCalled();
});

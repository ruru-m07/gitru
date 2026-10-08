import {
  collaboration,
  collaborationKeys,
  type RemoteAccount,
} from "@gitru/collaboration-client";
import type { LocalDraft } from "@gitru/commands";
import {
  onlineManager,
  QueryClient,
  QueryClientProvider,
} from "@tanstack/react-query";
import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, expect, it, vi } from "vitest";
import {
  fixtureAccount,
  fixtureAccounts,
} from "../../../tests/fixtures/collaboration";
import {
  mockTauriCommand,
  mockTauriCommandResult,
} from "../../../tests/mocks/tauri";
import { DraftRecovery } from "./draft-recovery";
import { CollaborationWorkspace } from "./workspace";

const account: RemoteAccount = {
  ...fixtureAccount,
  state: "disconnected",
  authorization_epoch: "2",
};
const other: RemoteAccount = {
  ...account,
  id: "actor-b",
  actor_id: "another-actor",
  login: "second-user",
};
const saved = {
  account_id: account.id,
  subject_id: "missing-subject",
  body: "My authored text 🪴",
  generation: "1",
};
const caches: QueryClient[] = [];
afterEach(() => {
  for (const cache of caches.splice(0)) cache.clear();
  onlineManager.setOnline(true);
});

function mount(component: React.ReactNode) {
  const cache = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  caches.push(cache);
  return {
    cache,
    ...render(
      <QueryClientProvider client={cache}>{component}</QueryClientProvider>,
    ),
  };
}
function readMocks() {
  mockTauriCommandResult("collaboration_accounts", {
    ...fixtureAccounts,
    accounts: [account],
  });
  const drafts = mockTauriCommandResult("collaboration_drafts", {
    drafts: [
      {
        subject_id: saved.subject_id,
        preview: saved.body,
        generation: saved.generation,
      },
    ],
    next_cursor: null,
  });
  const draft = mockTauriCommandResult("collaboration_draft", saved);
  return { drafts, draft };
}

it("recovers missing-subject drafts offline from an entirely disconnected workspace", async () => {
  onlineManager.setOnline(false);
  const { drafts, draft } = readMocks();
  const user = userEvent.setup();
  mount(<CollaborationWorkspace kind="issue" />);
  expect(
    await screen.findByText("Bring your remote work into Gitru"),
  ).toBeVisible();
  await user.click(screen.getByRole("button", { name: "Drafts" }));
  expect(await screen.findByText(/Disconnected account/)).toBeVisible();
  await user.click(
    await screen.findByRole("button", {
      name: "Open draft for missing-subject",
    }),
  );
  expect(await screen.findByLabelText("Private draft")).toHaveValue(saved.body);
  expect(drafts).toHaveBeenCalledWith({
    query: { account_id: account.id, cursor: null, limit: 50 },
  });
  expect(draft).toHaveBeenCalledWith({
    accountId: account.id,
    subjectId: saved.subject_id,
  });
  // The fail-closed native mocks would reject any provider item/refresh/credential call.
});

it("copies current text, requires save before export, and distinguishes native cancellation and failure", async () => {
  readMocks();
  const user = userEvent.setup();
  const copy = vi.spyOn(navigator.clipboard, "writeText").mockResolvedValue();
  const exportDraft = mockTauriCommandResult(
    "collaboration_export_draft",
    false,
  );
  mockTauriCommand("collaboration_save_draft", (payload) => ({
    ...(payload as { draft: typeof saved }).draft,
    generation: "2",
  }));
  mount(<DraftRecovery accounts={[account]} />);
  await user.click(
    await screen.findByRole("button", {
      name: "Open draft for missing-subject",
    }),
  );
  const text = await screen.findByLabelText("Private draft");
  await user.type(text, " edited");
  await user.click(screen.getByRole("button", { name: "Copy draft text" }));
  expect(copy).toHaveBeenCalledWith(`${saved.body} edited`);
  expect(
    screen.getByRole("button", { name: "Export saved draft" }),
  ).toBeDisabled();
  await user.click(screen.getByRole("button", { name: "Save draft" }));
  await waitFor(() =>
    expect(
      screen.getByRole("button", { name: "Export saved draft" }),
    ).toBeEnabled(),
  );
  await user.click(screen.getByRole("button", { name: "Export saved draft" }));
  expect(await screen.findByText("Export cancelled.")).toBeVisible();
  expect(exportDraft).toHaveBeenCalledWith({
    accountId: account.id,
    subjectId: saved.subject_id,
    generation: "2",
  });
  exportDraft.mockResolvedValueOnce(true);
  await user.click(screen.getByRole("button", { name: "Export saved draft" }));
  expect(await screen.findByText("Saved draft exported.")).toBeVisible();
  exportDraft.mockRejectedValueOnce({
    code: "storage",
    message: "sensitive path",
  });
  await user.click(screen.getByRole("button", { name: "Export saved draft" }));
  expect(await screen.findByRole("alert")).toHaveTextContent(
    "could not be read",
  );
  expect(screen.queryByText(/sensitive path/)).not.toBeInTheDocument();
  expect(text).toHaveValue(`${saved.body} edited`);
});

it("preserves concurrent edits and retains the previous text after explicitly reloading", async () => {
  readMocks();
  const user = userEvent.setup();
  const { cache } = mount(<DraftRecovery accounts={[account]} />);
  await user.click(
    await screen.findByRole("button", {
      name: "Open draft for missing-subject",
    }),
  );
  const text = await screen.findByLabelText("Private draft");
  await user.type(text, " unsaved");
  act(() => {
    cache.setQueryData<LocalDraft>(
      collaborationKeys.draft(account, saved.subject_id),
      {
        ...saved,
        body: "Another tab's saved text",
        generation: "2",
      },
    );
  });
  expect(text).toHaveValue(`${saved.body} unsaved`);
  expect(
    await screen.findByText(/This draft changed in another tab/),
  ).toBeVisible();
  expect(screen.getByRole("button", { name: "Save draft" })).toBeDisabled();
  expect(
    screen.getByRole("button", { name: "Export saved draft" }),
  ).toBeDisabled();
  await user.click(screen.getByRole("button", { name: "Reload saved draft" }));
  expect(text).toHaveValue("Another tab's saved text");
  expect(screen.getByLabelText("Previous draft text")).toHaveValue(
    `${saved.body} unsaved`,
  );
});

it("does not render a delayed prior actor read when the selected account becomes unavailable", async () => {
  readMocks();
  let finish!: (value: typeof saved) => void;
  mockTauriCommand("collaboration_draft", (payload) =>
    (payload as { accountId: string }).accountId === account.id
      ? new Promise((resolve) => {
          finish = resolve;
        })
      : { ...saved, account_id: other.id, body: "Second actor's text" },
  );
  const user = userEvent.setup();
  const { rerender, cache } = mount(<DraftRecovery accounts={[account]} />);
  await user.click(
    await screen.findByRole("button", {
      name: "Open draft for missing-subject",
    }),
  );
  await waitFor(() => expect(finish).toBeDefined());
  rerender(
    <QueryClientProvider client={cache}>
      <DraftRecovery accounts={[other]} />
    </QueryClientProvider>,
  );
  await user.click(
    await screen.findByRole("button", {
      name: "Open draft for missing-subject",
    }),
  );
  expect(await screen.findByLabelText("Private draft")).toHaveValue(
    "Second actor's text",
  );
  await act(async () => {
    finish(saved);
  });
  expect(screen.getByLabelText("Private draft")).toHaveValue(
    "Second actor's text",
  );
  expect(screen.queryByDisplayValue(saved.body)).not.toBeInTheDocument();
});

it("does not render a delayed prior subject read after selecting another draft", async () => {
  readMocks();
  let finish!: (value: typeof saved) => void;
  mockTauriCommandResult("collaboration_drafts", {
    drafts: [
      { subject_id: "first", preview: "first", generation: "1" },
      { subject_id: "second", preview: "second", generation: "1" },
    ],
    next_cursor: null,
  });
  mockTauriCommand("collaboration_draft", (payload) =>
    (payload as { subjectId: string }).subjectId === "first"
      ? new Promise((resolve) => {
          finish = resolve;
        })
      : { ...saved, subject_id: "second", body: "Second subject text" },
  );
  const user = userEvent.setup();
  mount(<DraftRecovery accounts={[account]} />);
  await user.click(
    await screen.findByRole("button", { name: "Open draft for first" }),
  );
  await waitFor(() => expect(finish).toBeDefined());
  await user.click(
    screen.getByRole("button", { name: "Open draft for second" }),
  );
  expect(await screen.findByLabelText("Private draft")).toHaveValue(
    "Second subject text",
  );
  await act(async () => {
    finish(saved);
  });
  expect(screen.getByLabelText("Private draft")).toHaveValue(
    "Second subject text",
  );
});

it("keeps authored edits intact when the same actor's provider grant changes", async () => {
  readMocks();
  const user = userEvent.setup();
  const { rerender, cache } = mount(
    <DraftRecovery
      accounts={[{ ...account, state: "active", authorization_epoch: "1" }]}
    />,
  );
  await user.click(
    await screen.findByRole("button", {
      name: "Open draft for missing-subject",
    }),
  );
  const text = await screen.findByLabelText("Private draft");
  await user.type(text, " unsaved");
  rerender(
    <QueryClientProvider client={cache}>
      <DraftRecovery accounts={[account]} />
    </QueryClientProvider>,
  );
  expect(text).toHaveValue(`${saved.body} unsaved`);
  expect(screen.getByText(/Disconnected account/)).toBeVisible();
  expect(collaborationKeys.draft(account, saved.subject_id)).toEqual(
    collaborationKeys.draft(
      { ...account, authorization_epoch: "1" },
      saved.subject_id,
    ),
  );
});

it("keeps the selected actor and authored edits when account ordering changes", async () => {
  readMocks();
  const user = userEvent.setup();
  const { rerender, cache } = mount(
    <DraftRecovery accounts={[account, other]} />,
  );
  await user.click(
    await screen.findByRole("button", {
      name: "Open draft for missing-subject",
    }),
  );
  const text = await screen.findByLabelText("Private draft");
  await user.type(text, " unsaved");
  rerender(
    <QueryClientProvider client={cache}>
      <DraftRecovery accounts={[other, account]} />
    </QueryClientProvider>,
  );
  expect(text).toHaveValue(`${saved.body} unsaved`);
  expect(
    screen.getByRole("combobox", { name: "Draft account" }),
  ).toHaveTextContent(`@${account.login}`);
});

it("returns pagination to the beginning on a collaboration reset without remounting the editor", async () => {
  readMocks();
  const drafts = mockTauriCommand("collaboration_drafts", (payload) =>
    (payload as { query: { cursor: string | null } }).query.cursor === null
      ? {
          drafts: [{ subject_id: "first", preview: "first", generation: "1" }],
          next_cursor: "next",
        }
      : {
          drafts: [
            { subject_id: "second", preview: "second", generation: "1" },
          ],
          next_cursor: null,
        },
  );
  mockTauriCommand("collaboration_draft", (payload) => ({
    ...saved,
    subject_id: (payload as { subjectId: string }).subjectId,
  }));
  mockTauriCommandResult("collaboration_connect_github", {
    ...account,
    state: "active",
    authorization_epoch: "3",
  });
  const user = userEvent.setup();
  mount(<DraftRecovery accounts={[account]} />);
  await user.click(await screen.findByRole("button", { name: "Next drafts" }));
  await user.click(
    await screen.findByRole("button", { name: "Open draft for second" }),
  );
  const text = await screen.findByLabelText("Private draft");
  await user.type(text, " unsaved");
  await act(async () => {
    await collaboration.connectGithub("fixture-only");
  });
  expect(
    await screen.findByRole("button", { name: "Open draft for first" }),
  ).toBeVisible();
  expect(
    screen.getByRole("button", { name: "Previous drafts" }),
  ).toBeDisabled();
  expect(text).toHaveValue(`${saved.body} unsaved`);
  expect(drafts).toHaveBeenCalledWith({
    query: { account_id: account.id, cursor: "next", limit: 50 },
  });
});

it("keeps user text after a failed save and failed catch-up read", async () => {
  const { draft } = readMocks();
  const user = userEvent.setup();
  mockTauriCommand("collaboration_save_draft", () => {
    throw { code: "stale_view" };
  });
  mount(<DraftRecovery accounts={[account]} />);
  await user.click(
    await screen.findByRole("button", {
      name: "Open draft for missing-subject",
    }),
  );
  const text = await screen.findByLabelText("Private draft");
  await user.type(text, " keep this");
  draft.mockRejectedValue({ code: "storage" });
  await user.click(screen.getByRole("button", { name: "Save draft" }));
  await waitFor(() => expect(screen.getAllByRole("alert")).toHaveLength(2));
  expect(text).toHaveValue(`${saved.body} keep this`);
});

it("recovers a dedicated comment draft for a missing subject without using the Private note", async () => {
  onlineManager.setOnline(false);
  readMocks();
  const commentBody = "Dedicated provider comment 📨";
  const commentDrafts = mockTauriCommandResult("collaboration_comment_drafts", {
    account_id: account.id,
    drafts: [
      {
        subject_id: saved.subject_id,
        preview: commentBody,
        generation: "3",
      },
    ],
    next_cursor: null,
    revision: "10",
    authorization_view: "2",
  });
  const commentDraft = mockTauriCommandResult("collaboration_comment_draft", {
    account_id: account.id,
    subject_id: saved.subject_id,
    body: commentBody,
    generation: "3",
    context: null,
    availability: "unavailable",
    reason: "missing_target",
    submission: null,
    revision: "10",
    authorization_view: "2",
  });
  const saveComment = mockTauriCommand(
    "collaboration_save_comment_draft",
    (payload) => {
      const request = (
        payload as {
          request: {
            account_id: string;
            subject_id: string;
            body: string;
          };
        }
      ).request;
      return {
        account_id: request.account_id,
        subject_id: request.subject_id,
        body: request.body,
        generation: "4",
        context: null,
        availability: "unavailable",
        reason: "missing_target",
        submission: null,
        revision: "11",
        authorization_view: "2",
      };
    },
  );
  const user = userEvent.setup();
  mount(<DraftRecovery accounts={[account]} />);
  await user.click(screen.getByRole("button", { name: "Comment drafts" }));
  await user.click(
    await screen.findByRole("button", {
      name: `Open comment draft for ${saved.subject_id}`,
    }),
  );
  const comment = await screen.findByRole("textbox", { name: "Comment" });
  expect(comment).toHaveValue(commentBody);
  expect(screen.queryByDisplayValue(saved.body)).not.toBeInTheDocument();
  expect(
    await screen.findByText(/provider target is unavailable/i),
  ).toBeVisible();
  await user.type(comment, " edited offline");
  await user.click(screen.getByRole("button", { name: "Save comment draft" }));
  await waitFor(() => expect(saveComment).toHaveBeenCalledTimes(1));
  expect(commentDrafts).toHaveBeenCalledWith({
    query: { account_id: account.id, cursor: null, limit: 50 },
  });
  expect(commentDraft).toHaveBeenCalledWith({
    accountId: account.id,
    subjectId: saved.subject_id,
  });
});

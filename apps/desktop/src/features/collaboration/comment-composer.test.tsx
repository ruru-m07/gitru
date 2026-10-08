import {
  type CommentDraftSnapshot,
  collaboration,
  collaborationKeys,
  type RemoteAccount,
} from "@gitru/collaboration-client";
import { commentDraftQueryOptions } from "@gitru/collaboration-client/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { mockTauriCommand } from "../../../tests/mocks/tauri";
import { CommentComposer } from "./comment-composer";

const account: RemoteAccount = {
  id: "github-comment-account",
  actor_id: "actor-7",
  provider: "github",
  host: "https://github.com",
  authorization_epoch: "7",
  login: "fixture",
  state: "active",
  display_name: null,
  notifications_supported: true,
};
const subjectId = "github:pull:67";
const available = (overrides: Partial<CommentDraftSnapshot> = {}) => ({
  account_id: account.id,
  subject_id: subjectId,
  body: "Saved comment",
  generation: "4",
  context: {
    account_id: account.id,
    subject_id: subjectId,
    authorization_epoch: account.authorization_epoch,
    authorization_view: "11",
    review_token: "a".repeat(64),
  },
  availability: "available" as const,
  reason: null,
  submission: null,
  revision: "20",
  authorization_view: "11",
  ...overrides,
});
const caches: QueryClient[] = [];
const stops: Array<() => void> = [];
let snapshot: CommentDraftSnapshot;

beforeEach(() => {
  snapshot = available();
  mockTauriCommand("collaboration_comment_draft", () => snapshot);
  mockTauriCommand("collaboration_created_comments", (payload) => {
    const query = (
      payload as { query: { account_id: string; subject_id: string } }
    ).query;
    return {
      account_id: query.account_id,
      subject_id: query.subject_id,
      comments: [],
      next_cursor: null,
      revision: "20",
      authorization_view: "11",
    };
  });
});

afterEach(() => {
  for (const stop of stops.splice(0)) stop();
  vi.restoreAllMocks();
  for (const cache of caches.splice(0)) cache.clear();
});

function setup() {
  const cache = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  caches.push(cache);
  render(
    <QueryClientProvider client={cache}>
      <CommentComposer account={account} subjectId={subjectId} />
    </QueryClientProvider>,
  );
  return { cache, user: userEvent.setup() };
}

describe("dedicated conversation comment composer", () => {
  it("keeps Private notes isolated and retries the exact saved generation and UUID", async () => {
    snapshot = available({
      body: "",
      generation: "0",
      context: null,
      availability: "unavailable",
      reason: "empty_draft",
    });
    const privateNote = mockTauriCommand("collaboration_draft", () => ({
      account_id: account.id,
      subject_id: subjectId,
      body: "Private note that must never be sent",
      generation: "9",
    }));
    const save = mockTauriCommand(
      "collaboration_save_comment_draft",
      (payload) => {
        const request = (
          payload as {
            request: {
              subject_id: string;
              body: string;
              expected_generation: string;
            };
          }
        ).request;
        expect(request).toMatchObject({
          subject_id: subjectId,
          body: "Explicit provider comment",
          expected_generation: "0",
        });
        snapshot = available({ body: request.body, generation: "1" });
        return snapshot;
      },
    );
    let attempts = 0;
    const send = mockTauriCommand("collaboration_send_comment", (payload) => {
      attempts += 1;
      if (attempts === 1) throw { code: "network" };
      const request = (payload as { request: { command_id: string } }).request;
      return {
        account_id: account.id,
        command_id: request.command_id,
        admitted_revision: "21",
        duplicate: false,
      };
    });
    const { cache, user } = setup();
    const editor = await screen.findByRole("textbox", { name: "Comment" });
    expect(editor).toHaveValue("");
    expect(privateNote).not.toHaveBeenCalled();
    await user.type(editor, "Explicit provider comment");
    await user.click(
      screen.getByRole("button", { name: "Save comment draft" }),
    );
    await waitFor(() => expect(save).toHaveBeenCalledTimes(1));
    await user.click(
      screen.getByRole("checkbox", { name: /delivered in the background/i }),
    );
    await user.click(
      screen.getByRole("button", { name: "Queue saved comment" }),
    );
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Your comment text and request identity are preserved",
    );
    const key = commentDraftQueryOptions(account, subjectId).queryKey;
    await act(async () => {
      await cache.cancelQueries({ queryKey: key });
      cache.setQueryData(key, {
        ...snapshot,
        context: null,
        availability: "unavailable",
        reason: "account_unavailable",
      });
    });
    await waitFor(() =>
      expect(
        screen.queryByRole("button", { name: "Retry exact comment" }),
      ).not.toBeInTheDocument(),
    );
    await act(async () => {
      cache.setQueryData(key, snapshot);
    });
    await user.click(
      await screen.findByRole("button", { name: "Retry exact comment" }),
    );
    await waitFor(() => expect(send).toHaveBeenCalledTimes(2));
    expect(send.mock.calls[1]?.[0]).toEqual(send.mock.calls[0]?.[0]);
    const request = (
      send.mock.calls[0]?.[0] as {
        request: {
          draft_generation: string;
          accept_background_delivery: boolean;
        };
      }
    ).request;
    expect(request).toMatchObject({
      draft_generation: "1",
      accept_background_delivery: true,
    });
    expect(editor).toHaveValue("Explicit provider comment");
    expect(privateNote).not.toHaveBeenCalled();
  });

  it("preserves typed comment text across a changed generation until explicit reload", async () => {
    mockTauriCommand("collaboration_save_comment_draft", () => {
      throw new Error("must not save stale context");
    });
    const { cache, user } = setup();
    const editor = await screen.findByRole("textbox", { name: "Comment" });
    await user.type(editor, " plus local text");
    snapshot = available({
      body: "Changed in another window",
      generation: "5",
      authorization_view: "12",
      context: {
        ...available().context!,
        authorization_view: "12",
        review_token: "b".repeat(64),
      },
      revision: "21",
    });
    await act(async () => {
      cache.setQueryData(
        commentDraftQueryOptions(account, subjectId).queryKey,
        snapshot,
      );
    });
    expect(editor).toHaveValue("Saved comment plus local text");
    expect(
      await screen.findByText(/draft or account context changed/i),
    ).toBeVisible();
    expect(
      screen.getByRole("button", { name: "Save comment draft" }),
    ).toBeDisabled();
    await user.click(
      screen.getByRole("button", { name: "Load latest comment draft" }),
    );
    expect(editor).toHaveValue("Changed in another window");
    await user.click(screen.getByText("Your previous comment text"));
    expect(screen.getByText("Saved comment plus local text")).toBeVisible();
  });

  it("shows local save failures while preserving unsaved comment text", async () => {
    mockTauriCommand("collaboration_save_comment_draft", () => {
      throw { code: "conflict" };
    });
    const { user } = setup();
    const editor = await screen.findByRole("textbox", { name: "Comment" });
    await user.type(editor, " remains local");
    await user.click(
      screen.getByRole("button", { name: "Save comment draft" }),
    );
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Your comment text is preserved",
    );
    expect(editor).toHaveValue("Saved comment remains local");
  });

  it("retains typed text but blocks fresh save, send and stale reload after a cached read fails", async () => {
    const { cache, user } = setup();
    const editor = await screen.findByRole("textbox", { name: "Comment" });
    await user.type(editor, " unfinished");
    snapshot = available({
      body: "New saved comment",
      generation: "5",
      authorization_view: "12",
      context: {
        ...available().context!,
        authorization_view: "12",
        review_token: "c".repeat(64),
      },
      revision: "21",
    });
    const key = commentDraftQueryOptions(account, subjectId).queryKey;
    await act(async () => {
      cache.setQueryData(key, snapshot);
    });
    mockTauriCommand("collaboration_comment_draft", () => {
      throw { code: "unavailable" };
    });
    await act(async () => {
      await cache.refetchQueries({ queryKey: key });
    });
    expect(editor).toHaveValue("Saved comment unfinished");
    await waitFor(() => {
      expect(
        screen.getByRole("button", { name: "Save comment draft" }),
      ).toBeDisabled();
      expect(
        screen.getByRole("button", { name: "Queue saved comment" }),
      ).toBeDisabled();
    });
    expect(
      screen.queryByRole("button", { name: "Load latest comment draft" }),
    ).not.toBeInTheDocument();
    expect(screen.getAllByRole("alert")).not.toHaveLength(0);
  });

  it("keeps an open comment but immediately removes send authority when disconnect refetch fails", async () => {
    let reads = 0;
    const read = mockTauriCommand("collaboration_comment_draft", () => {
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
    const editor = await screen.findByRole("textbox", { name: "Comment" });
    await user.type(editor, " remains local");

    let disconnecting!: Promise<void>;
    act(() => {
      disconnecting = collaboration.disconnect(account.id);
    });
    await waitFor(() => expect(read).toHaveBeenCalledTimes(2));
    expect(editor).toHaveValue("Saved comment remains local");
    expect(screen.getByText(/reconnect this account/i)).toBeVisible();
    expect(
      screen.getByRole("button", { name: "Save comment draft" }),
    ).toBeDisabled();
    expect(
      screen.getByRole("button", { name: "Queue saved comment" }),
    ).toBeDisabled();
    expect(screen.getAllByRole("alert")).not.toHaveLength(0);
    expect(
      cache.getQueryData(commentDraftQueryOptions(account, subjectId).queryKey),
    ).toMatchObject({
      body: "Saved comment",
      generation: "4",
      context: null,
      availability: "unavailable",
      reason: "account_unavailable",
    });

    finishDisconnect("21");
    await act(async () => disconnecting);
  });

  it("labels confirmed local receipts as a bounded Gitru history", async () => {
    mockTauriCommand("collaboration_created_comments", (payload) => {
      const query = (
        payload as { query: { account_id: string; subject_id: string } }
      ).query;
      return {
        account_id: query.account_id,
        subject_id: query.subject_id,
        comments: [
          {
            command_id: "00000000-0000-4000-8000-000000000001",
            draft_generation: "3",
            provider_id: "9007199254740993",
            url: "https://github.com/example/repo/issues/67#issuecomment-9007199254740993",
            body: "Confirmed provider comment",
            author: "fixture",
            created_at: "2026-10-08T00:00:00Z",
            observed_at: "2026-10-08T00:00:01Z",
          },
        ],
        next_cursor: null,
        revision: "21",
        authorization_view: "11",
      };
    });
    const { user } = setup();
    await screen.findByRole("textbox", { name: "Comment" });
    await user.click(screen.getByText("Submitted from Gitru"));
    expect(await screen.findByText("Confirmed provider comment")).toBeVisible();
    expect(
      screen.getByText(/not the full provider conversation/i),
    ).toBeVisible();
    expect(
      collaborationKeys.createdComments(account, {
        account_id: account.id,
        subject_id: subjectId,
        cursor: null,
        limit: 25,
      }),
    ).toContain("created-comments");
  });
});

it("a definite admission refusal retains text and allows editing without an exact retry", async () => {
  const send = mockTauriCommand("collaboration_send_comment", () => {
    throw { code: "invalid_input", message: "provider secret must not render" };
  });
  const save = mockTauriCommand(
    "collaboration_save_comment_draft",
    (payload) => {
      const request = (payload as { request: { body: string } }).request;
      snapshot = available({ body: request.body, generation: "5" });
      return snapshot;
    },
  );
  const { user } = setup();
  const editor = await screen.findByRole("textbox", { name: "Comment" });
  const consent = screen.getByRole("checkbox", {
    name: /delivered in the background/i,
  });
  await user.click(consent);
  await user.click(screen.getByRole("button", { name: "Queue saved comment" }));
  expect(await screen.findByRole("alert")).toHaveTextContent(
    "try a shorter body",
  );
  expect(screen.getByRole("alert")).not.toHaveTextContent("provider secret");
  expect(
    screen.queryByRole("button", { name: "Retry exact comment" }),
  ).not.toBeInTheDocument();
  expect(consent).not.toBeChecked();
  expect(editor).toHaveValue("Saved comment");
  await user.clear(editor);
  await user.type(editor, "Short comment");
  await user.click(screen.getByRole("button", { name: "Save comment draft" }));
  await waitFor(() => expect(save).toHaveBeenCalledTimes(1));
  expect(send).toHaveBeenCalledTimes(1);
  expect(
    screen.getByRole("button", { name: "Queue saved comment" }),
  ).toBeDisabled();
});

import type {
  RemoteAccount,
  TextEditSnapshot,
} from "@gitru/collaboration-client";
import { textEditQueryOptions } from "@gitru/collaboration-client/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { mockTauriCommand } from "../../../tests/mocks/tauri";
import { ResourceTextEditor } from "./text-edit";

const account: RemoteAccount = {
  id: "account-text-edit",
  actor_id: "actor-a",
  provider: "github",
  host: "https://github.com",
  authorization_epoch: "7",
  login: "fixture",
  state: "active",
  display_name: null,
  notifications_supported: true,
};
const subjectId = "pull-42";
const available = (token = "a".repeat(64)): TextEditSnapshot => ({
  context: {
    account_id: account.id,
    subject_id: subjectId,
    authorization_epoch: account.authorization_epoch,
    authorization_view: "11",
    review_token: token,
  },
  title: "Cached title",
  body: "Cached description",
  availability: "available",
  reason: null,
  pending_intent: null,
  revision: "20",
  authorization_view: "11",
});

let snapshot: TextEditSnapshot;
const caches: QueryClient[] = [];

beforeEach(() => {
  snapshot = available();
  mockTauriCommand("collaboration_text_edit_snapshot", () => snapshot);
});

afterEach(() => {
  for (const cache of caches.splice(0)) cache.clear();
});

function setup() {
  const cache = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  caches.push(cache);
  render(
    <QueryClientProvider client={cache}>
      <ResourceTextEditor account={account} subjectId={subjectId} />
    </QueryClientProvider>,
  );
  return { cache, user: userEvent.setup() };
}

async function openEditor(user: ReturnType<typeof userEvent.setup>) {
  await user.click(await screen.findByRole("button", { name: "Edit" }));
  return {
    title: screen.getByRole("textbox", { name: "Title" }),
    body: screen.getByRole("textbox", { name: "Description" }),
    consent: screen.getByRole("checkbox", {
      name: /best-effort update/i,
    }),
  };
}

describe("provider title and description editing", () => {
  it("submits only changed fields and keeps an empty description as an explicit clear", async () => {
    const submit = mockTauriCommand(
      "collaboration_submit_text_edit",
      (payload) => {
        const request = (payload as { request: { command_id: string } })
          .request;
        return {
          account_id: account.id,
          command_id: request.command_id,
          admitted_revision: "21",
          duplicate: false,
        };
      },
    );
    const { user } = setup();
    const fields = await openEditor(user);
    expect(fields.consent).toHaveAccessibleName(
      /a simultaneous edit may overwrite my change, or my change may overwrite theirs/i,
    );
    await user.clear(fields.body);
    await user.click(fields.consent);
    await user.click(screen.getByRole("button", { name: "Save and queue" }));

    await waitFor(() => expect(submit).toHaveBeenCalledTimes(1));
    const request = (
      submit.mock.calls[0]?.[0] as {
        request: {
          command_id: string;
          accept_best_effort: boolean;
          title: string | null;
          body: string | null;
        };
      }
    ).request;
    expect(request).toMatchObject({
      accept_best_effort: true,
      title: null,
      body: "",
    });
    expect(request.command_id).toMatch(
      /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i,
    );
    expect(
      await screen.findByText("Changes saved locally and queued for delivery."),
    ).toBeVisible();
  });

  it("preserves entered text and the exact request identity across an IPC retry", async () => {
    const submit = mockTauriCommand("collaboration_submit_text_edit", () => {
      throw { code: "network" };
    });
    const { user } = setup();
    const fields = await openEditor(user);
    await user.clear(fields.title);
    await user.type(fields.title, "Preserved title");
    await user.click(fields.consent);
    await user.click(screen.getByRole("button", { name: "Save and queue" }));

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Your text and request identity are preserved.",
    );
    expect(fields.title).toHaveValue("Preserved title");
    await user.click(
      screen.getByRole("button", { name: "Retry exact change" }),
    );
    await waitFor(() => expect(submit).toHaveBeenCalledTimes(2));
    expect(submit.mock.calls[1]?.[0]).toEqual(submit.mock.calls[0]?.[0]);
  });

  it("keeps typed text when native context changes until the user explicitly reloads", async () => {
    mockTauriCommand("collaboration_submit_text_edit", () => {
      throw new Error("must not submit stale context");
    });
    const { cache, user } = setup();
    const fields = await openEditor(user);
    await user.clear(fields.title);
    await user.type(fields.title, "Local unfinished title");

    snapshot = {
      ...available("b".repeat(64)),
      title: "New saved title",
      body: "New saved description",
      revision: "22",
    };
    await act(async () => {
      cache.setQueryData(
        textEditQueryOptions(account, subjectId).queryKey,
        snapshot,
      );
    });

    expect(fields.title).toHaveValue("Local unfinished title");
    expect(
      await screen.findByText(/This item changed after you started editing/),
    ).toBeVisible();
    expect(
      screen.getByRole("button", { name: "Save and queue" }),
    ).toBeDisabled();
    await user.click(
      screen.getByRole("button", { name: "Load latest saved version" }),
    );
    expect(fields.title).toHaveValue("New saved title");
    await user.click(screen.getByText("Your previous edit"));
    expect(screen.getByText("Local unfinished title")).toBeVisible();
  });

  it("reports bounded local validation and explicit pending intent", async () => {
    const { cache, user } = setup();
    const fields = await openEditor(user);
    await user.clear(fields.title);
    await user.type(fields.title, "a".repeat(257));
    expect(
      screen.getByText("Keep the title to 256 characters or fewer."),
    ).toBeVisible();
    expect(
      screen.getByRole("button", { name: "Save and queue" }),
    ).toBeDisabled();

    await user.click(screen.getByRole("button", { name: "Cancel" }));
    await act(async () => {
      cache.setQueryData(textEditQueryOptions(account, subjectId).queryKey, {
        ...snapshot,
        context: null,
        title: null,
        body: null,
        availability: "unavailable",
        reason: "pending_intent",
        pending_intent: {
          subject_id: subjectId,
          commands: [
            {
              command_id: "queued-command",
              state: "queued",
              fields: ["title"],
            },
          ],
        },
      } satisfies TextEditSnapshot);
    });
    expect(
      await screen.findByText(
        "A title or description change is already queued.",
      ),
    ).toBeVisible();
    expect(
      screen.queryByRole("button", { name: "Edit" }),
    ).not.toBeInTheDocument();
  });
});

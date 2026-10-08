import type {
  LabelSetSnapshot,
  RemoteAccount,
} from "@gitru/collaboration-client";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it } from "vitest";
import { mockTauriCommand } from "../../../tests/mocks/tauri";
import { ResourceLabelSet } from "./label-set";

const account: RemoteAccount = {
  id: "account-labels",
  actor_id: "actor-a",
  provider: "github",
  host: "github.com",
  authorization_epoch: "7",
  login: "fixture",
  state: "active",
  display_name: null,
  notifications_supported: true,
};
const subjectId = "pull-42";
const bug = { provider_id: "10", name: "bug", color: "d73a4a" };
const docs = { provider_id: "11", name: "docs/#?", color: null };
const help = { provider_id: "12", name: "help wanted", color: "008672" };

function available(token = "a".repeat(64)): LabelSetSnapshot {
  return {
    context: {
      account_id: account.id,
      subject_id: subjectId,
      authorization_epoch: account.authorization_epoch,
      authorization_view: "11",
      review_token: token,
    },
    canonical_labels: [bug],
    effective_labels: [bug],
    available_labels: [bug, docs, help],
    catalog_complete: false,
    catalog_truncated: false,
    availability: "available",
    reason: null,
    pending_intent: null,
    revision: "20",
    authorization_view: "11",
  };
}

const caches: QueryClient[] = [];

afterEach(() => {
  for (const cache of caches.splice(0)) cache.clear();
});

function setup(
  snapshot: LabelSetSnapshot | undefined = available(),
  queryError: unknown | null = null,
) {
  const cache = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  caches.push(cache);
  const view = render(
    <QueryClientProvider client={cache}>
      <ResourceLabelSet
        account={account}
        subjectId={subjectId}
        snapshot={snapshot}
        pending={false}
        queryError={queryError}
      />
    </QueryClientProvider>,
  );
  return {
    ...view,
    user: userEvent.setup(),
    update(next: LabelSetSnapshot | undefined, error: unknown | null = null) {
      view.rerender(
        <QueryClientProvider client={cache}>
          <ResourceLabelSet
            account={account}
            subjectId={subjectId}
            snapshot={next}
            pending={false}
            queryError={error}
          />
        </QueryClientProvider>,
      );
    },
  };
}

async function begin(user: ReturnType<typeof userEvent.setup>) {
  await user.click(screen.getByRole("button", { name: "Edit labels" }));
}

describe("cached GitHub label editor", () => {
  it("labels the saved catalog as incomplete and admits the exact typed delta only after consent", async () => {
    const submit = mockTauriCommand(
      "collaboration_submit_label_set",
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
    expect(screen.getByText(/observed, incomplete catalog/i)).toBeVisible();
    await begin(user);
    await user.click(screen.getByRole("checkbox", { name: "bug" }));
    await user.click(screen.getByRole("checkbox", { name: "docs/#?" }));
    expect(screen.getByText("1 to add · 1 to remove")).toBeVisible();
    const queue = screen.getByRole("button", {
      name: "Save and queue label changes",
    });
    expect(queue).toBeDisabled();
    const consent = screen.getByRole("checkbox", {
      name: /without a compare-and-swap token/i,
    });
    await user.click(consent);
    await user.click(queue);

    await waitFor(() => expect(submit).toHaveBeenCalledTimes(1));
    const request = (
      submit.mock.calls[0]?.[0] as {
        request: {
          context: unknown;
          command_id: string;
          add_labels: unknown[];
          remove_labels: unknown[];
          accept_best_effort: boolean;
        };
      }
    ).request;
    expect(request).toMatchObject({
      context: available().context,
      add_labels: [docs],
      remove_labels: [bug],
      accept_best_effort: true,
    });
    expect(request.command_id).toMatch(
      /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i,
    );
    expect(
      await screen.findByText(
        "Label change saved locally and queued for delivery.",
      ),
    ).toBeVisible();
  });

  it("preserves the exact request identity after a lost local receipt", async () => {
    let attempts = 0;
    const submit = mockTauriCommand(
      "collaboration_submit_label_set",
      (payload) => {
        attempts += 1;
        if (attempts === 1) throw { code: "network" };
        const request = (payload as { request: { command_id: string } })
          .request;
        return {
          account_id: account.id,
          command_id: request.command_id,
          admitted_revision: "21",
          duplicate: true,
        };
      },
    );
    const { user } = setup();
    await begin(user);
    await user.click(screen.getByRole("checkbox", { name: "docs/#?" }));
    await user.click(
      screen.getByRole("checkbox", {
        name: /without a compare-and-swap token/i,
      }),
    );
    await user.click(
      screen.getByRole("button", { name: "Save and queue label changes" }),
    );
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Your exact request identity is preserved.",
    );
    await user.click(
      screen.getByRole("button", { name: "Retry exact label change" }),
    );
    await waitFor(() => expect(submit).toHaveBeenCalledTimes(2));
    expect(submit.mock.calls[1]?.[0]).toEqual(submit.mock.calls[0]?.[0]);
    expect(
      await screen.findByText("This exact label change was already queued."),
    ).toBeVisible();
  });

  it("keeps the selection while fencing changed or failed saved authority", async () => {
    const submit = mockTauriCommand("collaboration_submit_label_set", () => {
      throw new Error("must not submit stale context");
    });
    const view = setup();
    await begin(view.user);
    await view.user.click(screen.getByRole("checkbox", { name: "docs/#?" }));
    view.update(available("b".repeat(64)));
    expect(
      await screen.findByText(/Labels changed after you started/),
    ).toBeVisible();
    expect(screen.getByRole("checkbox", { name: "docs/#?" })).toBeChecked();
    expect(
      screen.getByRole("button", { name: "Save and queue label changes" }),
    ).toBeDisabled();

    view.update(available("b".repeat(64)), { code: "storage" });
    expect(screen.getByRole("alert")).toHaveTextContent(
      "New label changes are disabled until the saved context reloads.",
    );
    expect(screen.getByRole("checkbox", { name: "docs/#?" })).toBeChecked();
    expect(submit).not.toHaveBeenCalled();
  });

  it("shows queued and unavailable evidence without offering another mutation", () => {
    const pending: LabelSetSnapshot = {
      ...available(),
      context: null,
      canonical_labels: [bug],
      effective_labels: [bug, docs],
      availability: "unavailable",
      reason: "pending_intent",
      pending_intent: {
        subject_id: subjectId,
        commands: [
          { command_id: "queued", state: "queued", fields: ["labels"] },
        ],
      },
    };
    setup(pending);
    expect(screen.getByText("Queued")).toBeVisible();
    expect(
      screen.getByText("A label change is already tracked in Saved changes."),
    ).toBeVisible();
    expect(
      screen.queryByRole("button", { name: "Edit labels" }),
    ).not.toBeInTheDocument();
  });
});

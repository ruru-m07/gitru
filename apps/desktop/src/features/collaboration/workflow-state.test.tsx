import type {
  RemoteAccount,
  WorkflowStateSnapshot,
} from "@gitru/collaboration-client";
import { workflowStateQueryOptions } from "@gitru/collaboration-client/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { mockTauriCommand } from "../../../tests/mocks/tauri";
import { ResourceWorkflowState } from "./workflow-state";

const account: RemoteAccount = {
  id: "account-workflow",
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
const available = (token = "a".repeat(64)): WorkflowStateSnapshot => ({
  context: {
    account_id: account.id,
    subject_id: subjectId,
    authorization_epoch: account.authorization_epoch,
    authorization_view: "11",
    review_token: token,
  },
  current_state: "open",
  availability: "available",
  reason: null,
  pending_intent: null,
  revision: "20",
  authorization_view: "11",
});

let snapshot: WorkflowStateSnapshot;
const caches: QueryClient[] = [];

beforeEach(() => {
  snapshot = available();
  mockTauriCommand("collaboration_workflow_state_snapshot", () => snapshot);
});

afterEach(() => {
  for (const cache of caches.splice(0)) cache.clear();
});

function setup({ sibling = false } = {}) {
  const cache = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  caches.push(cache);
  render(
    <QueryClientProvider client={cache}>
      <ResourceWorkflowState
        account={account}
        subjectId={subjectId}
        kind="pull_request"
      />
      {sibling ? (
        <label>
          Private note fixture
          <textarea defaultValue="Saved private note" />
        </label>
      ) : null}
    </QueryClientProvider>,
  );
  return { cache, user: userEvent.setup() };
}

async function reviewClose(user: ReturnType<typeof userEvent.setup>) {
  await user.click(
    await screen.findByRole("button", { name: "Close pull request" }),
  );
  return screen.getByRole("checkbox", { name: /best-effort update/i });
}

describe("provider workflow state", () => {
  it("requires explicit race consent and admits only the exact close intent", async () => {
    const submit = mockTauriCommand(
      "collaboration_submit_workflow_state",
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
    const consent = await reviewClose(user);
    expect(consent).toHaveAccessibleName(
      /A provider change after Gitru checks the item may win, or this change may overwrite it/i,
    );
    expect(
      screen.getByRole("button", { name: "Save and queue close" }),
    ).toBeDisabled();
    await user.click(consent);
    await user.click(
      screen.getByRole("button", { name: "Save and queue close" }),
    );

    await waitFor(() => expect(submit).toHaveBeenCalledTimes(1));
    const request = (
      submit.mock.calls[0]?.[0] as {
        request: {
          command_id: string;
          desired_state: string;
          accept_best_effort: boolean;
          context: unknown;
        };
      }
    ).request;
    expect(request).toMatchObject({
      desired_state: "closed",
      accept_best_effort: true,
      context: available().context,
    });
    expect(request.command_id).toMatch(
      /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i,
    );
    expect(
      await screen.findByText(
        "Status change saved locally and queued for delivery.",
      ),
    ).toBeVisible();
  });

  it("preserves the exact request identity across a lost IPC receipt", async () => {
    const submit = mockTauriCommand(
      "collaboration_submit_workflow_state",
      (payload) => {
        const request = (payload as { request: { command_id: string } })
          .request;
        snapshot = {
          ...available(),
          context: null,
          current_state: null,
          availability: "unavailable",
          reason: "pending_intent",
          pending_intent: {
            subject_id: subjectId,
            commands: [
              {
                command_id: request.command_id,
                state: "queued",
                fields: ["state"],
              },
            ],
          },
          revision: "21",
        };
        throw { code: "network" };
      },
    );
    const { user } = setup();
    await user.click(await reviewClose(user));
    await user.click(
      screen.getByRole("button", { name: "Save and queue close" }),
    );
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Your exact request identity is preserved.",
    );
    expect(
      await screen.findByText(
        "A status change is already tracked in Saved changes.",
      ),
    ).toBeVisible();
    await user.click(
      screen.getByRole("button", { name: "Retry exact status change" }),
    );
    await waitFor(() => expect(submit).toHaveBeenCalledTimes(2));
    expect(submit.mock.calls[1]?.[0]).toEqual(submit.mock.calls[0]?.[0]);
  });

  it("fences changed authority while preserving adjacent authored text", async () => {
    const submit = mockTauriCommand(
      "collaboration_submit_workflow_state",
      () => {
        throw new Error("must not submit stale context");
      },
    );
    const { cache, user } = setup({ sibling: true });
    await reviewClose(user);
    const note = screen.getByRole("textbox", { name: "Private note fixture" });
    await user.type(note, " plus unsaved text");

    snapshot = {
      ...available("b".repeat(64)),
      current_state: "closed",
      revision: "22",
    };
    await act(async () => {
      cache.setQueryData(
        workflowStateQueryOptions(account, subjectId).queryKey,
        snapshot,
      );
    });

    expect(note).toHaveValue("Saved private note plus unsaved text");
    expect(
      await screen.findByText(/This item changed after you started/),
    ).toBeVisible();
    expect(
      screen.getByRole("button", { name: "Save and queue close" }),
    ).toBeDisabled();
    expect(submit).not.toHaveBeenCalled();
    await user.click(
      screen.getByRole("button", { name: "Load latest saved status" }),
    );
    expect(
      screen.getByRole("button", { name: "Save and queue reopen" }),
    ).toBeDisabled();
    expect(note).toHaveValue("Saved private note plus unsaved text");
  });

  it("keeps cached status visible but disables a fresh request when its local read fails", async () => {
    const submit = mockTauriCommand(
      "collaboration_submit_workflow_state",
      () => {
        throw new Error("must not submit without current local authority");
      },
    );
    const { cache, user } = setup();
    const consent = await reviewClose(user);
    await user.click(consent);
    mockTauriCommand("collaboration_workflow_state_snapshot", () => {
      throw { code: "storage" };
    });
    await act(async () => {
      await cache.invalidateQueries({
        queryKey: workflowStateQueryOptions(account, subjectId).queryKey,
      });
    });

    expect(await screen.findByText("Open")).toBeVisible();
    expect(screen.getByRole("alert")).toHaveTextContent(
      "New status changes are disabled until the saved context reloads.",
    );
    expect(
      screen.getByRole("button", { name: "Save and queue close" }),
    ).toBeDisabled();
    expect(submit).not.toHaveBeenCalled();
  });

  it("keeps unsupported, merged, unknown and pending states visibly non-actionable", async () => {
    const { cache } = setup();
    await screen.findByRole("button", { name: "Close pull request" });
    const cases = [
      ["merged_pull_request", "Merged pull requests cannot be reopened."],
      [
        "unknown_state",
        "The saved provider status is unknown. Sync this item before changing it.",
      ],
      [
        "pending_intent",
        "A status change is already tracked in Saved changes.",
      ],
      [
        "unsupported_provider",
        "Changing status is not available for this provider yet.",
      ],
    ] as const;
    for (const [reason, message] of cases) {
      await act(async () => {
        cache.setQueryData(
          workflowStateQueryOptions(account, subjectId).queryKey,
          {
            ...snapshot,
            context: null,
            current_state: null,
            availability: "unavailable",
            reason,
            pending_intent:
              reason === "pending_intent"
                ? {
                    subject_id: subjectId,
                    commands: [
                      {
                        command_id: "queued-command",
                        state: "queued",
                        fields: ["state"],
                      },
                    ],
                  }
                : null,
          } satisfies WorkflowStateSnapshot,
        );
      });
      expect(await screen.findByText(message)).toBeVisible();
      expect(
        screen.queryByRole("button", { name: /pull request/ }),
      ).not.toBeInTheDocument();
    }
  });
});

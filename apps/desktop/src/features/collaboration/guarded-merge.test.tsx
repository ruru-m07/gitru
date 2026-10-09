import {
  type GuardedMergePreview,
  type GuardedMergeRequest,
  type GuardedMergeSnapshot,
  type RemoteAccount,
} from "@gitru/collaboration-client";
import { guardedMergeQueryOptions } from "@gitru/collaboration-client/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { mockTauriCommand } from "../../../tests/mocks/tauri";
import { GuardedPullMerge } from "./guarded-merge";

const account: RemoteAccount = {
  id: "merge-ui-a",
  actor_id: "actor-a",
  provider: "github",
  host: "github.com",
  login: "fixture",
  authorization_epoch: "7",
  state: "active",
  display_name: null,
  notifications_supported: false,
};
const head = "a".repeat(40),
  subject = "pull-42";
const context = {
  account_id: account.id,
  subject_id: subject,
  authorization_epoch: "7",
  authorization_view: "11",
  expected_head: head,
  grant_id: "123e4567-e89b-42d3-a456-426614174000",
};
const available: GuardedMergePreview = {
  context,
  expected_head: head,
  methods: ["merge", "squash"],
  can_push: true,
  mergeable: true,
  provider_mergeability: "clean",
  reason: null,
  observed_at: "2026-10-08T03:00:00Z",
  expires_in_seconds: 60,
  authorization_view: "11",
};
let snapshot: GuardedMergeSnapshot;
const caches: QueryClient[] = [];
beforeEach(() => {
  snapshot = {
    reason: null,
    latest: null,
    revision: "20",
    authorization_view: "11",
  };
  mockTauriCommand("collaboration_guarded_merge_snapshot", () => snapshot);
});
afterEach(() => {
  for (const c of caches.splice(0)) c.clear();
});
function setup() {
  const cache = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  caches.push(cache);
  const view = render(
    <QueryClientProvider client={cache}>
      <GuardedPullMerge account={account} subjectId={subject} headOid={head} />
    </QueryClientProvider>,
  );
  return { cache, view, user: userEvent.setup() };
}
async function inspect(user: ReturnType<typeof userEvent.setup>) {
  await user.click(
    await screen.findByRole("button", { name: "Check merge online" }),
  );
  await screen.findByText(head);
}
describe("guarded direct merge", () => {
  it("opens locally and requires online preview, an explicit method and exact-head consent", async () => {
    const preview = mockTauriCommand(
      "collaboration_preview_guarded_merge",
      () => available,
    );
    const sent: GuardedMergeRequest[] = [];
    mockTauriCommand("collaboration_submit_guarded_merge", (args) => {
      const r = (args as { request: GuardedMergeRequest }).request;
      sent.push(r);
      return {
        account_id: account.id,
        command_id: r.command_id,
        admitted_revision: "21",
        duplicate: false,
      };
    });
    const { user } = setup();
    await screen.findByRole("button", { name: "Check merge online" });
    expect(preview).not.toHaveBeenCalled();
    expect(sent).toHaveLength(0);
    await inspect(user);
    const merge = screen.getByRole("button", { name: "Merge inspected head" });
    expect(merge).toBeDisabled();
    await user.click(screen.getByRole("radio", { name: "Squash and merge" }));
    expect(merge).toBeDisabled();
    await user.click(
      screen.getByRole("checkbox", { name: /I inspected this head/ }),
    );
    await user.click(merge);
    await waitFor(() => expect(sent).toHaveLength(1));
    expect(sent[0]).toMatchObject({
      context,
      method: "squash",
      confirm_inspected_head: true,
    });
    expect(
      await screen.findByText(/Awaiting GitHub confirmation/),
    ).toBeVisible();
    expect(
      screen.queryByText("GitHub confirmed the recorded head is merged."),
    ).not.toBeInTheDocument();
  });
  it("preserves the exact command UUID when recovering a lost local receipt", async () => {
    mockTauriCommand("collaboration_preview_guarded_merge", () => available);
    const sent: GuardedMergeRequest[] = [];
    mockTauriCommand("collaboration_submit_guarded_merge", (args) => {
      const r = (args as { request: GuardedMergeRequest }).request;
      sent.push(r);
      if (sent.length === 1)
        throw { code: "network", message: "lost local IPC receipt" };
      return {
        account_id: account.id,
        command_id: r.command_id,
        admitted_revision: "21",
        duplicate: true,
      };
    });
    const { user } = setup();
    await inspect(user);
    await user.click(screen.getByRole("radio", { name: "Merge commit" }));
    await user.click(
      screen.getByRole("checkbox", { name: /I inspected this head/ }),
    );
    await user.click(
      screen.getByRole("button", { name: "Merge inspected head" }),
    );
    await user.click(
      await screen.findByRole("button", {
        name: "Recover exact local receipt",
      }),
    );
    await waitFor(() => expect(sent).toHaveLength(2));
    expect(sent[1]).toEqual(sent[0]);
    expect(await screen.findByText(/already recorded locally/)).toBeVisible();
  });
  it("hides a reviewed grant when the native authorization view changes", async () => {
    mockTauriCommand("collaboration_preview_guarded_merge", () => available);
    const submit = mockTauriCommand(
      "collaboration_submit_guarded_merge",
      () => {
        throw new Error("must not send");
      },
    );
    const { user, cache } = setup();
    await inspect(user);
    await act(async () => {
      cache.setQueryData(guardedMergeQueryOptions(account, subject).queryKey, {
        ...snapshot,
        authorization_view: "12",
        reason: "account_unavailable",
        revision: "21",
      });
    });
    await waitFor(() =>
      expect(screen.queryByRole("checkbox")).not.toBeInTheDocument(),
    );
    expect(screen.queryByText(head)).not.toBeInTheDocument();
    expect(submit).not.toHaveBeenCalled();
  });
  it("shows accepted and unknown as unconfirmed durable states", async () => {
    snapshot.latest = {
      command_id: "cmd",
      state: "accepted",
      method: "squash",
      expected_head: head,
      attempt_count: 1,
      attention: null,
    };
    snapshot.reason = "pending_command";
    const { cache } = setup();
    expect(
      await screen.findByText(
        /accepted the request; merge confirmation is pending/,
      ),
    ).toBeVisible();
    expect(
      screen.getByRole("button", { name: "Check merge online" }),
    ).toBeDisabled();
    const accepted = snapshot.latest;
    await act(async () => {
      cache.setQueryData(guardedMergeQueryOptions(account, subject).queryKey, {
        ...snapshot,
        latest: { ...accepted, state: "outcome_unknown" },
      });
    });
    expect(await screen.findByText(/outcome is uncertain/)).toBeVisible();
  });
  it("does not permit unknown permissions or changed provider heads", async () => {
    mockTauriCommand("collaboration_preview_guarded_merge", () => ({
      ...available,
      context: null,
      can_push: null,
      reason: "permission_unavailable",
      expires_in_seconds: 0,
    }));
    const { user } = setup();
    await user.click(
      await screen.findByRole("button", { name: "Check merge online" }),
    );
    expect(
      await screen.findByText(/push permission could not be verified/),
    ).toBeVisible();
    expect(
      screen.queryByRole("button", { name: "Merge inspected head" }),
    ).not.toBeInTheDocument();
  });
});

import {
  collaboration,
  type PullCheckoutPlan,
} from "@gitru/collaboration-client";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useAppStore } from "@/store/use-app-store";
import {
  fixtureAccount,
  fixtureItem,
} from "../../../tests/fixtures/collaboration";
import { fixtureMetadata } from "../../../tests/fixtures/resource-detail";
import {
  PullCheckoutDialogBody,
  PullRequestCheckoutButton,
} from "./pull-checkout-dialog";

vi.mock("@tauri-apps/plugin-store", () => ({
  Store: {
    load: async () => ({
      get: async () => null,
      set: async () => {},
      save: async () => {},
    }),
  },
}));

const clone = {
  local_repository_id: "registered-a",
  local_repository_name: "Clone A",
  link_id: "link-a",
  generation: "9007199254740993",
  state: "linked" as const,
};
const registration = {
  id: clone.local_repository_id,
  name: "Clone A",
  path: "/worktrees/clone-a",
  origin: "https://github.com/actor/project.git",
  current_branch: "main",
  ahead_behind: [0, 0] as [number, number],
  has_uncommitted_changes: false,
  last_updated: 1,
};
const exactOid = "a".repeat(40);
const basePlan: PullCheckoutPlan = {
  plan_id: "opaque-plan-a",
  local_repository_id: clone.local_repository_id,
  local_repository_name: "Clone A",
  source_repository: "actor/project",
  source_remote: "fork",
  source_branch: "feature",
  expected_oid: exactOid,
  local_branch: "pr/42",
  metadata_validated_at: "2026-10-05T00:00:00Z",
  metadata_stale: false,
  inspection: {
    current_branch: "main",
    current_head_oid: "b".repeat(40),
    detached: false,
    dirty: false,
    operation: "clean",
    object_available: false,
    action: "fetch_and_create_branch",
  },
};

let cache: QueryClient;
function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((finish) => {
    resolve = finish;
  });
  return { promise, resolve };
}
beforeEach(() => {
  cache = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  vi.spyOn(collaboration.transport, "localClones").mockResolvedValue({
    clones: [clone],
  });
  vi.spyOn(collaboration.transport, "planPullCheckout").mockResolvedValue(
    basePlan,
  );
  vi.spyOn(collaboration.transport, "executePullCheckout").mockResolvedValue({
    local_repository_id: clone.local_repository_id,
    branch: basePlan.local_branch,
    oid: basePlan.expected_oid,
    fetched: true,
  });
  useAppStore.setState({ repositories: [registration] });
});
afterEach(() => {
  cache.clear();
  useAppStore.setState({ repositories: [] });
  vi.restoreAllMocks();
});

function mountBody(onNavigate = vi.fn(async () => {})) {
  render(
    <QueryClientProvider client={cache}>
      <PullCheckoutDialogBody
        account={fixtureAccount}
        instanceId="github:https://github.com/"
        itemId={fixtureItem.id}
        repositoryId="fixture-repository"
        onNavigate={onNavigate}
      />
    </QueryClientProvider>,
  );
  return onNavigate;
}

describe("pull request checkout confirmation", () => {
  it("plans on explicit clone selection and fetches only after confirmation", async () => {
    const user = userEvent.setup();
    const navigate = mountBody();
    const plan = vi.mocked(collaboration.transport.planPullCheckout);
    const execute = vi.mocked(collaboration.transport.executePullCheckout);

    expect(await screen.findByText("Clone A")).toBeVisible();
    expect(screen.getByText("Local path: /worktrees/clone-a")).toBeVisible();
    expect(plan).not.toHaveBeenCalled();
    expect(execute).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "Inspect Clone A" }));

    await waitFor(() =>
      expect(plan).toHaveBeenCalledExactlyOnceWith({
        account_id: fixtureAccount.id,
        authorization_epoch: fixtureAccount.authorization_epoch,
        instance_id: "github:https://github.com/",
        subject_id: fixtureItem.id,
        local_repository_id: clone.local_repository_id,
        link_id: clone.link_id,
        link_generation: clone.generation,
      }),
    );
    expect(screen.getByText("actor/project:feature")).toBeVisible();
    expect(screen.getByText(exactOid.slice(0, 12))).toHaveAttribute(
      "title",
      exactOid,
    );
    expect(
      screen.getByText(/Confirmation fetches the saved source branch/),
    ).toBeVisible();
    expect(execute).not.toHaveBeenCalled();

    await user.click(
      screen.getByRole("button", { name: "Fetch and check out" }),
    );
    await waitFor(() =>
      expect(execute).toHaveBeenCalledExactlyOnceWith({
        plan_id: basePlan.plan_id,
      }),
    );
    expect(navigate).toHaveBeenCalledExactlyOnceWith(clone.local_repository_id);
  });

  it("associates distinct registered paths with duplicate clone names", async () => {
    const second = {
      ...clone,
      local_repository_id: "registered-b",
      link_id: "link-b",
      generation: "9007199254740994",
    };
    vi.mocked(collaboration.transport.localClones).mockResolvedValue({
      clones: [clone, second],
    });
    useAppStore.setState({
      repositories: [
        registration,
        {
          ...registration,
          id: second.local_repository_id,
          path: "/worktrees/clone-b",
        },
      ],
    });
    mountBody();

    const choices = await screen.findAllByRole("button", {
      name: "Inspect Clone A",
    });
    expect(choices).toHaveLength(2);
    const firstDescription = choices[0].getAttribute("aria-describedby");
    const secondDescription = choices[1].getAttribute("aria-describedby");
    expect(firstDescription).toBeTruthy();
    expect(secondDescription).toBeTruthy();
    expect(firstDescription).not.toBe(secondDescription);
    expect(document.getElementById(firstDescription ?? "")).toHaveTextContent(
      "Local path: /worktrees/clone-a",
    );
    expect(document.getElementById(secondDescription ?? "")).toHaveTextContent(
      "Local path: /worktrees/clone-b",
    );
  });

  it("retires the rendered token when the branch changes and requires replanning", async () => {
    const user = userEvent.setup();
    mountBody();
    const plan = vi.mocked(collaboration.transport.planPullCheckout);
    const execute = vi.mocked(collaboration.transport.executePullCheckout);
    plan.mockResolvedValueOnce(basePlan).mockResolvedValueOnce({
      ...basePlan,
      plan_id: "opaque-plan-b",
      local_branch: "review/42",
    });
    await user.click(
      await screen.findByRole("button", { name: "Inspect Clone A" }),
    );
    const branch = await screen.findByLabelText("Local branch");
    await user.clear(branch);
    await user.type(branch, "review/42");
    expect(
      screen.queryByRole("button", { name: "Fetch and check out" }),
    ).not.toBeInTheDocument();
    expect(execute).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "Inspect branch" }));
    await waitFor(() => expect(plan).toHaveBeenCalledTimes(2));
    expect(plan.mock.calls[1][0]).toMatchObject({ local_branch: "review/42" });
    await user.click(
      await screen.findByRole("button", { name: "Fetch and check out" }),
    );
    expect(execute).toHaveBeenCalledExactlyOnceWith({
      plan_id: "opaque-plan-b",
    });
  });

  it.each([
    ["dirty_worktree", /uncommitted changes/],
    ["active_operation", /active Git operation/],
    ["existing_branch_diverged", /points to another commit/],
  ] as const)("blocks confirmation for %s", async (blocker, message) => {
    vi.mocked(collaboration.transport.planPullCheckout).mockResolvedValue({
      ...basePlan,
      plan_id: undefined,
      inspection: {
        ...basePlan.inspection,
        action: undefined,
        blocker,
      },
    });
    const user = userEvent.setup();
    mountBody();
    await user.click(
      await screen.findByRole("button", { name: "Inspect Clone A" }),
    );
    expect(await screen.findByText(message)).toBeVisible();
    expect(
      screen.getByRole("button", { name: "Checkout blocked" }),
    ).toBeDisabled();
    expect(collaboration.transport.executePullCheckout).not.toHaveBeenCalled();
  });

  it("keeps failed execution local, clears its single-use plan, and does not navigate", async () => {
    vi.mocked(collaboration.transport.executePullCheckout).mockRejectedValue({
      code: "stale_view",
      message: "provider-secret-must-not-render",
    });
    const user = userEvent.setup();
    const navigate = mountBody();
    await user.click(
      await screen.findByRole("button", { name: "Inspect Clone A" }),
    );
    await user.click(
      await screen.findByRole("button", { name: "Fetch and check out" }),
    );
    expect(
      await screen.findByText(/head, link, remotes, or worktree changed/),
    ).toBeVisible();
    expect(screen.queryByText(/provider-secret/)).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Fetch and check out" }),
    ).not.toBeInTheDocument();
    expect(navigate).not.toHaveBeenCalled();
  });

  it("preserves checkout success when opening the repository fails", async () => {
    const user = userEvent.setup();
    const navigate = vi.fn(async () => {
      throw new Error("navigation failed");
    });
    mountBody(navigate);
    await user.click(
      await screen.findByRole("button", { name: "Inspect Clone A" }),
    );
    await user.click(
      await screen.findByRole("button", { name: "Fetch and check out" }),
    );

    expect(
      await screen.findByText(/Checked out pr\/42 at a{12} successfully/),
    ).toBeVisible();
    expect(
      screen.getByText(
        /Checkout succeeded, but Gitru could not open the repository/,
      ),
    ).toBeVisible();
    expect(
      screen.queryByRole("button", { name: "Fetch and check out" }),
    ).not.toBeInTheDocument();
    expect(collaboration.transport.executePullCheckout).toHaveBeenCalledTimes(
      1,
    );
    expect(navigate).toHaveBeenCalledExactlyOnceWith(clone.local_repository_id);
  });

  it("releases the dialog lock when an account-key remount retires a pending execution", async () => {
    const pending = deferred<never>();
    vi.mocked(collaboration.transport.executePullCheckout).mockReturnValue(
      pending.promise,
    );
    const busy = vi.fn();
    const user = userEvent.setup();
    const mounted = render(
      <QueryClientProvider client={cache}>
        <PullCheckoutDialogBody
          account={fixtureAccount}
          instanceId="github:https://github.com/"
          itemId={fixtureItem.id}
          repositoryId="fixture-repository"
          onExecutionBusy={busy}
          onNavigate={vi.fn(async () => {})}
        />
      </QueryClientProvider>,
    );
    await user.click(
      await screen.findByRole("button", { name: "Inspect Clone A" }),
    );
    await user.click(
      await screen.findByRole("button", { name: "Fetch and check out" }),
    );
    expect(busy).toHaveBeenCalledWith(true);
    mounted.unmount();
    expect(busy).toHaveBeenLastCalledWith(false);
  });

  it("shows the action only for a pull request with a saved known head", () => {
    const metadata = fixtureMetadata();
    const { rerender } = render(
      <PullRequestCheckoutButton
        account={fixtureAccount}
        item={fixtureItem}
        instanceId="github:https://github.com/"
        metadata={metadata}
      />,
    );
    expect(
      screen.getByRole("button", { name: "Check out locally" }),
    ).toBeVisible();
    rerender(
      <PullRequestCheckoutButton
        account={fixtureAccount}
        item={{ ...fixtureItem, kind: "issue" }}
        instanceId="github:https://github.com/"
        metadata={fixtureMetadata("issue")}
      />,
    );
    expect(
      screen.queryByRole("button", { name: "Check out locally" }),
    ).not.toBeInTheDocument();
    const retained = fixtureMetadata();
    retained.fields = retained.fields.map((field) =>
      field.field === "head"
        ? {
            ...field,
            observed_state: "omitted",
            stale_at: "2026-10-05T00:00:00Z",
          }
        : field,
    );
    rerender(
      <PullRequestCheckoutButton
        account={fixtureAccount}
        item={fixtureItem}
        instanceId="github:https://github.com/"
        metadata={retained}
      />,
    );
    expect(
      screen.getByRole("button", { name: "Check out locally" }),
    ).toBeVisible();
    const missing = fixtureMetadata();
    missing.fields = missing.fields.map((field) =>
      field.field === "head" ? { ...field, saved_state: "not_loaded" } : field,
    );
    rerender(
      <PullRequestCheckoutButton
        account={fixtureAccount}
        item={fixtureItem}
        instanceId="github:https://github.com/"
        metadata={missing}
      />,
    );
    expect(
      screen.queryByRole("button", { name: "Check out locally" }),
    ).not.toBeInTheDocument();
  });
});

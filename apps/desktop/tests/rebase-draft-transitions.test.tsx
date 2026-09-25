import type { RepoOperation } from "@gitru/commands";
import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, test, vi } from "vitest";
import { RebaseActionsBar } from "../src/features/git/rebase/rebase-actions-bar";
import { useCommitDraftStore } from "../src/store/use-commit-draft-store";

const mocks = vi.hoisted(() => ({
  abort: vi.fn(),
  continueRebase: vi.fn(),
  loadAbortPreview: vi.fn(),
  skip: vi.fn(),
  toastPromise: vi.fn((promise: Promise<unknown>) => promise),
}));

vi.mock("sonner", () => ({
  toast: {
    error: vi.fn(),
    promise: mocks.toastPromise,
  },
}));

vi.mock("@/hooks", () => ({
  useRebaseAbort: () => ({ isPending: false, mutateAsync: mocks.abort }),
  useRebaseAbortPreview: () => ({ mutateAsync: mocks.loadAbortPreview }),
  useRebaseContinue: () => ({
    isPending: false,
    mutateAsync: mocks.continueRebase,
  }),
  useRebaseSkip: () => ({ isPending: false, mutateAsync: mocks.skip }),
}));

function operation(overrides: Partial<RepoOperation> = {}): RepoOperation {
  return {
    kind: "rebaseInteractive",
    isRebasing: true,
    pauseReason: "edit",
    todo: [],
    conflictPaths: [],
    ...overrides,
  };
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((resolvePromise) => {
    resolve = resolvePromise;
  });
  return { promise, resolve };
}

function enterRebaseAmendMode() {
  const draft = useCommitDraftStore.getState();
  draft.applyAutofill(
    "rebase:edit:1:Edit summary",
    "Edit summary",
    "Edit body",
    [["Old Pair", "old@example.com"]],
  );
  draft.beginAmend("edit-head", "Amended summary", "Amended body", [
    ["Amend Pair", "amend@example.com"],
  ]);
  useCommitDraftStore.getState().setTitle("Unsaved amend message");
}

beforeEach(() => {
  useCommitDraftStore.setState({ repoKey: "repo-1" });
  useCommitDraftStore.getState().clear();
  mocks.abort.mockResolvedValue(
    operation({ kind: "clean", isRebasing: false }),
  );
  mocks.continueRebase.mockResolvedValue(operation());
  mocks.loadAbortPreview.mockResolvedValue({ warning: "Abort warning" });
  mocks.skip.mockResolvedValue(operation());
});

describe("rebase draft transitions", () => {
  test("Skip keeps the next reword autofill and discards stale amend restoration", async () => {
    const user = userEvent.setup();
    enterRebaseAmendMode();
    mocks.skip.mockImplementation(async () => {
      useCommitDraftStore
        .getState()
        .applyAutofill(
          "rebase:reword:2:Next summary",
          "Next summary",
          "# Next details",
          [["Next Pair", "next@example.com"]],
        );
      return operation({
        pauseReason: "reword",
        current: 2,
        commitMessage: "Next summary\n\n# Next details",
      });
    });

    render(<RebaseActionsBar operation={operation()} />);
    await user.click(screen.getByRole("button", { name: "Skip" }));

    await waitFor(() => expect(mocks.skip).toHaveBeenCalledOnce());
    expect(useCommitDraftStore.getState()).toMatchObject({
      amendCommitId: null,
      autofillKey: "rebase:reword:2:Next summary",
      coAuthors: [["Next Pair", "next@example.com"]],
      description: "# Next details",
      draftBeforeAmend: null,
      mode: "create",
      title: "Next summary",
    });

    useCommitDraftStore.getState().cancelAmend();
    expect(useCommitDraftStore.getState().title).toBe("Next summary");
  });

  test("Skip clears the stale amend target and message when the rebase finishes", async () => {
    const user = userEvent.setup();
    enterRebaseAmendMode();
    mocks.skip.mockResolvedValue(
      operation({ kind: "clean", isRebasing: false }),
    );

    render(<RebaseActionsBar operation={operation()} />);
    await user.click(screen.getByRole("button", { name: "Skip" }));

    await waitFor(() => {
      expect(useCommitDraftStore.getState()).toMatchObject({
        amendCommitId: null,
        autofillKey: null,
        coAuthors: [],
        description: "",
        draftBeforeAmend: null,
        mode: "create",
        title: "",
      });
    });
  });

  test.each([
    "Skip",
    "Continue",
  ] as const)("a slow %s completion cannot clear a new repository's null-key draft", async (action) => {
    const user = userEvent.setup();
    const pending = deferred<RepoOperation>();
    const mutation = action === "Skip" ? mocks.skip : mocks.continueRebase;
    mutation.mockReturnValue(pending.promise);
    useCommitDraftStore.getState().setTitle("Old repository draft");

    render(<RebaseActionsBar operation={operation()} />);
    await user.click(
      screen.getByRole("button", { name: new RegExp(`^${action}$`) }),
    );
    await waitFor(() => expect(mutation).toHaveBeenCalledOnce());

    act(() => {
      const draft = useCommitDraftStore.getState();
      draft.switchRepo("repo-2");
      useCommitDraftStore.getState().setTitle("New repository draft");
      useCommitDraftStore.getState().setDescription("Must survive");
      useCommitDraftStore
        .getState()
        .setCoAuthors([["New Pair", "new@example.com"]]);
    });

    await act(async () => {
      pending.resolve(operation({ kind: "clean", isRebasing: false }));
      await pending.promise;
    });

    expect(useCommitDraftStore.getState()).toMatchObject({
      autofillKey: null,
      coAuthors: [["New Pair", "new@example.com"]],
      description: "Must survive",
      repoKey: "repo-2",
      title: "New repository draft",
    });
  });
});

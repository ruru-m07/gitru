import { toast } from "sonner";
import type { FileSelectionIdentity, RepoKey } from "@/types/store";
import type { AppStoreGet, AppStoreSet } from "./store-helpers";
import {
  createDefaultRepoFileSelectionState,
  getTargetRepoKey,
  isSameSelectionIdentity,
  normalizeSelection,
} from "./store-helpers";

export const createFileSelectionSlice = (
  set: AppStoreSet,
  get: AppStoreGet,
) => ({
  selectionByRepo: {},

  setWorktreeSelectionForRepo: (selection: FileSelectionIdentity | null) => {
    const repoKey = getTargetRepoKey(get());

    if (!repoKey) {
      toast.error("No repository selected");
      return;
    }

    set((state) => {
      const current =
        state.selectionByRepo[repoKey] ?? createDefaultRepoFileSelectionState();
      const nextWorktree = selection
        ? normalizeSelection({
            ...selection,
            source: "worktree",
            stashReference: undefined,
            historyCommitHash: undefined,
          })
        : null;

      if (isSameSelectionIdentity(current.worktree, nextWorktree)) {
        return state;
      }

      return {
        selectionByRepo: {
          ...state.selectionByRepo,
          [repoKey]: {
            ...current,
            worktree: nextWorktree,
          },
        },
      };
    });

    if (selection) {
      get().setMainWindowView("FileDiff");
    }
  },

  setStashSelectionForRepo: (
    stashReference: string,
    selection: FileSelectionIdentity | null,
  ) => {
    const repoKey = getTargetRepoKey(get());

    if (!repoKey) {
      toast.error("No repository selected");
      return;
    }

    if (!stashReference) {
      return;
    }

    set((state) => {
      const current =
        state.selectionByRepo[repoKey] ?? createDefaultRepoFileSelectionState();
      const nextStashByReference = { ...current.stashByReference };
      const previousSelection = nextStashByReference[stashReference] ?? null;
      const nextSelection = selection
        ? normalizeSelection({
            ...selection,
            source: "stash",
            stashReference,
            historyCommitHash: undefined,
          })
        : null;

      if (isSameSelectionIdentity(previousSelection, nextSelection)) {
        return state;
      }

      nextStashByReference[stashReference] = nextSelection;

      return {
        selectionByRepo: {
          ...state.selectionByRepo,
          [repoKey]: {
            ...current,
            stashByReference: nextStashByReference,
          },
        },
      };
    });

    if (selection) {
      get().setMainWindowView("FileDiff");
    }
  },

  setHistorySelectionForRepo: (
    commitHash: string,
    selection: FileSelectionIdentity | null,
  ) => {
    const repoKey = getTargetRepoKey(get());

    if (!repoKey) {
      toast.error("No repository selected");
      return;
    }

    if (!commitHash) {
      return;
    }

    set((state) => {
      const current =
        state.selectionByRepo[repoKey] ?? createDefaultRepoFileSelectionState();
      const nextHistoryByCommit = { ...current.historyByCommit };
      const previousSelection = nextHistoryByCommit[commitHash] ?? null;
      const nextSelection = selection
        ? normalizeSelection({
            ...selection,
            source: "history",
            stashReference: undefined,
            historyCommitHash: commitHash,
          })
        : null;

      if (isSameSelectionIdentity(previousSelection, nextSelection)) {
        return state;
      }

      nextHistoryByCommit[commitHash] = nextSelection;

      return {
        selectionByRepo: {
          ...state.selectionByRepo,
          [repoKey]: {
            ...current,
            historyByCommit: nextHistoryByCommit,
          },
        },
      };
    });

    if (selection) {
      get().setMainWindowView("FileDiff");
    }
  },

  clearSelectionForRepo: (repoKey: RepoKey) =>
    set((state) => {
      const next = { ...state.selectionByRepo };
      delete next[repoKey];
      return { selectionByRepo: next };
    }),

  clearWorktreeSelectionForRepo: (repoPathArg?: RepoKey) => {
    const repoKey = getTargetRepoKey(get(), repoPathArg);
    if (!repoKey) return;

    set((state) => {
      const current =
        state.selectionByRepo[repoKey] ?? createDefaultRepoFileSelectionState();

      return {
        selectionByRepo: {
          ...state.selectionByRepo,
          [repoKey]: {
            ...current,
            worktree: null,
          },
        },
      };
    });
  },

  clearStashSelectionForRepo: (
    stashReference: string,
    repoPathArg?: RepoKey,
  ) => {
    const repoKey = getTargetRepoKey(get(), repoPathArg);
    if (!repoKey || !stashReference) return;

    set((state) => {
      const current =
        state.selectionByRepo[repoKey] ?? createDefaultRepoFileSelectionState();

      return {
        selectionByRepo: {
          ...state.selectionByRepo,
          [repoKey]: {
            ...current,
            stashByReference: {
              ...current.stashByReference,
              [stashReference]: null,
            },
          },
        },
      };
    });
  },

  clearHistorySelectionForRepo: (commitHash: string, repoPathArg?: RepoKey) => {
    const repoKey = getTargetRepoKey(get(), repoPathArg);
    if (!repoKey || !commitHash) return;

    set((state) => {
      const current =
        state.selectionByRepo[repoKey] ?? createDefaultRepoFileSelectionState();

      return {
        selectionByRepo: {
          ...state.selectionByRepo,
          [repoKey]: {
            ...current,
            historyByCommit: {
              ...current.historyByCommit,
              [commitHash]: null,
            },
          },
        },
      };
    });
  },

  pruneStashSelectionsForRepo: (
    repoKey: RepoKey,
    activeStashReferences: string[],
  ) => {
    const targetRepoKey = getTargetRepoKey(get(), repoKey) ?? repoKey;

    set((state) => {
      const current = state.selectionByRepo[targetRepoKey];
      if (!current) return state;

      const activeSet = new Set(activeStashReferences);
      const nextStashByReference = Object.fromEntries(
        Object.entries(current.stashByReference).filter(([reference]) =>
          activeSet.has(reference),
        ),
      );

      return {
        selectionByRepo: {
          ...state.selectionByRepo,
          [targetRepoKey]: {
            ...current,
            stashByReference: nextStashByReference,
          },
        },
      };
    });
  },
});

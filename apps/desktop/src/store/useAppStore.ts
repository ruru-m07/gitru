import { type RepositoryInfo, selectRepository } from "@gitru/commands";
import { toast } from "sonner";
import { create } from "zustand";
import {
  createJSONStorage,
  persist,
  subscribeWithSelector,
} from "zustand/middleware";
import { appState } from "@/state";
import { createTauriStorage } from "./tauriStoreAdapter";

export type SelectionSource = "worktree" | "stash" | "history";

export type FileSelectionIdentity = {
  filePath: string;
  fileNewPath?: string;
  source: SelectionSource;
  stashReference?: string;
  historyCommitHash?: string;
  selectedAt: number;
};

export type RepoFileSelectionState = {
  worktree: FileSelectionIdentity | null;
  stashByReference: Record<string, FileSelectionIdentity | null>;
  historyByCommit: Record<string, FileSelectionIdentity | null>;
};

export type ExternalOpener =
  | "vscode"
  | "cursor"
  | "finder"
  | "terminal"
  | "ghostty";

type RepoKey = string;
export type UpdateChannel = "stable" | "beta";
export type GitSidebarView = "changes" | "stash" | "history";
export type StashViewMode = "branch" | "all";
export type ChangesTab = "changes" | "history";

export type StashStatusFilterMap = Record<
  "modified" | "renamed" | "deleted" | "conflicted" | "untracked",
  boolean
>;

export type GitViewState = {
  leftPanelView: GitSidebarView;
  changesTab: ChangesTab;
  stashViewMode: StashViewMode;
  selectedStashReference: string | null;
  selectedHistoryCommitHash: string | null;
  stashStatusFilters: StashStatusFilterMap;
};

const DEFAULT_STASH_STATUS_FILTERS: StashStatusFilterMap = {
  modified: true,
  renamed: true,
  deleted: true,
  conflicted: true,
  untracked: true,
};

const createDefaultGitViewState = (): GitViewState => ({
  leftPanelView: "changes",
  changesTab: "changes",
  stashViewMode: "branch",
  selectedStashReference: null,
  selectedHistoryCommitHash: null,
  stashStatusFilters: { ...DEFAULT_STASH_STATUS_FILTERS },
});

const createDefaultRepoFileSelectionState = (): RepoFileSelectionState => ({
  worktree: null,
  stashByReference: {},
  historyByCommit: {},
});

const normalizeSelection = (
  selection: FileSelectionIdentity,
): FileSelectionIdentity => ({
  ...selection,
  selectedAt: selection.selectedAt || Date.now(),
});

const isSameSelectionIdentity = (
  left: FileSelectionIdentity | null,
  right: FileSelectionIdentity | null,
) => {
  if (!left && !right) return true;
  if (!left || !right) return false;

  return (
    left.filePath === right.filePath &&
    left.fileNewPath === right.fileNewPath &&
    left.source === right.source &&
    left.stashReference === right.stashReference &&
    left.historyCommitHash === right.historyCommitHash
  );
};

type AppState = {
  selectedRepository: RepositoryInfo | null;
  setSelectedRepository: (repo: RepositoryInfo | null) => void;

  repositories: RepositoryInfo[];
  setRepositories: (repos: RepositoryInfo[]) => void;

  repoSelectIsOpen: boolean;
  setRepoSelectIsOpen: (isOpen: boolean) => void;

  optimisticRepositoryCard: { name: string; path: string } | null;
  setOptimisticRepositoryCard: (
    card: { name: string; path: string } | null,
  ) => void;

  selectionByRepo: Record<RepoKey, RepoFileSelectionState>;

  setWorktreeSelectionForRepo: (
    selection: FileSelectionIdentity | null,
  ) => void;
  setStashSelectionForRepo: (
    stashReference: string,
    selection: FileSelectionIdentity | null,
  ) => void;
  setHistorySelectionForRepo: (
    commitHash: string,
    selection: FileSelectionIdentity | null,
  ) => void;
  clearSelectionForRepo: (repoKey: RepoKey) => void;
  clearWorktreeSelectionForRepo: (repoKey?: RepoKey) => void;
  clearStashSelectionForRepo: (
    stashReference: string,
    repoKey?: RepoKey,
  ) => void;
  clearHistorySelectionForRepo: (commitHash: string, repoKey?: RepoKey) => void;
  pruneStashSelectionsForRepo: (
    repoKey: RepoKey,
    activeStashReferences: string[],
  ) => void;

  mainWindowView: "FileDiff" | "HistoryGraph" | null;
  setMainWindowView: (view: "FileDiff" | "HistoryGraph" | null) => void;

  preferredExternalOpener: ExternalOpener;
  setPreferredExternalOpener: (opener: ExternalOpener) => void;

  updateChannel: UpdateChannel;
  setUpdateChannel: (channel: UpdateChannel) => void;

  gitViewByRepo: Record<RepoKey, GitViewState>;
  setGitViewStateForRepo: (
    partial: Partial<GitViewState>,
    repoPath?: string,
  ) => void;
};

const getTargetRepoPath = (
  selectedRepository: RepositoryInfo | null,
  repoPathArg?: string,
) => repoPathArg ?? selectedRepository?.path;

export const useAppStore = create<AppState>()(
  subscribeWithSelector(
    persist(
      (set, get) => ({
        selectedRepository: null,
        setSelectedRepository: async (repo) => {
          if (!repo) {
            set({ selectedRepository: null });
            return;
          }

          const result = await selectRepository({
            repoId: repo.id,
          });

          if (result) {
            set({ selectedRepository: repo });
            const r = appState.repository;
            setTimeout(async () => {
              await r?.invalidateAll();
            }, 100);
          }
        },

        repositories: [],
        setRepositories: (repos) => set({ repositories: repos }),

        repoSelectIsOpen: false,
        setRepoSelectIsOpen: (isOpen) => set({ repoSelectIsOpen: isOpen }),

        optimisticRepositoryCard: null,
        setOptimisticRepositoryCard: (card) =>
          set({ optimisticRepositoryCard: card }),

        selectionByRepo: {},

        setWorktreeSelectionForRepo: (selection) => {
          const repoPath = get().selectedRepository?.path;

          if (!repoPath) {
            toast.error("No repository selected");
            return;
          }

          set((state) => {
            const current =
              state.selectionByRepo[repoPath] ??
              createDefaultRepoFileSelectionState();
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
                [repoPath]: {
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

        setStashSelectionForRepo: (stashReference, selection) => {
          const repoPath = get().selectedRepository?.path;

          if (!repoPath) {
            toast.error("No repository selected");
            return;
          }

          if (!stashReference) {
            return;
          }

          set((state) => {
            const current =
              state.selectionByRepo[repoPath] ??
              createDefaultRepoFileSelectionState();
            const nextStashByReference = { ...current.stashByReference };
            const previousSelection =
              nextStashByReference[stashReference] ?? null;
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
                [repoPath]: {
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

        setHistorySelectionForRepo: (commitHash, selection) => {
          const repoPath = get().selectedRepository?.path;

          if (!repoPath) {
            toast.error("No repository selected");
            return;
          }

          if (!commitHash) {
            return;
          }

          set((state) => {
            const current =
              state.selectionByRepo[repoPath] ??
              createDefaultRepoFileSelectionState();
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
                [repoPath]: {
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

        clearSelectionForRepo: (repoKey) =>
          set((state) => {
            const next = { ...state.selectionByRepo };
            delete next[repoKey];
            return { selectionByRepo: next };
          }),

        clearWorktreeSelectionForRepo: (repoPathArg) => {
          const repoPath = getTargetRepoPath(
            get().selectedRepository,
            repoPathArg,
          );
          if (!repoPath) return;

          set((state) => {
            const current =
              state.selectionByRepo[repoPath] ??
              createDefaultRepoFileSelectionState();

            return {
              selectionByRepo: {
                ...state.selectionByRepo,
                [repoPath]: {
                  ...current,
                  worktree: null,
                },
              },
            };
          });
        },

        clearStashSelectionForRepo: (stashReference, repoPathArg) => {
          const repoPath = getTargetRepoPath(
            get().selectedRepository,
            repoPathArg,
          );
          if (!repoPath || !stashReference) return;

          set((state) => {
            const current =
              state.selectionByRepo[repoPath] ??
              createDefaultRepoFileSelectionState();

            return {
              selectionByRepo: {
                ...state.selectionByRepo,
                [repoPath]: {
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

        clearHistorySelectionForRepo: (commitHash, repoPathArg) => {
          const repoPath = getTargetRepoPath(
            get().selectedRepository,
            repoPathArg,
          );
          if (!repoPath || !commitHash) return;

          set((state) => {
            const current =
              state.selectionByRepo[repoPath] ??
              createDefaultRepoFileSelectionState();

            return {
              selectionByRepo: {
                ...state.selectionByRepo,
                [repoPath]: {
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

        pruneStashSelectionsForRepo: (repoKey, activeStashReferences) => {
          set((state) => {
            const current = state.selectionByRepo[repoKey];
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
                [repoKey]: {
                  ...current,
                  stashByReference: nextStashByReference,
                },
              },
            };
          });
        },

        mainWindowView: null,
        setMainWindowView: (view) =>
          set((state) =>
            state.mainWindowView === view ? state : { mainWindowView: view },
          ),

        preferredExternalOpener: "vscode",
        setPreferredExternalOpener: (opener) =>
          set({ preferredExternalOpener: opener }),

        updateChannel: "stable",
        setUpdateChannel: (channel) => set({ updateChannel: channel }),

        gitViewByRepo: {},
        setGitViewStateForRepo: (partial, repoPathArg) => {
          const repoPath = repoPathArg ?? get().selectedRepository?.path;
          if (!repoPath) {
            return;
          }

          set((state) => {
            const current =
              state.gitViewByRepo[repoPath] ?? createDefaultGitViewState();
            const nextFilters = partial.stashStatusFilters
              ? {
                  ...DEFAULT_STASH_STATUS_FILTERS,
                  ...current.stashStatusFilters,
                  ...partial.stashStatusFilters,
                }
              : current.stashStatusFilters;

            return {
              gitViewByRepo: {
                ...state.gitViewByRepo,
                [repoPath]: {
                  ...current,
                  ...partial,
                  stashStatusFilters: nextFilters,
                },
              },
            };
          });
        },
      }),
      {
        name: "app-data",
        storage: createJSONStorage(() => createTauriStorage()),
        version: 3,
        partialize: (state) => ({
          selectedRepository: state.selectedRepository,
          repositories: state.repositories,
          repoSelectIsOpen: state.repoSelectIsOpen,
          optimisticRepositoryCard: state.optimisticRepositoryCard,
          preferredExternalOpener: state.preferredExternalOpener,
          updateChannel: state.updateChannel,
          gitViewByRepo: state.gitViewByRepo,
        }),
        migrate: (persistedState: unknown, version) => {
          const state = (persistedState ?? {}) as {
            selectionByRepo?: Record<string, RepoFileSelectionState>;
            selectedFileByRepo?: Record<
              string,
              {
                filePath?: string;
                fileNewPath?: string;
                stashReference?: string;
              } | null
            >;
          };

          if (version >= 3) {
            return state;
          }

          const normalizeSelectionByRepo = (
            value: Record<string, RepoFileSelectionState> | undefined,
          ): Record<string, RepoFileSelectionState> =>
            Object.fromEntries(
              Object.entries(value ?? {}).map(([repoKey, repoSelection]) => {
                const current =
                  repoSelection ?? createDefaultRepoFileSelectionState();
                return [
                  repoKey,
                  {
                    ...createDefaultRepoFileSelectionState(),
                    ...current,
                    stashByReference: {
                      ...(current.stashByReference ?? {}),
                    },
                    historyByCommit: {
                      ...(current.historyByCommit ?? {}),
                    },
                  },
                ];
              }),
            );

          if (version >= 2) {
            return {
              ...state,
              selectionByRepo: normalizeSelectionByRepo(state.selectionByRepo),
            };
          }

          if (!state.selectedFileByRepo) {
            return {
              ...state,
              selectionByRepo: normalizeSelectionByRepo(state.selectionByRepo),
            };
          }

          const migratedSelectionByRepo: Record<
            string,
            RepoFileSelectionState
          > = { ...(state.selectionByRepo ?? {}) };

          for (const [repoKey, legacySelection] of Object.entries(
            state.selectedFileByRepo,
          )) {
            if (!legacySelection?.filePath) {
              continue;
            }

            const current =
              migratedSelectionByRepo[repoKey] ??
              createDefaultRepoFileSelectionState();
            const selectedAt = Date.now();

            if (legacySelection.stashReference) {
              migratedSelectionByRepo[repoKey] = {
                ...current,
                stashByReference: {
                  ...current.stashByReference,
                  [legacySelection.stashReference]: {
                    filePath: legacySelection.filePath,
                    fileNewPath: legacySelection.fileNewPath,
                    source: "stash",
                    stashReference: legacySelection.stashReference,
                    selectedAt,
                  },
                },
              };
              continue;
            }

            migratedSelectionByRepo[repoKey] = {
              ...current,
              worktree: {
                filePath: legacySelection.filePath,
                fileNewPath: legacySelection.fileNewPath,
                source: "worktree",
                selectedAt,
              },
            };
          }

          const { selectedFileByRepo: _ignored, ...rest } = state;

          return {
            ...rest,
            selectionByRepo: normalizeSelectionByRepo(migratedSelectionByRepo),
          };
        },
      },
    ),
  ),
);

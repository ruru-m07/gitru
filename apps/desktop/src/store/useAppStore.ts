import { type RepositoryInfo } from "@gitru/commands";
import { toast } from "sonner";
import { create } from "zustand";
import {
  createJSONStorage,
  persist,
  subscribeWithSelector,
} from "zustand/middleware";
import { createTauriStorage } from "./tauriStoreAdapter";

export type SelectionSource = "worktree" | "stash" | "history";

export type FileSelectionIdentity = {
  filePath: string;
  fileNewPath?: string;
  source: SelectionSource;
  stashReference?: string;
  historyCommitHash?: string;
  worktreeScope?: "staged" | "unstaged" | "conflicted";
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

export type WorkspaceTab = {
  id: string;
  title: string;
  routePath: string;
  repositoryId: string | null;
  createdAt: number;
  updatedAt: number;
};

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
    left.historyCommitHash === right.historyCommitHash &&
    left.worktreeScope === right.worktreeScope
  );
};

type AppState = {
  selectedRepository: RepositoryInfo | null;
  setSelectedRepository: (repo: RepositoryInfo | null) => void;

  tabs: WorkspaceTab[];
  activeTabId: string | null;
  ensureActiveTab: (payload?: {
    routePath?: string;
    repositoryId?: string | null;
    title?: string;
  }) => void;
  createTab: (payload?: {
    routePath?: string;
    repositoryId?: string | null;
    title?: string;
  }) => WorkspaceTab;
  activateTab: (tabId: string) => void;
  closeTab: (tabId: string) => void;
  syncActiveTab: (payload: {
    routePath?: string;
    repositoryId?: string | null;
    title?: string;
  }) => void;

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

const DEFAULT_TAB_ID = "tab-main";
const DEFAULT_TAB_ROUTE = "/app/git";

const getTabTitleFromRoute = (routePath: string) => {
  const pathname = routePath.split("?")[0];

  if (pathname.startsWith("/app/")) {
    const segment = pathname.slice("/app/".length).split("/")[0];

    if (segment) {
      return segment.charAt(0).toUpperCase() + segment.slice(1);
    }
  }

  return "Workspace";
};

const generateTabId = () => {
  if (typeof crypto !== "undefined" && "randomUUID" in crypto) {
    return `tab-${crypto.randomUUID()}`;
  }

  return `tab-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
};

const createWorkspaceTab = (payload?: {
  id?: string;
  routePath?: string;
  repositoryId?: string | null;
  title?: string;
}): WorkspaceTab => {
  const routePath = payload?.routePath || DEFAULT_TAB_ROUTE;
  const now = Date.now();

  return {
    id: payload?.id ?? generateTabId(),
    title: payload?.title?.trim() || getTabTitleFromRoute(routePath),
    routePath,
    repositoryId: payload?.repositoryId ?? null,
    createdAt: now,
    updatedAt: now,
  };
};

const createDefaultTab = () =>
  createWorkspaceTab({
    id: DEFAULT_TAB_ID,
    routePath: DEFAULT_TAB_ROUTE,
    title: "Git",
    repositoryId: null,
  });

const normalizePersistedTabs = (value: unknown): WorkspaceTab[] => {
  if (!Array.isArray(value)) {
    return [];
  }

  return value
    .map((item) => {
      if (!item || typeof item !== "object") {
        return null;
      }

      const tab = item as Partial<WorkspaceTab>;

      if (!tab.id || typeof tab.id !== "string") {
        return null;
      }

      const routePath =
        typeof tab.routePath === "string" && tab.routePath.length > 0
          ? tab.routePath
          : DEFAULT_TAB_ROUTE;

      const createdAt =
        typeof tab.createdAt === "number" ? tab.createdAt : Date.now();
      const updatedAt =
        typeof tab.updatedAt === "number" ? tab.updatedAt : createdAt;

      return {
        id: tab.id,
        title:
          typeof tab.title === "string" && tab.title.trim().length > 0
            ? tab.title.trim()
            : getTabTitleFromRoute(routePath),
        routePath,
        repositoryId:
          typeof tab.repositoryId === "string" ? tab.repositoryId : null,
        createdAt,
        updatedAt,
      } satisfies WorkspaceTab;
    })
    .filter((tab): tab is WorkspaceTab => tab !== null);
};

const ensurePersistedTabState = (
  state: Record<string, unknown>,
): Record<string, unknown> => {
  const parsedTabs = normalizePersistedTabs(state.tabs);
  const tabs = parsedTabs.length > 0 ? parsedTabs : [createDefaultTab()];

  const persistedActiveTabId =
    typeof state.activeTabId === "string" ? state.activeTabId : null;
  const activeTabId =
    persistedActiveTabId && tabs.some((tab) => tab.id === persistedActiveTabId)
      ? persistedActiveTabId
      : (tabs[0]?.id ?? null);

  return {
    ...state,
    tabs,
    activeTabId,
  };
};

export const useAppStore = create<AppState>()(
  subscribeWithSelector(
    persist(
      (set, get) => ({
        selectedRepository: null,
        setSelectedRepository: async (repo) => {
          set((state) => {
            if ((state.selectedRepository?.id ?? null) === (repo?.id ?? null)) {
              return state;
            }

            return { selectedRepository: repo };
          });
        },

        tabs: [createDefaultTab()],
        activeTabId: DEFAULT_TAB_ID,
        ensureActiveTab: (payload) => {
          set((state) => {
            const hasActiveTab =
              !!state.activeTabId &&
              state.tabs.some((tab) => tab.id === state.activeTabId);

            if (state.tabs.length > 0 && hasActiveTab) {
              return state;
            }

            if (state.tabs.length > 0) {
              return {
                activeTabId: state.tabs[0]?.id ?? null,
              };
            }

            const fallbackTab = createWorkspaceTab({
              routePath: payload?.routePath,
              repositoryId: payload?.repositoryId ?? null,
              title: payload?.title,
            });

            return {
              tabs: [fallbackTab],
              activeTabId: fallbackTab.id,
            };
          });
        },
        createTab: (payload) => {
          const newTab = createWorkspaceTab({
            routePath: payload?.routePath,
            repositoryId: payload?.repositoryId ?? null,
            title: payload?.title,
          });

          set((state) => ({
            tabs: [...state.tabs, newTab],
            activeTabId: newTab.id,
          }));

          return newTab;
        },
        activateTab: (tabId) => {
          set((state) => {
            if (state.activeTabId === tabId) {
              return state;
            }

            const exists = state.tabs.some((tab) => tab.id === tabId);

            if (!exists) {
              return state;
            }

            return {
              activeTabId: tabId,
            };
          });
        },
        closeTab: (tabId) => {
          set((state) => {
            if (state.tabs.length <= 1) {
              return state;
            }

            const closingIndex = state.tabs.findIndex(
              (tab) => tab.id === tabId,
            );

            if (closingIndex === -1) {
              return state;
            }

            const nextTabs = state.tabs.filter((tab) => tab.id !== tabId);
            let nextActiveTabId = state.activeTabId;

            if (state.activeTabId === tabId) {
              const fallbackIndex = Math.max(0, closingIndex - 1);
              nextActiveTabId =
                nextTabs[fallbackIndex]?.id ?? nextTabs[0]?.id ?? null;
            } else if (
              nextActiveTabId &&
              !nextTabs.some((tab) => tab.id === nextActiveTabId)
            ) {
              nextActiveTabId = nextTabs[0]?.id ?? null;
            }

            return {
              tabs: nextTabs,
              activeTabId: nextActiveTabId,
            };
          });
        },
        syncActiveTab: (payload) => {
          set((state) => {
            if (!state.activeTabId) {
              return state;
            }

            const activeIndex = state.tabs.findIndex(
              (tab) => tab.id === state.activeTabId,
            );

            if (activeIndex === -1) {
              return state;
            }

            const activeTab = state.tabs[activeIndex];
            const nextRoutePath = payload.routePath ?? activeTab.routePath;
            const nextRepositoryId =
              payload.repositoryId === undefined
                ? activeTab.repositoryId
                : payload.repositoryId;
            const nextTitle = payload.title?.trim() || activeTab.title;

            const didChange =
              nextRoutePath !== activeTab.routePath ||
              nextRepositoryId !== activeTab.repositoryId ||
              nextTitle !== activeTab.title;

            if (!didChange) {
              return state;
            }

            const nextTabs = [...state.tabs];
            nextTabs[activeIndex] = {
              ...activeTab,
              routePath: nextRoutePath,
              repositoryId: nextRepositoryId,
              title: nextTitle,
              updatedAt: Date.now(),
            };

            return {
              tabs: nextTabs,
            };
          });
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

            const nextState: GitViewState = {
              ...current,
              ...partial,
              stashStatusFilters: nextFilters,
            };

            const hasChanged =
              current.leftPanelView !== nextState.leftPanelView ||
              current.changesTab !== nextState.changesTab ||
              current.stashViewMode !== nextState.stashViewMode ||
              current.selectedStashReference !==
                nextState.selectedStashReference ||
              current.selectedHistoryCommitHash !==
                nextState.selectedHistoryCommitHash ||
              current.stashStatusFilters.modified !==
                nextState.stashStatusFilters.modified ||
              current.stashStatusFilters.renamed !==
                nextState.stashStatusFilters.renamed ||
              current.stashStatusFilters.deleted !==
                nextState.stashStatusFilters.deleted ||
              current.stashStatusFilters.conflicted !==
                nextState.stashStatusFilters.conflicted ||
              current.stashStatusFilters.untracked !==
                nextState.stashStatusFilters.untracked;

            if (!hasChanged) {
              return state;
            }

            return {
              gitViewByRepo: {
                ...state.gitViewByRepo,
                [repoPath]: nextState,
              },
            };
          });
        },
      }),
      {
        name: "app-data",
        storage: createJSONStorage(() => createTauriStorage()),
        version: 4,
        partialize: (state) => ({
          selectedRepository: state.selectedRepository,
          tabs: state.tabs,
          activeTabId: state.activeTabId,
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

          if (version >= 4) {
            return ensurePersistedTabState(state as Record<string, unknown>);
          }

          if (version >= 3) {
            return ensurePersistedTabState(state as Record<string, unknown>);
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
            return ensurePersistedTabState({
              ...state,
              selectionByRepo: normalizeSelectionByRepo(state.selectionByRepo),
            });
          }

          if (!state.selectedFileByRepo) {
            return ensurePersistedTabState({
              ...state,
              selectionByRepo: normalizeSelectionByRepo(state.selectionByRepo),
            });
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

          return ensurePersistedTabState({
            ...rest,
            selectionByRepo: normalizeSelectionByRepo(migratedSelectionByRepo),
          });
        },
      },
    ),
  ),
);

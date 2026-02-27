import {
  type FileStatusKind,
  type RepositoryInfo,
  selectRepository,
} from "@gitru/commands";
import { toast } from "sonner";
import { create } from "zustand";
import {
  createJSONStorage,
  persist,
  subscribeWithSelector,
} from "zustand/middleware";
import { appState } from "@/state";
import { createTauriStorage } from "./tauriStoreAdapter";

export type SelectedFile = {
  filePath?: string;
  fileNewPath?: string;
  status?: FileStatusKind[];
  stashReference?: string;
};

export type ExternalOpener =
  | "vscode"
  | "cursor"
  | "finder"
  | "terminal"
  | "ghostty";

type RepoKey = string;
export type UpdateChannel = "stable" | "beta";
export type GitSidebarView = "changes" | "stash";
export type StashViewMode = "branch" | "all";

export type StashStatusFilterMap = Record<
  "modified" | "renamed" | "deleted" | "conflicted" | "untracked",
  boolean
>;

export type GitViewState = {
  leftPanelView: GitSidebarView;
  stashViewMode: StashViewMode;
  selectedStashReference: string | null;
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
  stashViewMode: "branch",
  selectedStashReference: null,
  stashStatusFilters: { ...DEFAULT_STASH_STATUS_FILTERS },
});

type AppState = {
  selectedRepository: RepositoryInfo | null;
  setSelectedRepository: (repo: RepositoryInfo | null) => void;

  repositories: RepositoryInfo[];
  setRepositories: (repos: RepositoryInfo[]) => void;

  repoSelectIsOpen: boolean;
  setRepoSelectIsOpen: (isOpen: boolean) => void;

  selectedFileByRepo: Record<RepoKey, SelectedFile | null>;

  setSelectedFileForRepo: (file: SelectedFile | null) => void;

  clearSelectedFileForRepo: (repoKey: RepoKey) => void;

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

          let result = await selectRepository({
            repoId: repo.id,
          });

          if (result) {
            set({ selectedRepository: repo });
            const r = appState.repository;
            // await r?.invalidateAll();
            setTimeout(async () => {
              await r?.invalidateAll();
            }, 100);
          }
        },

        repositories: [],
        setRepositories: (repos) => set({ repositories: repos }),

        repoSelectIsOpen: false,
        setRepoSelectIsOpen: (isOpen) => set({ repoSelectIsOpen: isOpen }),

        selectedFileByRepo: {},

        setSelectedFileForRepo: (file) => {
          const repoPath = get().selectedRepository?.path;

          if (!repoPath) {
            toast.error("No repository selected");
            return;
          }

          set((state) => ({
            selectedFileByRepo: {
              ...state.selectedFileByRepo,
              [repoPath]: file,
            },
          }));

          if (file) {
            get().setMainWindowView("FileDiff");
          }
        },

        clearSelectedFileForRepo: (repoKey) =>
          set((state) => {
            const next = { ...state.selectedFileByRepo };
            delete next[repoKey];
            return { selectedFileByRepo: next };
          }),

        mainWindowView: null,
        setMainWindowView: (view) => set({ mainWindowView: view }),

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
            const current = state.gitViewByRepo[repoPath] ?? createDefaultGitViewState();
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
      },
    ),
  ),
);

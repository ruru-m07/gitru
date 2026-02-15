import type { FileStatusKind, RepositoryInfo } from "@gitru/commands";
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
};

type RepoKey = string;

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
};

export const useAppStore = create<AppState>()(
  subscribeWithSelector(
    persist(
      (set, get) => ({
        selectedRepository: null,
        setSelectedRepository: async (repo) => {
          set({ selectedRepository: repo });

          const r = appState.repository;
          await r?.invalidateAll();

          setTimeout(async () => {
            await r?.invalidateAll();
          }, 100);
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
      }),
      {
        name: "app-data",
        storage: createJSONStorage(() => createTauriStorage()),
      },
    ),
  ),
);

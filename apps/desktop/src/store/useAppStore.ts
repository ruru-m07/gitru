import type { FileStatusKind, RepoSitoryStore } from "@gitru/commands";
import { toast } from "sonner";
import { create } from "zustand";
import {
  createJSONStorage,
  persist,
  subscribeWithSelector,
} from "zustand/middleware";
import { createTauriStorage } from "./tauriStoreAdapter";

export type SelectedFile = {
  filePath: string;
  fileNewPath?: string;
  status: FileStatusKind[];
};

type RepoKey = string;

type AppState = {
  selectedRepository: RepoSitoryStore | null;
  setSelectedRepository: (repo: RepoSitoryStore | null) => void;

  repositories: RepoSitoryStore[];
  setRepositories: (repos: RepoSitoryStore[]) => void;

  repoSelectIsOpen: boolean;
  setRepoSelectIsOpen: (isOpen: boolean) => void;

  selectedFileByRepo: Record<RepoKey, SelectedFile | null>;

  setSelectedFileForRepo: (file: SelectedFile | null) => void;

  clearSelectedFileForRepo: (repoKey: RepoKey) => void;
};

export const useAppStore = create<AppState>()(
  subscribeWithSelector(
    persist(
      (set, get) => ({
        selectedRepository: null,
        setSelectedRepository: (repo) => set({ selectedRepository: repo }),

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
        },

        clearSelectedFileForRepo: (repoKey) =>
          set((state) => {
            const next = { ...state.selectedFileByRepo };
            delete next[repoKey];
            return { selectedFileByRepo: next };
          }),
      }),
      {
        name: "app-data",
        storage: createJSONStorage(() => createTauriStorage()),
      },
    ),
  ),
);

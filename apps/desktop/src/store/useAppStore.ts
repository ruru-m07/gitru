import { create } from "zustand";
import { createJSONStorage, persist } from "zustand/middleware";
import type { RepoSitoryStore } from "@/tauri/types";
import { createTauriStorage } from "./tauriStoreAdapter";

type AppState = {
	selectedRepository: RepoSitoryStore | null;
	setSelectedRepository: (repo: RepoSitoryStore | null) => void;
	repositories: RepoSitoryStore[];
	setRepositories: (repos: RepoSitoryStore[]) => void;
	repoSelectIsOpen: boolean;
	setRepoSelectIsOpen: (isOpen: boolean) => void;
};

export const useAppStore = create<AppState>()(
	persist(
		(set) => ({
			selectedRepository: null,
			setSelectedRepository: (repo) => set({ selectedRepository: repo }),
			repositories: [],
			setRepositories: (repos) => set({ repositories: repos }),
			repoSelectIsOpen: false,
			setRepoSelectIsOpen: (isOpen) => set({ repoSelectIsOpen: isOpen }),
		}),
		{
			name: "app-data",
			storage: createJSONStorage(() => createTauriStorage()),
		},
	),
);

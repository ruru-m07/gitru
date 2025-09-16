import { create } from "zustand";
import { createJSONStorage, persist } from "zustand/middleware";
import { createTauriStorage } from "./tauriStoreAdapter";

type AppState = {
	lastPage: string;
	setLastPage: (page: string) => void;
};

export const useAppStore = create<AppState>()(
	persist(
		(set) => ({
			lastPage: "/",
			setLastPage: (page) => set({ lastPage: page }),
		}),
		{
			name: "app-data",
			storage: createJSONStorage(() => createTauriStorage()),
		},
	),
);

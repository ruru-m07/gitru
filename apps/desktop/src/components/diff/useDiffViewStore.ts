import { create } from "zustand";
import { persist } from "zustand/middleware";
import type { FileStatusKind } from "@/tauri";

type ViewMode = "split" | "unified";

interface DiffViewState {
	viewMode: ViewMode;
	setViewMode: (mode: ViewMode) => void;
	selectedFilePath?: string;
	setSelectedFilePath: (path: string) => void;
	selectedFileStatus?: FileStatusKind[];
	setSelectedFileStatus: (status: FileStatusKind[]) => void;
}

export const useDiffViewStore = create<DiffViewState>()(
	persist(
		(set) => ({
			viewMode:
				(typeof localStorage !== "undefined"
					? (localStorage.getItem("gitDiffViewMode") as ViewMode)
					: null) || "unified",
			setViewMode: (mode) => {
				localStorage.setItem("gitDiffViewMode", mode);
				set({ viewMode: mode });
			},
			selectedFilePath: undefined,
			setSelectedFilePath: (path) => set({ selectedFilePath: path }),
			selectedFileStatus: undefined,
			setSelectedFileStatus: (status) => set({ selectedFileStatus: status }),
		}),
		{
			name: "diff-view-data",
		},
	),
);

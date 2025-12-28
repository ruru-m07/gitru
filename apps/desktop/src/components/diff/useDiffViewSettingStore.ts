import { create } from "zustand";
import { persist } from "zustand/middleware";

type ViewMode = "split" | "unified";

interface DiffViewState {
  viewMode: ViewMode;
  setViewMode: (mode: ViewMode) => void;
}

export const useDiffViewerSettings = create<DiffViewState>()(
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
    }),
    {
      name: "diff-view-settings-data",
    },
  ),
);

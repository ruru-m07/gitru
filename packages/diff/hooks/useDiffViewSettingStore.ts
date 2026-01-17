import { create } from "zustand";
import { persist } from "zustand/middleware";

type ViewMode = "split" | "unified";

interface DiffViewState {
  diffStyle: ViewMode;
  setDiffStyle: (mode: ViewMode) => void;
  overflow: "scroll" | "wrap";
  setOverflow: (overflow: "scroll" | "wrap") => void;
}

export const useDiffViewerSettings = create<DiffViewState>()(
  persist(
    (set) => ({
      diffStyle: "unified",
      setDiffStyle: (mode) => set({ diffStyle: mode }),
      overflow: "scroll",
      setOverflow: (overflow) => set({ overflow }),
    }),
    {
      name: "diff-view-settings-data",
    },
  ),
);

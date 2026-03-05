import { create } from "zustand";
import { persist } from "zustand/middleware";

type ViewMode = "split" | "unified";
export type ImageDiffMode = "twoUp" | "swipe" | "onionSkin" | "difference";
export type DifferenceDiffProvider = "worker" | "cssOnly" | "odiffNode";

interface DiffViewState {
  diffStyle: ViewMode;
  setDiffStyle: (mode: ViewMode) => void;
  imageDiffMode: ImageDiffMode;
  setImageDiffMode: (mode: ImageDiffMode) => void;
  differenceDiffProvider: DifferenceDiffProvider;
  setDifferenceDiffProvider: (provider: DifferenceDiffProvider) => void;
  overflow: "scroll" | "wrap";
  setOverflow: (overflow: "scroll" | "wrap") => void;
}

export const useDiffViewerSettings = create<DiffViewState>()(
  persist(
    (set) => ({
      diffStyle: "unified",
      setDiffStyle: (mode) => set({ diffStyle: mode }),
      imageDiffMode: "twoUp",
      setImageDiffMode: (mode) => set({ imageDiffMode: mode }),
      differenceDiffProvider: "worker",
      setDifferenceDiffProvider: (provider) =>
        set({ differenceDiffProvider: provider }),
      overflow: "scroll",
      setOverflow: (overflow) => set({ overflow }),
    }),
    {
      name: "diff-view-settings-data",
    },
  ),
);

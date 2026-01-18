import type { FileStatusKind } from "@gitru/commands";
import { create } from "zustand";
import { persist } from "zustand/middleware";

interface DiffViewState {
  selectedFilePath?: {
    path: string;
    newPath?: string;
  };
  setSelectedFilePath: (path?: { path: string; newPath?: string }) => void;
  selectedFileStatus?: FileStatusKind[];
  setSelectedFileStatus: (status: FileStatusKind[]) => void;
}

export const useDiffViewStore = create<DiffViewState>()(
  persist(
    (set) => ({
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

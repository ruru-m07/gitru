import type { FileStatus } from "@gitru/commands";
import { create } from "zustand";

interface FileSelectionState {
  // Multi-selected files (paths)
  selectedFiles: Set<string>;
  // Last clicked file for shift-click range selection
  lastClickedFile: string | null;
  // Current focus index for keyboard navigation
  focusedIndex: number;
  // All files in current view (for navigation)
  allFiles: FileStatus[];

  // Actions
  setAllFiles: (files: FileStatus[]) => void;
  setFocusedIndex: (index: number) => void;
  selectFile: (path: string) => void;
  deselectFile: (path: string) => void;
  toggleFileSelection: (path: string) => void;
  selectRange: (fromPath: string, toPath: string) => void;
  selectAll: () => void;
  clearSelection: () => void;
  isSelected: (path: string) => boolean;

  // Handle click with modifiers
  handleFileClick: (
    file: FileStatus,
    index: number,
    event: { shiftKey: boolean; metaKey: boolean; ctrlKey: boolean },
  ) => void;

  // Keyboard navigation
  moveUp: () => void;
  moveDown: () => void;
}

export const useFileSelectionStore = create<FileSelectionState>((set, get) => ({
  selectedFiles: new Set<string>(),
  lastClickedFile: null,
  focusedIndex: -1,
  allFiles: [],

  setAllFiles: (files) => set({ allFiles: files }),

  setFocusedIndex: (index) => set({ focusedIndex: index }),

  selectFile: (path) =>
    set((state) => ({
      selectedFiles: new Set([...state.selectedFiles, path]),
      lastClickedFile: path,
    })),

  deselectFile: (path) =>
    set((state) => {
      const newSet = new Set(state.selectedFiles);
      newSet.delete(path);
      return { selectedFiles: newSet };
    }),

  toggleFileSelection: (path) =>
    set((state) => {
      const newSet = new Set(state.selectedFiles);
      if (newSet.has(path)) {
        newSet.delete(path);
      } else {
        newSet.add(path);
      }
      return { selectedFiles: newSet, lastClickedFile: path };
    }),

  selectRange: (fromPath, toPath) =>
    set((state) => {
      const { allFiles } = state;
      const fromIndex = allFiles.findIndex((f) => f.path === fromPath);
      const toIndex = allFiles.findIndex((f) => f.path === toPath);

      if (fromIndex === -1 || toIndex === -1) return state;

      const start = Math.min(fromIndex, toIndex);
      const end = Math.max(fromIndex, toIndex);

      const newSet = new Set(state.selectedFiles);
      for (let i = start; i <= end; i++) {
        newSet.add(allFiles[i].path);
      }

      return { selectedFiles: newSet };
    }),

  selectAll: () =>
    set((state) => ({
      selectedFiles: new Set(state.allFiles.map((f) => f.path)),
    })),

  clearSelection: () =>
    set({
      selectedFiles: new Set<string>(),
      lastClickedFile: null,
    }),

  isSelected: (path) => get().selectedFiles.has(path),

  handleFileClick: (file, index, event) => {
    const state = get();
    const { shiftKey, metaKey, ctrlKey } = event;
    const isCmdOrCtrl = metaKey || ctrlKey;

    if (shiftKey && state.lastClickedFile) {
      // Shift+click: select range from last clicked to current
      state.selectRange(state.lastClickedFile, file.path);
      set({ focusedIndex: index });
    } else if (isCmdOrCtrl) {
      // Cmd/Ctrl+click: toggle individual selection
      state.toggleFileSelection(file.path);
      set({ focusedIndex: index });
    } else {
      // Normal click: clear selection and select only this file
      set({
        selectedFiles: new Set([file.path]),
        lastClickedFile: file.path,
        focusedIndex: index,
      });
    }
  },

  moveUp: () =>
    set((state) => {
      const newIndex = Math.max(0, state.focusedIndex - 1);
      if (newIndex !== state.focusedIndex && state.allFiles[newIndex]) {
        return {
          focusedIndex: newIndex,
          selectedFiles: new Set([state.allFiles[newIndex].path]),
          lastClickedFile: state.allFiles[newIndex].path,
        };
      }
      return state;
    }),

  moveDown: () =>
    set((state) => {
      const newIndex = Math.min(
        state.allFiles.length - 1,
        state.focusedIndex + 1,
      );
      if (newIndex !== state.focusedIndex && state.allFiles[newIndex]) {
        return {
          focusedIndex: newIndex,
          selectedFiles: new Set([state.allFiles[newIndex].path]),
          lastClickedFile: state.allFiles[newIndex].path,
        };
      }
      return state;
    }),
}));

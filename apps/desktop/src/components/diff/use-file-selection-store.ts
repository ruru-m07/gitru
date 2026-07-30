import type { FileStatus } from "@gitru/commands";
import { create } from "zustand";

type FileSelectionEntry = {
  key: string;
  file: FileStatus;
  sectionType?: string;
};

interface FileSelectionState {
  // Multi-selected files (selection keys)
  selectedFiles: Set<string>;
  // Last clicked file for shift-click range selection
  lastClickedFile: string | null;
  // Current focus index for keyboard navigation
  focusedIndex: number;
  // All files in current view (for navigation)
  allFiles: FileSelectionEntry[];

  // Actions
  setAllFiles: (files: FileSelectionEntry[]) => void;
  setFocusedIndex: (index: number) => void;
  selectFile: (key: string) => void;
  deselectFile: (key: string) => void;
  toggleFileSelection: (key: string) => void;
  selectRange: (fromKey: string, toKey: string) => void;
  selectAll: () => void;
  clearSelection: () => void;
  isSelected: (key: string) => boolean;

  // Handle click with modifiers
  handleFileClick: (
    file: FileStatus,
    index: number,
    event: { shiftKey: boolean; metaKey: boolean; ctrlKey: boolean },
    selectionKey: string,
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

  setAllFiles: (files) =>
    set((state) => {
      const prev = state.allFiles;
      if (
        prev.length === files.length &&
        prev.every((entry, index) => entry.key === files[index]?.key)
      ) {
        return state;
      }
      return { allFiles: files };
    }),

  setFocusedIndex: (index) => set({ focusedIndex: index }),

  selectFile: (key) =>
    set((state) => ({
      selectedFiles: new Set([...state.selectedFiles, key]),
      lastClickedFile: key,
    })),

  deselectFile: (key) =>
    set((state) => {
      const newSet = new Set(state.selectedFiles);
      newSet.delete(key);
      return { selectedFiles: newSet };
    }),

  toggleFileSelection: (key) =>
    set((state) => {
      const newSet = new Set(state.selectedFiles);
      if (newSet.has(key)) {
        newSet.delete(key);
      } else {
        newSet.add(key);
      }
      return { selectedFiles: newSet, lastClickedFile: key };
    }),

  selectRange: (fromKey, toKey) =>
    set((state) => {
      const { allFiles } = state;
      const fromIndex = allFiles.findIndex((f) => f.key === fromKey);
      const toIndex = allFiles.findIndex((f) => f.key === toKey);

      if (fromIndex === -1 || toIndex === -1) return state;

      const start = Math.min(fromIndex, toIndex);
      const end = Math.max(fromIndex, toIndex);

      const newSet = new Set(state.selectedFiles);
      for (let i = start; i <= end; i++) {
        newSet.add(allFiles[i].key);
      }

      return { selectedFiles: newSet };
    }),

  selectAll: () =>
    set((state) => ({
      selectedFiles: new Set(state.allFiles.map((f) => f.key)),
    })),

  clearSelection: () =>
    set({
      selectedFiles: new Set<string>(),
      lastClickedFile: null,
    }),

  isSelected: (key) => get().selectedFiles.has(key),

  handleFileClick: (_file, index, event, selectionKey) => {
    const state = get();
    const { shiftKey, metaKey, ctrlKey } = event;
    const isCmdOrCtrl = metaKey || ctrlKey;

    if (shiftKey && state.lastClickedFile) {
      // Shift+click: select range from last clicked to current
      state.selectRange(state.lastClickedFile, selectionKey);
      set({ focusedIndex: index });
    } else if (isCmdOrCtrl) {
      // Cmd/Ctrl+click: toggle individual selection
      state.toggleFileSelection(selectionKey);
      set({ focusedIndex: index });
    } else {
      // Normal click: clear selection and select only this file
      set({
        selectedFiles: new Set([selectionKey]),
        lastClickedFile: selectionKey,
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
        };
      }
      return state;
    }),
}));

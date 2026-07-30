import { Virtualizer } from "@tanstack/react-virtual";
import { useCallback, useEffect, useState } from "react";

import { useFileSelectionStore } from "@/components/diff/use-file-selection-store";
import { FileSelectionIdentity } from "@/types/store";

import { getWorktreeScope } from "./utils";

export function useListKeyboardNavigation({
  parentRef,
  fileItems,
  fileIndexToItemIndex,
  onFileClick,
  setSelectedFilePath,
  virtualizer,
}: {
  parentRef: React.RefObject<HTMLDivElement | null>;
  fileItems: Array<{
    key: string;
    file: import("@gitru/commands").FileStatus;
    sectionType?: import("./types").FileListSection["type"];
  }>;
  fileIndexToItemIndex: number[];
  onFileClick: (
    file: import("@gitru/commands").FileStatus,
    index: number,
    event: { shiftKey: boolean; metaKey: boolean; ctrlKey: boolean },
    selectionKey: string,
  ) => void;
  setSelectedFilePath: (file: FileSelectionIdentity | null) => void;
  virtualizer: Virtualizer<HTMLDivElement, Element>;
}) {
  const [isContainerFocused, setIsContainerFocused] = useState(false);
  const selectedFiles = useFileSelectionStore((state) => state.selectedFiles);
  const focusedIndex = useFileSelectionStore((state) => state.focusedIndex);
  const setFocusedIndex = useFileSelectionStore(
    (state) => state.setFocusedIndex,
  );
  const clearSelection = useFileSelectionStore((state) => state.clearSelection);

  useEffect(() => {
    if (fileItems.length === 0) {
      if (focusedIndex !== -1) {
        setFocusedIndex(-1);
      }
      return;
    }

    if (focusedIndex >= fileItems.length) {
      setFocusedIndex(fileItems.length - 1);
    }
  }, [fileItems.length, focusedIndex, setFocusedIndex]);

  const focusContainer = useCallback(() => {
    parentRef.current?.focus();
  }, [parentRef]);

  const handleContainerFocus = useCallback(
    (event: React.FocusEvent<HTMLDivElement>) => {
      if (event.currentTarget !== event.target) return;
      setIsContainerFocused(true);
      if (focusedIndex === -1 && fileItems.length > 0) {
        const firstSelected = fileItems.findIndex((entry) =>
          selectedFiles.has(entry.key),
        );
        setFocusedIndex(firstSelected >= 0 ? firstSelected : 0);
      }
    },
    [fileItems, focusedIndex, selectedFiles, setFocusedIndex],
  );

  const handleContainerBlur = useCallback(
    (event: React.FocusEvent<HTMLDivElement>) => {
      if (event.currentTarget !== event.target) return;
      setIsContainerFocused(false);
    },
    [],
  );

  const handleKeyDown = useCallback(
    (event: React.KeyboardEvent<HTMLDivElement>) => {
      if (event.currentTarget !== event.target) return;

      if (event.key === "ArrowDown" || event.key === "ArrowUp") {
        event.preventDefault();
        if (fileItems.length === 0) return;

        const delta = event.key === "ArrowDown" ? 1 : -1;
        const startIndex = focusedIndex === -1 ? 0 : focusedIndex;
        const nextFocusedIndex = Math.max(
          0,
          Math.min(fileItems.length - 1, startIndex + delta),
        );
        const entry = fileItems[nextFocusedIndex];
        if (!entry) return;
        const { file, key: selectionKey, sectionType } = entry;

        if (event.metaKey || event.ctrlKey) {
          setFocusedIndex(nextFocusedIndex);
        } else {
          onFileClick(
            file,
            nextFocusedIndex,
            {
              shiftKey: event.shiftKey,
              metaKey: event.metaKey,
              ctrlKey: event.ctrlKey,
            },
            selectionKey,
          );

          if (!event.shiftKey && !event.metaKey && !event.ctrlKey) {
            setSelectedFilePath({
              filePath: file.path,
              fileNewPath: file.new_path,
              source: "worktree",
              worktreeScope: getWorktreeScope(sectionType),
              selectedAt: Date.now(),
            });
          }
        }

        const itemIndex = fileIndexToItemIndex[nextFocusedIndex];
        if (itemIndex !== undefined) {
          virtualizer.scrollToIndex(itemIndex, { align: "auto" });
        }
        return;
      }

      if (event.key === "Enter") {
        event.preventDefault();
        if (fileItems.length === 0) return;
        const activeIndex = focusedIndex >= 0 ? focusedIndex : 0;
        const entry = fileItems[activeIndex];
        if (!entry) return;
        const { file, key: selectionKey, sectionType } = entry;

        onFileClick(
          file,
          activeIndex,
          {
            shiftKey: event.shiftKey,
            metaKey: event.metaKey,
            ctrlKey: event.ctrlKey,
          },
          selectionKey,
        );

        if (!event.shiftKey && !event.metaKey && !event.ctrlKey) {
          setSelectedFilePath({
            filePath: file.path,
            fileNewPath: file.new_path,
            source: "worktree",
            worktreeScope: getWorktreeScope(sectionType),
            selectedAt: Date.now(),
          });
        }
      }

      if (event.key === "Escape") {
        event.preventDefault();
        clearSelection();
      }
    },
    [
      fileItems,
      fileIndexToItemIndex,
      focusedIndex,
      clearSelection,
      onFileClick,
      setFocusedIndex,
      setSelectedFilePath,
      virtualizer,
    ],
  );

  return {
    isContainerFocused,
    focusedIndex,
    focusContainer,
    handleContainerFocus,
    handleContainerBlur,
    handleKeyDown,
  };
}

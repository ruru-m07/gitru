import { cn } from "@gitru/ui/lib/utils";
import { useVirtualizer } from "@tanstack/react-virtual";
import { memo, useCallback, useEffect, useId, useRef, useState } from "react";

import { useFileSelectionStore } from "@/components/diff/use-file-selection-store";

import { FileRow } from "./file-row";
import { SectionHeader } from "./section-header";
import {
  FileListSection,
  FileRowContextAction,
  VirtualizedFileListProps,
} from "./types";
import { useListKeyboardNavigation } from "./use-list-keyboard-navigation";
import { useVirtualItems } from "./use-virtual-items";
import { getWorktreeScope, ITEM_HEIGHT, SECTION_HEADER_HEIGHT } from "./utils";

export type { FileListSection, FileRowContextAction, VirtualizedFileListProps };

export const VirtualizedFileList = memo(function VirtualizedFileList({
  sections,
  sectionMode = "accordion",
  searchQuery = "",
  onFileClick,
  onAdd,
  onUnstage,
  onDiscard,
  renderDiscard,
  setSelectedFilePath,
  getContextActions,
  selectedFilePath,
  className,
  defaultExpandedSections,
}: VirtualizedFileListProps) {
  const parentRef = useRef<HTMLDivElement>(null);
  const listId = useId();
  const setAllFiles = useFileSelectionStore((state) => state.setAllFiles);
  const selectedFiles = useFileSelectionStore((state) => state.selectedFiles);

  const [expandedSections, setExpandedSections] = useState<Set<string>>(() => {
    if (defaultExpandedSections) {
      return new Set(defaultExpandedSections);
    }
    return new Set(sections.map((s) => s.id));
  });

  const toggleSection = useCallback((sectionId: string) => {
    setExpandedSections((prev) => {
      const next = new Set(prev);
      if (next.has(sectionId)) {
        next.delete(sectionId);
      } else {
        next.add(sectionId);
      }
      return next;
    });
  }, []);

  const { items, fileItems, fileIndexByKey, fileIndexToItemIndex } =
    useVirtualItems(sections, sectionMode, expandedSections);

  useEffect(() => {
    setAllFiles(fileItems);
  }, [fileItems, setAllFiles]);

  const virtualizer = useVirtualizer({
    count: items.length,
    getScrollElement: () => parentRef.current,
    getItemKey: (index) => {
      const item = items[index];
      if (item.type === "header") return `header:${item.sectionId}`;
      return `file:${item.file.path}:${item.file.new_path ?? ""}:${item.sectionId}`;
    },
    estimateSize: (index) =>
      items[index].type === "header" ? SECTION_HEADER_HEIGHT : ITEM_HEIGHT,
    overscan: 15,
  });

  const virtualItems = virtualizer.getVirtualItems();

  const {
    isContainerFocused,
    focusedIndex,
    focusContainer,
    handleContainerFocus,
    handleContainerBlur,
    handleKeyDown,
  } = useListKeyboardNavigation({
    parentRef,
    fileItems,
    fileIndexToItemIndex,
    onFileClick,
    setSelectedFilePath,
    virtualizer,
  });

  return (
    <div
      ref={parentRef}
      role="listbox"
      aria-label="File status list"
      aria-multiselectable={true}
      aria-activedescendant={
        isContainerFocused && focusedIndex >= 0
          ? `${listId}-option-${focusedIndex}`
          : undefined
      }
      tabIndex={0}
      onFocus={handleContainerFocus}
      onBlur={handleContainerBlur}
      onKeyDown={handleKeyDown}
      className={cn(
        "select-none h-full overflow-auto focus-visible:outline-none",
        className,
      )}
    >
      <div
        style={{
          height: `${virtualizer.getTotalSize()}px`,
          width: "100%",
          position: "relative",
        }}
      >
        {virtualItems.map((virtualRow) => {
          const item = items[virtualRow.index];

          if (item.type === "header") {
            const isExpanded = expandedSections.has(item.sectionId);

            return (
              <div
                key={virtualRow.key}
                data-index={virtualRow.index}
                role="presentation"
                style={{
                  position: "absolute",
                  top: 0,
                  left: 0,
                  width: "100%",
                  height: `${virtualRow.size}px`,
                  transform: `translateY(${virtualRow.start}px)`,
                }}
              >
                <SectionHeader
                  sectionId={item.sectionId}
                  sectionName={item.sectionName}
                  sectionType={item.sectionType}
                  count={item.count}
                  isExpanded={isExpanded}
                  onToggle={toggleSection}
                  actions={item.actions}
                />
              </div>
            );
          }

          const matchesScope = selectedFilePath?.scope
            ? selectedFilePath.scope === getWorktreeScope(item.sectionType)
            : true;
          const isSelected =
            matchesScope &&
            selectedFilePath?.path === item.file.path &&
            (selectedFilePath?.newPath ?? "") === (item.file.new_path ?? "");
          const isMultiSelected = selectedFiles.has(item.selectionKey);
          const isChangesSection = item.sectionType === "changes";
          const isStagedSection = item.sectionType === "staged";
          const isConflictSection = item.sectionType === "conflicted";
          const fileIndex = fileIndexByKey.get(item.selectionKey) ?? -1;
          const isFocused = isContainerFocused && focusedIndex === fileIndex;
          const listSize = fileItems.length;
          const prevSelected =
            fileIndex > 0
              ? selectedFiles.has(fileItems[fileIndex - 1]?.key)
              : false;
          const nextSelected =
            fileIndex >= 0 && fileIndex < fileItems.length - 1
              ? selectedFiles.has(fileItems[fileIndex + 1]?.key)
              : false;
          const isGroupStart =
            (isSelected || isMultiSelected) && !prevSelected && nextSelected;
          const isGroupMiddle =
            (isSelected || isMultiSelected) && prevSelected && nextSelected;
          const isGroupEnd =
            (isSelected || isMultiSelected) && prevSelected && !nextSelected;

          return (
            <div
              key={virtualRow.key}
              data-index={virtualRow.index}
              style={{
                position: "absolute",
                top: 0,
                left: 0,
                width: "100%",
                height: `${virtualRow.size}px`,
                transform: `translateY(${virtualRow.start}px)`,
              }}
            >
              <FileRow
                file={item.file}
                selectionKey={item.selectionKey}
                index={virtualRow.index}
                fileIndex={fileIndex}
                sectionId={item.sectionId}
                sectionName={item.sectionName}
                sectionType={item.sectionType}
                onFileClick={onFileClick}
                onAdd={
                  isChangesSection || isConflictSection ? onAdd : undefined
                }
                onUnstage={isStagedSection ? onUnstage : undefined}
                onDiscard={
                  isChangesSection || isConflictSection ? onDiscard : undefined
                }
                renderDiscard={
                  isChangesSection || isConflictSection
                    ? renderDiscard
                    : undefined
                }
                searchQuery={searchQuery}
                setSelectedFilePath={setSelectedFilePath}
                isSelected={isSelected || isMultiSelected}
                isFocused={isFocused}
                isGroupStart={isGroupStart}
                isGroupMiddle={isGroupMiddle}
                isGroupEnd={isGroupEnd}
                getContextActions={getContextActions}
                optionId={`${listId}-option-${fileIndex}`}
                ariaPosInSet={fileIndex + 1}
                ariaSetSize={listSize}
                onRequestFocus={focusContainer}
              />
            </div>
          );
        })}
      </div>
    </div>
  );
});

export default VirtualizedFileList;

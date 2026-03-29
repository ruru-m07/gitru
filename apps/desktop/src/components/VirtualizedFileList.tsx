import {
  type FileStatus,
  GetStatusResponse,
  openWithApp,
} from "@gitru/commands";
import { Badge } from "@gitru/ui/components/badge";
import { Button } from "@gitru/ui/components/button";
import * as contextMenu from "@gitru/ui/components/context-menu";
import { Label } from "@gitru/ui/components/label";
import { cn } from "@gitru/ui/lib/utils";
import { UseMutateAsyncFunction } from "@tanstack/react-query";
import { useVirtualizer } from "@tanstack/react-virtual";
import {
  BetweenHorizontalEnd,
  BetweenHorizontalStart,
  BetweenVerticalEnd,
  ChevronDown,
  ChevronRight,
  ClipboardCopy,
  CopyPlus,
  Diff,
  GitCommitHorizontal,
  Minus,
  Plus,
  Undo2,
} from "lucide-react";
import {
  memo,
  useCallback,
  useEffect,
  useId,
  useMemo,
  useRef,
  useState,
} from "react";
import { toast } from "sonner";
import { useFileSelectionStore } from "@/components/diff/useFileSelectionStore";
import { getStatusIcon } from "@/components/getStatusIcon";
import {
  type FileSelectionIdentity,
  selectActiveRepository,
  useAppStore,
} from "@/store/useAppStore";

export interface FileListSection {
  id: string;
  name: string;
  type?: "changes" | "staged" | "conflicted" | "stash" | "custom";
  files: GetStatusResponse["files"];
  actions?: {
    onAddAll?: () => Promise<unknown>;
    onUnstageAll?: () => Promise<unknown>;
    onDiscardAll?: () => void;
    renderDiscardAll?: () => React.ReactNode;
  };
}

export interface FileRowContextAction {
  id: string;
  label: string;
  icon?: React.ReactNode;
  onSelect: (file: FileStatus) => void | Promise<void>;
  destructive?: boolean;
  disabled?: boolean;
}

export interface VirtualizedFileListProps {
  sections: FileListSection[];
  sectionMode?: "accordion" | "flat";
  searchQuery?: string;
  onFileClick: (
    file: FileStatus,
    index: number,
    event: { shiftKey: boolean; metaKey: boolean; ctrlKey: boolean },
    selectionKey: string,
  ) => void;
  onAdd?: UseMutateAsyncFunction<string, string, string | string[], unknown>;
  onUnstage?: UseMutateAsyncFunction<
    string,
    string,
    string | string[],
    unknown
  >;
  onDiscard?: (filePath: string | string[]) => void;
  renderDiscard?: (filePath: string | string[]) => React.ReactNode;
  setSelectedFilePath: (file: FileSelectionIdentity | null) => void;
  getContextActions?: (context: {
    file: FileStatus;
    sectionId: string;
    sectionName: string;
    sectionType?: FileListSection["type"];
  }) => FileRowContextAction[];
  selectedFilePath?: {
    path: string;
    newPath?: string;
    scope?: FileListSection["type"];
  };
  className?: string;
  defaultExpandedSections?: string[];
}

type VirtualItem =
  | {
      type: "header";
      sectionId: string;
      sectionName: string;
      sectionType?: FileListSection["type"];
      count: number;
      actions?: FileListSection["actions"];
    }
  | {
      type: "file";
      file: FileStatus;
      selectionKey: string;
      sectionId: string;
      sectionName: string;
      sectionType?: FileListSection["type"];
    };

const ITEM_HEIGHT = 32;
const SECTION_HEADER_HEIGHT = 36;
type MatchRange = { start: number; end: number };
const EMPTY_CONTEXT_ACTIONS: FileRowContextAction[] = [];
const getFileTargets = (file: FileStatus) =>
  file.new_path ? Array.from(new Set([file.path, file.new_path])) : file.path;
const buildSelectionKey = (
  file: FileStatus,
  sectionId: string,
  sectionType?: FileListSection["type"],
) =>
  `${sectionType ?? "custom"}:${sectionId}:${file.path}:${file.new_path ?? ""}`;
const getWorktreeScope = (sectionType?: FileListSection["type"]) => {
  if (sectionType === "staged") return "staged";
  if (sectionType === "changes") return "unstaged";
  if (sectionType === "conflicted") return "conflicted";
  return undefined;
};

const hasRegexFlags = (flags: string) => /^[dgimsuvy]*$/.test(flags);

const ensureGlobalFlag = (flags: string) =>
  flags.includes("g") ? flags : `${flags}g`;

const escapeForRegex = (value: string) =>
  value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");

const findLiteralRanges = (value: string, query: string) => {
  const ranges: MatchRange[] = [];
  const lowerValue = value.toLowerCase();
  const lowerQuery = query.toLowerCase();
  let searchIndex = 0;

  while (searchIndex < value.length) {
    const index = lowerValue.indexOf(lowerQuery, searchIndex);
    if (index === -1) break;
    ranges.push({ start: index, end: index + query.length });
    searchIndex = index + query.length;
  }

  return ranges;
};

const buildPatternRegex = (query: string) => {
  const normalizedQuery = query.trim();
  if (!normalizedQuery || normalizedQuery === "*") return null;

  if (normalizedQuery.startsWith("/") && normalizedQuery.length > 1) {
    const lastSlashIndex = normalizedQuery.lastIndexOf("/");
    if (lastSlashIndex > 0) {
      const pattern = normalizedQuery.slice(1, lastSlashIndex);
      const flags = normalizedQuery.slice(lastSlashIndex + 1) || "i";
      if (hasRegexFlags(flags)) {
        return new RegExp(pattern, ensureGlobalFlag(flags));
      }
    }
  }

  try {
    return new RegExp(normalizedQuery, "gi");
  } catch {
    try {
      const escapedGlob = normalizedQuery
        .replace(/[.+^${}()|[\]\\]/g, "\\$&")
        .replace(/\*/g, ".*")
        .replace(/\?/g, ".");
      return new RegExp(escapedGlob, "gi");
    } catch {
      return new RegExp(escapeForRegex(normalizedQuery), "gi");
    }
  }
};

const findPatternRanges = (value: string, regex: RegExp) => {
  const ranges: MatchRange[] = [];
  regex.lastIndex = 0;
  let match = regex.exec(value);

  while (match) {
    const matchedText = match[0];
    if (!matchedText) {
      regex.lastIndex += 1;
      match = regex.exec(value);
      continue;
    }

    ranges.push({
      start: match.index,
      end: match.index + matchedText.length,
    });
    match = regex.exec(value);
  }

  return ranges;
};

const getMatchRanges = (value: string, query: string) => {
  const normalizedQuery = query.trim();
  if (!normalizedQuery || normalizedQuery === "*") return [];

  const literalRanges = findLiteralRanges(value, normalizedQuery);
  if (literalRanges.length > 0) {
    return literalRanges;
  }

  const patternRegex = buildPatternRegex(normalizedQuery);
  if (!patternRegex) return [];
  return findPatternRanges(value, patternRegex);
};

const renderHighlightedSlice = (
  value: string,
  ranges: MatchRange[],
  offset: number,
) => {
  if (!value.length || ranges.length === 0) return value;
  const nodes: React.ReactNode[] = [];
  let cursor = offset;
  const segmentEnd = offset + value.length;

  for (const range of ranges) {
    const start = Math.max(range.start, offset);
    const end = Math.min(range.end, segmentEnd);
    if (start >= end) continue;

    if (cursor < start) {
      nodes.push(
        <span key={`text-${cursor}`}>
          {value.slice(cursor - offset, start - offset)}
        </span>,
      );
    }
    nodes.push(
      <mark
        key={`match-${start}`}
        className="bg-black/15 dark:bg-yellow-400/30 text-foreground rounded-[2px]"
      >
        {value.slice(start - offset, end - offset)}
      </mark>,
    );
    cursor = end;
  }

  if (cursor < segmentEnd) {
    nodes.push(
      <span key={`text-${cursor}`}>{value.slice(cursor - offset)}</span>,
    );
  }

  return nodes;
};

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
  const [isContainerFocused, setIsContainerFocused] = useState(false);
  const selectedFiles = useFileSelectionStore((state) => state.selectedFiles);
  const focusedIndex = useFileSelectionStore((state) => state.focusedIndex);
  const setAllFiles = useFileSelectionStore((state) => state.setAllFiles);
  const setFocusedIndex = useFileSelectionStore(
    (state) => state.setFocusedIndex,
  );
  const clearSelection = useFileSelectionStore((state) => state.clearSelection);

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
    useMemo(() => {
      const result: VirtualItem[] = [];
      const fileItems: Array<{
        key: string;
        file: FileStatus;
        sectionType?: FileListSection["type"];
      }> = [];
      const fileIndexByKey = new Map<string, number>();
      const fileIndexToItemIndex: number[] = [];

      for (const section of sections) {
        if (section.files.length === 0) continue;

        if (sectionMode === "flat") {
          for (const file of section.files) {
            const selectionKey = buildSelectionKey(
              file,
              section.id,
              section.type,
            );
            result.push({
              type: "file",
              file,
              selectionKey,
              sectionId: section.id,
              sectionName: section.name,
              sectionType: section.type,
            });
            fileIndexByKey.set(selectionKey, fileItems.length);
            fileIndexToItemIndex.push(result.length - 1);
            fileItems.push({
              key: selectionKey,
              file,
              sectionType: section.type,
            });
          }
          continue;
        }

        result.push({
          type: "header",
          sectionId: section.id,
          sectionName: section.name,
          sectionType: section.type,
          count: section.files.length,
          actions: section.actions,
        });

        if (expandedSections.has(section.id)) {
          for (const file of section.files) {
            const selectionKey = buildSelectionKey(
              file,
              section.id,
              section.type,
            );
            result.push({
              type: "file",
              file,
              selectionKey,
              sectionId: section.id,
              sectionName: section.name,
              sectionType: section.type,
            });
            fileIndexByKey.set(selectionKey, fileItems.length);
            fileIndexToItemIndex.push(result.length - 1);
            fileItems.push({
              key: selectionKey,
              file,
              sectionType: section.type,
            });
          }
        }
      }

      return {
        items: result,
        fileItems,
        fileIndexByKey,
        fileIndexToItemIndex,
      };
    }, [sections, expandedSections, sectionMode]);

  useEffect(() => {
    setAllFiles(fileItems);
  }, [fileItems, setAllFiles]);

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

  const focusContainer = useCallback(() => {
    parentRef.current?.focus();
  }, []);

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
        const { file, key: selectionKey } = entry;

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
              worktreeScope: getWorktreeScope(entry.sectionType),
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
        const { file, key: selectionKey } = entry;

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
            worktreeScope: getWorktreeScope(entry.sectionType),
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

interface SectionHeaderProps {
  sectionId: string;
  sectionName: string;
  sectionType?: FileListSection["type"];
  count: number;
  isExpanded: boolean;
  onToggle: (sectionId: string) => void;
  actions?: FileListSection["actions"];
}

const SectionHeader = memo(function SectionHeader({
  sectionId,
  sectionName,
  sectionType,
  count,
  isExpanded,
  onToggle,
  actions,
}: SectionHeaderProps) {
  const isChangesSection = sectionType === "changes";
  const isStagedSection = sectionType === "staged";
  const isConflictSection = sectionType === "conflicted";

  return (
    <div className="sticky top-0 z-20">
      <div className="flex items-center hover:bg-accent/50 justify-between w-full pr-2">
        <button
          type="button"
          className="flex items-center gap-1.5 rounded pl-2.5 py-2 cursor-pointer flex-1 justify-start"
          onClick={() => onToggle(sectionId)}
        >
          {isExpanded ? (
            <ChevronDown size={16} className="text-muted-foreground" />
          ) : (
            <ChevronRight size={16} className="text-muted-foreground" />
          )}
          <span className="text-sm font-medium">{sectionName}</span>
        </button>
        <div className="flex items-center gap-1">
          {(isChangesSection || isConflictSection) && (
            <div className="flex items-center">
              {/* Use custom render function if provided, otherwise use default button */}
              {actions?.renderDiscardAll
                ? actions.renderDiscardAll()
                : actions?.onDiscardAll && (
                    <Button
                      onClick={(e) => {
                        e.stopPropagation();
                        actions.onDiscardAll?.();
                      }}
                      variant="ghost"
                      size="icon-sm"
                    >
                      <Undo2 size={18} strokeWidth={1.25} />
                    </Button>
                  )}
              {actions?.onAddAll && (
                <Button
                  onClick={async (e) => {
                    e.stopPropagation();
                    await actions.onAddAll?.();
                  }}
                  variant="ghost"
                  size="icon-sm"
                >
                  <Plus size={18} strokeWidth={1.25} />
                </Button>
              )}
            </div>
          )}
          {isStagedSection && actions?.onUnstageAll && (
            <Button
              variant="ghost"
              size="icon-sm"
              onClick={async (e) => {
                e.stopPropagation();
                await actions.onUnstageAll?.();
              }}
            >
              <Minus size={18} strokeWidth={1.25} />
            </Button>
          )}
          <Badge variant="secondary" className="tabular-nums font-mono text-xs">
            {count}
          </Badge>
        </div>
      </div>
    </div>
  );
});

interface FileRowProps {
  file: FileStatus;
  selectionKey: string;
  searchQuery: string;
  index: number;
  fileIndex: number;
  sectionId: string;
  sectionName: string;
  sectionType?: FileListSection["type"];
  onFileClick: (
    file: FileStatus,
    index: number,
    event: { shiftKey: boolean; metaKey: boolean; ctrlKey: boolean },
    selectionKey: string,
  ) => void;
  onAdd?: UseMutateAsyncFunction<string, string, string | string[], unknown>;
  onUnstage?: UseMutateAsyncFunction<
    string,
    string,
    string | string[],
    unknown
  >;
  onDiscard?: (filePath: string | string[]) => void;
  renderDiscard?: (filePath: string | string[]) => React.ReactNode;
  setSelectedFilePath: (file: FileSelectionIdentity | null) => void;
  isSelected: boolean;
  isFocused: boolean;
  isGroupStart: boolean;
  isGroupMiddle: boolean;
  isGroupEnd: boolean;
  getContextActions?: VirtualizedFileListProps["getContextActions"];
  optionId: string;
  ariaPosInSet: number;
  ariaSetSize: number;
  onRequestFocus: () => void;
}

const FileRow = memo(
  function FileRow({
    file,
    selectionKey,
    searchQuery,
    index,
    fileIndex,
    sectionId,
    sectionName,
    sectionType,
    onFileClick,
    onAdd,
    onUnstage,
    onDiscard,
    renderDiscard,
    setSelectedFilePath,
    isSelected,
    isFocused,
    isGroupStart,
    isGroupMiddle,
    isGroupEnd,
    getContextActions,
    optionId,
    ariaPosInSet,
    ariaSetSize,
    onRequestFocus,
  }: FileRowProps) {
    const path = file.path;
    const fileTargets = getFileTargets(file);
    const lastSlashIndex = path.lastIndexOf("/");
    const hasDirectory = lastSlashIndex !== -1;
    const directoryPath = hasDirectory ? path.slice(0, lastSlashIndex) : "";
    const fileName = hasDirectory ? path.slice(lastSlashIndex + 1) : path;
    const matchRanges = useMemo(
      () => getMatchRanges(path, searchQuery),
      [path, searchQuery],
    );

    const contextActions = useMemo(
      () =>
        getContextActions?.({
          file,
          sectionId,
          sectionName,
          sectionType,
        }) ?? EMPTY_CONTEXT_ACTIONS,
      [file, getContextActions, sectionId, sectionName, sectionType],
    );

    const activeRepository = useAppStore(selectActiveRepository);
    const getSelectionTargets = useCallback(() => {
      const { selectedFiles, allFiles } = useFileSelectionStore.getState();

      if (!isSelected || selectedFiles.size <= 1) {
        return Array.isArray(fileTargets) ? fileTargets : [fileTargets];
      }

      const targets: string[] = [];

      for (const selectedEntry of allFiles) {
        if (!selectedFiles.has(selectedEntry.key)) continue;
        if (selectedEntry.sectionType !== sectionType) continue;
        const targetsForFile = getFileTargets(selectedEntry.file);
        if (Array.isArray(targetsForFile)) {
          targets.push(...targetsForFile);
        } else {
          targets.push(targetsForFile);
        }
      }

      const uniqueTargets = Array.from(new Set(targets));
      if (uniqueTargets.length > 0) {
        return uniqueTargets;
      }

      return Array.isArray(fileTargets) ? fileTargets : [fileTargets];
    }, [fileTargets, isSelected]);

    return (
      <contextMenu.ContextMenu>
        <contextMenu.ContextMenuTrigger
          className={cn(
            "group",
            "border-y-transparent",
            "dark:[&[data-state=open]>div]:bg-blue-900/50!",
            "[&[data-state=open]>div]:bg-blue-50!",
            "dark:[&[data-state=open]>div]:bg-blue-900/50!",
            "[&[data-state=open]>span]:bg-blue-400!",
          )}
          asChild
        >
          <div
            data-index={index}
            id={optionId}
            role="option"
            aria-selected={isSelected}
            aria-posinset={ariaPosInSet}
            aria-setsize={ariaSetSize}
            data-focused={isFocused ? "true" : "false"}
            className={cn(
              "[--pattern-fg:color-mix(in_srgb,var(--primary)_20%,transparent)] transition-none flex relative select-none cursor-pointer items-center h-full hover:bg-muted",
              "data-[focused=true]:bg-muted-foreground/20!",
              isSelected &&
                "bg-muted-foreground/10 hover:bg-muted-foreground/15",
            )}
            onClick={(e) => {
              onRequestFocus();
              if (fileIndex < 0) return;
              onFileClick(
                file,
                fileIndex,
                {
                  shiftKey: e.shiftKey,
                  metaKey: e.metaKey,
                  ctrlKey: e.ctrlKey,
                },
                selectionKey,
              );

              if (!e.shiftKey && !e.metaKey && !e.ctrlKey) {
                setSelectedFilePath({
                  filePath: file.path,
                  fileNewPath: file.new_path,
                  source: "worktree",
                  worktreeScope: getWorktreeScope(sectionType),
                  selectedAt: Date.now(),
                });
              }
            }}
            onDoubleClick={async () => {
              if (!file.path) return;
              await openWithApp({
                filePath: `${activeRepository?.path}/${file.new_path || file.path}`,
              });
            }}
          >
            {isSelected && (
              <span
                className="absolute -left-1 rounded-md w-1.75 bg-primary z-50!"
                style={
                  isGroupMiddle
                    ? { top: 0, height: "100%", borderRadius: "0" }
                    : isGroupStart
                      ? {
                          top: "25%",
                          height: "75%",
                          WebkitBorderBottomLeftRadius: "0",
                          WebkitBorderBottomRightRadius: "0",
                        }
                      : isGroupEnd
                        ? {
                            top: 0,
                            height: "75%",
                            WebkitBorderTopLeftRadius: "0",
                            WebkitBorderTopRightRadius: "0",
                          }
                        : {
                            top: "50%",
                            height: "16px",
                            transform: "translateY(-50%)",
                          }
                }
              />
            )}
            <div
              data-slot="file-row"
              className="flex items-center w-full min-w-0 pl-2 pr-0.5 py-0.5"
            >
              <div className="shrink-0">{getStatusIcon(file.status, 18)}</div>
              <div className="flex items-center ml-1.5 min-w-0 flex-1">
                <Label className="flex cursor-pointer items-center min-w-0 text-sm w-full gap-0 font-[450]">
                  <span className="inline-flex w-fit max-w-full min-w-0 items-center gap-0">
                    {hasDirectory && (
                      <>
                        <span className="text-muted-foreground min-w-0 flex-1 truncate">
                          {renderHighlightedSlice(
                            directoryPath,
                            matchRanges,
                            0,
                          )}
                        </span>
                        <span className="text-muted-foreground shrink-0">
                          /
                        </span>
                      </>
                    )}
                    <span className="text-foreground! min-w-0 shrink truncate">
                      {renderHighlightedSlice(
                        fileName,
                        matchRanges,
                        hasDirectory ? lastSlashIndex + 1 : 0,
                      )}
                    </span>
                  </span>
                </Label>
              </div>
              {onAdd && (
                <div className="flex ml-2 shrink-0">
                  {renderDiscard
                    ? renderDiscard(fileTargets)
                    : onDiscard && (
                        <Button
                          onClick={(e) => {
                            e.stopPropagation();
                            onDiscard(fileTargets);
                          }}
                          size="icon-sm"
                          variant="ghost"
                        >
                          <Undo2 size={18} strokeWidth={1.25} />
                        </Button>
                      )}
                  <Button
                    onClick={async (e) => {
                      e.stopPropagation();
                      const success = await onAdd(fileTargets);
                      if (success) {
                        toast.success("File staged");
                      } else {
                        toast.error("Failed to stage file");
                      }
                    }}
                    size="icon-sm"
                    variant="ghost"
                  >
                    <Plus size={18} strokeWidth={1.25} />
                  </Button>
                </div>
              )}
              {onUnstage && (
                <div className="flex ml-2 shrink-0">
                  <Button
                    onClick={async (e) => {
                      e.stopPropagation();
                      const success = await onUnstage(fileTargets);
                      if (success) {
                        toast.success("File unstaged");
                      } else {
                        toast.error("Failed to unstage file");
                      }
                    }}
                    size="icon-sm"
                    variant="ghost"
                  >
                    <Minus size={18} strokeWidth={1.25} />
                  </Button>
                </div>
              )}
            </div>
          </div>
        </contextMenu.ContextMenuTrigger>
        <contextMenu.ContextMenuContent className="w-52">
          <contextMenu.ContextMenuLabel>
            <div className="flex items-center gap-2">
              {getStatusIcon(file.status)}
              <span className="truncate font-normal">
                {file.path.split("/").slice(-1)[0]}
              </span>
            </div>
          </contextMenu.ContextMenuLabel>
          <contextMenu.ContextMenuSeparator />
          {isSelected && (
            <>
              {onAdd && (
                <contextMenu.ContextMenuItem
                  onSelect={async () => {
                    const selectedTargets = getSelectionTargets();
                    const success = await onAdd(selectedTargets);
                    if (success) {
                      toast.success("Selected files staged");
                    } else {
                      toast.error("Failed to stage selected files");
                    }
                  }}
                >
                  <Plus size={16} className="mr-2" />
                  Stage Selected
                </contextMenu.ContextMenuItem>
              )}
              {onUnstage && (
                <contextMenu.ContextMenuItem
                  onSelect={async () => {
                    const selectedTargets = getSelectionTargets();
                    const success = await onUnstage(selectedTargets);
                    if (success) {
                      toast.success("Selected files unstaged");
                    } else {
                      toast.error("Failed to unstage selected files");
                    }
                  }}
                >
                  <Minus size={16} className="mr-2" />
                  Unstage Selected
                </contextMenu.ContextMenuItem>
              )}
              {onDiscard && (
                <contextMenu.ContextMenuItem
                  className="hover:text-destructive! hover:bg-destructive/10!"
                  onSelect={() => {
                    onDiscard(getSelectionTargets());
                  }}
                >
                  <Undo2 size={16} className="mr-2" />
                  Discard Selected
                </contextMenu.ContextMenuItem>
              )}
              <contextMenu.ContextMenuSeparator />
            </>
          )}
          {contextActions.map((action) => (
            <contextMenu.ContextMenuItem
              key={action.id}
              disabled={action.disabled}
              className={cn(
                action.destructive &&
                  "hover:text-destructive! hover:bg-destructive/10!",
              )}
              onSelect={async () => {
                await action.onSelect(file);
              }}
            >
              {action.icon ? (
                <span className="mr-2 inline-flex">{action.icon}</span>
              ) : null}
              {action.label}
            </contextMenu.ContextMenuItem>
          ))}
          {contextActions.length > 0 && <contextMenu.ContextMenuSeparator />}
          {onAdd && (
            <contextMenu.ContextMenuItem
              onSelect={async () => {
                const success = await onAdd(fileTargets);
                if (success) {
                  toast.success("File staged");
                } else {
                  toast.error("Failed to stage file");
                }
              }}
            >
              <Plus size={16} className="mr-2" />
              Stage Changes
            </contextMenu.ContextMenuItem>
          )}
          {onUnstage && (
            <contextMenu.ContextMenuItem
              onSelect={async () => {
                const success = await onUnstage(fileTargets);
                if (success) {
                  toast.success("File unstaged");
                } else {
                  toast.error("Failed to unstage file");
                }
              }}
            >
              <Minus size={16} className="mr-2" />
              Unstage Changes
            </contextMenu.ContextMenuItem>
          )}
          <contextMenu.ContextMenuItem
            onSelect={() => {
              setSelectedFilePath({
                filePath: file.path,
                fileNewPath: file.new_path,
                source: "worktree",
                worktreeScope: getWorktreeScope(sectionType),
                selectedAt: Date.now(),
              });
            }}
          >
            <Diff size={16} className="mr-2" />
            Open Diff
          </contextMenu.ContextMenuItem>
          <contextMenu.ContextMenuSeparator />
          <contextMenu.ContextMenuItem>
            <ClipboardCopy size={16} className="mr-2" />
            Copy Relative Path
          </contextMenu.ContextMenuItem>
          <contextMenu.ContextMenuItem>
            <BetweenVerticalEnd size={16} className="mr-2" />
            Copy Diff Hunk
          </contextMenu.ContextMenuItem>
          <contextMenu.ContextMenuItem>
            <BetweenHorizontalEnd size={16} className="mr-2" />
            Copy Old File Contents
          </contextMenu.ContextMenuItem>
          <contextMenu.ContextMenuItem>
            <BetweenHorizontalStart size={16} className="mr-2" />
            Copy New File Contents
          </contextMenu.ContextMenuItem>
          <contextMenu.ContextMenuSeparator />
          <contextMenu.ContextMenuItem>
            <GitCommitHorizontal size={16} className="mr-2" />
            Quick Commit
          </contextMenu.ContextMenuItem>
          <contextMenu.ContextMenuItem>
            <CopyPlus size={16} className="mr-2" />
            Amend Commit
          </contextMenu.ContextMenuItem>
          {onDiscard && (
            <>
              <contextMenu.ContextMenuSeparator />
              <contextMenu.ContextMenuItem
                className="hover:text-destructive! hover:bg-destructive/10!"
                onSelect={() => {
                  onDiscard(fileTargets);
                }}
              >
                <Undo2 size={16} className="mr-2" />
                Discard Changes
              </contextMenu.ContextMenuItem>
            </>
          )}
        </contextMenu.ContextMenuContent>
      </contextMenu.ContextMenu>
    );
  },
  (prevProps, nextProps) => {
    return (
      prevProps.file === nextProps.file &&
      prevProps.searchQuery === nextProps.searchQuery &&
      prevProps.index === nextProps.index &&
      prevProps.fileIndex === nextProps.fileIndex &&
      prevProps.sectionId === nextProps.sectionId &&
      prevProps.sectionName === nextProps.sectionName &&
      prevProps.sectionType === nextProps.sectionType &&
      Boolean(prevProps.onAdd) === Boolean(nextProps.onAdd) &&
      Boolean(prevProps.onUnstage) === Boolean(nextProps.onUnstage) &&
      Boolean(prevProps.onDiscard) === Boolean(nextProps.onDiscard) &&
      Boolean(prevProps.renderDiscard) === Boolean(nextProps.renderDiscard) &&
      prevProps.isSelected === nextProps.isSelected &&
      prevProps.isFocused === nextProps.isFocused &&
      prevProps.isGroupStart === nextProps.isGroupStart &&
      prevProps.isGroupMiddle === nextProps.isGroupMiddle &&
      prevProps.isGroupEnd === nextProps.isGroupEnd &&
      prevProps.optionId === nextProps.optionId &&
      prevProps.ariaPosInSet === nextProps.ariaPosInSet &&
      prevProps.ariaSetSize === nextProps.ariaSetSize &&
      prevProps.onRequestFocus === nextProps.onRequestFocus &&
      prevProps.getContextActions === nextProps.getContextActions
    );
  },
);

export default VirtualizedFileList;

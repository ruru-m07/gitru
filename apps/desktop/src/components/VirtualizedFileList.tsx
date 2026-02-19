import { type FileStatus, GetStatusResponse } from "@gitru/commands";
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
import { memo, useCallback, useMemo, useRef, useState } from "react";
import { toast } from "sonner";

import { getStatusIcon } from "@/components/getStatusIcon";
import { SelectedFile } from "@/store/useAppStore";

export interface FileListSection {
  id: string;
  name: string;
  files: GetStatusResponse["files"];
  actions?: {
    onAddAll?: () => Promise<unknown>;
    onUnstageAll?: () => Promise<unknown>;
    onDiscardAll?: () => void;
    renderDiscardAll?: () => React.ReactNode;
  };
}

export interface VirtualizedFileListProps {
  sections: FileListSection[];
  searchQuery?: string;
  onFileClick: (
    file: FileStatus,
    index: number,
    event: { shiftKey: boolean; metaKey: boolean; ctrlKey: boolean },
  ) => void;
  onAdd?: UseMutateAsyncFunction<string, string, string, unknown>;
  onUnstage?: UseMutateAsyncFunction<string, string, string, unknown>;
  onDiscard?: (filePath: string) => void;
  renderDiscard?: (filePath: string) => React.ReactNode;
  setSelectedFilePath: (file: SelectedFile | null) => void;
  selectedFilePath?: {
    path: string;
    newPath?: string;
  };
  className?: string;
  defaultExpandedSections?: string[];
}

type VirtualItem =
  | {
      type: "header";
      sectionId: string;
      sectionName: string;
      count: number;
      actions?: FileListSection["actions"];
    }
  | {
      type: "file";
      file: FileStatus;
      sectionId: string;
      sectionName: string;
    };

const ITEM_HEIGHT = 32;
const SECTION_HEADER_HEIGHT = 36;
type MatchRange = { start: number; end: number };

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
  searchQuery = "",
  onFileClick,
  onAdd,
  onUnstage,
  onDiscard,
  renderDiscard,
  setSelectedFilePath,
  selectedFilePath,
  className,
  defaultExpandedSections,
}: VirtualizedFileListProps) {
  const parentRef = useRef<HTMLDivElement>(null);

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

  const items = useMemo(() => {
    const result: VirtualItem[] = [];

    for (const section of sections) {
      if (section.files.length === 0) continue;

      result.push({
        type: "header",
        sectionId: section.id,
        sectionName: section.name,
        count: section.files.length,
        actions: section.actions,
      });

      if (expandedSections.has(section.id)) {
        for (const file of section.files) {
          result.push({
            type: "file",
            file,
            sectionId: section.id,
            sectionName: section.name,
          });
        }
      }
    }

    return result;
  }, [sections, expandedSections]);

  const virtualizer = useVirtualizer({
    count: items.length,
    getScrollElement: () => parentRef.current,
    estimateSize: (index) =>
      items[index].type === "header" ? SECTION_HEADER_HEIGHT : ITEM_HEIGHT,
    overscan: 15,
  });

  const virtualItems = virtualizer.getVirtualItems();

  return (
    <div ref={parentRef} className={cn("h-full overflow-auto", className)}>
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
                  count={item.count}
                  isExpanded={isExpanded}
                  onToggle={toggleSection}
                  actions={item.actions}
                />
              </div>
            );
          }

          const isSelected = selectedFilePath?.path === item.file.path;
          const isChangesSection =
            item.sectionName === "Changes" ||
            item.sectionName === "Unstaged Changes";
          const isStagedSection =
            item.sectionName === "Staged Changes" ||
            item.sectionName === "Staged";
          const isConflictSection = item.sectionName === "Conflicted";

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
                index={virtualRow.index}
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
                isSelected={isSelected}
                selectedFilePath={selectedFilePath}
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
  count: number;
  isExpanded: boolean;
  onToggle: (sectionId: string) => void;
  actions?: FileListSection["actions"];
}

const SectionHeader = memo(function SectionHeader({
  sectionId,
  sectionName,
  count,
  isExpanded,
  onToggle,
  actions,
}: SectionHeaderProps) {
  const isChangesSection =
    sectionName === "Changes" || sectionName === "Unstaged Changes";
  const isStagedSection =
    sectionName === "Staged Changes" || sectionName === "Staged";
  const isConflictSection = sectionName === "Conflicted";

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
  searchQuery: string;
  index: number;
  onFileClick: (
    file: FileStatus,
    index: number,
    event: { shiftKey: boolean; metaKey: boolean; ctrlKey: boolean },
  ) => void;
  onAdd?: UseMutateAsyncFunction<string, string, string, unknown>;
  onUnstage?: UseMutateAsyncFunction<string, string, string, unknown>;
  onDiscard?: (filePath: string) => void;
  renderDiscard?: (filePath: string) => React.ReactNode;
  setSelectedFilePath: (file: SelectedFile | null) => void;
  isSelected: boolean;
  selectedFilePath?: {
    path: string;
    newPath?: string;
  };
}

const FileRow = memo(
  function FileRow({
    file,
    searchQuery,
    index,
    onFileClick,
    onAdd,
    onUnstage,
    onDiscard,
    renderDiscard,
    setSelectedFilePath,
    isSelected,
    selectedFilePath: _selectedFilePath,
  }: FileRowProps) {
    const path = file.path;
    const lastSlashIndex = path.lastIndexOf("/");
    const hasDirectory = lastSlashIndex !== -1;
    const directoryPath = hasDirectory ? path.slice(0, lastSlashIndex) : "";
    const fileName = hasDirectory ? path.slice(lastSlashIndex + 1) : path;
    const matchRanges = getMatchRanges(path, searchQuery);

    return (
      <contextMenu.ContextMenu>
        <contextMenu.ContextMenuTrigger
          className={cn(
            `dark:[&[data-state=open]>div]:bg-blue-900/50! [&[data-state=open]>div]:bg-blue-50! border-y-transparent [&[data-state=open]>div]:border-y [&[data-state=open]>div]:border-y-blue-400! [&[data-state=open]>div]:border-dashed!`,
          )}
          asChild
        >
          <div
            data-index={index}
            className={cn(
              "[--pattern-fg:color-mix(in_srgb,var(--primary)_20%,transparent)] flex relative select-none cursor-pointer hover:bg-muted items-center h-full",
              isSelected && "bg-secondary hover:bg-muted-foreground/15!",
            )}
            onClick={(e) => {
              onFileClick(file, index, {
                shiftKey: e.shiftKey,
                metaKey: e.metaKey,
                ctrlKey: e.ctrlKey,
              });

              if (!e.shiftKey && !e.metaKey && !e.ctrlKey) {
                setSelectedFilePath({
                  filePath: file.path,
                  fileNewPath: file.new_path,
                  status: file.status,
                });
              }
            }}
            onDoubleClick={() => {}}
          >
            {isSelected && (
              <div className="absolute top-1/2 -translate-y-1/2 -left-1 rounded-md w-1.75 bg-primary h-4" />
            )}
            <div className="flex items-center w-full min-w-0 pl-2 pr-0.5 py-0.5">
              <div className="shrink-0">{getStatusIcon(file.status, 18)}</div>
              <div className="flex items-center ml-1.5 min-w-0 flex-1">
                <Label className="flex cursor-pointer items-center min-w-0 text-sm w-full gap-0">
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
                    ? renderDiscard(file.path)
                    : onDiscard && (
                        <Button
                          onClick={(e) => {
                            e.stopPropagation();
                            onDiscard(file.path);
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
                      const success = await onAdd(file.path);
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
                      const success = await onUnstage(file.path);
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
          <contextMenu.ContextMenuItem>
            <Plus size={16} className="mr-2" />
            Stage Changes
          </contextMenu.ContextMenuItem>
          <contextMenu.ContextMenuItem>
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
          <contextMenu.ContextMenuSeparator />
          <contextMenu.ContextMenuItem className="hover:text-destructive! hover:bg-destructive/10!">
            <Undo2 size={16} className="mr-2" />
            Discard Changes
          </contextMenu.ContextMenuItem>
        </contextMenu.ContextMenuContent>
      </contextMenu.ContextMenu>
    );
  },
  (prevProps, nextProps) => {
    return (
      prevProps.file === nextProps.file &&
      prevProps.searchQuery === nextProps.searchQuery &&
      prevProps.index === nextProps.index &&
      prevProps.onAdd === nextProps.onAdd &&
      prevProps.onUnstage === nextProps.onUnstage &&
      prevProps.onDiscard === nextProps.onDiscard &&
      prevProps.renderDiscard === nextProps.renderDiscard &&
      prevProps.isSelected === nextProps.isSelected &&
      prevProps.selectedFilePath?.path === nextProps.selectedFilePath?.path &&
      prevProps.selectedFilePath?.newPath ===
        nextProps.selectedFilePath?.newPath
    );
  },
);

export default VirtualizedFileList;

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
  onFileClick: (
    file: FileStatus,
    index: number,
    event: { shiftKey: boolean; metaKey: boolean; ctrlKey: boolean },
  ) => void;
  onAdd?: UseMutateAsyncFunction<
    { success: boolean; message?: string },
    string,
    string,
    unknown
  >;
  onUnstage?: UseMutateAsyncFunction<
    { success: boolean; message?: string },
    string,
    string,
    unknown
  >;
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

export const VirtualizedFileList = memo(function VirtualizedFileList({
  sections,
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
                onAdd={isChangesSection ? onAdd : undefined}
                onUnstage={isStagedSection ? onUnstage : undefined}
                onDiscard={isChangesSection ? onDiscard : undefined}
                renderDiscard={isChangesSection ? renderDiscard : undefined}
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
          {isChangesSection && (
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
  index: number;
  onFileClick: (
    file: FileStatus,
    index: number,
    event: { shiftKey: boolean; metaKey: boolean; ctrlKey: boolean },
  ) => void;
  onAdd?: UseMutateAsyncFunction<
    { success: boolean; message?: string },
    string,
    string,
    unknown
  >;
  onUnstage?: UseMutateAsyncFunction<
    { success: boolean; message?: string },
    string,
    string,
    unknown
  >;
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
    return (
      <contextMenu.ContextMenu>
        <contextMenu.ContextMenuTrigger
          className={cn(
            `dark:[&[data-state=open]>div]:bg-blue-900/50! [&[data-state=open]>div]:bg-blue-50! [&[data-state=open]>div]:border [&[data-state=open]>div]:border-y-blue-400! [&[data-state=open]>div]:border-dashed! [&[data-state=open]>div]:border-l-border!`,
          )}
          asChild
        >
          <div
            data-index={index}
            className={cn(
              "[--pattern-fg:color-mix(in_srgb,var(--primary)_20%,transparent)] flex relative select-none cursor-pointer hover:bg-muted border border-transparent hover:border hover:border-l hover:border-l-border items-center pl-2 pr-0.5 py-0.5 h-full",
              isSelected &&
                "bg-muted-foreground/10! hover:bg-muted-foreground/15!",
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
          >
            {isSelected && (
              <div className="absolute top-1/2 -translate-y-1/2 -left-1 rounded-md w-2 bg-primary h-6" />
            )}
            <div className="flex items-center w-full min-w-0">
              <div className="shrink-0">{getStatusIcon(file.status, 18)}</div>
              <div className="flex items-center ml-1.5 min-w-0 flex-1">
                <Label className="flex cursor-pointer items-center min-w-0 text-sm w-full gap-0">
                  {file?.path.split("/").slice(0, -1).join("/") && (
                    <>
                      <span className="text-muted-foreground truncate">
                        {file.path.split("/").slice(0, -1).join("/")}
                      </span>
                      <span className="text-muted-foreground">/</span>
                    </>
                  )}
                  <span className="shrink-0 text-foreground!">
                    {file?.path.split("/").slice(-1)[0]}
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

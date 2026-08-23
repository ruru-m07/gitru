import { openWithApp } from "@gitru/commands";
import { Button } from "@gitru/ui/components/button";
import * as contextMenu from "@gitru/ui/components/context-menu";
import { Label } from "@gitru/ui/components/label";
import { cn } from "@gitru/ui/lib/utils";
import {
  BetweenHorizontalEnd,
  BetweenHorizontalStart,
  BetweenVerticalEnd,
  ClipboardCopy,
  CopyPlus,
  Diff,
  GitCommitHorizontal,
  Minus,
  Plus,
  Undo2,
} from "lucide-react";
import { memo, useCallback, useMemo } from "react";
import { toast } from "sonner";

import { useFileSelectionStore } from "@/components/diff/use-file-selection-store";
import { getStatusIcon } from "@/components/get-status-icon";
import { selectActiveRepository, useAppStore } from "@/store/use-app-store";

import { FileRowProps } from "./types";
import {
  EMPTY_CONTEXT_ACTIONS,
  getMatchRanges,
  getStageFileTargets,
  getUnstageFileTargets,
  getWorktreeScope,
  renderHighlightedSlice,
} from "./utils";

export const FileRow = memo(
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
    const stageTargets = getStageFileTargets(file);
    const unstageTargets = getUnstageFileTargets(file);
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
    const getStageSelectionTargets = useCallback(() => {
      const { selectedFiles, allFiles } = useFileSelectionStore.getState();

      if (!isSelected || selectedFiles.size <= 1) {
        return [stageTargets];
      }

      const targets: string[] = [];

      for (const selectedEntry of allFiles) {
        if (!selectedFiles.has(selectedEntry.key)) continue;
        if (selectedEntry.sectionType !== sectionType) continue;
        targets.push(getStageFileTargets(selectedEntry.file));
      }

      const uniqueTargets = Array.from(new Set(targets));
      if (uniqueTargets.length > 0) {
        return uniqueTargets;
      }

      return [stageTargets];
    }, [isSelected, sectionType, stageTargets]);

    const getUnstageSelectionTargets = useCallback(() => {
      const { selectedFiles, allFiles } = useFileSelectionStore.getState();

      if (!isSelected || selectedFiles.size <= 1) {
        return unstageTargets;
      }

      const targets: string[] = [];

      for (const selectedEntry of allFiles) {
        if (!selectedFiles.has(selectedEntry.key)) continue;
        if (selectedEntry.sectionType !== sectionType) continue;
        targets.push(...getUnstageFileTargets(selectedEntry.file));
      }

      const uniqueTargets = Array.from(new Set(targets));
      if (uniqueTargets.length > 0) {
        return uniqueTargets;
      }

      return unstageTargets;
    }, [isSelected, sectionType, unstageTargets]);

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
                    ? renderDiscard(stageTargets)
                    : onDiscard && (
                        <Button
                          onClick={(e) => {
                            e.stopPropagation();
                            onDiscard(stageTargets);
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
                      const success = await onAdd(stageTargets);
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
                      const success = await onUnstage(unstageTargets);
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
                    const selectedTargets = getStageSelectionTargets();
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
                    const selectedTargets = getUnstageSelectionTargets();
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
                    onDiscard(getStageSelectionTargets());
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
                const success = await onAdd(stageTargets);
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
                const success = await onUnstage(unstageTargets);
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
                  onDiscard(stageTargets);
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

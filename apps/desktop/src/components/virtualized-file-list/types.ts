import { GetStatusResponse } from "@gitru/commands";
import { UseMutateAsyncFunction } from "@tanstack/react-query";

import { FileSelectionIdentity } from "@/types/store";

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
  onSelect: (
    file: import("@gitru/commands").FileStatus,
  ) => void | Promise<void>;
  destructive?: boolean;
  disabled?: boolean;
}

export interface VirtualizedFileListProps {
  sections: FileListSection[];
  sectionMode?: "accordion" | "flat";
  searchQuery?: string;
  onFileClick: (
    file: import("@gitru/commands").FileStatus,
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
    file: import("@gitru/commands").FileStatus;
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

export type VirtualItem =
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
      file: import("@gitru/commands").FileStatus;
      selectionKey: string;
      sectionId: string;
      sectionName: string;
      sectionType?: FileListSection["type"];
    };

export type MatchRange = { start: number; end: number };

export interface SectionHeaderProps {
  sectionId: string;
  sectionName: string;
  sectionType?: FileListSection["type"];
  count: number;
  isExpanded: boolean;
  onToggle: (sectionId: string) => void;
  actions?: FileListSection["actions"];
}

export interface FileRowProps {
  file: import("@gitru/commands").FileStatus;
  selectionKey: string;
  searchQuery: string;
  index: number;
  fileIndex: number;
  sectionId: string;
  sectionName: string;
  sectionType?: FileListSection["type"];
  onFileClick: VirtualizedFileListProps["onFileClick"];
  onAdd?: VirtualizedFileListProps["onAdd"];
  onUnstage?: VirtualizedFileListProps["onUnstage"];
  onDiscard?: VirtualizedFileListProps["onDiscard"];
  renderDiscard?: VirtualizedFileListProps["renderDiscard"];
  setSelectedFilePath: VirtualizedFileListProps["setSelectedFilePath"];
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

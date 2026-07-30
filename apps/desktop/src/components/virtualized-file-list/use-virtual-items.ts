import { useMemo } from "react";

import { FileListSection, VirtualItem } from "./types";
import { buildSelectionKey } from "./utils";

export function useVirtualItems(
  sections: FileListSection[],
  sectionMode: "accordion" | "flat",
  expandedSections: Set<string>,
) {
  return useMemo(() => {
    const result: VirtualItem[] = [];
    const fileItems: Array<{
      key: string;
      file: import("@gitru/commands").FileStatus;
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
}

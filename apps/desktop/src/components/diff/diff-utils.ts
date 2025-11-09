import { diffChars, diffWords, structuredPatch } from "diff";
import { type BundledLanguage } from "shiki";
import { DiffHunk, DiffRow, DiffSegment } from "./diff-types";
import {
  DEFAULT_LANGUAGE,
  EXTENSION_LANGUAGE_MAP,
  SPECIAL_FILENAMES,
} from "./highlighter";

const DEFAULT_CONTEXT_LINES = 3;

export const splitLines = (content: string): string[] => {
  if (!content) {
    return [];
  }

  const lines = content.split(/\r?\n/);
  if (lines.length > 0 && lines[lines.length - 1] === "") {
    lines.pop();
  }

  return lines;
};

export const escapeHtml = (value: string): string => {
  return value
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;");
};

export const formatRange = (start: number, count: number) => {
  const normalizedStart = Math.max(start, 0);
  if (count <= 0) {
    return `${normalizedStart},0`;
  }

  return count === 1 ? `${normalizedStart}` : `${normalizedStart},${count}`;
};

export const buildRowsFromSingleContent = (
  content: string,
  variant: "added" | "removed",
): DiffRow[] => {
  const lines = splitLines(content);
  return lines.map((line, index) =>
    variant === "added"
      ? {
          type: "added" as const,
          content: line,
          lineNumberNew: index + 1,
        }
      : {
          type: "removed" as const,
          content: line,
          lineNumberOld: index + 1,
        },
  );
};

export const buildHunksFromDiff = (
  before: string,
  after: string,
  options: { treatAsNewFile: boolean; treatAsDeletedFile: boolean },
): DiffHunk[] => {
  const patch = structuredPatch("head", "workdir", before, after, "", "", {
    context: DEFAULT_CONTEXT_LINES,
    // TODO: will be made configurable later
    ignoreWhitespace: false,
  });

  if (patch.hunks.length === 0) {
    if (options.treatAsNewFile) {
      const rows = buildRowsFromSingleContent(after, "added");
      return [
        {
          id: "hunk-0",
          header: `@@ -0,0 +1,${Math.max(rows.length, 1)} @@`,
          lines: rows,
          additions: rows.length,
          deletions: 0,
          oldStart: 0,
          newStart: rows.length > 0 ? 1 : 0,
          oldLines: 0,
          newLines: rows.length,
        },
      ];
    }

    if (options.treatAsDeletedFile) {
      const rows = buildRowsFromSingleContent(before, "removed");
      return [
        {
          id: "hunk-0",
          header: `@@ -1,${Math.max(rows.length, 1)} +0,0 @@`,
          lines: rows,
          additions: 0,
          deletions: rows.length,
          oldStart: rows.length > 0 ? 1 : 0,
          newStart: 0,
          oldLines: rows.length,
          newLines: 0,
        },
      ];
    }

    return [];
  }

  return patch.hunks.map((hunk, index) => {
    let oldLine = hunk.oldStart;
    let newLine = hunk.newStart;
    const rows: DiffRow[] = [];
    let additions = 0;
    let deletions = 0;

    for (const rawLine of hunk.lines) {
      if (!rawLine.length) continue;
      const indicator = rawLine[0];
      const content = rawLine.slice(1);

      switch (indicator) {
        case " ":
          rows.push({
            type: "context",
            content,
            lineNumberOld: oldLine++,
            lineNumberNew: newLine++,
          });
          break;
        case "-":
          rows.push({
            type: "removed",
            content,
            lineNumberOld: oldLine++,
          });
          deletions += 1;
          break;
        case "+":
          rows.push({
            type: "added",
            content,
            lineNumberNew: newLine++,
          });
          additions += 1;
          break;
        case "\\":
          rows.push({
            type: "context",
            content: rawLine,
            metadata: true,
          });
          break;
        default:
          break;
      }
    }

    const header = `@@ -${formatRange(hunk.oldStart, hunk.oldLines)} +${formatRange(hunk.newStart, hunk.newLines)} @@`;

    return {
      id: `hunk-${index}`,
      header,
      lines: rows,
      additions,
      deletions,
      oldStart: hunk.oldStart,
      newStart: hunk.newStart,
      oldLines: hunk.oldLines,
      newLines: hunk.newLines,
    };
  });
};

export const buildSkipSegment = (
  oldStart: number,
  oldEnd: number,
  newStart: number,
  newEnd: number,
  index: number,
  beforeLines: string[],
  afterLines: string[],
): DiffSegment | null => {
  const hasOld = oldStart > 0 && oldEnd >= oldStart;
  const hasNew = newStart > 0 && newEnd >= newStart;

  if (!hasOld && !hasNew) {
    return null;
  }

  let currentOld = hasOld ? oldStart : 0;
  let currentNew = hasNew ? newStart : 0;
  const lines: DiffRow[] = [];

  while ((hasOld && currentOld <= oldEnd) || (hasNew && currentNew <= newEnd)) {
    const oldLineNumber =
      hasOld && currentOld <= oldEnd ? currentOld : undefined;
    const newLineNumber =
      hasNew && currentNew <= newEnd ? currentNew : undefined;
    const content =
      (typeof newLineNumber === "number"
        ? afterLines[newLineNumber - 1]
        : undefined) ??
      (typeof oldLineNumber === "number"
        ? beforeLines[oldLineNumber - 1]
        : "") ??
      "";

    lines.push({
      type: "context",
      content,
      lineNumberOld: oldLineNumber,
      lineNumberNew: newLineNumber,
    });

    if (typeof oldLineNumber === "number") {
      currentOld += 1;
    }
    if (typeof newLineNumber === "number") {
      currentNew += 1;
    }
  }

  if (lines.length === 0) {
    return null;
  }

  return {
    type: "skip",
    id: `skip-${index}-${oldStart}-${oldEnd}-${newStart}-${newEnd}`,
    oldStart: hasOld ? oldStart : 0,
    oldEnd: hasOld ? oldEnd : 0,
    newStart: hasNew ? newStart : 0,
    newEnd: hasNew ? newEnd : 0,
    lines,
  };
};

export const buildDiffSegments = (
  hunks: DiffHunk[],
  beforeLines: string[],
  afterLines: string[],
): DiffSegment[] => {
  if (hunks.length === 0) {
    return [];
  }

  const segments: DiffSegment[] = [];
  let previousOldEnd = 0;
  let previousNewEnd = 0;

  const totalOldLines = beforeLines.length;
  const totalNewLines = afterLines.length;

  hunks.forEach((hunk, index) => {
    const gapOldStart = previousOldEnd + 1;
    const gapOldEnd = hunk.oldStart > 0 ? Math.max(hunk.oldStart - 1, 0) : 0;
    const gapNewStart = previousNewEnd + 1;
    const gapNewEnd = hunk.newStart > 0 ? Math.max(hunk.newStart - 1, 0) : 0;

    const skip = buildSkipSegment(
      gapOldStart,
      gapOldEnd,
      gapNewStart,
      gapNewEnd,
      index,
      beforeLines,
      afterLines,
    );

    if (skip) {
      segments.push(skip);
    }

    segments.push({ type: "hunk", hunk });

    const hunkOldEnd =
      hunk.oldLines > 0 ? hunk.oldStart + hunk.oldLines - 1 : hunk.oldStart - 1;
    const hunkNewEnd =
      hunk.newLines > 0 ? hunk.newStart + hunk.newLines - 1 : hunk.newStart - 1;

    previousOldEnd = Math.max(previousOldEnd, hunkOldEnd);
    previousNewEnd = Math.max(previousNewEnd, hunkNewEnd);
  });

  const trailingSkip = buildSkipSegment(
    previousOldEnd + 1,
    totalOldLines,
    previousNewEnd + 1,
    totalNewLines,
    hunks.length,
    beforeLines,
    afterLines,
  );

  if (trailingSkip) {
    segments.push(trailingSkip);
  }

  return segments;
};

export const formatBytes = (bytes: number) => {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
};

export const resolveLanguage = (filePath: string): BundledLanguage => {
  const fileName = filePath.split("/").pop() || "";
  const lower = fileName.toLowerCase();

  if (lower.includes(".")) {
    const extension = lower.split(".").pop() || "";
    return EXTENSION_LANGUAGE_MAP[extension] ?? DEFAULT_LANGUAGE;
  }

  return SPECIAL_FILENAMES[lower] ?? DEFAULT_LANGUAGE;
};

const inlineDiffCache = new Map<string, ReturnType<typeof diffWords>>();

export const computeInlineDiff = (
  oldText: string,
  newText: string,
): Array<{ value: string; added?: boolean; removed?: boolean }> => {
  const key = `${oldText}\x00${newText}`;

  if (inlineDiffCache.has(key)) {
    return inlineDiffCache.get(key)!;
  }

  // Skip inline diff for very long lines
  if (oldText.length > 500 || newText.length > 500) {
    return [
      { value: oldText, removed: true },
      { value: newText, added: true },
    ];
  }

  const changes = diffWords(oldText, newText);

  if (changes.length === 1 && !changes[0].added && !changes[0].removed) {
    const charDiff = diffChars(oldText, newText);
    inlineDiffCache.set(key, charDiff);
    return charDiff;
  }

  inlineDiffCache.set(key, changes);
  return changes;
};

import { PATTERNS } from "../constants";
import type {
  DiffHunk,
  DiffLine,
  DiffLineType,
  FileChangeType,
  FileDiff,
  HunkHeader,
  ParsedPatch,
} from "../types";
import { detectLanguage, splitLines, trimTrailingNewline } from "./common";

/**
 * Parse a line type from its prefix character
 */
function parseLineType(
  line: string,
): { type: DiffLineType; content: string } | null {
  if (line.length === 0) return null;

  const firstChar = line[0];
  const content = line.slice(1);

  switch (firstChar) {
    case " ":
      return { type: "context", content };
    case "+":
      return { type: "addition", content };
    case "-":
      return { type: "deletion", content };
    case "\\":
      // "\ No newline at end of file" - metadata
      return { type: "metadata", content };
    default:
      return null;
  }
}

/**
 * Parse a hunk header line: @@ -start,count +start,count @@ context
 */
function parseHunkHeader(line: string): HunkHeader | null {
  const match = line.match(PATTERNS.HUNK_HEADER);
  if (!match) return null;

  return {
    oldStart: parseInt(match[1] ?? "1", 10),
    oldCount: parseInt(match[2] ?? "1", 10),
    newStart: parseInt(match[3] ?? "1", 10),
    newCount: parseInt(match[4] ?? "1", 10),
    context: match[5]?.trim(),
    raw: line,
  };
}

/**
 * Parse a single file diff section
 */
function parseFileDiff(
  section: string,
  _isGitDiff: boolean,
  index: number,
  cacheKeyPrefix?: string,
): FileDiff | null {
  const lines = splitLines(section);
  if (lines.length === 0) return null;

  let newPath = "";
  let oldPath: string | undefined;
  let changeType: FileChangeType = "modified";
  let mode: string | undefined;
  let oldMode: string | undefined;
  let isBinary = false;

  const hunks: DiffHunk[] = [];
  let currentHunk: DiffHunk | null = null;
  let oldLineNumber = 0;
  let newLineNumber = 0;
  let lastHunkEnd = 0;

  for (const rawLine of lines) {
    const line = trimTrailingNewline(rawLine);

    // Parse diff --git header
    const gitMatch = line.match(PATTERNS.GIT_FILE_HEADER);
    if (gitMatch) {
      oldPath = gitMatch[1] ?? gitMatch[2] ?? "";
      newPath = gitMatch[3] ?? gitMatch[4] ?? "";
      if (oldPath !== newPath) {
        changeType = "renamed";
      }
      continue;
    }

    // Parse --- and +++ headers
    const fileHeaderMatch = line.match(PATTERNS.FILE_HEADER);
    if (fileHeaderMatch) {
      const [, prefix = "", path = ""] = fileHeaderMatch;
      if (path === "/dev/null") {
        if (prefix === "---") {
          changeType = "added";
        } else {
          changeType = "deleted";
        }
      } else if (prefix === "---") {
        oldPath = path;
        if (!newPath) newPath = path;
      } else if (prefix === "+++") {
        newPath = path;
      }
      continue;
    }

    // Parse new file mode
    const newFileModeMatch = line.match(PATTERNS.NEW_FILE_MODE);
    if (newFileModeMatch && newFileModeMatch[1]) {
      changeType = "added";
      mode = newFileModeMatch[1];
      continue;
    }

    // Parse deleted file mode
    const deletedFileModeMatch = line.match(PATTERNS.DELETED_FILE_MODE);
    if (deletedFileModeMatch && deletedFileModeMatch[1]) {
      changeType = "deleted";
      mode = deletedFileModeMatch[1];
      continue;
    }

    // Parse similarity index
    const similarityMatch = line.match(PATTERNS.SIMILARITY_INDEX);
    if (similarityMatch && similarityMatch[1]) {
      const similarity = parseInt(similarityMatch[1], 10);
      changeType = similarity === 100 ? "copied" : "renamed";
      continue;
    }

    // Parse rename from
    const renameFromMatch = line.match(PATTERNS.RENAME_FROM);
    if (renameFromMatch && renameFromMatch[1]) {
      oldPath = renameFromMatch[1];
      changeType = "renamed";
      continue;
    }

    // Parse rename to
    const renameToMatch = line.match(PATTERNS.RENAME_TO);
    if (renameToMatch && renameToMatch[1]) {
      newPath = renameToMatch[1];
      continue;
    }

    // Parse index line
    const indexMatch = line.match(PATTERNS.INDEX_LINE);
    if (indexMatch && indexMatch[1]) {
      mode = indexMatch[1];
      continue;
    }

    // Check for binary files
    if (PATTERNS.BINARY_FILES.test(line)) {
      isBinary = true;
      continue;
    }

    // Parse hunk header
    const hunkHeader = parseHunkHeader(line);
    if (hunkHeader) {
      // Calculate collapsed lines before this hunk
      const collapsedBefore = Math.max(
        hunkHeader.oldStart - 1 - lastHunkEnd,
        0,
      );
      lastHunkEnd = hunkHeader.oldStart + hunkHeader.oldCount - 1;

      currentHunk = {
        header: hunkHeader,
        lines: [],
        collapsedBefore,
      };
      hunks.push(currentHunk);
      oldLineNumber = hunkHeader.oldStart;
      newLineNumber = hunkHeader.newStart;
      continue;
    }

    // Skip lines before first hunk
    if (!currentHunk) continue;

    // Parse diff line
    const parsedLine = parseLineType(rawLine);
    if (!parsedLine) continue;

    const diffLine: DiffLine = {
      content: parsedLine.content,
      type: parsedLine.type,
      oldLineNumber: null,
      newLineNumber: null,
    };

    // Handle metadata (no newline at EOF)
    if (parsedLine.type === "metadata") {
      const prevLine = currentHunk.lines[currentHunk.lines.length - 1];
      if (prevLine) {
        prevLine.noNewlineAtEOF = true;
        // Also trim the newline from the content
        prevLine.content = trimTrailingNewline(prevLine.content);
      }
      continue;
    }

    // Assign line numbers based on type
    switch (parsedLine.type) {
      case "context":
        diffLine.oldLineNumber = oldLineNumber++;
        diffLine.newLineNumber = newLineNumber++;
        break;
      case "deletion":
        diffLine.oldLineNumber = oldLineNumber++;
        break;
      case "addition":
        diffLine.newLineNumber = newLineNumber++;
        break;
    }

    currentHunk.lines.push(diffLine);
  }

  if (!newPath) return null;

  return {
    newPath,
    oldPath: oldPath !== newPath ? oldPath : undefined,
    changeType,
    language: detectLanguage(newPath),
    hunks,
    mode,
    oldMode,
    isBinary,
    cacheKey: cacheKeyPrefix ? `${cacheKeyPrefix}-${index}` : undefined,
  };
}

/**
 * Parse a unified diff or git diff patch string into structured data.
 *
 * @param patch - The raw patch string
 * @param cacheKeyPrefix - Optional prefix for generating cache keys
 * @returns Parsed patch with file diffs
 *
 * @example
 * ```ts
 * const patch = `
 * diff --git a/file.ts b/file.ts
 * --- a/file.ts
 * +++ b/file.ts
 * @@ -1,3 +1,4 @@
 *  line 1
 * +new line
 *  line 2
 *  line 3
 * `;
 * const result = parsePatch(patch);
 * ```
 */
export function parsePatch(
  patch: string,
  cacheKeyPrefix?: string,
): ParsedPatch {
  if (!patch || !patch.trim()) {
    return { files: [] };
  }

  const isGitDiff = PATTERNS.GIT_DIFF_SPLIT.test(patch);
  const splitPattern = isGitDiff
    ? PATTERNS.GIT_DIFF_SPLIT
    : PATTERNS.UNIFIED_DIFF_SPLIT;

  const sections = patch.split(splitPattern);
  const files: FileDiff[] = [];
  let metadata: string | undefined;

  for (let i = 0; i < sections.length; i++) {
    const section = sections[i];
    if (!section || !section.trim()) continue;

    // Check if this section is actually a diff
    const isDiffSection = isGitDiff
      ? section.startsWith("diff --git")
      : section.match(/^---\s+\S/m);

    if (!isDiffSection) {
      // This is probably metadata (commit info, etc.)
      if (!metadata) {
        metadata = section;
      }
      continue;
    }

    const fileDiff = parseFileDiff(
      section,
      isGitDiff,
      files.length,
      cacheKeyPrefix,
    );
    if (fileDiff) {
      files.push(fileDiff);
    }
  }

  return { metadata, files };
}

/**
 * Parse a simple two-file diff created from before/after content.
 * Useful when you have the actual file contents rather than a patch.
 *
 * @param oldContent - Original file content
 * @param newContent - New file content
 * @param filePath - File path for language detection
 * @returns Parsed file diff
 */
export function parseDiffFromContents(
  oldContent: string,
  newContent: string,
  filePath: string,
): FileDiff {
  // Use the diff library to create a patch, then parse it
  // For now, we'll implement a simple line-by-line diff

  const oldLines = splitLines(oldContent);
  const newLines = splitLines(newContent);

  // Simple LCS-based diff
  const { hunks } = computeDiff(oldLines, newLines);

  let changeType: FileChangeType = "modified";
  if (oldContent === "" && newContent !== "") {
    changeType = "added";
  } else if (oldContent !== "" && newContent === "") {
    changeType = "deleted";
  }

  return {
    newPath: filePath,
    changeType,
    language: detectLanguage(filePath),
    hunks,
  };
}

/**
 * Simple Myers diff algorithm implementation
 */
function computeDiff(
  oldLines: string[],
  newLines: string[],
): { hunks: DiffHunk[] } {
  const hunks: DiffHunk[] = [];
  const contextLines = 3;

  // Find longest common subsequence using dynamic programming
  const lcs = computeLCS(oldLines, newLines);

  // Build diff lines from LCS
  let oldIdx = 0;
  let newIdx = 0;
  let lcsIdx = 0;
  const diffLines: DiffLine[] = [];

  while (oldIdx < oldLines.length || newIdx < newLines.length) {
    const lcsItem = lcs[lcsIdx];
    const oldLine = oldLines[oldIdx];
    const newLine = newLines[newIdx];

    if (
      lcsItem &&
      oldLine !== undefined &&
      trimTrailingNewline(oldLine) === trimTrailingNewline(lcsItem.text)
    ) {
      // Context line
      diffLines.push({
        content: oldLine,
        type: "context",
        oldLineNumber: oldIdx + 1,
        newLineNumber: newIdx + 1,
      });
      oldIdx++;
      newIdx++;
      lcsIdx++;
    } else if (
      newLine !== undefined &&
      (!lcsItem ||
        trimTrailingNewline(newLine) !== trimTrailingNewline(lcsItem.text))
    ) {
      // Addition
      diffLines.push({
        content: newLine,
        type: "addition",
        oldLineNumber: null,
        newLineNumber: newIdx + 1,
      });
      newIdx++;
    } else if (oldLine !== undefined) {
      // Deletion
      diffLines.push({
        content: oldLine,
        type: "deletion",
        oldLineNumber: oldIdx + 1,
        newLineNumber: null,
      });
      oldIdx++;
    }
  }

  // Group into hunks based on context
  if (diffLines.length === 0) {
    return { hunks };
  }

  // Find change regions and create hunks
  let hunkStart = -1;
  let hunkLines: DiffLine[] = [];
  let lastChangeIdx = -contextLines - 1;

  for (let i = 0; i < diffLines.length; i++) {
    const line = diffLines[i];
    if (!line) continue;
    const isChange = line.type !== "context";

    if (isChange) {
      // Start new hunk if needed
      if (hunkStart === -1 || i - lastChangeIdx > contextLines * 2) {
        // Save previous hunk
        if (
          hunkLines.length > 0 &&
          hunkLines.some((l) => l.type !== "context")
        ) {
          const firstLine = hunkLines[0];
          if (firstLine) {
            hunks.push({
              header: {
                oldStart: firstLine.oldLineNumber ?? 1,
                oldCount: hunkLines.filter(
                  (l) => l.type === "context" || l.type === "deletion",
                ).length,
                newStart: firstLine.newLineNumber ?? 1,
                newCount: hunkLines.filter(
                  (l) => l.type === "context" || l.type === "addition",
                ).length,
                raw: "",
              },
              lines: hunkLines,
              collapsedBefore: 0,
            });
          }
        }

        // Start new hunk with context before
        hunkStart = Math.max(0, i - contextLines);
        hunkLines = diffLines.slice(hunkStart, i);
      }

      hunkLines.push(line);
      lastChangeIdx = i;
    } else if (hunkStart !== -1 && i - lastChangeIdx <= contextLines) {
      // Add context after change
      hunkLines.push(line);
    }
  }

  // Save last hunk
  if (hunkLines.length > 0 && hunkLines.some((l) => l.type !== "context")) {
    const firstLine = hunkLines[0];
    if (firstLine) {
      hunks.push({
        header: {
          oldStart: firstLine.oldLineNumber ?? 1,
          oldCount: hunkLines.filter(
            (l) => l.type === "context" || l.type === "deletion",
          ).length,
          newStart: firstLine.newLineNumber ?? 1,
          newCount: hunkLines.filter(
            (l) => l.type === "context" || l.type === "addition",
          ).length,
          raw: "",
        },
        lines: hunkLines,
        collapsedBefore: 0,
      });
    }
  }

  return { hunks };
}

interface LCSItem {
  text: string;
  oldIdx: number;
  newIdx: number;
}

/**
 * Compute longest common subsequence
 */
function computeLCS(oldLines: string[], newLines: string[]): LCSItem[] {
  const m = oldLines.length;
  const n = newLines.length;

  // Build LCS table
  const dp: number[][] = Array(m + 1)
    .fill(null)
    .map(() => Array(n + 1).fill(0));

  for (let i = 1; i <= m; i++) {
    for (let j = 1; j <= n; j++) {
      const oldLine = oldLines[i - 1];
      const newLine = newLines[j - 1];
      const dpRow = dp[i];
      const dpPrevRow = dp[i - 1];
      if (!dpRow || !dpPrevRow) continue;

      if (
        oldLine !== undefined &&
        newLine !== undefined &&
        trimTrailingNewline(oldLine) === trimTrailingNewline(newLine)
      ) {
        dpRow[j] = (dpPrevRow[j - 1] ?? 0) + 1;
      } else {
        dpRow[j] = Math.max(dpPrevRow[j] ?? 0, dpRow[j - 1] ?? 0);
      }
    }
  }

  // Backtrack to find LCS
  const lcs: LCSItem[] = [];
  let i = m;
  let j = n;

  while (i > 0 && j > 0) {
    const oldLine = oldLines[i - 1];
    const newLine = newLines[j - 1];
    const dpRow = dp[i];
    const dpPrevRow = dp[i - 1];

    if (
      oldLine !== undefined &&
      newLine !== undefined &&
      trimTrailingNewline(oldLine) === trimTrailingNewline(newLine)
    ) {
      lcs.unshift({ text: oldLine, oldIdx: i - 1, newIdx: j - 1 });
      i--;
      j--;
    } else if ((dpPrevRow?.[j] ?? 0) > (dpRow?.[j - 1] ?? 0)) {
      i--;
    } else {
      j--;
    }
  }

  return lcs;
}

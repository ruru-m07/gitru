import { type FileStatus } from "@gitru/commands";

import {
  getGitStagePathspec,
  getGitUnstagePathspecs,
} from "@/lib/git-pathspec";

import { FileListSection, MatchRange } from "./types";

export const ITEM_HEIGHT = 32;
export const SECTION_HEADER_HEIGHT = 36;
export const EMPTY_CONTEXT_ACTIONS: import("./types").FileRowContextAction[] =
  [];

export const getStageFileTargets = (file: FileStatus) =>
  getGitStagePathspec(file);

export const getUnstageFileTargets = (file: FileStatus) =>
  getGitUnstagePathspecs(file);

export const buildSelectionKey = (
  file: FileStatus,
  sectionId: string,
  sectionType?: FileListSection["type"],
) =>
  `${sectionType ?? "custom"}:${sectionId}:${file.path}:${file.new_path ?? ""}`;

export const getWorktreeScope = (sectionType?: FileListSection["type"]) => {
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

export const getMatchRanges = (value: string, query: string) => {
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

export const renderHighlightedSlice = (
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

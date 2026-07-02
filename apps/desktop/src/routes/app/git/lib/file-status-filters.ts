import type { GetStatusResponse } from "@gitru/commands";

export type FileStatusFilter =
  | "modified"
  | "renamed"
  | "conflicted"
  | "deleted"
  | "untracked";

export const DEFAULT_STATUS_FILTERS: Record<FileStatusFilter, boolean> = {
  modified: true,
  renamed: true,
  deleted: true,
  conflicted: true,
  untracked: true,
};

export const hasActiveStatusFilters = (
  filters: Record<FileStatusFilter, boolean>,
) =>
  !filters.modified ||
  !filters.renamed ||
  !filters.deleted ||
  !filters.conflicted ||
  !filters.untracked;

export const matchesStatusFilters = (
  file: GetStatusResponse["files"][number],
  filters: Record<FileStatusFilter, boolean>,
) => {
  const isModified = file.status.some((s) => s.includes("Modified"));
  const isRenamed = file.status.some((s) => s.includes("Renamed"));
  const isDeleted = file.status.some((s) => s.includes("Deleted"));
  const isConflicted = file.status.some((s) => s.includes("Conflicted"));
  const isUntracked = file.status.some((s) => s.includes("New"));

  return (
    (filters.modified && isModified) ||
    (filters.renamed && isRenamed) ||
    (filters.deleted && isDeleted) ||
    (filters.conflicted && isConflicted) ||
    (filters.untracked && isUntracked)
  );
};
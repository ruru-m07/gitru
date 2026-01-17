export type DiffRowType = "added" | "removed" | "context";
export interface DiffRow {
  type: DiffRowType;
  content: string;
  lineNumberOld?: number;
  lineNumberNew?: number;
  metadata?: boolean;
}
export interface DiffHunk {
  id: string;
  header: string;
  lines: DiffRow[];
  additions: number;
  deletions: number;
  oldStart: number;
  newStart: number;
  oldLines: number;
  newLines: number;
}
export type DiffSegment =
  | { type: "hunk"; hunk: DiffHunk }
  | {
      type: "skip";
      id: string;
      oldStart: number;
      oldEnd: number;
      newStart: number;
      newEnd: number;
      lines: DiffRow[];
    };

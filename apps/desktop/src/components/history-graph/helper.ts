import type { GraphRef, GraphRow } from "@gitru/commands";
import { PALETTE } from "./palette";

export type ProcessedRow = {
  row: GraphRow;
  color: string;
  branchRefs: GraphRef[];
  tags: GraphRef[];
  remoteRefs: GraphRef[];
  localBranchRefs: GraphRef[];
};

function getRowColor(row: GraphRow): string {
  const inputIndex = row.input_swimlanes.findIndex(
    (lane) => lane.id === row.oid,
  );
  const circleIndex =
    inputIndex === -1 ? row.input_swimlanes.length : inputIndex;
  const lane =
    row.output_swimlanes[circleIndex] ?? row.input_swimlanes[circleIndex];
  return PALETTE[(lane?.color ?? 0) % PALETTE.length] ?? PALETTE[0];
}

function getBranchRefs(refs: GraphRef[]) {
  return refs
    .filter((ref) => ref.kind === "Local" || ref.kind === "Remote")
    .filter(
      (ref) =>
        ref.name !== "refs/heads/HEAD" &&
        ref.name !== "refs/remotes/origin/HEAD",
    );
}

function getRemoteRefs(refs: GraphRef[]) {
  return refs
    .filter((ref) => ref.name !== "refs/remotes/origin/HEAD")
    .filter((ref) => ref.name.startsWith("refs/remotes/origin/"));
}

function getLocalBranchRefs(refs: GraphRef[]) {
  return refs
    .filter((ref) => ref.name !== "refs/heads/HEAD")
    .filter((ref) => ref.name.startsWith("refs/heads/"));
}

export function processeRows(rows: GraphRow[]): ProcessedRow[] {
  return rows.map((row) => ({
    row,
    color: getRowColor(row),
    branchRefs: getBranchRefs(row.branch_refs),
    tags: row.refs.filter((ref) => ref.kind === "Tag"),
    remoteRefs: getRemoteRefs(row.refs),
    localBranchRefs: getLocalBranchRefs(row.refs),
  }));
}

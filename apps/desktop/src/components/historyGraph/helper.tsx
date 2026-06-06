import { GraphRef, GraphRow } from "@gitru/commands";
import { PALETTE } from "./columns/lanes/color-palettes";

export type GraphLayout = {
  maxLane: number;
};

export type ProcessedRow = {
  row: GraphRow;
  color: string;
  tags: ReturnType<typeof getTags>;
  remoteRefs: ReturnType<typeof getRemoteRefs>;
  localBranchRefs: ReturnType<typeof getLocalBranchRefs>;
};

export type GraphColumnsType = {
  rows: ProcessedRow[];
  layout: GraphLayout;
  scrollRef: React.RefObject<HTMLDivElement | null>;
} & React.HTMLAttributes<HTMLDivElement>;

function computeGraphLayout(rows: GraphRow[]): GraphLayout {
  let maxLane = 0;

  for (const row of rows) {
    const inputMax = row.input_swimlanes.length - 1;
    const outputMax = row.output_swimlanes.length - 1;
    if (inputMax > maxLane) maxLane = inputMax;
    if (outputMax > maxLane) maxLane = outputMax;
  }

  return { maxLane };
}

function buildRefList(row: GraphRow): GraphRef[] {
  const prioritized = [
    ...row.heads,
    ...row.tags,
    ...row.remotes,
    ...row.stashes,
  ];

  const seen = new Set<string>();
  const refs: GraphRef[] = [];

  for (const ref of prioritized) {
    const key = `${ref.kind}-${ref.name}`;
    if (!seen.has(key)) {
      seen.add(key);
      refs.push(ref);
    }
  }
  for (const ref of row.refs) {
    const key = `${ref.kind}-${ref.name}`;
    if (!seen.has(key)) {
      seen.add(key);
      refs.push(ref);
    }
  }

  return refs.slice(0, 6);
}

function refDisplayName(ref: GraphRef) {
  return ref.display_name || ref.name;
}

function getCurrentBranchRef(row: GraphRow) {
  return row.heads.find((ref) => ref.is_head) ?? null;
}

function isCurrentBranchRow(row: GraphRow) {
  return getCurrentBranchRef(row) !== null;
}

function buildCurrentBranchPath(rows: GraphRow[]) {
  const path = new Set<string>();
  const tipRow = rows.find((row) => isCurrentBranchRow(row));

  if (!tipRow) {
    return path;
  }

  let currentOid: string | null = tipRow.oid;
  while (currentOid && !path.has(currentOid)) {
    path.add(currentOid);
    const currentRow = rows.find((row) => row.oid === currentOid);
    if (!currentRow || currentRow.parents.length === 0) {
      break;
    }

    currentOid = currentRow.parents[0]?.oid ?? null;
  }

  return path;
}

function badgeVariantForRef(ref: GraphRef) {
  switch (ref.kind) {
    case "Local":
      return "secondary";
    case "Remote":
      return "info";
    case "Tag":
      return "outline";
    case "Stash":
      return "warning";
    default:
      return "default";
  }
}

function getRowColor(row: GraphRow): string {
  const input = row.input_swimlanes;
  const output = row.output_swimlanes;

  const inputIdx = input.findIndex((n) => n.id === row.oid);
  const circleIdx = inputIdx !== -1 ? inputIdx : input.length;

  const color =
    circleIdx < output.length
      ? PALETTE[output[circleIdx].color % PALETTE.length]
      : circleIdx < input.length
        ? PALETTE[input[circleIdx].color % PALETTE.length]
        : PALETTE[0];

  return color;
}

function getTags(refs: GraphRef[]) {
  return refs.filter((ref) => ref.kind === "Tag");
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

function processeRows(rows: GraphRow[]): ProcessedRow[] {
  return rows.map((row) => {
    return {
      row,
      color: getRowColor(row),
      tags: getTags(row.refs),
      remoteRefs: getRemoteRefs(row.refs),
      localBranchRefs: getLocalBranchRefs(row.refs),
    };
  });
}

function getColumnStyle(col: number, rows: number): React.CSSProperties {
  return {
    gridRow: `span ${rows}`,
    gridColumn: col,
    display: "grid",
    gridTemplateRows: "subgrid",
  };
}

export {
  badgeVariantForRef,
  buildRefList,
  buildCurrentBranchPath,
  computeGraphLayout,
  getCurrentBranchRef,
  isCurrentBranchRow,
  refDisplayName,
  getRowColor,
  getTags,
  getRemoteRefs,
  getLocalBranchRefs,
  processeRows,
  getColumnStyle,
};

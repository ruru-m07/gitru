export type ChunkActionMetadata = {
  source: "worktree" | "stash" | "history";
  filePath: string;
  fileNewPath: string | null;
  stashReference: string | null;
  commitHash: string | null;
  hunkIndex: number;
  changeIndex: number;
  side: "additions" | "deletions";
  additions: {
    start: number | null;
    end: number | null;
    count: number;
  };
  deletions: {
    start: number | null;
    end: number | null;
    count: number;
  };
};

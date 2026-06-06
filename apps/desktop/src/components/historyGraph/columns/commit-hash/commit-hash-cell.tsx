import React from "react";
import { ProcessedRow } from "../../helper";

type CommitHashCellProps = {
  row: ProcessedRow;
};

const CommitHashCell = React.memo(({ row }: CommitHashCellProps) => {
  const { row: commitRow } = row;

  return (
    <span className="flex items-center text-sm text-muted-foreground font-mono gap-2 border-l px-2 min-w-0">
      {commitRow.commit.id.slice(0, 7)}
    </span>
  );
});

export default CommitHashCell;

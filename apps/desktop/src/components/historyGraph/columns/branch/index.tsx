import React from "react";
import { useGetCurrentBranch, useGetStatusAheadBehind } from "@/hooks";
import { GraphColumnsType } from "../../helper";
import BranchCell from "./branch-cell";

const Branch = React.memo(({ rows, ...restProps }: GraphColumnsType) => {
  const { data: currentBranch } = useGetCurrentBranch();
  const { data: statusAheadBehind } = useGetStatusAheadBehind();

  return (
    <div
      className="overflow-x-hidden text-nowrap max-w-full min-w-0"
      {...restProps}
    >
      {rows.map((row) => {
        return (
          <BranchCell
            key={row.row.oid}
            currentBranch={currentBranch ?? null}
            row={row}
            enablePushButton={(statusAheadBehind?.ahead || 0) > 0}
          />
        );
      })}
    </div>
  );
});

export default Branch;

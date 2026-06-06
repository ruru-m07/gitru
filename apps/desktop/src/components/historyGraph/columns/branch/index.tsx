import React from "react";
import { useGetCurrentBranch } from "@/hooks";
import { GraphColumnsType } from "../../helper";
import BranchCell from "./branch-cell";

const Branch = React.memo(({ rows, ...restProps }: GraphColumnsType) => {
  const { data: currentBranch } = useGetCurrentBranch();

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
          />
        );
      })}
    </div>
  );
});

export default Branch;

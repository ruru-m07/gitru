import { Branch } from "@gitru/commands";
import { Button } from "@gitru/ui/components/button";
import { cn } from "@gitru/ui/lib/utils";
import { ArrowUpFromLine } from "lucide-react";
import React from "react";
import { CSSVars } from "@/types/app";
import { ProcessedRow } from "../../helper";
import { ROW_H } from "../lanes/lane-graph";
import BranchBadge from "./branch-badge";

type BranchCellProps = {
  row: ProcessedRow;
  currentBranch: Branch | null;
  enablePushButton?: boolean;
};

const BranchCell = React.memo(
  ({ row, currentBranch, enablePushButton }: BranchCellProps) => {
    const isAnyRefs = row.row.refs.length > 0;

    const isCurrentBranch = currentBranch
      ? row.row.refs.some((ref) => ref.display_name === currentBranch.name)
      : false;

    return (
      <div
        data-cell
        data-cell-id={row.row.oid}
        className="flex items-center gap-1 relative max-w-full min-w-0 pl-1 pr-2"
      >
        {row.localBranchRefs.length > 0 && (
          <>
            {row.localBranchRefs.map((ref) => (
              <BranchBadge
                key={ref.name}
                ref={ref}
                row={row}
                currentBranch={currentBranch}
                type="local"
              />
            ))}
          </>
        )}
        {row.remoteRefs.length > 0 && (
          <>
            {row.remoteRefs.map((ref) => (
              <BranchBadge
                key={ref.name}
                ref={ref}
                row={row}
                currentBranch={currentBranch}
                type="remote"
              />
            ))}
          </>
        )}
        {row.tags.length > 0 && (
          <>
            {row.tags.map((tag) => (
              <BranchBadge
                key={tag.name}
                ref={tag}
                row={row}
                currentBranch={currentBranch}
                type="tag"
              />
            ))}
          </>
        )}
        {isAnyRefs && (
          <div className="absolute py-0.5 top-0 right-0 h-full z-10">
            <div
              className="h-full w-0.5"
              style={{
                // backgroundColor: `color-mix(in oklab, ${row.color} 70%, var(--color-background))`,
                background: isCurrentBranch
                  ? `repeating-linear-gradient(
                0deg,
                color-mix(in oklab, ${row.color} 90%, var(--color-background)) 0px,
                color-mix(in oklab, ${row.color} 90%, var(--color-background)) 2px,
                var(--color-background) 2px,
                var(--color-background) 4px
              )`
                  : `color-mix(in oklab, ${row.color} 90%, var(--color-background))`,
              }}
            />
          </div>
        )}
        {isAnyRefs && (
          <div
            className="absolute left-1 -translate-y-1/2 inset-0 mt-0.5 z-0"
            style={{
              height: 2,
              top: ROW_H / 2 - 2,
              width: "100%",
              // backgroundColor: `color-mix(in oklab, ${row.color} 30%, var(--color-background))`,
              background: `repeating-linear-gradient(
              to right,
              color-mix(in oklab, ${row.color} 30%, var(--color-background)) 0px,
              color-mix(in oklab, ${row.color} 30%, var(--color-background)) 6px,
              transparent 4px,
              transparent 8px
            )`,
              transform: "translateZ(0)",
              willChange: "contents",
              contain: "layout paint size",
            }}
          />
        )}
        {isCurrentBranch && enablePushButton && (
          <BadgeStyleButton row={row}>
            <ArrowUpFromLine className="opacity-100!" />
          </BadgeStyleButton>
        )}
      </div>
    );
  },
);

export default BranchCell;

const BadgeStyleButton = ({
  children,
  row,
}: {
  children: React.ReactNode;
  row: ProcessedRow;
}) => {
  const style: CSSVars = {
    backgroundColor: `color-mix(in oklab, ${row.color} 20%, var(--color-background))`,
    "--icon-color": `color-mix(in oklab, ${row.color} 50%, var(--color-foreground))`,
  };

  return (
    <Button
      style={style}
      className={cn(
        "rounded-sm min-w-0 w-fit px-1.5 py-1 flex items-center justify-center gap-1 relative z-10",
        "**:[svg]:size-3.5 **:[svg]:shrink-0 **:[svg]:stroke-[1.2] **:[svg]:text-(--icon-color)",
        "hover:bg-[color-mix(in_oklab,var(--icon-color)_20%,var(--color-background))]!",
        "border-dashed border-(--icon-color)/45!",
        "[&_svg:not([class*='opacity-'])]:opacity-100",
      )}
      variant={"outline"}
      size={"icon-xs"}
    >
      {children}
    </Button>
  );
};

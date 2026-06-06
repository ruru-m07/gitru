import { Branch, GraphRef } from "@gitru/commands";
import { cn } from "@gitru/ui/lib/utils";
import { CircleDotDashed, Cloud, GitBranch, Tag } from "lucide-react";
import { ProcessedRow } from "../../helper";

type BranchBadgeProps = {
  row: ProcessedRow;
  currentBranch: Branch | null;
  ref: GraphRef;
  type?: "local" | "remote" | "tag";
};

type CSSVars = React.CSSProperties & {
  [key: `--${string}`]: string | number;
};

const BranchBadge = ({ row, ref, currentBranch, type }: BranchBadgeProps) => {
  const style: CSSVars = {
    backgroundColor: `color-mix(in oklab, ${row.color} 20%, var(--color-background))`,
    "--icon-color": `color-mix(in oklab, ${row.color} 50%, var(--color-foreground))`,
  };

  return (
    <span
      className={cn(
        "rounded-sm min-w-0 w-fit px-1.5 py-1 flex items-center justify-center gap-1 relative z-10",
        "**:[svg]:size-3.5 **:[svg]:shrink-0 **:[svg]:stroke-[1.2] **:[svg]:text-(--icon-color)",
      )}
      style={style}
    >
      {currentBranch?.name ===
      row.row.refs.find((r) => r.kind === "Local")?.display_name ? (
        <CircleDotDashed />
      ) : type === "local" ? (
        <GitBranch />
      ) : type === "remote" ? (
        <Cloud />
      ) : type === "tag" ? (
        <Tag />
      ) : null}
      <span className="truncate min-w-0 text-xs text-(--icon-color)">
        {ref.display_name}
      </span>
    </span>
  );
};

export default BranchBadge;

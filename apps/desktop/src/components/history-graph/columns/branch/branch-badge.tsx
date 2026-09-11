import { Branch, GraphRef } from "@gitru/commands";
import { Button } from "@gitru/ui/components/button";
import { useCommandNavigation } from "@gitru/ui/components/command";
import { cn } from "@gitru/ui/lib/utils";
import { CircleDotDashed, Cloud, GitBranch, Tag } from "lucide-react";
import { CSSVars } from "@/types/app";
import { ProcessedRow } from "../../helper";

type BranchBadgeProps = {
  row: ProcessedRow;
  currentBranch: Branch | null;
  ref: GraphRef;
  type?: "local" | "remote" | "tag";
};

const BranchBadge = ({ row, ref, currentBranch, type }: BranchBadgeProps) => {
  const navigation = useCommandNavigation();
  const style: CSSVars = {
    backgroundColor: `color-mix(in oklab, ${row.color} 20%, var(--color-background))`,
    "--icon-color": `color-mix(in oklab, ${row.color} 50%, var(--color-foreground))`,
  };

  const content = (
    <>
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
    </>
  );

  const className = cn(
    "rounded-sm min-w-0 w-fit h-auto px-1.5 py-1 flex items-center justify-center gap-1 relative z-10",
    "**:[svg]:size-3.5 **:[svg]:shrink-0 **:[svg]:stroke-[1.2] **:[svg]:text-(--icon-color)",
  );

  if (type === "local" || type === "remote") {
    return (
      <Button
        type="button"
        variant="ghost"
        className={className}
        style={style}
        aria-label={`Manage ${type} branch ${ref.display_name}`}
        onClick={(event) => {
          event.stopPropagation();
          navigation.setOpen(true);
          navigation.push("branch-actions", {
            branchName: ref.display_name,
            isRemote: type === "remote",
          });
        }}
      >
        {content}
      </Button>
    );
  }

  return (
    <span className={className} style={style}>
      {content}
    </span>
  );
};

export default BranchBadge;

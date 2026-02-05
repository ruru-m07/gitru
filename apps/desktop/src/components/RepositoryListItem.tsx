import { type RepositoryInfo } from "@gitru/commands";
import { Badge } from "@gitru/ui/components/badge";
import { Button } from "@gitru/ui/components/button";
import {
  Tooltip,
  TooltipPopup,
  TooltipTrigger,
} from "@gitru/ui/components/tooltip";
import {
  ArrowDown,
  ArrowUp,
  CircleDashed,
  GitBranch,
  Minus,
} from "lucide-react";

interface RepositoryListItemProps {
  repo: RepositoryInfo;
  isSelected: boolean;
  onSelect: () => void;
  onRemove: () => void;
}

export function RepositoryListItem({
  repo,
  isSelected,
  onSelect,
  onRemove,
}: RepositoryListItemProps) {
  return (
    <button
      className={`py-1 px-2 flex border-l w-full hover:border-border hover:bg-accent/55 cursor-pointer ${
        isSelected ? "bg-accent border-border" : ""
      }`}
      type="button"
      onClick={onSelect}
    >
      <div className="min-w-7 flex items-center _justify-center">
        {isSelected && (
          <div className="size-2.5 ml-px bg-primary rounded-full ring-3 ring-primary/50" />
        )}
      </div>
      <div className="flex w-full justify-between items-center gap-2">
        <div className="flex flex-col items-start gap-1 flex-1 min-w-0">
          <div className="flex items-center">
            <span className="font-medium text-sm text-left">{repo.name}</span>
            {repo?.has_uncommitted_changes && (
              <Tooltip>
                <TooltipTrigger>
                  <Badge variant={"warning"} className="ml-1">
                    <CircleDashed />
                  </Badge>
                </TooltipTrigger>
                <TooltipPopup className={"dark"}>
                  Uncommitted changes
                </TooltipPopup>
              </Tooltip>
            )}
          </div>
        </div>
        <div className="flex items-center gap-1 ml-1">
          {(repo.ahead_behind?.[0] || 0) > 0 ? (
            <Badge variant={"error"} className="ml-1">
              <ArrowUp />
              {repo.ahead_behind?.[0] || 0}
            </Badge>
          ) : null}
          {(repo.ahead_behind?.[1] || 0) > 0 ? (
            <Badge variant={"warning"} className="ml-1">
              <ArrowDown />
              {repo.ahead_behind?.[1] || 0}
            </Badge>
          ) : null}
          <Badge variant={"info"}>
            <GitBranch />
            {repo.current_branch}
          </Badge>
        </div>

        <Button
          variant={"ghost"}
          size={"icon-sm"}
          className="shrink-0"
          onClick={(e) => {
            e.stopPropagation();
            onRemove();
          }}
        >
          <Minus size={16} />
        </Button>
      </div>
    </button>
  );
}

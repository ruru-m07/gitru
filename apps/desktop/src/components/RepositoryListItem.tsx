import { type RepositoryInfo } from "@gitru/commands";
import { Badge } from "@gitru/ui/components/badge";
import { Button } from "@gitru/ui/components/button";
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuLabel,
  ContextMenuSeparator,
  ContextMenuTrigger,
} from "@gitru/ui/components/context-menu";
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
import { memo } from "react";

interface RepositoryListItemProps {
  repo: RepositoryInfo;
  isSelected: boolean;
  onSelect: () => void;
  onRemove: () => void;
  dataRepoId?: string;
}

export const RepositoryListItem = memo(function RepositoryListItem({
  repo,
  isSelected,
  onSelect,
  onRemove,
  dataRepoId,
}: RepositoryListItemProps) {
  return (
    <ContextMenu>
      <ContextMenuTrigger asChild>
        <button
          data-repo-id={dataRepoId}
          className={`py-1 px-2 flex w-full h-10 hover:bg-secondary cursor-pointer ${
            isSelected ? "bg-secondary hover:bg-accent" : ""
          }`}
          type="button"
          onClick={onSelect}
        >
          <div className="min-w-7 flex items-center _justify-center">
            {isSelected && (
              <div className="size-2.5 ml-px bg-primary rounded-full ring-3 ring-primary/50" />
            )}
          </div>
          <div className="flex w-full justify-between items-center gap-2 overflow-hidden">
            <div className="flex flex-col items-start gap-1 flex-1">
              <div className="flex items-center">
                <span className="font-medium text-sm text-left text-nowrap">
                  {repo.name}
                </span>
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
            <div className="flex items-center gap-1 ml-1 min-w-0">
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
              <Badge
                variant={"info"}
                className="flex items-center gap-1 min-w-0 flex-1"
              >
                <GitBranch />
                <span className="truncate max-w-full min-w-0">
                  {repo.current_branch}
                </span>
              </Badge>
            </div>
          </div>
        </button>
      </ContextMenuTrigger>
      <ContextMenuContent className="min-w-40">
        <ContextMenuLabel>{repo.name}</ContextMenuLabel>
        <ContextMenuSeparator />
        <ContextMenuItem asChild>
          <Button
            variant="ghost"
            size="sm"
            className="w-full justify-start"
            onClick={(e) => {
              e.stopPropagation();
              onRemove();
            }}
          >
            <Minus size={16} />
            Remove
          </Button>
        </ContextMenuItem>
      </ContextMenuContent>
    </ContextMenu>
  );
});

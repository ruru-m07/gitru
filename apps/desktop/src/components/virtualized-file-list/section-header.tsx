import { Badge } from "@gitru/ui/components/badge";
import { Button } from "@gitru/ui/components/button";
import { ChevronDown, ChevronRight, Minus, Plus, Undo2 } from "lucide-react";
import { memo } from "react";

import { SectionHeaderProps } from "./types";

export const SectionHeader = memo(function SectionHeader({
  sectionId,
  sectionName,
  sectionType,
  count,
  isExpanded,
  onToggle,
  actions,
}: SectionHeaderProps) {
  const isChangesSection = sectionType === "changes";
  const isStagedSection = sectionType === "staged";
  const isConflictSection = sectionType === "conflicted";

  return (
    <div className="sticky top-0 z-20">
      <div className="flex items-center hover:bg-accent/50 justify-between w-full pr-2">
        <button
          type="button"
          className="flex items-center gap-1.5 rounded pl-2.5 py-2 cursor-pointer flex-1 justify-start"
          onClick={() => onToggle(sectionId)}
        >
          {isExpanded ? (
            <ChevronDown size={16} className="text-muted-foreground" />
          ) : (
            <ChevronRight size={16} className="text-muted-foreground" />
          )}
          <span className="text-sm font-medium">{sectionName}</span>
        </button>
        <div className="flex items-center gap-1">
          {(isChangesSection || isConflictSection) && (
            <div className="flex items-center">
              {actions?.renderDiscardAll
                ? actions.renderDiscardAll()
                : actions?.onDiscardAll && (
                    <Button
                      onClick={(e) => {
                        e.stopPropagation();
                        actions.onDiscardAll?.();
                      }}
                      variant="ghost"
                      size="icon-sm"
                    >
                      <Undo2 size={18} strokeWidth={1.25} />
                    </Button>
                  )}
              {actions?.onAddAll && (
                <Button
                  onClick={async (e) => {
                    e.stopPropagation();
                    await actions.onAddAll?.();
                  }}
                  variant="ghost"
                  size="icon-sm"
                >
                  <Plus size={18} strokeWidth={1.25} />
                </Button>
              )}
            </div>
          )}
          {isStagedSection && actions?.onUnstageAll && (
            <Button
              variant="ghost"
              size="icon-sm"
              onClick={async (e) => {
                e.stopPropagation();
                await actions.onUnstageAll?.();
              }}
            >
              <Minus size={18} strokeWidth={1.25} />
            </Button>
          )}
          <Badge variant="secondary" className="tabular-nums font-mono text-xs">
            {count}
          </Badge>
        </div>
      </div>
    </div>
  );
});

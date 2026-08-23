import {
  Avatar,
  AvatarFallback,
  AvatarImage,
} from "@gitru/ui/components/avatar";
import {
  Tooltip,
  TooltipPopup,
  TooltipTrigger,
} from "@gitru/ui/components/tooltip";
import React from "react";
import { ProcessedRow } from "../../helper";

type CommittersCellProps = {
  row: ProcessedRow;
};

const CommittersCell = React.memo(({ row }: CommittersCellProps) => {
  const { row: commitRow } = row;

  return (
    <div
      data-cell
      data-cell-id={commitRow.oid}
      className="flex items-center gap-2 border-l px-2 min-w-0"
    >
      <div className="flex group">
        <Tooltip>
          <TooltipTrigger
            style={{
              zIndex: (commitRow.commit?.authors.co_authors.length || 0) + 1,
            }}
          >
            <Avatar className="ring-2 ring-background rounded-sm size-4.5">
              <AvatarImage
                alt={commitRow.commit?.authors.author.name}
                src={`https://avatars.githubusercontent.com/u/e?email=${commitRow.commit?.authors.author.email}&s=64`}
              />
              <AvatarFallback>
                {commitRow.commit?.authors.author.name
                  .split(" ")
                  .map((n) => n[0])
                  .join("")
                  .toUpperCase()}
              </AvatarFallback>
            </Avatar>
          </TooltipTrigger>
          <TooltipPopup side="bottom">
            {commitRow.commit?.authors.author.name}
          </TooltipPopup>
        </Tooltip>
        {commitRow.commit?.authors.co_authors
          .slice(0, 2)
          .map((coAuthor, idx) => (
            <Tooltip key={`${idx}-tooltip-coauthor`}>
              <TooltipTrigger
                style={{
                  zIndex: commitRow.commit?.authors.co_authors.length - idx,
                }}
                key={`${idx}-tooltip-trigger-coauthor`}
              >
                <Avatar className="will-change-auto ring-2 ring-background rounded-sm size-4.5 ml-[-0.2rem] group-hover:ml-0.5 transition-all duration-100">
                  <AvatarImage
                    alt="U1"
                    src={`https://avatars.githubusercontent.com/u/e?email=${coAuthor.email}&s=64`}
                  />
                  <AvatarFallback>
                    {coAuthor.name
                      .split(" ")
                      .map((n) => n[0])
                      .join("")
                      .toUpperCase()}
                  </AvatarFallback>
                </Avatar>
              </TooltipTrigger>
              <TooltipPopup side="bottom">{coAuthor.name}</TooltipPopup>
            </Tooltip>
          ))}
        {commitRow.commit?.authors.co_authors.length > 2 && (
          <Tooltip>
            <TooltipTrigger>
              <div className="will-change-auto flex items-center bg-secondary ring-2 ring-background rounded-sm h-4.5 px-1 -ml-1 transition-all duration-100">
                <span className="text-xs text-nowrap tabular-nums font-mono">
                  +{commitRow.commit?.authors.co_authors.length - 2}
                </span>
              </div>
            </TooltipTrigger>
            <TooltipPopup side="bottom">
              <div className="flex flex-col gap-2">
                {commitRow.commit?.authors.co_authors
                  .slice(2)
                  .map((coAuthor) => (
                    <div className="flex gap-1 items-center">
                      <Avatar className="will-change-auto ring-2 ring-background rounded-sm size-4.5 ml-[-0.2rem] group-hover:ml-0.5 transition-all duration-100">
                        <AvatarImage
                          alt="U1"
                          src={`https://avatars.githubusercontent.com/u/e?email=${coAuthor.email}&s=64`}
                        />
                        <AvatarFallback>
                          {coAuthor.name
                            .split(" ")
                            .map((n) => n[0])
                            .join("")
                            .toUpperCase()}
                        </AvatarFallback>
                      </Avatar>
                      <span>{coAuthor.name}</span>
                    </div>
                  ))}
              </div>
            </TooltipPopup>
          </Tooltip>
        )}
      </div>
    </div>
  );
});

export default CommittersCell;

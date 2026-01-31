import { BranchInfo } from "@gitru/commands";
import {
  Avatar,
  AvatarFallback,
  AvatarImage,
} from "@gitru/ui/components/avatar";
import { Badge } from "@gitru/ui/components/badge";
import {
  CommandListView,
  CommandViewConfig,
} from "@gitru/ui/components/command";
import { Kbd } from "@gitru/ui/components/kbd";
import {
  Tooltip,
  TooltipPopup,
  TooltipTrigger,
} from "@gitru/ui/components/tooltip";
import {
  ArrowDownIcon,
  ArrowUpIcon,
  Cloud,
  CornerDownLeftIcon,
  GitBranch,
  GitBranchPlus,
} from "lucide-react";
import { useGetBranches, useGetCurrentBranch } from "@/hooks";
import { timeAgoFromUnixSeconds } from "@/lib/time";

export interface BranchItem extends Partial<BranchInfo> {
  name: BranchInfo["name"];
  isActive?: boolean;
}

export function useBranchListView(): CommandViewConfig<
  "branch-list",
  BranchItem
> {
  const { data: branches } = useGetBranches("Local");
  const { data: currentBranch } = useGetCurrentBranch();

  function createNewBranchAction(query: string) {
    const action =
      query &&
      !(branches || []).some((b) =>
        b.name.toLowerCase().includes(query.toLowerCase()),
      )
        ? ({
            name: `Create Branch ${query}`,
            isActive: true,
          } satisfies BranchItem)
        : null;

    return action;
  }

  return {
    id: "branch-list",
    input: {
      placeholder: "Search branches...",
      autoFocus: true,
    },
    footer(context) {
      console.log(context);
      return (
        <>
          <div className="flex items-center gap-2">
            <span>Back</span>
            <Kbd>Esc</Kbd>
          </div>
          <div className="flex items-center gap-4">
            <div className="flex items-center gap-2">
              <span>Checkout</span>
              <Kbd>
                <CornerDownLeftIcon />
              </Kbd>
            </div>
          </div>
        </>
      );
    },
    command: {
      items: (context) => {
        const { query } = context;

        const action = createNewBranchAction(query);

        return action ? [action, ...(branches || [])] : branches || [];
      },
      filter: (item, query: string) => {
        if (!query) return true;
        const searchName = item?.name?.toLowerCase() || "";
        const q = query.toLowerCase();
        return searchName.includes(q);
      },
    },
    render: (context) => {
      const { query, navigate } = context;
      const action = createNewBranchAction(query);

      const allItems = action ? [action, ...(branches || [])] : branches || [];

      return (
        <CommandListView<BranchItem>
          items={allItems}
          getItemKey={(item) => item.name}
          getItemValue={(item) => item.name}
          showSeparators={false}
          renderItemContent={(item) => (
            <>
              {item.isActive ? (
                <div className="flex items-center gap-2 flex-1">
                  <GitBranchPlus className="size-4" />
                  <span>{item.name}</span>
                </div>
              ) : (
                <div className={"flex-col items-start w-full"}>
                  <div className="flex items-center justify-between w-full">
                    <div className="flex items-center">
                      {item.is_remote ? (
                        <Cloud className="mr-2 h-4 w-4" />
                      ) : (
                        <GitBranch className="mr-2 h-4 w-4" />
                      )}
                      <span className="flex-1">{item.display_name}</span>
                    </div>
                    <div className="flex items-center gap-1">
                      {item.name === currentBranch?.name && (
                        <Badge>Current</Badge>
                      )}
                      {item.ahead ? (
                        <Badge variant={"secondary"}>
                          <ArrowUpIcon className="inline-block size-3" />
                          {item.ahead}
                        </Badge>
                      ) : null}
                      {item.behind ? (
                        <Badge variant={"secondary"}>
                          <ArrowDownIcon className="inline-block size-3" />
                          {item.behind}
                        </Badge>
                      ) : null}
                    </div>
                  </div>
                  <div className="flex items-center gap-2">
                    <div className="flex group">
                      <Tooltip>
                        <TooltipTrigger
                          style={{
                            zIndex:
                              (item.commit?.authors.co_authors.length || 0) + 1,
                          }}
                        >
                          <Avatar className="ring-2 ring-background rounded-sm size-4">
                            <AvatarImage
                              alt={item.commit?.authors.author.name}
                              src={`https://avatars.githubusercontent.com/u/e?email=${item?.commit?.authors.author.email}&s=64`}
                            />
                            <AvatarFallback>
                              {item.commit?.authors.author.name
                                .split(" ")
                                .map((n) => n[0])
                                .join("")
                                .toUpperCase()}
                            </AvatarFallback>
                          </Avatar>
                        </TooltipTrigger>
                        <TooltipPopup side="bottom">
                          {item.commit?.authors.author.name}
                        </TooltipPopup>
                      </Tooltip>
                      {item.commit?.authors.co_authors.map((coAuthor, idx) => (
                        <Tooltip key={`${idx}-tooltip-coauthor`}>
                          <TooltipTrigger
                            style={{
                              zIndex:
                                (item.commit?.authors.co_authors.length || 0) -
                                idx,
                            }}
                            key={`${idx}-tooltip-trigger-coauthor`}
                          >
                            <Avatar className="ring-2 ring-background rounded-sm size-4 -ml-[0.2rem] group-hover:ml-0.5 transition-all duration-100">
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
                          <TooltipPopup side="bottom">
                            {coAuthor.name}
                          </TooltipPopup>
                        </Tooltip>
                      ))}
                    </div>
                    <span className="font-normal">
                      {item.commit?.authors.author.name}
                    </span>
                    <span className="text-muted-foreground font-light text-xs shrink-0">
                      ( {timeAgoFromUnixSeconds(item.commit?.timestamp || 0)} )
                    </span>
                    <span className="text-muted-foreground font-light text-xs truncate max-w-96">
                      {"• \u00A0  "} {item.commit?.summary}
                    </span>
                  </div>
                </div>
              )}
            </>
          )}
          onSelect={(item) => {
            if (item.isActive) {
              navigate.push("create-branch", { initialName: query });
            } else {
              navigate.push("confirm-checkout", { branch: item.name });
            }
          }}
          emptyState={() => (
            <div className="flex flex-col items-center gap-2 py-8">
              <p className="text-sm text-muted-foreground">No branches found</p>
            </div>
          )}
        />
      );
    },
  };
}

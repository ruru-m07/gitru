import { RepositoryInfo } from "@gitru/commands";
import {
  Avatar,
  AvatarFallback,
  AvatarImage,
} from "@gitru/ui/components/avatar";
import { Badge } from "@gitru/ui/components/badge";
import {
  CommandItem,
  CommandListView,
  CommandViewConfig,
} from "@gitru/ui/components/command";
import { EmptyMedia } from "@gitru/ui/components/empty";
import { Kbd, KbdGroup } from "@gitru/ui/components/kbd";
import {
  Tooltip,
  TooltipPopup,
  TooltipTrigger,
} from "@gitru/ui/components/tooltip";
import {
  ArrowDown,
  ArrowDownIcon,
  ArrowUp,
  ArrowUpIcon,
  BadgeQuestionMark,
  CircleDashed,
  CornerDownLeftIcon,
  FolderDot,
  GitBranch,
  SearchIcon,
} from "lucide-react";
import { useRepositories } from "@/hooks/use-repositories";
import { getAvatarByProvider } from "@/lib/get-avatar-by-git-provider";
import { parseOrigin } from "@/lib/parse-origin";
import { selectActiveRepository, useAppStore } from "@/store/use-app-store";

export function useSwitchRepositoryView(): CommandViewConfig<
  "switch-repository",
  RepositoryInfo
> {
  const { repositories } = useRepositories();

  const setSelectedRepository = useAppStore(
    (state) => state.setSelectedRepository,
  );
  const activeRepository = useAppStore(selectActiveRepository);

  return {
    id: "switch-repository",
    input: {
      placeholder: "Search repositories...",
      autoFocus: true,
    },
    footer() {
      return (
        <>
          <div className="flex items-center gap-2">
            <span>Close</span>
            <Kbd>Esc</Kbd>
          </div>
          <div className="flex items-center gap-4">
            <div className="flex items-center gap-2">
              <span>Navigate</span>
              <KbdGroup>
                <Kbd>
                  <ArrowUpIcon />
                </Kbd>
                <Kbd>
                  <ArrowDownIcon />
                </Kbd>
              </KbdGroup>
            </div>
            <div className="flex items-center gap-2">
              <span>Open</span>
              <Kbd>
                <CornerDownLeftIcon />
              </Kbd>
            </div>
          </div>
        </>
      );
    },
    command: {
      items: () => {
        return repositories.sort((a, b) =>
          a.id === activeRepository?.id
            ? -1
            : b.id === activeRepository?.id
              ? 1
              : a.name.localeCompare(b.name),
        );
      },
      getItemValue: (item) => {
        const a = item;
        return `${item.path} ${a.name}`.trim();
      },
      filter: (value: unknown, query: string) => {
        if (!query) return true;
        const q = query.toLowerCase();
        return String(value).toLowerCase().includes(q);
      },
    },
    render: (context) => {
      const { query } = context;

      return (
        <CommandListView<RepositoryInfo>
          items={context.filteredCommandItems as RepositoryInfo[]}
          showSeparators={false}
          itemsArePreFiltered={context.filteredCommandItems !== undefined}
          filter={context.filter}
          getItemKey={(item) => `${item.name} ${item.path}`.trim()}
          getItemValue={(item) => `${item.name} ${item.path}`.trim()}
          renderItem={(item) => {
            const origin = parseOrigin(item.origin || "");
            const icon = getAvatarByProvider(origin?.provider);

            return (
              <CommandItem
                onClick={() => {
                  setSelectedRepository(item);
                  context.close();
                }}
                className={"items-start"}
                value={item.name}
              >
                <div className="flex flex-col">
                  <span className="text-muted-foreground flex items-center">
                    <div className="size-3.5 text-lg text-foreground mr-1">
                      {icon || <BadgeQuestionMark className="size-3.5" />}
                    </div>
                    {origin ? (
                      <div>
                        <span>/</span>
                        <Avatar className="rounded-sm size-4 -translate-y-px mx-1">
                          <AvatarImage alt="User" src={origin.avatarUrl} />
                          <AvatarFallback>
                            {origin.owner.charAt(0).toUpperCase()}
                          </AvatarFallback>
                        </Avatar>
                        <span>{origin.owner}</span>
                        <span>/</span>
                        <span className="text-foreground">{origin.repo}</span>
                        {item?.has_uncommitted_changes && (
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
                    ) : (
                      <div>
                        <span className="text-foreground">{item.name}</span>
                      </div>
                    )}
                  </span>
                  <span className="text-sm text-muted-foreground inline-flex items-center gap-1">
                    <FolderDot className="size-4" />
                    {/* we remove first "/" if at starting */}
                    {item.path.replace(/^\/+/, "")}
                  </span>
                </div>
                <div className="ml-auto flex gap-1">
                  {activeRepository?.id === item.id && (
                    <Badge variant={"success"} className="ml-1">
                      current
                    </Badge>
                  )}
                  {(item.ahead_behind?.[0] || 0) > 0 ? (
                    <Badge variant={"error"} className="ml-1">
                      <ArrowUp />
                      {item.ahead_behind?.[0] || 0}
                    </Badge>
                  ) : null}
                  {(item.ahead_behind?.[1] || 0) > 0 ? (
                    <Badge variant={"warning"} className="ml-1">
                      <ArrowDown />
                      {item.ahead_behind?.[1] || 0}
                    </Badge>
                  ) : null}
                  <Badge variant={"info"} className="ml-1">
                    <GitBranch />
                    {item.current_branch}
                  </Badge>
                </div>
              </CommandItem>
            );
          }}
          onSelect={async (item) => {
            setSelectedRepository(item);
            context.close();
          }}
          emptyState={() => (
            <div className="wrap-break-word flex flex-col flex-wrap items-center gap-2">
              <EmptyMedia variant="icon">
                <SearchIcon />
              </EmptyMedia>
              <p>
                No results found for{" "}
                <strong className="font-medium text-foreground">{query}</strong>
              </p>
            </div>
          )}
        />
      );
    },
  };
}

import {
  CommandListView,
  CommandShortcut,
  CommandViewConfig,
} from "@gitru/ui/components/command";
import { EmptyMedia } from "@gitru/ui/components/empty";
import { Kbd, KbdGroup } from "@gitru/ui/components/kbd";
import { useNavigate } from "@tanstack/react-router";
import {
  ArrowDownIcon,
  ArrowDownToLine,
  ArrowUpFromLine,
  ArrowUpIcon,
  CornerDownLeftIcon,
  FolderGit2,
  FolderRoot,
  GitBranchIcon,
  GitBranchPlus,
  GitPullRequestArrow,
  Inbox,
  PlusIcon,
  RefreshCcw,
  SearchIcon,
  Settings,
} from "lucide-react";
import { toast } from "sonner";
import { useGitFetch, useGitPull, useGitPush } from "@/hooks";

export interface ActionItem {
  id: string;
  label: string;
  shortcut?: string[];
  keywords?: string[];
  icon?: React.ReactNode;
  iconKey?: string;
  redirect?: string;
}

const iconMap: Record<string, React.ReactNode> = {
  inbox: <Inbox className="size-4" />,
  pullRequest: <GitPullRequestArrow className="size-4" />,
  issues: <Settings className="size-4" />,
  localGit: <FolderGit2 className="size-4" />,
  plus: <PlusIcon className="size-4" />,
  gitbranchPlus: <GitBranchPlus className="size-4" />,
  gitBranch: <GitBranchIcon className="size-4" />,
  repositories: <FolderRoot className="size-4" />,
  arrowDownToLine: <ArrowDownToLine className="size-4" />,
  arrowUpFromLine: <ArrowUpFromLine className="size-4" />,
  refreshCcw: <RefreshCcw className="size-4" />,
};

export function useRootView(): CommandViewConfig<"root", ActionItem> {
  const { mutateAsync: fetch } = useGitFetch();
  const { mutateAsync: pull } = useGitPull();
  const { mutateAsync: push } = useGitPush();

  const tanstackNavigate = useNavigate();

  return {
    id: "root",
    input: {
      placeholder: "Search actions or branches...",
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
        return [
          {
            id: "inbox",
            label: "Inbox",
            shortcut: ["G", "N"],
            iconKey: "inbox",
            keywords: ["inbox", "mail", "messages"],
            redirect: "/app/inbox",
          },
          {
            id: "pull-request",
            label: "Pull request",
            shortcut: ["G", "P"],
            iconKey: "pullRequest",
            keywords: ["pull", "request", "pr"],
            redirect: "/app/pulls",
          },
          {
            id: "issues",
            label: "Issues",
            shortcut: ["G", "I"],
            iconKey: "issues",
            keywords: ["issues", "bug", "tracker"],
            redirect: "/app/issues",
          },
          {
            id: "local-git",
            label: "Local git",
            shortcut: ["G", "G"],
            iconKey: "localGit",
            keywords: ["git", "repository", "version control"],
            redirect: "/app/git",
          },
          {
            id: "new-branch",
            label: "New Branch",
            shortcut: ["⌘", "⇧", "N"],
            value: "new-branch",
            keywords: ["create", "new", "branch"],
            iconKey: "gitbranchPlus",
          },
          {
            id: "checkout-branch",
            label: "Checkout Branch",
            shortcut: ["⌘", "⇧", "C"],
            keywords: ["switch", "checkout"],
            value: "checkout-branch",
            iconKey: "gitBranch",
          },
          {
            id: "fetch-changes",
            label: "Fetch Changes",
            shortcut: ["⌘", "⇧", "F"],
            value: "fetch-changes",
            iconKey: "refreshCcw",
            onClick() {
              toast.promise(fetch(), {
                loading: "Fetching changes...",
                success: (data) =>
                  data.success
                    ? "Fetch completed"
                    : (data.message ?? "Fetch failed"),
                error: (err) => err ?? "Fetch error",
              });
            },
          },
          {
            id: "pull-changes",
            label: "Pull Changes",
            shortcut: ["⌘", "⇧", "P"],
            value: "pull-changes",
            iconKey: "arrowDownToLine",
            onClick() {
              toast.promise(pull(), {
                loading: "Pulling changes...",
                success: (data) =>
                  data.success
                    ? "Pull completed"
                    : (data.message ?? "Pull failed"),
                error: (err) => err ?? "Pull error",
              });
            },
          },
          {
            id: "push-changes",
            label: "Push Changes",
            shortcut: ["⌘", "⇧", "U"],
            value: "push-changes",
            iconKey: "arrowUpFromLine",
            onClick() {
              toast.promise(push(), {
                loading: "Pushing changes...",
                success: (data) => data,
                error: (err) => err ?? "Push error",
              });
            },
          },
          {
            id: "switch-repository",
            label: "Switch Repository",
            shortcut: ["⌘", "⇧", "R"],
            iconKey: "repositories",
          },
        ];
      },
      getItemValue: (item) => {
        const a = item;
        return `${a.label} ${(a.keywords ?? []).join(" ")}`.trim();
      },
      filter: (value: unknown, query: string) => {
        if (!query) return true;
        const q = query.toLowerCase();
        return String(value).toLowerCase().includes(q);
      },
    },
    render: (context) => {
      const { query } = context;
      const navigate = context.navigate;

      return (
        <CommandListView
          items={context.filteredCommandItems as ActionItem[] | undefined}
          showSeparators={false}
          itemsArePreFiltered={context.filteredCommandItems !== undefined}
          filter={context.filter}
          getItemKey={(item: ActionItem) => item.id}
          getItemValue={(item: ActionItem) =>
            `${item.label} ${(item.keywords ?? []).join(" ")}`.trim()
          }
          renderItemContent={(item: ActionItem) => (
            <div className="flex items-center gap-2 flex-1">
              {item.iconKey && iconMap[item.iconKey]}
              <span className="flex-1">{item.label}</span>
              {item.shortcut && (
                <CommandShortcut className="gap-2 flex">
                  {item.shortcut.map((key) => (
                    <Kbd key={key}>{key}</Kbd>
                  ))}
                </CommandShortcut>
              )}
            </div>
          )}
          onSelect={(item: ActionItem) => {
            if (item.redirect) {
              tanstackNavigate({
                to: item.redirect,
              });
            } else if (item.id === "new-branch") {
              navigate.push("create-branch");
            } else if (item.id === "checkout-branch") {
              navigate.push("branch-list");
            }
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

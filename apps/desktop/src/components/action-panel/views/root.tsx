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
  Download,
  FolderGit2,
  FolderRoot,
  GitBranchIcon,
  GitBranchPlus,
  GitPullRequestArrow,
  Inbox,
  Pickaxe,
  PlusIcon,
  RefreshCcw,
  SearchIcon,
  Settings,
  SwatchBook,
} from "lucide-react";
import { toast } from "sonner";
import {
  useGetStatusAheadBehind,
  useGitFetch,
  useGitPublishBranch,
  useGitPull,
  useGitPush,
} from "@/hooks";
import { selectActiveRepositoryPath, useAppStore } from "@/store/use-app-store";

export interface ActionItem {
  id: string;
  label: string;
  shortcut?: string[];
  keywords?: string[];
  icon?: React.ReactNode;
  iconKey?: string;
  redirect?: string;
  onCallBack?: () => Promise<void> | void;
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
  theme: <SwatchBook className="size-4" />,
  updates: <Download className="size-4" />,
  search: <SearchIcon className="size-4" />,
  pickaxe: <Pickaxe className="size-4" />,
};

export function useRootView(): CommandViewConfig<"root", ActionItem> {
  const { mutateAsync: fetch } = useGitFetch();
  const { mutateAsync: pull } = useGitPull();
  const { mutateAsync: push } = useGitPush();
  const { mutateAsync: publishBranch } = useGitPublishBranch();

  const { data: aheadBehind } = useGetStatusAheadBehind();

  const tanstackNavigate = useNavigate();
  const activeRepositoryPath = useAppStore(selectActiveRepositoryPath);
  const setMainWindowView = useAppStore((state) => state.setMainWindowView);
  const setGitViewStateForRepo = useAppStore(
    (state) => state.setGitViewStateForRepo,
  );
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
      items: (ctx) => {
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
            keywords: ["git", "version control"],
            redirect: "/app/git",
          },
          {
            id: "pickaxe",
            label: "Pickaxe",
            shortcut: ["G", "T"],
            iconKey: "pickaxe",
            keywords: ["search", "history", "grep", "pickaxe", "log -S"],
            async onCallBack() {
              setMainWindowView("Pickaxe");
              tanstackNavigate({ to: "/app/git" });
              ctx.close();
            },
          },
          {
            id: "stash-changes",
            label: "Stashed Changes",
            shortcut: ["G", "S"],
            iconKey: "localGit",
            keywords: ["stash", "stashed", "restore"],
            async onCallBack() {
              if (activeRepositoryPath) {
                setGitViewStateForRepo(
                  {
                    leftPanelView: "stash",
                    stashViewMode: "all",
                  },
                  activeRepositoryPath,
                );
              }

              tanstackNavigate({ to: "/app/git" });
              ctx.close();
            },
          },
          {
            id: "new-branch",
            label: "New Branch",
            shortcut: ["⌘", "⇧", "N"],
            keywords: ["create", "new", "branch"],
            iconKey: "gitbranchPlus",
          },
          {
            id: "checkout-branch",
            label: "Checkout Branch",
            shortcut: ["⌘", "⇧", "C"],
            keywords: ["switch", "checkout"],
            iconKey: "gitBranch",
          },
          {
            id: "rebase-onto",
            label: "Rebase Onto…",
            shortcut: ["⌘", "⇧", "R"],
            keywords: ["rebase", "interactive", "onto", "squash"],
            iconKey: "gitBranch",
          },
          {
            id: "fetch-changes",
            label: "Fetch Changes",
            shortcut: ["⌘", "⇧", "F"],
            iconKey: "refreshCcw",
            async onCallBack() {
              toast.promise(fetch(), {
                loading: "Fetching changes...",
                success: (data) => {
                  ctx.close();
                  return data;
                },
                error: (err) => err ?? "Fetch error",
              });
            },
          },
          {
            id: "pull-changes",
            label: "Pull Changes",
            shortcut: ["⌘", "⇧", "P"],
            iconKey: "arrowDownToLine",
            async onCallBack() {
              toast.promise(pull(), {
                loading: "Pulling changes...",
                success: (data) => {
                  ctx.close();

                  return data;
                },
                error: (err) => err ?? "Pull error",
              });
            },
          },
          {
            id: "push-changes",
            label: `Push Changes ${aheadBehind?.is_published ? (aheadBehind?.ahead ? `(${aheadBehind.ahead})` : "") : "(published branch)"}`,
            shortcut: ["⌘", "⇧", "U"],
            iconKey: "arrowUpFromLine",
            async onCallBack() {
              if (aheadBehind && !aheadBehind.is_published) {
                toast.promise(publishBranch(), {
                  loading: "Publishing branch...",
                  success: (data) => {
                    ctx.close();

                    return data;
                  },
                  error: (err) => err ?? "Publish error",
                });
              } else if (aheadBehind && aheadBehind.ahead > 0) {
                toast.promise(push(), {
                  loading: "Pushing changes...",
                  success: (data) => {
                    ctx.close();

                    return data;
                  },
                  error: (err) => err ?? "Push error",
                });
              } else {
                toast.info("No changes to push");
              }
            },
          },
          {
            id: "switch-repository",
            label: "Switch Repository",
            shortcut: ["⌘", "⇧", "R"],
            iconKey: "repositories",
          },
          {
            id: "clone-repository",
            label: "Clone Repository",
            shortcut: ["⌘", "⇧", "L"],
            iconKey: "repositories",
            keywords: ["clone", "repository", "remote", "git clone"],
          },
          {
            id: "init-repository",
            label: "Initialize Repository",
            shortcut: ["⌘", "⇧", "I"],
            iconKey: "repositories",
            keywords: ["init", "initialize", "repository", "git init"],
          },
          {
            id: "switch-theme",
            label: "Switch Theme",
            shortcut: ["⌘", "⇧", "T"],
            iconKey: "theme",
            keywords: ["theme", "color", "appearance", "dark", "light"],
          },
          {
            id: "switch-update-channel",
            label: "Switch Update Channel",
            iconKey: "updates",
            keywords: ["update", "channel", "stable", "beta"],
          },
        ] satisfies ActionItem[];
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
          onSelect={async (item: ActionItem) => {
            if (item.redirect) {
              tanstackNavigate({
                to: item.redirect,
              });
              context.close();
            } else if (item.onCallBack) {
              await item.onCallBack();
            } else if (item.id === "new-branch") {
              navigate.push("create-branch");
            } else if (item.id === "switch-repository") {
              navigate.push("switch-repository");
            } else if (item.id === "clone-repository") {
              navigate.push("clone-repository");
            } else if (item.id === "init-repository") {
              navigate.push("init-repository");
            } else if (item.id === "checkout-branch") {
              navigate.push("branch-list");
            } else if (item.id === "rebase-onto") {
              navigate.push("rebase-onto");
            } else if (item.id === "switch-theme") {
              navigate.push("switch-theme");
            } else if (item.id === "switch-update-channel") {
              navigate.push("switch-update-channel");
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

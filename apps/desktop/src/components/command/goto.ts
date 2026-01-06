import { FolderGit2, GitPullRequestArrow, Inbox, Settings } from "lucide-react";
import { Item } from "./type";

export const goto: Item[] = [
  {
    label: "Inbox",
    shortcut: ["G", "N"],
    value: "inbox",
    icon: Inbox,
    redirect: "/app/inbox",
  },
  {
    label: "Pull request",
    shortcut: ["G", "P"],
    value: "pull-request",
    icon: GitPullRequestArrow,
    redirect: "/app/pulls",
  },
  {
    label: "Issues",
    shortcut: ["G", "I"],
    value: "issues",
    icon: Settings,
    redirect: "/app/issues",
  },
  {
    label: "Local git",
    shortcut: ["G", "G"],
    value: "git",
    icon: FolderGit2,
    redirect: "/app/git",
  },
];

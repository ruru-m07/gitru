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

export const actions: Item[] = [
  { label: "Next diff", shortcut: ["⌘", "↑"], value: "next-diff" },
  { label: "Previous diff", shortcut: ["⌘", "↓"], value: "previous-diff" },
  { label: "New Branch", shortcut: ["⌘", "⇧", "N"], value: "new-branch" },
  {
    label: "Checkout Branch",
    shortcut: ["⌘", "⇧", "C"],
    value: "checkout-branch",
  },
  {
    label: "Switch Repository",
    shortcut: ["⌘", "⇧", "R"],
    value: "switch-repository",
  },
  { label: "Pull Changes", shortcut: ["⌘", "⇧", "P"], value: "pull-changes" },
  { label: "Push Changes", shortcut: ["⌘", "⇧", "U"], value: "push-changes" },
];

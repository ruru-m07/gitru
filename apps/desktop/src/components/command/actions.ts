import { toast } from "sonner";
import { useGitFetch, useGitPull, useGitPush } from "@/hooks";
import { Item } from "./type";

export function useActions(): Item[] {
  const { mutateAsync: fetch } = useGitFetch();
  const { mutateAsync: pull } = useGitPull();
  const { mutateAsync: push } = useGitPush();

  return [
    {
      label: "Next diff",
      shortcut: ["⌘", "↑"],
      value: "next-diff",
    },
    {
      label: "Previous diff",
      shortcut: ["⌘", "↓"],
      value: "previous-diff",
    },
    {
      label: "New Branch",
      shortcut: ["⌘", "⇧", "N"],
      value: "new-branch",
    },
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
    {
      label: "Fetch Changes",
      shortcut: ["⌘", "⇧", "F"],
      value: "fetch-changes",
      onClick() {
        toast.promise(fetch(), {
          loading: "Fetching changes...",
          success: (data) =>
            data.success ? "Fetch completed" : (data.message ?? "Fetch failed"),
          error: (err) => err ?? "Fetch error",
        });
      },
    },
    {
      label: "Pull Changes",
      shortcut: ["⌘", "⇧", "P"],
      value: "pull-changes",
      onClick() {
        toast.promise(pull(), {
          loading: "Pulling changes...",
          success: (data) =>
            data.success ? "Pull completed" : (data.message ?? "Pull failed"),
          error: (err) => err ?? "Pull error",
        });
      },
    },
    {
      label: "Push Changes",
      shortcut: ["⌘", "⇧", "U"],
      value: "push-changes",
      onClick() {
        toast.promise(push(), {
          loading: "Pushing changes...",
          success: (data) =>
            data.success ? "Push completed" : (data.message ?? "Push failed"),
          error: (err) => err ?? "Push error",
        });
      },
    },
  ];
}

import {
  AlertDialog,
  AlertDialogClose,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogPopup,
  AlertDialogTitle,
} from "@gitru/ui/components/alert-dialog";
import { Button } from "@gitru/ui/components/button";
import {
  CommandListView,
  type CommandViewConfig,
} from "@gitru/ui/components/command";
import { Kbd } from "@gitru/ui/components/kbd";
import {
  CloudDownload,
  GitBranch,
  GitBranchMinus,
  Pencil,
  Trash2,
  Unlink,
} from "lucide-react";
import { type ReactNode, useState } from "react";
import { toast } from "sonner";
import { branchDeleteBlockReason } from "@/features/git/lib/branch-action-availability";
import {
  useGetBranches,
  useGetCurrentBranch,
  useGitDeleteBranch,
  useGitFetch,
  useGitUnsetBranchUpstream,
} from "@/hooks";

export interface BranchTarget {
  branchName: string;
  isRemote: boolean;
}

interface BranchActionItem {
  id:
    | "checkout"
    | "rename"
    | "set-upstream"
    | "unset-upstream"
    | "fetch"
    | "delete";
  label: string;
  description: string;
  icon: ReactNode;
  disabledReason?: string;
}

export function useBranchActionsView(): CommandViewConfig<
  "branch-actions",
  BranchTarget
> {
  const { data: localBranches } = useGetBranches("Local");
  const { data: remoteBranches } = useGetBranches("Remote");
  const { data: currentBranch } = useGetCurrentBranch();
  const deleteBranch = useGitDeleteBranch();
  const unsetUpstream = useGitUnsetBranchUpstream();
  const fetch = useGitFetch();
  const [deleteOpen, setDeleteOpen] = useState(false);

  return {
    id: "branch-actions",
    input: { placeholder: "Branch actions...", autoFocus: false },
    header: () => <div />,
    render: (context) => {
      const { props, navigate, close } = context;
      const branches = props.isRemote ? remoteBranches : localBranches;
      const branch = branches?.find(
        (candidate) => candidate.name === props.branchName,
      );
      const currentInfo = localBranches?.find(
        (candidate) => candidate.name === currentBranch?.name,
      );

      if (!branch) {
        return (
          <div className="p-6 text-sm text-muted-foreground">
            This branch is no longer available. Fetch and try again.
          </div>
        );
      }

      const deleteBlockReason = branchDeleteBlockReason(
        branch,
        currentBranch?.name,
        currentInfo?.upstream,
      );
      const actions: BranchActionItem[] = [
        ...(!branch.is_head
          ? [
              {
                id: "checkout" as const,
                label: branch.is_remote
                  ? "Checkout and track"
                  : "Checkout branch",
                description: branch.is_remote
                  ? "Create a local branch that tracks this remote branch."
                  : "Switch the working tree to this branch.",
                icon: <GitBranch />,
              },
            ]
          : []),
        ...(!branch.is_remote
          ? [
              {
                id: "rename" as const,
                label: "Rename branch",
                description:
                  "Rename this local branch and keep its upstream configuration.",
                icon: <Pencil />,
              },
              {
                id: "set-upstream" as const,
                label: branch.upstream ? "Change upstream" : "Set upstream",
                description: branch.upstream
                  ? `Currently tracking ${branch.upstream}.`
                  : "Choose a remote branch to track.",
                icon: <CloudDownload />,
              },
              ...(branch.upstream
                ? [
                    {
                      id: "unset-upstream" as const,
                      label: "Unset upstream",
                      description: `Stop tracking ${branch.upstream}.`,
                      icon: <Unlink />,
                    },
                  ]
                : []),
            ]
          : []),
        {
          id: "fetch" as const,
          label: "Fetch and prune",
          description:
            "Refresh remote branches and remove stale tracking refs.",
          icon: <GitBranchMinus />,
        },
        {
          id: "delete" as const,
          label: branch.is_remote
            ? "Delete remote branch"
            : "Delete local branch",
          description: branch.is_merged
            ? "Delete this fully merged branch."
            : "This branch has commits that are not merged into HEAD.",
          icon: <Trash2 />,
          disabledReason: deleteBlockReason ?? undefined,
        },
      ];

      const runDelete = async () => {
        const result = await deleteBranch.mutateAsync({
          branch: branch.name,
          isRemote: branch.is_remote,
          force: !branch.is_merged,
        });
        toast.success(result);
        setDeleteOpen(false);
        close();
      };

      return (
        <>
          <div className="px-4 py-3 border-b">
            <p className="text-xs text-muted-foreground">
              {branch.is_remote ? "Remote branch" : "Local branch"}
            </p>
            <p className="font-medium truncate">{branch.display_name}</p>
          </div>
          <CommandListView
            items={actions}
            showSeparators={false}
            getItemKey={(item) => item.id}
            getItemValue={(item) => `${item.label} ${item.description}`}
            renderItemContent={(item) => (
              <div className="flex items-start gap-3 py-1 w-full">
                <span className="mt-0.5 text-muted-foreground [&_svg]:size-4">
                  {item.icon}
                </span>
                <span className="flex flex-col min-w-0">
                  <span>{item.label}</span>
                  <span className="text-xs text-muted-foreground">
                    {item.disabledReason ?? item.description}
                  </span>
                </span>
              </div>
            )}
            onSelect={async (item) => {
              if (item.disabledReason) {
                toast.error(item.disabledReason);
                return;
              }
              if (item.id === "checkout") {
                navigate.push("confirm-checkout", { branch: branch.name });
              } else if (item.id === "rename") {
                navigate.push("rename-branch", { branchName: branch.name });
              } else if (item.id === "set-upstream") {
                navigate.push("select-upstream", { branchName: branch.name });
              } else if (item.id === "unset-upstream") {
                const result = await unsetUpstream.mutateAsync({
                  branch: branch.name,
                });
                toast.success(result);
                close();
              } else if (item.id === "fetch") {
                const result = await fetch.mutateAsync();
                toast.success(result);
              } else if (item.id === "delete") {
                setDeleteOpen(true);
              }
            }}
          />
          <div className="flex justify-end border-t px-4 py-3">
            <Button type="button" variant="outline" onClick={navigate.back}>
              <Kbd>Esc</Kbd>
              Back
            </Button>
          </div>
          <AlertDialog open={deleteOpen} onOpenChange={setDeleteOpen}>
            <AlertDialogPopup>
              <AlertDialogHeader>
                <AlertDialogTitle>
                  Delete {branch.is_remote ? "remote" : "local"} branch?
                </AlertDialogTitle>
                <AlertDialogDescription>
                  {branch.is_merged
                    ? `Delete ${branch.name}? The branch tip is already reachable from HEAD.`
                    : `Force delete ${branch.name}? It contains commits that are not merged into HEAD.`}
                </AlertDialogDescription>
              </AlertDialogHeader>
              <AlertDialogFooter>
                <AlertDialogClose
                  render={<Button type="button" variant="outline" />}
                >
                  Cancel
                </AlertDialogClose>
                <Button
                  type="button"
                  variant="destructive"
                  disabled={deleteBranch.isPending}
                  onClick={() => void runDelete()}
                >
                  {branch.is_merged ? "Delete branch" : "Force delete branch"}
                </Button>
              </AlertDialogFooter>
            </AlertDialogPopup>
          </AlertDialog>
        </>
      );
    },
  };
}

import type { BranchInfo } from "@gitru/commands";
import { Badge } from "@gitru/ui/components/badge";
import {
  CommandListView,
  type CommandViewConfig,
} from "@gitru/ui/components/command";
import { Cloud, CornerDownLeft } from "lucide-react";
import { toast } from "sonner";
import { useGetBranches, useGitSetBranchUpstream } from "@/hooks";

export interface SelectUpstreamProps {
  branchName: string;
}

export function useSelectUpstreamView(): CommandViewConfig<
  "select-upstream",
  SelectUpstreamProps
> {
  const { data: remoteBranches } = useGetBranches("Remote");
  const setUpstream = useGitSetBranchUpstream();

  return {
    id: "select-upstream",
    input: { placeholder: "Search remote branches...", autoFocus: true },
    render: (context) => (
      <CommandListView<BranchInfo>
        items={remoteBranches ?? undefined}
        showSeparators={false}
        getItemKey={(branch) => branch.name}
        getItemValue={(branch) => branch.name}
        renderItemContent={(branch) => (
          <div className="flex items-center gap-2 w-full">
            <Cloud className="size-4 text-muted-foreground" />
            <span className="truncate flex-1">{branch.display_name}</span>
            {branch.is_protected ? <Badge>Default</Badge> : null}
          </div>
        )}
        getItemShortcut={() => <CornerDownLeft className="size-3" />}
        onSelect={async (branch) => {
          const result = await setUpstream.mutateAsync({
            branch: context.props.branchName,
            upstream: branch.name,
          });
          toast.success(result);
          context.close();
        }}
        emptyState={() => (
          <p className="text-sm text-muted-foreground">
            No remote branches found.
          </p>
        )}
      />
    ),
  };
}

import { Button } from "@gitru/ui/components/button";
import { CommandPanel, CommandViewConfig } from "@gitru/ui/components/command";
import { Kbd } from "@gitru/ui/components/kbd";
import { GitBranchPlus } from "lucide-react";
import { toast } from "sonner";
import { useGetStatusAheadBehind, useRebasePlan, useRebaseStart } from "@/hooks";

export function useRebaseOntoView(): CommandViewConfig<"rebase-onto", undefined> {
  const { data: aheadBehind } = useGetStatusAheadBehind();
  const { mutateAsync: plan } = useRebasePlan();
  const { mutateAsync: start } = useRebaseStart();

  const defaultOnto = aheadBehind?.upstream_branch || "origin/main";

  return {
    id: "rebase-onto",
    input: {
      placeholder: "Rebase onto (branch, tag, or commit)…",
      autoFocus: true,
    },
    render: (context) => {
      const { query, navigate, close } = context;
      const onto = (query.trim() || defaultOnto).trim();

      const runPlain = async (autostash: boolean) => {
        await toast.promise(
          start({
            onto,
            autostash,
          }),
          {
            loading: "Rebasing…",
            success: (op) =>
              op.isRebasing ? "Rebase paused — resolve to continue" : "Rebase finished",
            error: (e) => e?.message ?? e ?? "Rebase failed",
          },
        );
        close();
      };

      const runInteractive = async () => {
        try {
          const built = await plan({ onto });
          if (built.entries.length === 0) {
            toast.message("Nothing to rebase onto that target");
            return;
          }
          await toast.promise(
            start({
              onto,
              upstream: built.upstream,
              entries: built.entries,
              autostash: false,
            }),
            {
              loading: "Starting interactive rebase…",
              success: "Interactive rebase started",
              error: (e) => e?.message ?? e ?? "Failed to start rebase",
            },
          );
          close();
        } catch (error) {
          toast.error(
            error instanceof Error ? error.message : "Failed to plan rebase",
          );
        }
      };

      return (
        <CommandPanel className="p-4">
          <div className="flex flex-col gap-4">
            <div className="flex gap-2 items-start">
              <GitBranchPlus className="size-5 mt-0.5 shrink-0" />
              <div>
                <p className="font-medium">Rebase onto</p>
                <p className="text-sm text-muted-foreground mt-1">
                  Target:{" "}
                  <code className="text-xs bg-muted px-1.5 py-0.5 rounded">
                    {onto}
                  </code>
                </p>
                <p className="text-sm text-muted-foreground mt-2">
                  Plain rebase replays all commits. Interactive opens the
                  GitLens-style todo planner powered by Gitru&apos;s sequencer.
                </p>
              </div>
            </div>

            <div className="flex justify-between gap-2 flex-wrap">
              <Button onClick={() => navigate.back()} variant="outline">
                <Kbd>Esc</Kbd>
                Back
              </Button>
              <div className="flex gap-2 flex-wrap justify-end">
                <Button variant="secondary" onClick={() => runPlain(true)}>
                  Autostash + rebase
                </Button>
                <Button variant="secondary" onClick={() => runInteractive()}>
                  Interactive
                </Button>
                <Button onClick={() => runPlain(false)}>Rebase</Button>
              </div>
            </div>
          </div>
        </CommandPanel>
      );
    },
  };
}

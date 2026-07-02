import { Button } from "@gitru/ui/components/button";
import { Separator } from "@gitru/ui/components/separator";
import { ArrowUpFromLine, ChevronDown, ChevronsUp, GitBranch, Loader2 } from "lucide-react";
import { useGetCurrentBranch, useGetStatusAheadBehind, useGitPush } from "@/hooks";

export function MainActionBar() {
  const { data: currentBranch } = useGetCurrentBranch();
  const { data: statusAheadBehind } = useGetStatusAheadBehind();

  const { mutateAsync: push, isPending } = useGitPush();

  return (
    <div className="w-full justify-between min-h-14 max-h-14 h-14 border-b flex">
      <div className="min-h-14 max-h-14 h-14 flex w-full">
        <Button
          className="flex justify-between items-center min-h-full rounded-none border-x-0 max-w-72 w-full"
          variant="ghost"
        >
          <div className="flex items-center gap-4 min-w-0 flex-1">
            <GitBranch className="size-7.5" strokeWidth={1.5} />

            <div className="flex flex-col items-start min-w-0 flex-1">
              <span className="text-xs text-muted-foreground font-[450]">
                Current Branch
              </span>
              <span className="truncate block w-full text-left">
                {currentBranch?.display_name}
              </span>
            </div>
          </div>

          <ChevronDown size={18} />
        </Button>
        <Separator orientation="vertical" className={"border-0"} />
        {statusAheadBehind && statusAheadBehind.is_published ? (
          (statusAheadBehind && statusAheadBehind.ahead > 0) ||
          (statusAheadBehind && statusAheadBehind.behind > 0) ? (
            <>
              <Button
                className="flex justify-between items-center min-h-full rounded-none border-x-0 w-72"
                variant={"ghost"}
                onClick={async () => {
                  await push();
                }}
              >
                <div className="flex items-center justify-center gap-4">
                  {isPending ? (
                    <Loader2
                      className="animate-spin size-7.5"
                      strokeWidth={1.5}
                    />
                  ) : (
                    <ChevronsUp className="size-8 rotate-180" />
                  )}
                  <div className="flex-col flex items-start">
                    <span className="text-xs text-muted-foreground font-[450]">
                      {statusAheadBehind.ahead > 0
                        ? "Push to Origin"
                        : "Pull from Origin"}
                    </span>
                    <span>
                      {statusAheadBehind
                        ? `${statusAheadBehind.ahead} / ${statusAheadBehind.behind}`
                        : "0 / 0"}
                    </span>
                  </div>
                </div>
                <ChevronDown size={18} />
              </Button>
              <Separator orientation="vertical" className={"border-0"} />
            </>
          ) : null
        ) : (
          <>
            <Button
              className="flex border-x-0 justify-between items-center min-h-full rounded-none max-w-60 w-60"
              variant={"ghost"}
              onClick={async () => {
                await push();
              }}
            >
              <div className="flex items-center gap-4 min-w-0 flex-1">
                {isPending ? (
                  <Loader2
                    className="animate-spin size-7.5"
                    strokeWidth={1.5}
                  />
                ) : (
                  <ArrowUpFromLine className="size-7.5" strokeWidth={1.5} />
                )}
                <div className="flex flex-col flex-1 items-start min-w-0">
                  <span className="text-xs text-muted-foreground font-normal">
                    Publish Branch
                  </span>
                  <span className="truncate block w-full text-left">
                    Published as {currentBranch?.name}
                  </span>
                </div>
              </div>
            </Button>
            <Separator orientation="vertical" className={"border-0"} />
            <Button
              className="flex border-x-0 justify-between items-center min-h-full rounded-none"
              variant={"ghost"}
              onClick={async () => {}}
            >
              <ChevronDown size={18} />
            </Button>
            <Separator orientation="vertical" className={"border-0"} />
          </>
        )}
      </div>
      <div></div>
    </div>
  );
};

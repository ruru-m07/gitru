import { Button } from "@gitru/ui/components/button";
import { CommandPanel, CommandViewConfig } from "@gitru/ui/components/command";
import { Kbd, KbdGroup } from "@gitru/ui/components/kbd";
import { cn } from "@gitru/ui/lib/utils";
import { CornerDownLeft, GitBranch, TriangleAlert } from "lucide-react";
import { useHotkeys } from "react-hotkeys-hook";
import { toast } from "sonner";
import {
  useGetCurrentBranch,
  useGitCreateBranch,
  useHasUncommittedChanges,
} from "@/hooks";

interface CreateBranchProps {
  branchName?: string;
  quickAction?: boolean;
}

type CreateBranchStrategy = "default" | "stash" | "bring";

export function useCreateBranchView(): CommandViewConfig<
  "create-branch",
  CreateBranchProps
> {
  const { data: hasUncommittedChanges } = useHasUncommittedChanges();
  const { mutateAsync: createBranch } = useGitCreateBranch();
  const { data: currentBranch } = useGetCurrentBranch();

  const handleCreate = async (
    branchName: string,
    strategy: CreateBranchStrategy,
  ) => {
    const strategyConfig = {
      default: {
        request: { branchName },
        messages: {
          loading: `Creating new branch ${branchName}...`,
          success: `Created and checked out ${branchName}`,
          error: "Create and checkout error",
        },
      },
      stash: {
        request: { branchName, strategy: "StashOnCurrentBranch" as const },
        messages: {
          loading: `Stashing changes and creating new branch ${branchName}...`,
          success: `Stashed changes and created new branch ${branchName}`,
          error: "Stash and checkout error",
        },
      },
      bring: {
        request: { branchName, strategy: "BringChanges" as const },
        messages: {
          loading: `Bringing changes and creating new branch ${branchName}...`,
          success: `Brought changes and created new branch ${branchName}`,
          error: "Bring and checkout error",
        },
      },
    };

    const config = strategyConfig[strategy];

    await toast.promise(createBranch(config.request), {
      loading: config.messages.loading,
      success: config.messages.success,
      error: (err) => err ?? config.messages.error,
    });
  };

  return {
    id: "create-branch",
    input: {
      placeholder: "Enter branch name...",
      autoFocus: false,
    },
    header: (context) => {
      const { props } = context;
      return props?.quickAction ? <div /> : null;
    },
    render: (context) => {
      const { query, navigate, close, props } = context;

      const branchName = props?.quickAction
        ? props.branchName!
        : simplifyBranchName(query);

      useHotkeys(
        "enter",
        async () => {
          if (!props?.quickAction || hasUncommittedChanges) return;
          await handleCreate(branchName, "default");
          close();
        },
        { enabled: !!props?.quickAction, enableOnFormTags: true },
      );

      useHotkeys(
        "shift+enter",
        async () => {
          if (!props?.quickAction || !hasUncommittedChanges) return;
          await handleCreate(branchName, "stash");
          close();
        },
        { enabled: !!props?.quickAction },
      );

      useHotkeys(
        "mod+enter",
        async () => {
          if (!props?.quickAction || !hasUncommittedChanges) return;
          await handleCreate(branchName, "bring");
          close();
        },
        { enabled: !!props?.quickAction },
      );

      return (
        <CommandPanel className="p-4">
          <div className="flex flex-col gap-4">
            {/* Header Section */}
            <div>
              <div className="flex gap-2 items-center">
                {hasUncommittedChanges ? (
                  <TriangleAlert size={20} className="text-amber-600" />
                ) : (
                  <GitBranch size={20} />
                )}
                <p className="font-normal">
                  {hasUncommittedChanges ? (
                    <>
                      Uncommitted changes in
                      <span className="ml-1 text-amber-500 font-[550]">
                        {currentBranch?.name}
                      </span>
                      <span className="mx-1">{" → "}</span>
                      <span className="text-green-500 font-[550]">
                        {branchName}
                      </span>
                    </>
                  ) : (
                    <>
                      Create Branch <b>{currentBranch?.name}</b>
                      <span className="mx-1">{" → "}</span>
                      <span className="bg-muted font-sans px-1.5 py-0.5 rounded">
                        {branchName}
                      </span>
                    </>
                  )}
                </p>
              </div>
              <p className="text-sm mt-2">
                {hasUncommittedChanges ? (
                  "You have uncommitted changes that haven't been committed yet."
                ) : (
                  <>
                    Are you sure you want to checkout{" "}
                    <code className="text-xs bg-muted px-1.5 py-0.5 rounded">
                      {branchName}
                    </code>
                    ?
                  </>
                )}
              </p>
            </div>

            {/* Actions Section */}
            <div
              className={cn(
                "flex justify-end gap-2",
                hasUncommittedChanges && "justify-between",
              )}
            >
              <Button onClick={() => navigate.back()} variant="outline">
                <Kbd>Esc</Kbd>
                Back
              </Button>

              <div className="flex items-center gap-2">
                {hasUncommittedChanges ? (
                  <>
                    <Button
                      onClick={async () => {
                        await handleCreate(branchName, "stash");
                        close();
                      }}
                      variant="outline"
                      disabled={!branchName}
                    >
                      <KbdGroup className="-me-1">
                        <Kbd>Shift</Kbd>
                        <Kbd>
                          <CornerDownLeft className="size-3" />
                        </Kbd>
                      </KbdGroup>
                      Stash & Create
                    </Button>
                    <Button
                      onClick={async () => {
                        await handleCreate(branchName, "bring");
                        close();
                      }}
                      variant="default"
                      disabled={!branchName}
                    >
                      <KbdGroup className="-me-1">
                        <Kbd className="bg-background/20 text-primary-foreground">
                          &#8984;
                        </Kbd>
                        <Kbd className="bg-background/20 text-primary-foreground">
                          <CornerDownLeft className="size-3" />
                        </Kbd>
                      </KbdGroup>
                      Bring & Create
                    </Button>
                  </>
                ) : (
                  <Button
                    onClick={async () => {
                      await handleCreate(branchName, "default");
                      close();
                    }}
                    variant="default"
                    disabled={!branchName}
                  >
                    <Kbd className="bg-background/20 text-primary-foreground">
                      <CornerDownLeft className="size-3" />
                    </Kbd>
                    Create & Checkout
                  </Button>
                )}
              </div>
            </div>
          </div>
        </CommandPanel>
      );
    },
  };
}

// ? if we give name like "hello world" it becomes "hello-world"
export function simplifyBranchName(name: string) {
  return name.trim().replace(/\s+/g, "-");
}

import { Button } from "@gitru/ui/components/button";
import { CommandPanel, CommandViewConfig } from "@gitru/ui/components/command";
import { Kbd, KbdGroup } from "@gitru/ui/components/kbd";
import { cn } from "@gitru/ui/lib/utils";
import { CornerDownLeft, GitBranch, TriangleAlert } from "lucide-react";
import { useHotkeys } from "react-hotkeys-hook";
import { toast } from "sonner";
import { useGitSwitchBranch, useHasUncommittedChanges } from "@/hooks";

export function useConfirmCheckoutView(): CommandViewConfig<
  "confirm-checkout",
  undefined
> {
  const { data: hasUncommittedChanges } = useHasUncommittedChanges();
  const { mutateAsync: switchBranch } = useGitSwitchBranch();

  const handleCheckout = async (branch: string) => {
    toast.promise(
      async () => {
        await switchBranch({
          branchName: branch,
        });
      },
      {
        loading: `Checking out ${branch}...`,
        success: `Checked out ${branch}`,
        error: (err) => err ?? "Checkout error",
      },
    );
  };

  const handleStashAndCheckout = async (branch: string) => {
    toast.promise(
      async () => {
        await switchBranch({
          branchName: branch,
          strategy: "StashOnCurrentBranch",
        });
      },
      {
        loading: `Stashing changes and checking out ${branch}...`,
        success: `Stashed changes and checked out ${branch}`,
        error: (err) => err ?? "Stash and checkout error",
      },
    );
  };

  const handleBringAndCheckout = async (branch: string) => {
    toast.promise(
      async () => {
        await switchBranch({
          branchName: branch,
          strategy: "BringChanges",
        });
      },
      {
        loading: `Bringing changes and checking out ${branch}...`,
        success: `Brought changes and checked out ${branch}`,
        error: (err) => err ?? "Bring and checkout error",
      },
    );
  };

  return {
    id: "confirm-checkout",
    input: {
      placeholder: "Confirm checkout...",
      autoFocus: false,
    },
    header() {
      return <div />;
    },
    render: (context) => {
      const { props, navigate, close } = context;
      const branch = (props as any)?.branch || "unknown";

      useHotkeys("enter", async () => {
        if (hasUncommittedChanges) {
          return;
        }
        await handleCheckout(branch).then(() => {
          close();
        });
      }, []);

      useHotkeys("shift+enter", async () => {
        if (!hasUncommittedChanges) {
          return;
        }
        await handleStashAndCheckout(branch).then(() => {
          close();
        });
      }, []);

      useHotkeys("mod+enter", async () => {
        if (!hasUncommittedChanges) {
          return;
        }
        await handleBringAndCheckout(branch).then(() => {
          close();
        });
      }, []);

      return (
        <CommandPanel className="p-4">
          <div className="flex flex-col gap-4">
            <div>
              <div className="flex gap-2 items-center">
                {hasUncommittedChanges ? (
                  <TriangleAlert size={20} className="text-amber-600" />
                ) : (
                  <GitBranch size={20} />
                )}
                <p className="font-medium _text-muted-foreground">
                  {hasUncommittedChanges ? "Uncommitted changes" : "Checkout"}
                  <span className="ml-1 font-[550] text-foreground">
                    "{branch}"
                  </span>
                </p>
              </div>
              {hasUncommittedChanges ? (
                <p className="text-sm mt-2">
                  You have uncommitted changes that haven’t been committed yet.
                </p>
              ) : (
                <p className="text-sm mt-2">
                  Are you sure you want to checkout{" "}
                  <code className="text-xs bg-muted px-1.5 py-0.5 rounded">
                    {branch}
                  </code>
                  ?
                </p>
              )}
            </div>
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
                        await handleStashAndCheckout(branch).then(() => {
                          close();
                        });
                      }}
                      variant="outline"
                    >
                      <KbdGroup className="-me-1">
                        <Kbd>Shift</Kbd>
                        <Kbd>
                          <CornerDownLeft className="size-3" />
                        </Kbd>
                      </KbdGroup>
                      Stash & Checkout
                    </Button>
                    <Button
                      onClick={async () => {
                        await handleBringAndCheckout(branch).then(() => {
                          close();
                        });
                      }}
                      variant="default"
                    >
                      <KbdGroup className="-me-1">
                        <Kbd className="bg-background/20 text-primary-foreground">
                          &#8984;
                        </Kbd>
                        <Kbd className="bg-background/20 text-primary-foreground">
                          <CornerDownLeft className="size-3" />
                        </Kbd>
                      </KbdGroup>
                      Bring & Checkout
                    </Button>
                  </>
                ) : (
                  <Button
                    onClick={async () => {
                      await handleCheckout(branch).then(() => {
                        close();
                      });
                    }}
                    variant="default"
                  >
                    <Kbd className="bg-background/20 text-primary-foreground">
                      <CornerDownLeft className="size-3" />
                    </Kbd>
                    Checkout
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

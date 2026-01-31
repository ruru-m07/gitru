import { Button } from "@gitru/ui/components/button";
import { CommandPanel, CommandViewConfig } from "@gitru/ui/components/command";
import { Kbd } from "@gitru/ui/components/kbd";
import { CornerDownLeft } from "lucide-react";

export function useConfirmCheckoutView(): CommandViewConfig<
  "confirm-checkout",
  undefined
> {
  return {
    id: "confirm-checkout",
    input: {
      placeholder: "Confirm checkout...",
      autoFocus: false,
    },
    render: (context) => {
      const { props, navigate, close } = context;
      const branch = (props as any)?.branch || "unknown";

      return (
        <CommandPanel className="p-4">
          <div className="flex flex-col gap-4">
            <div>
              <p className="font-medium">Checkout branch?</p>
              <p className="text-sm mt-2">
                Are you sure you want to checkout{" "}
                <code className="text-xs bg-muted px-1.5 py-0.5 rounded">
                  {branch}
                </code>
                ?
              </p>
            </div>
            <div className="flex justify-end gap-2">
              <Button onClick={() => navigate.back()} variant="outline">
                <Kbd>Esc</Kbd>
                Back
              </Button>
              <Button
                onClick={() => {
                  console.log("Checking out:", branch);
                  close();
                }}
                variant="default"
                autoFocus
              >
                <Kbd className="bg-background/20 text-primary-foreground">
                  <CornerDownLeft className="size-3" />
                </Kbd>
                Checkout
              </Button>
            </div>
          </div>
        </CommandPanel>
      );
    },
  };
}

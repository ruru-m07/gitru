import { Button } from "@gitru/ui/components/button";
import { CommandViewConfig } from "@gitru/ui/components/command";

export function useCreateBranchView(): CommandViewConfig<
  "create-branch",
  undefined
> {
  return {
    id: "create-branch",
    input: {
      placeholder: "Enter branch name...",
      autoFocus: true,
    },
    render: (context) => {
      const { query, navigate, close } = context;

      return (
        <div className="p-4 flex flex-col gap-4">
          <div>
            <label className="text-sm font-medium">New branch name</label>
            <p className="text-xs text-muted-foreground mt-1">
              {query || "Type a name..."}
            </p>
          </div>
          <div className="flex gap-2">
            <Button
              onClick={() => {
                if (query.trim()) {
                  console.log("Creating branch:", query);
                  close();
                }
              }}
              variant="default"
              className="flex-1"
            >
              Create
            </Button>
            <Button onClick={() => navigate.back()} variant="outline">
              Cancel
            </Button>
          </div>
        </div>
      );
    },
  };
}

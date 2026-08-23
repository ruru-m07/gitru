import { Button } from "@gitru/ui/components/button";
import { cn } from "@gitru/ui/lib/utils";
import { ChevronDown, ChevronUp } from "lucide-react";
import {
  selectActiveRepoSelectIsOpen,
  selectActiveRepository,
  useAppStore,
} from "@/store/use-app-store";

export const TogglePanelButton = () => {
  const repoSelectIsOpen = useAppStore(selectActiveRepoSelectIsOpen);
  const setRepoSelectIsOpen = useAppStore((state) => state.setRepoSelectIsOpen);
  const activeRepository = useAppStore(selectActiveRepository);

  return (
    <Button
      onClick={() => {
        setRepoSelectIsOpen(!repoSelectIsOpen);
      }}
      className={cn(
        "rounded-none justify-between min-h-13.75 max-h-13.75 pl-2",
        repoSelectIsOpen && "bg-accent",
      )}
      variant={"ghost"}
    >
      <div className="flex-col flex items-start">
        <span className="text-xs text-muted-foreground font-[430]">
          Current Repository
        </span>
        <span>{activeRepository?.name || "No repository selected"}</span>
      </div>
      {repoSelectIsOpen ? <ChevronUp size={18} /> : <ChevronDown size={18} />}
    </Button>
  );
};

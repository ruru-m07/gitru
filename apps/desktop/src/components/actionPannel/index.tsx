import {
  CommandPanelRoot,
  CommandViewConfig,
  createCommandViewRegistry,
} from "@gitru/ui/components/command";
import { useState } from "react";
import { BranchItem, useBranchListView } from "./views/branch-list";
import { useConfirmCheckoutView } from "./views/confirm-checkout";
import { useCreateBranchView } from "./views/create-branch";
import { ActionItem, useRootView } from "./views/root";

type RootAction = CommandViewConfig<"root", ActionItem>;
type BranchListAction = CommandViewConfig<"branch-list", BranchItem>;
type CreateBranchAction = CommandViewConfig<"create-branch", undefined>;
type ConfirmCheckoutAction = CommandViewConfig<"confirm-checkout", undefined>;

type Action =
  | RootAction
  | BranchListAction
  | CreateBranchAction
  | ConfirmCheckoutAction;

export const ActionPannel = () => {
  const [open, setOpen] = useState(false);

  const views = [
    useRootView(),
    useBranchListView(),
    useCreateBranchView(),
    useConfirmCheckoutView(),
  ] as const satisfies Action[];

  const viewRegistry = createCommandViewRegistry(views);

  return (
    <CommandPanelRoot
      open={open}
      onOpenChange={setOpen}
      views={viewRegistry}
      initialViewId="root"
      resetOnOpen={true}
      className="max-w-2xl"
    />
  );
};

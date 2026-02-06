import { RepositoryInfo } from "@gitru/commands";
import {
  CommandPanelRoot,
  CommandViewConfig,
  createCommandViewRegistry,
} from "@gitru/ui/components/command";
import { useState } from "react";
import { BranchItem, useBranchListView } from "./views/branch-list";
import { useConfirmCheckoutView } from "./views/confirm-checkout";
import { CreateBranchProps, useCreateBranchView } from "./views/create-branch";
import { ActionItem, useRootView } from "./views/root";
import { useSwitchRepositoryView } from "./views/switch-repository";

type RootAction = CommandViewConfig<"root", ActionItem>;
type BranchListAction = CommandViewConfig<"branch-list", BranchItem>;
type CreateBranchAction = CommandViewConfig<"create-branch", CreateBranchProps>;
type ConfirmCheckoutAction = CommandViewConfig<"confirm-checkout", undefined>;
type SwitchRepositoryAction = CommandViewConfig<
  "switch-repository",
  RepositoryInfo
>;

type Action =
  | RootAction
  | BranchListAction
  | CreateBranchAction
  | ConfirmCheckoutAction
  | SwitchRepositoryAction;

export const ActionPannel = () => {
  const [open, setOpen] = useState(false);

  const views = [
    useRootView(),
    useBranchListView(),
    useCreateBranchView(),
    useConfirmCheckoutView(),
    useSwitchRepositoryView(),
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

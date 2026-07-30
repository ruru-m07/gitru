import { RepositoryInfo } from "@gitru/commands";
import {
  CommandPanelRoot,
  CommandViewConfig,
  createCommandViewRegistry,
} from "@gitru/ui/components/command";
import { BranchItem, useBranchListView } from "./views/branch-list";
import { useCloneRepositoryView } from "./views/clone-repository";
import { useConfirmCheckoutView } from "./views/confirm-checkout";
import { CreateBranchProps, useCreateBranchView } from "./views/create-branch";
import { useInitRepositoryView } from "./views/init-repository";
import { ActionItem, useRootView } from "./views/root";
import { useSwitchRepositoryView } from "./views/switch-repository";
import { ThemeItem, useSwitchThemeView } from "./views/switch-theme";
import {
  type UpdateChannelItem,
  useSwitchUpdateChannelView,
} from "./views/switch-update-channel";

type RootAction = CommandViewConfig<"root", ActionItem>;
type BranchListAction = CommandViewConfig<"branch-list", BranchItem>;
type CreateBranchAction = CommandViewConfig<"create-branch", CreateBranchProps>;
type ConfirmCheckoutAction = CommandViewConfig<"confirm-checkout", undefined>;
type CloneRepositoryAction = CommandViewConfig<"clone-repository", undefined>;
type InitRepositoryAction = CommandViewConfig<"init-repository", undefined>;
type SwitchRepositoryAction = CommandViewConfig<
  "switch-repository",
  RepositoryInfo
>;
type SwitchThemeAction = CommandViewConfig<"switch-theme", ThemeItem>;
type SwitchUpdateChannelAction = CommandViewConfig<
  "switch-update-channel",
  UpdateChannelItem
>;

type Action =
  | RootAction
  | BranchListAction
  | CreateBranchAction
  | ConfirmCheckoutAction
  | CloneRepositoryAction
  | InitRepositoryAction
  | SwitchRepositoryAction
  | SwitchThemeAction
  | SwitchUpdateChannelAction;

export const ActionPanel = ({ children }: { children: React.ReactNode }) => {
  const views = [
    useRootView(),
    useBranchListView(),
    useCreateBranchView(),
    useConfirmCheckoutView(),
    useCloneRepositoryView(),
    useInitRepositoryView(),
    useSwitchRepositoryView(),
    useSwitchThemeView(),
    useSwitchUpdateChannelView(),
  ] as const satisfies Action[];

  const viewRegistry = createCommandViewRegistry(views);

  return (
    <CommandPanelRoot
      views={viewRegistry}
      initialViewId="root"
      resetOnOpen={true}
      className="max-w-2xl"
    >
      {children}
    </CommandPanelRoot>
  );
};

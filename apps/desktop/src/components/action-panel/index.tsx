import type { RepositoryInfo } from "@gitru/commands";
import {
  CommandPanelRoot,
  CommandViewConfig,
  createCommandViewRegistry,
} from "@gitru/ui/components/command";
import {
  type BranchTarget,
  useBranchActionsView,
} from "./views/branch-actions";
import { type BranchListProps, useBranchListView } from "./views/branch-list";
import { useCloneRepositoryView } from "./views/clone-repository";
import { useConfirmCheckoutView } from "./views/confirm-checkout";
import {
  type CreateBranchProps,
  useCreateBranchView,
} from "./views/create-branch";
import { useInitRepositoryView } from "./views/init-repository";
import { useRebaseOntoView } from "./views/rebase-onto";
import {
  type RenameBranchProps,
  useRenameBranchView,
} from "./views/rename-branch";
import { type ActionItem, useRootView } from "./views/root";
import {
  type SelectUpstreamProps,
  useSelectUpstreamView,
} from "./views/select-upstream";
import { useSwitchRepositoryView } from "./views/switch-repository";
import { type ThemeItem, useSwitchThemeView } from "./views/switch-theme";
import {
  type UpdateChannelItem,
  useSwitchUpdateChannelView,
} from "./views/switch-update-channel";

type RootAction = CommandViewConfig<"root", ActionItem>;
type BranchListAction = CommandViewConfig<"branch-list", BranchListProps>;
type BranchActionsAction = CommandViewConfig<"branch-actions", BranchTarget>;
type CreateBranchAction = CommandViewConfig<"create-branch", CreateBranchProps>;
type RenameBranchAction = CommandViewConfig<"rename-branch", RenameBranchProps>;
type SelectUpstreamAction = CommandViewConfig<
  "select-upstream",
  SelectUpstreamProps
>;
type ConfirmCheckoutAction = CommandViewConfig<"confirm-checkout", undefined>;
type CloneRepositoryAction = CommandViewConfig<"clone-repository", undefined>;
type InitRepositoryAction = CommandViewConfig<"init-repository", undefined>;
type RebaseOntoAction = CommandViewConfig<"rebase-onto", undefined>;
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
  | BranchActionsAction
  | CreateBranchAction
  | RenameBranchAction
  | SelectUpstreamAction
  | ConfirmCheckoutAction
  | CloneRepositoryAction
  | InitRepositoryAction
  | RebaseOntoAction
  | SwitchRepositoryAction
  | SwitchThemeAction
  | SwitchUpdateChannelAction;

export const ActionPanel = ({ children }: { children: React.ReactNode }) => {
  const views = [
    useRootView(),
    useBranchListView(),
    useBranchActionsView(),
    useCreateBranchView(),
    useRenameBranchView(),
    useSelectUpstreamView(),
    useConfirmCheckoutView(),
    useCloneRepositoryView(),
    useInitRepositoryView(),
    useRebaseOntoView(),
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

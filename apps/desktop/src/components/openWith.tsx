import { openWithApp } from "@gitru/commands";
import { CursorIcon, GhosttyIcon, VSCodeIcon } from "@gitru/icon";
import { Button } from "@gitru/ui/components/button";
import { Group, GroupSeparator } from "@gitru/ui/components/group";
import {
  Menu,
  MenuItem,
  MenuPopup,
  MenuTrigger,
} from "@gitru/ui/components/menu";
import { Check, ChevronDownIcon } from "lucide-react";
import {
  useGetCurrentBranchStash,
  useGetStatus,
  useStashList,
  useStashShow,
} from "@/hooks";
import { resolveFileSelection } from "@/lib/gitSelectionResolver";
import { ExternalOpener, useAppStore } from "@/store/useAppStore";

const EXTERNAL_OPENER_OPTIONS: {
  value: ExternalOpener;
  label: string;
  buttonLabel: string;
}[] = [
  { value: "vscode", label: "VS Code", buttonLabel: "VS Code" },
  { value: "cursor", label: "Cursor", buttonLabel: "Cursor" },
  { value: "finder", label: "Finder", buttonLabel: "Finder" },
  { value: "terminal", label: "Terminal", buttonLabel: "Terminal" },
  { value: "ghostty", label: "Ghostty", buttonLabel: "Ghostty" },
];

const DEFAULT_OPENER: ExternalOpener = "vscode";

const getOpenerOption = (opener: ExternalOpener) =>
  EXTERNAL_OPENER_OPTIONS.find((option) => option.value === opener) ||
  EXTERNAL_OPENER_OPTIONS[0];

function getAppIcon(appName: string) {
  switch (appName) {
    case "vscode":
      return <VSCodeIcon />;
    case "cursor":
      return <CursorIcon />;
    case "ghostty":
      return <GhosttyIcon />;
    case "finder":
      return <GhosttyIcon />;
    case "terminal":
      return <GhosttyIcon />;
    default:
      return <></>;
  }
}

const OpenWith = () => {
  const selectedRepository = useAppStore((state) => state.selectedRepository);
  const selectionByRepo = useAppStore((state) => state.selectionByRepo);
  const gitViewByRepo = useAppStore((state) => state.gitViewByRepo);
  const preferredExternalOpener = useAppStore(
    (state) => state.preferredExternalOpener,
  );
  const setPreferredExternalOpener = useAppStore(
    (state) => state.setPreferredExternalOpener,
  );

  const repoPath = selectedRepository?.path ?? "";
  const gitViewState = gitViewByRepo[repoPath];
  const activeSource =
    gitViewState?.leftPanelView === "stash" ? "stash" : "worktree";
  const { data: currentBranchStash } = useGetCurrentBranchStash();
  const { data: status } = useGetStatus();
  const { data: stashes } = useStashList();
  const activeStashReference =
    gitViewState?.leftPanelView === "stash"
      ? gitViewState?.stashViewMode === "branch"
        ? (currentBranchStash?.reference ?? null)
        : (gitViewState?.selectedStashReference ?? null)
      : null;
  const { data: stashShow } = useStashShow(activeStashReference);
  const activeSelection =
    activeSource === "stash" && activeStashReference
      ? (selectionByRepo[repoPath]?.stashByReference[activeStashReference] ??
        null)
      : (selectionByRepo[repoPath]?.worktree ?? null);
  const resolvedSelection = resolveFileSelection({
    selection: activeSelection,
    files:
      activeSource === "stash"
        ? (stashShow?.files ?? [])
        : (status?.files ?? []),
    context: {
      source: activeSource,
      stashReference: activeStashReference,
      availableStashReferences: (stashes ?? []).map((stash) => stash.reference),
    },
  });
  const canOpen = resolvedSelection.state === "valid";

  const selectedOpenerOption = getOpenerOption(preferredExternalOpener);

  const openSelectedFile = async (opener: ExternalOpener) => {
    if (!canOpen || !selectedRepository?.path) return;
    setPreferredExternalOpener(opener);
    const selectedFile = resolvedSelection.identity;

    await openWithApp({
      filePath: `${selectedRepository.path}/${selectedFile.fileNewPath || selectedFile.filePath}`,
      app: opener,
    });
  };

  return (
    <Group aria-label="Repository actions">
      <Button
        onClick={async () => {
          await openSelectedFile(preferredExternalOpener || DEFAULT_OPENER);
        }}
        variant="outline"
        size={"xs"}
        className="pl-2.5"
        disabled={!canOpen}
      >
        {getAppIcon(selectedOpenerOption.value)}
        <span className="ml-1">{selectedOpenerOption.buttonLabel}</span>
      </Button>
      <GroupSeparator />
      <Menu>
        <MenuTrigger
          render={<Button aria-label="Menu" size="icon-xs" variant="outline" />}
        >
          <ChevronDownIcon aria-hidden="true" />
        </MenuTrigger>
        <MenuPopup align="end">
          {EXTERNAL_OPENER_OPTIONS.map((option) => (
            <MenuItem
              key={option.value}
              onClick={async () => {
                await openSelectedFile(option.value);
              }}
              className="justify-between gap-6"
              disabled={!canOpen}
            >
              <div className="flex items-center gap-2">
                <div className="size-4">{getAppIcon(option.value)}</div>
                <span>{option.label}</span>
              </div>
              {option.value === preferredExternalOpener ? (
                <Check size={14} />
              ) : null}
            </MenuItem>
          ))}
        </MenuPopup>
      </Menu>
    </Group>
  );
};

export default OpenWith;

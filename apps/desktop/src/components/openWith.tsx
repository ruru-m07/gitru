import { ExternalOpener, useAppStore } from '@/store/useAppStore';
import {
  Menu,
  MenuItem,
  MenuPopup,
  MenuTrigger,
} from "@gitru/ui/components/menu";
import { openWithApp } from "@gitru/commands";
import { CursorIcon, GhosttyIcon, VSCodeIcon } from "@gitru/icon";
import { Group, GroupSeparator } from '@gitru/ui/components/group';
import { Button } from '@gitru/ui/components/button';
import { Check, ChevronDownIcon } from 'lucide-react';

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
  const selectedFileByRepo = useAppStore((state) => state.selectedFileByRepo);
  const preferredExternalOpener = useAppStore(
    (state) => state.preferredExternalOpener,
  );
  const setPreferredExternalOpener = useAppStore(
    (state) => state.setPreferredExternalOpener,
  );

  const selectedFile = selectedFileByRepo[selectedRepository?.path || ""];
    
  const selectedOpenerOption = getOpenerOption(preferredExternalOpener);

  const openSelectedFile = async (opener: ExternalOpener) => {
    if (!selectedFile?.filePath || !selectedRepository?.path) return;
    setPreferredExternalOpener(opener);

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
          >
            {getAppIcon(selectedOpenerOption.value)}
            <span className="ml-1">{selectedOpenerOption.buttonLabel}</span>
          </Button>
          <GroupSeparator />
          <Menu>
            <MenuTrigger
              render={
                <Button aria-label="Menu" size="icon-xs" variant="outline" />
              }
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
  )
}

export default OpenWith
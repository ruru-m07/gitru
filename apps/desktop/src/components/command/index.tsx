"use client";

import {
  Command,
  CommandCollection,
  CommandDialog,
  CommandDialogPopup,
  CommandEmpty,
  CommandFooter,
  CommandGroup,
  CommandGroupLabel,
  CommandInput,
  CommandItem,
  CommandList,
  CommandPanel,
  CommandSeparator,
  CommandShortcut,
} from "@gitru/ui/components/command";
import { Kbd, KbdGroup } from "@gitru/ui/components/kbd";
import { FileRouteTypes, useNavigate } from "@tanstack/react-router";
import {
  ArrowDownIcon,
  ArrowUpIcon,
  CornerDownLeftIcon,
  FolderGit2,
  GitPullRequestArrow,
  Inbox,
  LucideIcon,
  Settings,
} from "lucide-react";
import * as React from "react";
import { useHotkeys } from "react-hotkeys-hook";

export interface Item {
  value: string;
  label: string;
  shortcut?: string | string[];
  icon?: LucideIcon;
  redirect?: FileRouteTypes["to"];
}

export interface Group {
  value: string;
  items: Item[];
}

export const goto: Item[] = [
  {
    label: "Inbox",
    shortcut: ["G", "N"],
    value: "inbox",
    icon: Inbox,
    redirect: "/app/inbox",
  },
  {
    label: "Pull request",
    shortcut: ["G", "P"],
    value: "pull-request",
    icon: GitPullRequestArrow,
    redirect: "/app/pulls",
  },
  {
    label: "Issues",
    shortcut: ["G", "I"],
    value: "issues",
    icon: Settings,
    redirect: "/app/issues",
  },
  {
    label: "Local git",
    shortcut: ["G", "G"],
    value: "git",
    icon: FolderGit2,
    redirect: "/app/git",
  },
];

export const actions: Item[] = [
  { label: "Next diff", shortcut: ["⌘", "↑"], value: "next-diff" },
  { label: "Previous diff", shortcut: ["⌘", "↓"], value: "previous-diff" },
  { label: "New Branch", shortcut: ["⌘", "⇧", "N"], value: "new-branch" },
  {
    label: "Checkout Branch",
    shortcut: ["⌘", "⇧", "C"],
    value: "checkout-branch",
  },
  {
    label: "Switch Repository",
    shortcut: ["⌘", "⇧", "R"],
    value: "switch-repository",
  },
  { label: "Pull Changes", shortcut: ["⌘", "⇧", "P"], value: "pull-changes" },
  { label: "Push Changes", shortcut: ["⌘", "⇧", "U"], value: "push-changes" },
];

export const commands: Item[] = [];

export const groupedItems: Group[] = [
  { items: goto, value: "Go to" },
  { items: actions, value: "Actions" },
  { items: commands, value: "Commands" },
];

export default function CommandBox() {
  const [open, setOpen] = React.useState(false);
  const navigate = useNavigate();

  const handleItemClick = React.useCallback((item: Item) => {
    setOpen(false);
    if (item.redirect) {
      navigate({
        to: item.redirect,
      });
      return;
    }
  }, []);

  useHotkeys(
    "meta+k, shift+meta+p",
    () => {
      setOpen((open) => !open);
    },
    {
      enabled: true,
      enableOnFormTags: true,
      enableOnContentEditable: true,
    },
  );

  const [waitingForSecondKey, setWaitingForSecondKey] = React.useState(false);

  useHotkeys(
    "g",
    () => {
      setWaitingForSecondKey(true);
      setTimeout(() => setWaitingForSecondKey(false), 1000); // 1 second timeout
    },
    { enabled: !waitingForSecondKey },
  );

  useHotkeys(
    "n, p, i, g",
    (e, h) => {
      if (!waitingForSecondKey) return;

      const secondKey = h.keys?.[0]?.toLowerCase();
      const fullShortcut = `g ${secondKey}`;

      const item = goto.find((item) => {
        if (!item.shortcut) return false;
        return (
          Array.isArray(item.shortcut) &&
          item.shortcut.join(" ").toLowerCase() === fullShortcut
        );
      });

      if (item) {
        e.preventDefault();
        handleItemClick(item);
      }
      setWaitingForSecondKey(false);
    },
    { enabled: waitingForSecondKey },
  );

  return (
    <CommandDialog onOpenChange={setOpen} open={open}>
      <CommandDialogPopup className={"max-h-200"}>
        <Command items={groupedItems}>
          <CommandInput placeholder="Search for apps and commands..." />
          <CommandPanel>
            <CommandEmpty>No results found.</CommandEmpty>
            <CommandList>
              {(group: Group, _index: number) => (
                <React.Fragment key={group.value}>
                  <CommandGroup items={group.items}>
                    <CommandGroupLabel>{group.value}</CommandGroupLabel>
                    <CommandCollection>
                      {(item: Item) => (
                        <CommandItem
                          key={item.value}
                          onClick={() => handleItemClick(item)}
                          value={item.value}
                        >
                          {item.icon && <item.icon className="mr-2 h-4 w-4" />}
                          <span className="flex-1">{item.label}</span>
                          {item.shortcut && (
                            <CommandShortcut>
                              {Array.isArray(item.shortcut) ? (
                                <KbdGroup>
                                  {item.shortcut.map((sc, i) => (
                                    <Kbd key={i}>{sc}</Kbd>
                                  ))}
                                </KbdGroup>
                              ) : (
                                item.shortcut
                              )}
                            </CommandShortcut>
                          )}
                        </CommandItem>
                      )}
                    </CommandCollection>
                  </CommandGroup>
                  <CommandSeparator />
                </React.Fragment>
              )}
            </CommandList>
          </CommandPanel>
          <CommandFooter>
            <div className="flex items-center gap-4">
              <div className="flex items-center gap-2">
                <KbdGroup>
                  <Kbd>
                    <ArrowUpIcon />
                  </Kbd>
                  <Kbd>
                    <ArrowDownIcon />
                  </Kbd>
                </KbdGroup>
                <span>Navigate</span>
              </div>
              <div className="flex items-center gap-2">
                <Kbd>
                  <CornerDownLeftIcon />
                </Kbd>
                <span>Open</span>
              </div>
            </div>
            <div className="flex items-center gap-2">
              <Kbd>Esc</Kbd>
            </div>
          </CommandFooter>
        </Command>
      </CommandDialogPopup>
    </CommandDialog>
  );
}

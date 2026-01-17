"use client";

import { BranchInfo } from "@gitru/commands";
import {
  Avatar,
  AvatarFallback,
  AvatarImage,
} from "@gitru/ui/components/avatar";
import { Badge } from "@gitru/ui/components/badge";
import { Button } from "@gitru/ui/components/button";
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
import {
  Tooltip,
  TooltipPopup,
  TooltipTrigger,
} from "@gitru/ui/components/tooltip";
import { useNavigate } from "@tanstack/react-router";
import {
  ArrowDownIcon,
  ArrowLeft,
  ArrowUpIcon,
  Cloud,
  CornerDownLeftIcon,
  GitBranch,
  SearchIcon,
} from "lucide-react";
import * as React from "react";
import { useHotkeys } from "react-hotkeys-hook";
import { useGetBranches } from "@/hooks";
import { timeAgoFromUnixSeconds } from "@/lib/time";
import { useActions } from "./actions";
import { goto } from "./goto";
import { CommandView, Group, Item } from "./type";

export default function CommandBox() {
  const [open, setOpen] = React.useState(false);
  const [waitingForSecondKey, setWaitingForSecondKey] = React.useState(false);
  const [view, setView] = React.useState<CommandView>({ type: "root" });
  const [inputValue, setInputValue] = React.useState("");

  const navigate = useNavigate();
  const actions = useActions();

  const { data: branches } = useGetBranches("Local");
  const { data: remoteBranches } = useGetBranches("Remote");

  const handleItemClick = React.useCallback((item: Item) => {
    if (item.onClick) {
      item.onClick();
    }

    if (item.value === "checkout-branch") {
      setView({ type: "checkout-branch" });
      setInputValue("");
      return;
    }

    if (item.redirect) {
      navigate({
        to: item.redirect,
      });
      setOpen(false);
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

  const groupedItems = React.useMemo<Group[]>(() => {
    switch (view.type) {
      case "root":
        return [
          { value: "Go to", items: goto },
          { value: "Actions", items: actions },
        ];

      case "checkout-branch":
        return [
          {
            value: "Branches",
            items:
              (branches
                ?.filter((b) => !b.is_remote)
                .map((b) => ({
                  label: b.display_name,
                  value: `checkout:${b.display_name}`,
                  customCommandItem: (
                    <>
                      <CustomCommandItem
                        branch={b}
                        onClick={() => {
                          console.log("branch", b);
                          // call git checkout here
                          setInputValue("");
                          setOpen(false);
                          setView({ type: "root" });
                          return;
                        }}
                      />
                    </>
                  ),
                  data: b,
                })) as Item[]) ?? [],
          },
          {
            value: "Remote Branches",
            items:
              (remoteBranches
                ?.filter((b) => b.is_remote)
                .map((b) => ({
                  label: b.display_name,
                  value: `checkout:${b.display_name}`,
                  customCommandItem: (
                    <>
                      <CustomCommandItem
                        branch={b}
                        onClick={() => {
                          console.log("branch", b);
                          // call git checkout here
                          setInputValue("");
                          setOpen(false);
                          setView({ type: "root" });
                          return;
                        }}
                      />
                    </>
                  ),
                  data: b,
                })) as Item[]) ?? [],
          },
        ];
    }
  }, [view, branches, remoteBranches]);

  return (
    <CommandDialog
      onOpenChange={() => {
        setInputValue("");
        setView({ type: "root" });
        setOpen(false);
      }}
      open={open}
    >
      <CommandDialogPopup className={"max-h-200 max-w-175"}>
        <Command key={view.type} items={groupedItems}>
          <CommandInput
            autoFocus
            value={inputValue}
            onChange={(e) => setInputValue(e.target.value)}
            placeholder="Search for apps and commands..."
            startAddon={
              view.type !== "root" ? (
                <Button
                  size={"icon-sm"}
                  variant={"ghost"}
                  className="z-10 -translate-x-3"
                  onClick={() => {
                    setView({ type: "root" });
                    setInputValue("");
                  }}
                >
                  <ArrowLeft />
                </Button>
              ) : (
                <SearchIcon className="-translate-x-1" />
              )
            }
          />
          <CommandPanel>
            <CommandEmpty>No results found.</CommandEmpty>
            <CommandList>
              {(group: Group, _index: number) => (
                <React.Fragment key={group.value}>
                  <CommandGroup items={group.items}>
                    <CommandGroupLabel>{group.value}</CommandGroupLabel>
                    <CommandCollection>
                      {(item: Item) => (
                        <>
                          {item.customCommandItem ? (
                            item.customCommandItem
                          ) : (
                            <CommandItem
                              key={item.value}
                              onClick={() => handleItemClick(item)}
                              value={item.value}
                            >
                              {item.icon && (
                                <item.icon className="mr-2 h-4 w-4" />
                              )}
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
                        </>
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

const CustomCommandItem = ({
  branch,
  onClick,
}: {
  branch: BranchInfo;
  onClick: () => void;
}) => {
  return (
    <CommandItem
      key={`checkout:${branch.display_name}:${branch.is_head}:${branch.name}`}
      onClick={onClick}
      value={`checkout:${branch.display_name}:${branch.is_head}:${branch.name}`}
      className={"flex-col items-start"}
    >
      <div className="flex items-center justify-between w-full">
        <div className="flex items-center">
          {branch.is_remote ? (
            <Cloud className="mr-2 h-4 w-4" />
          ) : (
            <GitBranch className="mr-2 h-4 w-4" />
          )}
          <span className="flex-1">{branch.display_name}</span>
        </div>
        <div className="flex items-center gap-1">
          {branch.ahead ? (
            <Badge variant={"secondary"}>
              <ArrowUpIcon className="inline-block size-3" />
              {branch.ahead}
            </Badge>
          ) : null}
          {branch.behind ? (
            <Badge variant={"secondary"}>
              <ArrowDownIcon className="inline-block size-3" />
              {branch.behind}
            </Badge>
          ) : null}
        </div>
      </div>
      <div className="flex items-center gap-2">
        <div className="flex group">
          <Tooltip>
            <TooltipTrigger
              style={{
                zIndex: branch.commit.authors.co_authors.length + 1,
              }}
            >
              <Avatar className="ring-2 ring-background rounded-sm size-4">
                <AvatarImage
                  alt={branch.commit.authors.author.name}
                  src={`https://avatars.githubusercontent.com/u/e?email=${branch.commit.authors.author.email}&s=64`}
                />
                <AvatarFallback>
                  {branch.commit.authors.author.name
                    .split(" ")
                    .map((n) => n[0])
                    .join("")
                    .toUpperCase()}
                </AvatarFallback>
              </Avatar>
            </TooltipTrigger>
            <TooltipPopup side="bottom">
              {branch.commit.authors.author.name}
            </TooltipPopup>
          </Tooltip>
          {branch.commit.authors.co_authors.map((coAuthor, idx) => (
            <Tooltip key={`${idx}-tooltip-coauthor`}>
              <TooltipTrigger
                style={{
                  zIndex: branch.commit.authors.co_authors.length - idx,
                }}
                key={`${idx}-tooltip-trigger-coauthor`}
              >
                <Avatar className="ring-2 ring-background rounded-sm size-4 -ml-[0.2rem] group-hover:ml-0.5 transition-all duration-100">
                  <AvatarImage
                    alt="U1"
                    src={`https://avatars.githubusercontent.com/u/e?email=${coAuthor.email}&s=64`}
                  />
                  <AvatarFallback>
                    {coAuthor.name
                      .split(" ")
                      .map((n) => n[0])
                      .join("")
                      .toUpperCase()}
                  </AvatarFallback>
                </Avatar>
              </TooltipTrigger>
              <TooltipPopup side="bottom">{coAuthor.name}</TooltipPopup>
            </Tooltip>
          ))}
        </div>
        <span className="font-normal">{branch.commit.authors.author.name}</span>
        <span className="text-muted-foreground font-light text-xs shrink-0">
          ( {timeAgoFromUnixSeconds(branch.commit.timestamp)} )
        </span>
        <span className="text-muted-foreground font-light text-xs truncate max-w-96">
          {"• \u00A0  "} {branch.commit.summary}
        </span>
      </div>
    </CommandItem>
  );
};

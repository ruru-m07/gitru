import { Button } from "@gitru/ui/components/button";
import { Group, GroupSeparator } from "@gitru/ui/components/group";
import { Label } from "@gitru/ui/components/label";
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@gitru/ui/components/popover";
import { Separator } from "@gitru/ui/components/separator";
import { Switch } from "@gitru/ui/components/switch";
import { cn } from "@gitru/ui/lib/utils";
import { Diff, Settings, TextWrap } from "lucide-react";
import { useDiffViewerSettings } from "@/components/diff/use-diff-view-setting-store";
import { SplitSVG } from "@/components/svgs/split-svg";
import { UnifiedSVG } from "@/components/svgs/unified-svg";

export const SettingsPopover = () => {
  return (
    <Popover>
      <PopoverTrigger
        render={
          <Button
            size="icon-xs"
            variant="outline"
            className="relative"
            aria-label="Open notifications"
          />
        }
      >
        <Settings size={16} aria-hidden="true" />
      </PopoverTrigger>
      <SettingsPopoverContent />
    </Popover>
  );
};

const SettingsPopoverContent = () => {
  const { setDiffStyle, diffStyle, overflow, setOverflow } =
    useDiffViewerSettings();

  return (
    <PopoverContent className="fit mr-4 px-0 mt-0.5 w-96">
      <Label className="text-muted-foreground">Settings</Label>
      <Separator className={"mt-2 mb-3"} />
      <div className="flex flex-col gap-4 mt-1">
        <div>
          <Label className="flex items-center gap-2 mb-3">
            <Diff size={16} />
            Diff Style
          </Label>
          <div className="flex items-center justify-center ">
            <Group className="flex w-full h-full">
              <Button
                className={cn(
                  "rounded-none w-32 h-32! shadow-none first:rounded-s-md last:rounded-e-md focus-visible:z-10",
                  diffStyle === "unified" &&
                    "border-primary/40 bg-primary/10! hover:bg-primary/13!",
                )}
                variant="outline"
                onClick={() => {
                  setDiffStyle("unified");
                }}
              >
                <UnifiedSVG />
              </Button>
              <GroupSeparator className="bg-primary/40" />
              <Button
                className={cn(
                  "rounded-none w-32 h-32! shadow-none rounded-r-md border-l-0 focus-visible:z-10",
                  diffStyle === "split" &&
                    "border-primary/40 bg-primary/10! hover:bg-primary/13!",
                )}
                variant="outline"
                onClick={() => {
                  setDiffStyle("split");
                }}
              >
                <SplitSVG />
              </Button>
            </Group>
          </div>
        </div>
        <Separator />
        <div className="flex justify-between">
          <Label htmlFor="wrapping" className="flex items-center gap-2">
            <TextWrap size={16} />
            Wrapping
          </Label>
          <Switch
            checked={overflow === "wrap"}
            onCheckedChange={(checked) => {
              setOverflow(checked ? "wrap" : "scroll");
            }}
            id="wrapping"
          />
        </div>
      </div>
    </PopoverContent>
  );
};

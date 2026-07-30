import { Kbd, KbdGroup } from "@gitru/ui/components/kbd";
import { GitruBorderedSVG } from "@/components/svgs/gitru-bordered";

export const EmptyStateScreen = () => {
  return (
    <div className="w-full flex justify-center max-h-[calc(var(--layout-height)---spacing(14))] h-full bg-background">
      <div className="w-full h-full flex flex-col items-center justify-center -mt-20">
        <GitruBorderedSVG />
        <div className="flex flex-col gap-0.5 w-60 select-none">
          <div className="flex items-center justify-between">
            <span className="text-muted-foreground text-sm font-light">
              Command Panel
            </span>
            <span>
              <KbdGroup>
                <Kbd>⌘</Kbd>
                <Kbd>K</Kbd>
              </KbdGroup>
            </span>
          </div>
          <div className="flex items-center justify-between">
            <span className="text-muted-foreground text-sm font-light">
              New Branch
            </span>
            <span>
              <KbdGroup>
                <Kbd>⌘</Kbd>
                <Kbd>⇧</Kbd>
                <Kbd>N</Kbd>
              </KbdGroup>
            </span>
          </div>
          <div className="flex items-center justify-between">
            <span className="text-muted-foreground text-sm font-light">
              Pull Changes
            </span>
            <span>
              <KbdGroup>
                <Kbd>⌘</Kbd>
                <Kbd>⇧</Kbd>
                <Kbd>P</Kbd>
              </KbdGroup>
            </span>
          </div>
        </div>
      </div>
    </div>
  );
};

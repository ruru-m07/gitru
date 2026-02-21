import { Badge } from "@gitru/ui/components/badge";
import { Tabs, TabsList, TabsTab } from "@gitru/ui/components/tabs";
import {
  Tooltip,
  TooltipCreateHandle,
  TooltipPopup,
  TooltipProvider,
  TooltipTrigger,
} from "@gitru/ui/components/tooltip";

import { useLocation, useNavigate } from "@tanstack/react-router";
import { LucideIcon } from "lucide-react";
import type React from "react";
import type { ComponentType } from "react";
import { JSX } from "react";

type Item = {
  icon: JSX.Element | LucideIcon | any;
  name: string;
  href: string;
  badge?: string;
};

const tooltipHandle = TooltipCreateHandle<ComponentType>();

const SideBarItems: React.FC<{
  items: Item[];
}> = ({ items }) => {
  const location = useLocation();
  const tanstackNavigate = useNavigate();

  return (
    <div>
      <TooltipProvider>
        <Tabs
          orientation="vertical"
          className="items-center"
          value={location.href}
        >
          <TabsList className="bg-transparent! gap-1 select-none">
            {items.map((item: Item) => (
              <TooltipTrigger
                className="after:absolute after:left-full after:h-full after:w-1"
                handle={tooltipHandle}
                payload={() => {
                  return <span>{item.name}</span>;
                }}
                render={
                  <TabsTab
                    aria-label={item.name}
                    value={item.href}
                    className={
                      "size-7! p-0 flex! items-center! justify-center!"
                    }
                    onClick={() => {
                      tanstackNavigate({
                        to: item.href,
                      });
                    }}
                  />
                }
              >
                <item.icon aria-hidden="true" />
                {item.badge && (
                  <Badge
                    className="absolute size-3.5 text-[10px] end-0 top-0 rounded-full not-in-data-active:opacity-64 [--color-primary:black]!"
                    size="sm"
                    variant={"default"}
                  >
                    {item.badge}
                  </Badge>
                )}
              </TooltipTrigger>
            ))}
          </TabsList>
        </Tabs>
        <Tooltip handle={tooltipHandle}>
          {({ payload: Payload }) => (
            <TooltipPopup side="right">
              {Payload !== undefined && <Payload />}
            </TooltipPopup>
          )}
        </Tooltip>
      </TooltipProvider>
    </div>
  );
};

export default SideBarItems;

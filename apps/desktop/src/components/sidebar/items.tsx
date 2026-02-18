import { Badge } from "@gitru/ui/components/badge";
import { Tabs, TabsList, TabsTab } from "@gitru/ui/components/tabs";

import { useLocation, useNavigate } from "@tanstack/react-router";
import { LucideIcon } from "lucide-react";
import type React from "react";
import { JSX } from "react";

type Item = {
  icon: JSX.Element | LucideIcon | any;
  name: string;
  href: string;
  badge?: string;
};

const SideBarItems: React.FC<{
  items: Item[];
}> = ({ items }) => {
  const location = useLocation();
  const tanstackNavigate = useNavigate();

  return (
    <div>
      <Tabs
        orientation="vertical"
        className="items-center"
        value={location.href}
      >
        <TabsList className="gap-1 bg-transparent!">
          {items.map((item: Item) => (
            <TabsTab
              aria-label={item.name}
              value={item.href}
              className={"size-7! p-0 flex! items-center! justify-center!"}
              onClick={() => {
                tanstackNavigate({
                  to: item.href,
                });
              }}
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
            </TabsTab>
          ))}
        </TabsList>
      </Tabs>
    </div>
  );
};

export default SideBarItems;

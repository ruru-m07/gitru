import { buttonVariants } from "@gitru/ui/components/button";

import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@gitru/ui/components/tooltip";
import { cn } from "@gitru/ui/lib/utils";
import { Link, useLocation } from "@tanstack/react-router";
import type { LucideIcon } from "lucide-react";
import type React from "react";

type Item = {
  icon: LucideIcon;
  name: string;
  href: string;
};

const SideBarItems: React.FC<{
  items: Item[];
}> = ({ items }) => {
  const location = useLocation();
  return items.map((item: Item) => (
    <div key={item.name}>
      <Tooltip>
        <TooltipTrigger>
          <Link
            to={item.href}
            className={cn(
              buttonVariants({
                variant: location.pathname === item.href ? "default" : "ghost",
                size: "icon",
              }),
              "size-8",
              // location.pathname === item.href && "bg-accent",
            )}
          >
            <item.icon size={14} strokeWidth={2} aria-hidden="true" />
          </Link>
        </TooltipTrigger>
        <TooltipContent side="left" className="px-2 py-1 text-xs">
          {item.name}
        </TooltipContent>
      </Tooltip>
    </div>
  ));
};

export default SideBarItems;

import { buttonVariants } from "@noutify/ui/components/button";

import {
	Tooltip,
	TooltipContent,
	TooltipTrigger,
} from "@noutify/ui/components/tooltip";
import { cn } from "@noutify/ui/lib/utils";
import { Link } from "@tanstack/react-router";
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
	return items.map((item: Item) => (
		<div key={item.name}>
			<Tooltip>
				<TooltipTrigger>
					<Link
						to={item.href}
						className={cn(
							buttonVariants({ variant: "ghost", size: "icon" }),
							"size-8",
						)}
					>
						<item.icon
							className="opacity-60"
							size={14}
							strokeWidth={2}
							aria-hidden="true"
						/>
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

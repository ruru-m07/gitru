"use client";

import { cn } from "@gitru/ui/lib/utils";
import * as React from "react";

const Label = React.forwardRef<
	HTMLLabelElement,
	React.LabelHTMLAttributes<HTMLLabelElement>
>(
	// eslint-disable-next-line react/prop-types
	({ className, ...props }, ref) => (
		<label
			ref={ref}
			className={cn(
				"text-sm font-medium leading-4 text-foreground peer-disabled:cursor-not-allowed peer-disabled:opacity-70",
				className,
			)}
			{...props}
		/>
	),
);
Label.displayName = "Label";

export { Label };

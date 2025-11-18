"use client";

import { Toggle as TogglePrimitive } from "@base-ui-components/react/toggle";
import { ToggleGroup as ToggleGroupPrimitive } from "@base-ui-components/react/toggle-group";
import { Separator } from "@gitru/ui/components/separator";
import {
  Toggle as ToggleComponent,
  toggleVariants,
} from "@gitru/ui/components/toggle";

import { cn } from "@gitru/ui/lib/utils";
import { type VariantProps } from "class-variance-authority";
import * as React from "react";

const ToggleGroupContext = React.createContext<
  VariantProps<typeof toggleVariants>
>({
  size: "default",
  variant: "default",
});

function ToggleGroup({
  className,
  variant = "default",
  size = "default",
  orientation = "horizontal",
  children,
  ...props
}: ToggleGroupPrimitive.Props & VariantProps<typeof toggleVariants>) {
  return (
    <ToggleGroupPrimitive
      data-slot="toggle-group"
      data-variant={variant}
      data-size={size}
      orientation={orientation}
      className={cn(
        "flex w-fit *:focus-visible:z-10",
        orientation === "horizontal"
          ? "*:pointer-coarse:after:min-w-auto"
          : "*:pointer-coarse:after:min-h-auto",
        variant === "default"
          ? "gap-0.5"
          : orientation === "horizontal"
            ? "*:not-first:rounded-s-none *:not-first:border-s-0 *:not-last:rounded-e-none *:not-last:border-e-0 *:not-first:before:-start-[0.5px] *:not-first:before:rounded-s-none *:not-last:before:-end-[0.5px] *:not-last:before:rounded-e-none"
            : "flex-col *:not-first:rounded-t-none *:not-first:border-t-0 *:not-last:rounded-b-none *:not-last:border-b-0 *:not-first:before:-top-[0.5px] *:not-first:before:rounded-t-none *:not-last:before:-bottom-[0.5px] *:not-last:before:hidden *:not-last:before:rounded-b-none dark:*:first:before:block dark:*:last:before:hidden",
        className,
      )}
      {...props}
    >
      <ToggleGroupContext.Provider value={{ variant, size }}>
        {children}
      </ToggleGroupContext.Provider>
    </ToggleGroupPrimitive>
  );
}

function Toggle({
  className,
  children,
  variant,
  size,
  ...props
}: TogglePrimitive.Props & VariantProps<typeof toggleVariants>) {
  const context = React.useContext(ToggleGroupContext);

  const resolvedVariant = context.variant || variant;
  const resolvedSize = context.size || size;

  return (
    <ToggleComponent
      data-variant={resolvedVariant}
      data-size={resolvedSize}
      className={className}
      variant={resolvedVariant}
      size={resolvedSize}
      {...props}
    >
      {children}
    </ToggleComponent>
  );
}

function ToggleGroupSeparator({
  className,
  orientation = "vertical",
  ...props
}: {
  className?: string;
} & React.ComponentProps<typeof Separator>) {
  return (
    <Separator orientation={orientation} className={className} {...props} />
  );
}

export { ToggleGroup, Toggle, Toggle as ToggleGroupItem, ToggleGroupSeparator };

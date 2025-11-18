"use client";

import { Fieldset as FieldsetPrimitive } from "@base-ui-components/react/fieldset";

import { cn } from "@gitru/ui/lib/utils";

function Fieldset({ className, ...props }: FieldsetPrimitive.Root.Props) {
  return (
    <FieldsetPrimitive.Root
      data-slot="fieldset"
      className={cn("flex w-full max-w-64 flex-col gap-6", className)}
      {...props}
    />
  );
}
function FieldsetLegend({
  className,
  ...props
}: FieldsetPrimitive.Legend.Props) {
  return (
    <FieldsetPrimitive.Legend
      data-slot="fieldset-legend"
      className={cn("font-semibold", className)}
      {...props}
    />
  );
}

export { Fieldset, FieldsetLegend };

"use client";

import { Switch as SwitchPrimitive } from "@base-ui-components/react/switch";

import { cn } from "@gitru/ui/lib/utils";

function Switch({ className, ...props }: SwitchPrimitive.Root.Props) {
  return (
    <SwitchPrimitive.Root
      data-slot="switch"
      className={cn(
        "group/switch inline-flex h-[1.125rem] w-7.5 shrink-0 items-center rounded-full p-px inset-shadow-[0_1px_--theme(--color-black/4%)] transition-all outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-1 focus-visible:ring-offset-background disabled:cursor-not-allowed disabled:opacity-64 data-checked:bg-primary! data-unchecked:bg-input! cursor-pointer",
        className,
      )}
      {...props}
    >
      <SwitchPrimitive.Thumb
        data-slot="switch-thumb"
        className={cn(
          "pointer-events-none block size-4 rounded-full bg-background! shadow-sm transition-[translate,width] group-active/switch:w-4.5 data-checked:translate-x-3 data-checked:group-active/switch:translate-x-2.5 data-unchecked:translate-x-0",
        )}
      />
    </SwitchPrimitive.Root>
  );
}

export { Switch };

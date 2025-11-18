"use client";

import { Dialog as SheetPrimitive } from "@base-ui-components/react/dialog";
import { cn } from "@gitru/ui/lib/utils";
import { cva, type VariantProps } from "class-variance-authority";
import { XIcon } from "lucide-react";

const Sheet = SheetPrimitive.Root;

function SheetTrigger(props: SheetPrimitive.Trigger.Props) {
  return <SheetPrimitive.Trigger data-slot="sheet-trigger" {...props} />;
}

function SheetPortal(props: SheetPrimitive.Portal.Props) {
  return <SheetPrimitive.Portal {...props} />;
}

function SheetClose(props: SheetPrimitive.Close.Props) {
  return <SheetPrimitive.Close data-slot="sheet-close" {...props} />;
}

const sheetPopupVariants = cva(
  "fixed z-50 flex flex-col gap-4 overflow-y-auto bg-popover text-popover-foreground shadow-lg transition-[opacity,translate] duration-300 ease-in-out will-change-transform [--sheet-inset:0px] data-ending-style:opacity-0 data-starting-style:opacity-0",
  {
    variants: {
      inset: {
        true: "sm:rounded-xl sm:[--sheet-inset:1rem]",
      },
      side: {
        right:
          "inset-y-[var(--sheet-inset)] right-[var(--sheet-inset)] h-dvh w-[calc(100%-(--spacing(12)))] max-w-sm data-ending-style:translate-x-12 data-starting-style:translate-x-12 sm:h-[calc(100dvh-var(--sheet-inset)*2)]",
        left: "inset-y-[var(--sheet-inset)] left-[var(--sheet-inset)] h-dvh w-[calc(100%-(--spacing(12)))] max-w-sm data-ending-style:-translate-x-12 data-starting-style:-translate-x-12 sm:h-[calc(100dvh-var(--sheet-inset)*2)]",
        top: "inset-x-[var(--sheet-inset)] top-[var(--sheet-inset)] h-auto max-h-[calc(100dvh-var(--sheet-inset)*2)] data-ending-style:-translate-y-12 data-starting-style:-translate-y-12",
        bottom:
          "inset-x-[var(--sheet-inset)] bottom-[var(--sheet-inset)] h-auto max-h-[calc(100dvh-var(--sheet-inset)*2)] data-ending-style:translate-y-12 data-starting-style:translate-y-12",
      },
    },
    defaultVariants: {
      inset: false,
      side: "right",
    },
  },
);

function SheetBackdrop({ className, ...props }: SheetPrimitive.Backdrop.Props) {
  return (
    <SheetPrimitive.Backdrop
      data-slot="sheet-backdrop"
      className={cn(
        "fixed inset-0 z-50 bg-black/32 backdrop-blur-sm transition-all duration-200 data-ending-style:opacity-0 data-starting-style:opacity-0",
        className,
      )}
      {...props}
    />
  );
}

function SheetPopup({
  className,
  children,
  showCloseButton = true,
  side = "right",
  inset = false,
  ...props
}: SheetPrimitive.Popup.Props & {
  showCloseButton?: boolean;
} & VariantProps<typeof sheetPopupVariants>) {
  return (
    <SheetPortal>
      <SheetBackdrop />
      <SheetPrimitive.Popup
        data-slot="sheet-popup"
        className={cn(sheetPopupVariants({ inset, side }), className)}
        {...props}
      >
        {children}
        {showCloseButton && (
          <SheetPrimitive.Close className="absolute end-2 top-2 inline-flex size-7 shrink-0 cursor-pointer items-center justify-center rounded-md border border-transparent opacity-72 transition-[color,background-color,box-shadow,opacity] outline-none hover:opacity-100 focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-1 focus-visible:ring-offset-background pointer-coarse:after:absolute pointer-coarse:after:size-full pointer-coarse:after:min-h-11 pointer-coarse:after:min-w-11 [&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*='size-'])]:size-4">
            <XIcon />
            <span className="sr-only">Close</span>
          </SheetPrimitive.Close>
        )}
      </SheetPrimitive.Popup>
    </SheetPortal>
  );
}

function SheetHeader({ className, ...props }: React.ComponentProps<"div">) {
  return (
    <div
      data-slot="sheet-header"
      className={cn("flex flex-col gap-1.5 p-4", className)}
      {...props}
    />
  );
}

function SheetFooter({ className, ...props }: React.ComponentProps<"div">) {
  return (
    <div
      data-slot="sheet-footer"
      className={cn("mt-auto flex flex-col gap-2 p-4", className)}
      {...props}
    />
  );
}

function SheetTitle({ className, ...props }: SheetPrimitive.Title.Props) {
  return (
    <SheetPrimitive.Title
      data-slot="sheet-title"
      className={cn("font-semibold", className)}
      {...props}
    />
  );
}

function SheetDescription({
  className,
  ...props
}: SheetPrimitive.Description.Props) {
  return (
    <SheetPrimitive.Description
      data-slot="sheet-description"
      className={cn("text-sm text-muted-foreground", className)}
      {...props}
    />
  );
}

export {
  Sheet,
  SheetTrigger,
  SheetPortal,
  SheetClose,
  SheetBackdrop,
  SheetBackdrop as SheetOverlay,
  SheetPopup,
  SheetPopup as SheetContent,
  SheetHeader,
  SheetFooter,
  SheetTitle,
  SheetDescription,
  sheetPopupVariants,
};

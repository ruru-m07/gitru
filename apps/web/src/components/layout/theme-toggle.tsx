"use client";
import { cva } from "class-variance-authority";
import { Airplay, Moon, Sun } from "lucide-react";
import { useTheme } from "next-themes";
import { ComponentProps, useEffect, useState } from "react";
import { cn } from "../../lib/cn";
import { buttonVariants } from "../ui/button";

const itemVariants = cva("size-6.5 p-1.5 text-fd-muted-foreground", {
  variants: {
    active: {
      true: "bg-fd-accent text-fd-accent-foreground",
      false: "text-fd-muted-foreground",
    },
  },
});

const full = [
  ["light", Sun] as const,
  ["dark", Moon] as const,
  ["system", Airplay] as const,
];

export function ThemeToggle({
  className,
  mode = "light-dark",
  ...props
}: ComponentProps<"div"> & {
  mode?: "light-dark" | "light-dark-system";
}) {
  const { setTheme, theme, resolvedTheme } = useTheme();
  const [mounted, setMounted] = useState(false);

  useEffect(() => {
    setMounted(true);
  }, []);

  const container = cn(
    "inline-flex items-center rounded-full border p-1 *:rounded-full",
    className,
  );

  if (mode === "light-dark") {
    const value = mounted ? resolvedTheme : null;

    return (
      <button
        className={cn(
          buttonVariants({ size: "icon-sm", color: "ghost" }),
          "text-fd-muted-foreground",
        )}
        aria-label={`Toggle Theme`}
        onClick={() => setTheme(value === "light" ? "dark" : "light")}
        data-theme-toggle=""
      >
        {value === "light" ? (
          <Sun className="scale-105" fill="currentColor" />
        ) : (
          <Moon className="scale-105" fill="currentColor" />
        )}
      </button>
    );
  }

  const value = mounted ? theme : null;

  return (
    <div className={container} data-theme-toggle="" {...props}>
      {full.map(([key, Icon]) => (
        <button
          key={key}
          aria-label={key}
          className={cn(itemVariants({ active: value === key }))}
          onClick={() => setTheme(key)}
        >
          <Icon className="size-full" fill="currentColor" />
        </button>
      ))}
    </div>
  );
}

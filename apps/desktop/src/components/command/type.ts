import { FileRouteTypes } from "@tanstack/react-router";
import { LucideIcon } from "lucide-react";

export interface Item {
  value: string;
  label: string;
  shortcut?: string | string[];
  icon?: LucideIcon;
  customCommandItem?: React.ReactNode;
  redirect?: FileRouteTypes["to"];
  onClick?: () => void;
}

export interface Group {
  value: string;
  items: Item[];
}

export type CommandView = { type: "root" } | { type: "checkout-branch" };

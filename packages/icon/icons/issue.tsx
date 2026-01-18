import { cn } from "@gitru/ui/lib/utils";
import * as React from "react";

export const Issue = ({
  className,
  ...props
}: React.SVGProps<SVGSVGElement>) => (
  <svg
    width="24"
    height="24"
    viewBox="0 0 24 24"
    fill="none"
    xmlns="http://www.w3.org/2000/svg"
    className={cn("", className)}
    {...props}
  >
    <path
      d="M12 2C15.2172 2 17.6906 2.97378 19.3584 4.6416C21.0262 6.30942 22 8.78276 22 12C22 15.2172 21.0262 17.6906 19.3584 19.3584C17.6906 21.0262 15.2172 22 12 22C8.78276 22 6.30942 21.0262 4.6416 19.3584C2.97378 17.6906 2 15.2172 2 12C2 8.78276 2.97378 6.30942 4.6416 4.6416C6.30942 2.97378 8.78276 2 12 2Z"
      stroke="currentColor"
      strokeWidth="2"
    />
    <rect
      x="10.167"
      y="10.168"
      width="3.66667"
      height="3.66666"
      rx="1.83333"
      fill="currentColor"
    />
  </svg>
);

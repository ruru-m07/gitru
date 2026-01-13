import { cn } from "@gitru/ui/lib/utils";
import * as React from "react";

export const Git = ({ className, ...props }: React.SVGProps<SVGSVGElement>) => (
  <svg
    width="24"
    height="24"
    viewBox="0 0 24 24"
    fill="none"
    xmlns="http://www.w3.org/2000/svg"
    className={cn("", className)}
    {...props}
  >
    <g clip-path="url(#clip0_1970_2128)">
      <path
        d="M5.75 22C7.13071 22 8.25 20.8807 8.25 19.5C8.25 18.1193 7.13071 17 5.75 17C4.36929 17 3.25 18.1193 3.25 19.5C3.25 20.8807 4.36929 22 5.75 22Z"
        stroke="currentColor"
        stroke-width="1.5"
      />
      <path
        d="M5.75 7C7.13071 7 8.25 5.88071 8.25 4.5C8.25 3.11929 7.13071 2 5.75 2C4.36929 2 3.25 3.11929 3.25 4.5C3.25 5.88071 4.36929 7 5.75 7Z"
        stroke="currentColor"
        stroke-width="1.5"
      />
      <path
        d="M18.25 14.5C19.6307 14.5 20.75 13.3807 20.75 12C20.75 10.6193 19.6307 9.5 18.25 9.5C16.8693 9.5 15.75 10.6193 15.75 12C15.75 13.3807 16.8693 14.5 18.25 14.5Z"
        stroke="currentColor"
        stroke-width="1.5"
      />
      <path
        d="M5.77694 7.34766V16.2577M15.0146 12.0044L9.52462 12.0045C8.15247 12.0045 5.57777 10.9053 5.77322 7.97354"
        stroke="currentColor"
        stroke-width="1.5"
        stroke-linecap="round"
        stroke-linejoin="round"
      />
    </g>
    <defs>
      <clipPath id="clip0_1970_2128">
        <rect width="24" height="24" fill="currentColor" />
      </clipPath>
    </defs>
  </svg>
);

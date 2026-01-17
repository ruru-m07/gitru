import { cn } from "@gitru/ui/lib/utils";
import * as React from "react";

export const Renamed = ({
  className,
  ...props
}: React.SVGProps<SVGSVGElement>) => (
  <svg
    width="24"
    height="24"
    viewBox="0 0 24 24"
    fill="none"
    xmlns="http://www.w3.org/2000/svg"
    className={cn("text-[#8E61EF] dark:text-[#966EEE]", className)}
    {...props}
  >
    <rect
      x="17"
      y="10"
      width="2"
      height="8"
      rx="1"
      transform="rotate(90 17 10)"
      fill="currentColor"
    />
    <rect
      x="11"
      y="16"
      width="2"
      height="6"
      rx="1"
      transform="rotate(-180 11 16)"
      fill="currentColor"
    />
    <path
      d="M12 1C14.4477 1 16.3465 1.13284 17.8271 1.46191C19.2964 1.78846 20.2925 2.29443 20.999 3.00098C21.7056 3.70752 22.2115 4.70364 22.5381 6.17285C22.8672 7.65353 23 9.55232 23 12C23 14.4477 22.8672 16.3465 22.5381 17.8271C22.2115 19.2964 21.7056 20.2925 20.999 20.999C20.2925 21.7056 19.2964 22.2115 17.8271 22.5381C16.3465 22.8672 14.4477 23 12 23C9.55232 23 7.65353 22.8672 6.17285 22.5381C4.70364 22.2115 3.70752 21.7056 3.00098 20.999C2.29443 20.2925 1.78846 19.2964 1.46191 17.8271C1.13284 16.3465 1 14.4477 1 12C1 9.55232 1.13284 7.65353 1.46191 6.17285C1.78846 4.70364 2.29443 3.70752 3.00098 3.00098C3.70752 2.29443 4.70364 1.78846 6.17285 1.46191C7.65353 1.13284 9.55232 1 12 1Z"
      stroke="currentColor"
      strokeWidth="2"
    />
    <path
      d="M14 8.17127L14.7071 8.87838C15.7071 9.87838 16.2071 10.3784 16.2071 10.9997C16.2071 11.621 15.7071 12.121 14.7071 13.121L14 13.8281"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
    />
  </svg>
);

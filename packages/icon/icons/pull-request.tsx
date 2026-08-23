import { cn } from "@gitru/ui/lib/utils";
import * as React from "react";

export const PullRequest = ({
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
      d="M6.5 8.66797V17.3346"
      stroke="currentColor"
      strokeWidth="1.5"
      strokeLinecap="round"
      strokeLinejoin="round"
    />
    <path
      d="M19.5003 17.3346V13.0013C19.5003 9.93717 19.5003 8.40511 18.5484 7.45321C17.5965 6.5013 16.0644 6.5013 13.0003 6.5013H11.917M11.917 6.5013C11.917 5.74272 14.0775 4.32546 14.6253 3.79297M11.917 6.5013C11.917 7.25988 14.0775 8.67714 14.6253 9.20964"
      stroke="currentColor"
      strokeWidth="1.5"
      strokeLinecap="round"
      strokeLinejoin="round"
    />
    <path
      d="M6.49967 21.6654C7.69629 21.6654 8.66634 20.6953 8.66634 19.4987C8.66634 18.3021 7.69629 17.332 6.49967 17.332C5.30306 17.332 4.33301 18.3021 4.33301 19.4987C4.33301 20.6953 5.30306 21.6654 6.49967 21.6654Z"
      stroke="currentColor"
      strokeWidth="1.5"
    />
    <path
      d="M6.49967 8.66536C7.69629 8.66536 8.66634 7.69532 8.66634 6.4987C8.66634 5.30208 7.69629 4.33203 6.49967 4.33203C5.30306 4.33203 4.33301 5.30208 4.33301 6.4987C4.33301 7.69532 5.30306 8.66536 6.49967 8.66536Z"
      stroke="currentColor"
      strokeWidth="1.5"
    />
    <path
      d="M19.4997 21.6654C20.6963 21.6654 21.6663 20.6953 21.6663 19.4987C21.6663 18.3021 20.6963 17.332 19.4997 17.332C18.3031 17.332 17.333 18.3021 17.333 19.4987C17.333 20.6953 18.3031 21.6654 19.4997 21.6654Z"
      stroke="currentColor"
      strokeWidth="1.5"
    />
  </svg>
);

import { cn } from "@gitru/ui/lib/utils";
import React from "react";

const PageLayout = ({
  children,
  className,
}: {
  className?: string;
  children?: React.ReactNode;
}) => {
  return (
    <div
      className={cn(
        "ml-(--main-actual-content-padding) bg-background h-full w-full ring ring-ring/20 rounded-lg overflow-hidden shadow-[0px_0px_2px_1px_#0000001A] flex flex-col",
        className,
      )}
    >
      {children}
    </div>
  );
};

export default PageLayout;

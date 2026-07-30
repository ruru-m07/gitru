import { Button } from "@gitru/ui/components/button";
import { ChevronLeft, ChevronRight } from "lucide-react";

import { emitWebviewNavigation } from "@/lib/emit-webview-navigation";

type NavigationButtonsProps = {
  activeTabId: string | null;
  canGoBack: boolean;
  canGoForward: boolean;
  isRootShellMode: boolean;
  goBack: () => Promise<{ current_path?: string } | null | undefined>;
  goForward: () => Promise<{ current_path?: string } | null | undefined>;
  navigate: (options: { to: string }) => void;
};

export const NavigationButtons = ({
  activeTabId,
  canGoBack,
  canGoForward,
  isRootShellMode,
  goBack,
  goForward,
  navigate,
}: NavigationButtonsProps) => (
  <div className="flex items-center mr-3 translate-y-px">
    <Button
      onClick={() => {
        console.log(
          "[NavButton] Back clicked. Session:",
          activeTabId,
          "canGoBack:",
          canGoBack,
        );
        void goBack().then((state) => {
          console.log("[NavButton] goBack returned:", state);
          if (state?.current_path && activeTabId) {
            console.log("[NavButton] Navigating to:", state.current_path);
            if (isRootShellMode) {
              void emitWebviewNavigation(
                activeTabId,
                state.current_path,
                "back",
              );
            } else {
              void navigate({ to: state.current_path });
            }
          }
        });
      }}
      disabled={!canGoBack}
      size={"icon-sm"}
      className="hover:bg-foreground/7! [&_svg]:size-4.75! [&_svg]:stroke-[1.5]"
      variant="ghost"
    >
      <ChevronLeft size={16} aria-hidden="true" />
    </Button>
    <Button
      onClick={() => {
        console.log(
          "[NavButton] Forward clicked. Session:",
          activeTabId,
          "canGoForward:",
          canGoForward,
        );
        void goForward().then((state) => {
          console.log("[NavButton] goForward returned:", state);
          if (state?.current_path && activeTabId) {
            console.log("[NavButton] Navigating to:", state.current_path);
            if (isRootShellMode) {
              void emitWebviewNavigation(
                activeTabId,
                state.current_path,
                "forward",
              );
            } else {
              void navigate({ to: state.current_path });
            }
          }
        });
      }}
      disabled={!canGoForward}
      size={"icon-sm"}
      className="hover:bg-foreground/7! [&_svg]:size-4.75! [&_svg]:stroke-[1.5]"
      variant="ghost"
    >
      <ChevronRight aria-hidden="true" />
    </Button>
  </div>
);

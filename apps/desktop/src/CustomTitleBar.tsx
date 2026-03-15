import { Button } from "@gitru/ui/components/button";
import { useCanGoBack, useRouterState } from "@tanstack/react-router";
import {
  ArrowLeft,
  ArrowRight,
  ChevronRight,
  GitPullRequestArrow,
  Plus,
} from "lucide-react";

type CustomTitleBarProps = {
  restrictedPaths: string[];
};

const CustomTitleBar = ({ restrictedPaths = [] }: CustomTitleBarProps) => {
  const routerState = useRouterState();
  const pathname = routerState.location.pathname;

  const canGoBack = useCanGoBack();

  if (restrictedPaths.includes(pathname)) {
    return null;
  }

  return (
    <div
      className="h-(--main-custom-header-height) flex items-center justify-between relative pl-4 mr-1 select-none _border-b"
      data-tauri-drag-region
      style={{
        // @ts-expect-error - ¯\_(ツ)_/¯
        WebkitAppRegion: "drag",
      }}
    >
      {restrictedPaths.includes(pathname) ? null : (
        <>
          <div
            className="flex items-center absolute"
            style={{
              // @ts-expect-error - ¯\_(ツ)_/¯
              WebkitAppRegion: "no-drag",
              paddingLeft: "70px",
            }}
          >
            <Button
              onClick={() => window.history.back()}
              disabled={!canGoBack}
              size={"icon"}
              className="size-7"
              variant="ghost"
            >
              <ArrowLeft size={16} aria-hidden="true" />
            </Button>
            <Button
              onClick={() => window.history.forward()}
              disabled={true} // TODO;
              size={"icon"}
              className="size-7"
              variant="ghost"
            >
              <ArrowRight size={16} aria-hidden="true" />
            </Button>
          </div>

          <div />

          <div
            // className="flex-1 max-w-[16rem] mx-4"
            className="flex items-center max-w-160 mx-4 gap-2"
            style={{
              // @ts-expect-error - ¯\_(ツ)_/¯
              WebkitAppRegion: "no-drag",
            }}
          >
            {/* <div className="flex items-center gap-1.5 text-sm">
              <GitPullRequestArrow
                className="text-green-600 mr-1"
                size={18}
                strokeWidth={2}
                aria-hidden="true"
              />{" "}
              <div className="flex items-center gap-1 text-muted-foreground font-medium">
                ruru-m07
                <ChevronRight size={14} />
                gitru
                <ChevronRight size={14} />
                pulls
                <ChevronRight size={14} />
                <span className="text-foreground">#69</span>
              </div>
            </div> */}
          </div>

          <div
            className="flex items-center"
            style={{
              // @ts-expect-error - ¯\_(ツ)_/¯
              WebkitAppRegion: "no-drag",
            }}
          >
            <Button size={"icon"} variant={"ghost"} disabled>
              <Plus size={16} aria-hidden="true" />
            </Button>
          </div>
        </>
      )}
    </div>
  );
};

export default CustomTitleBar;

import {
  Avatar,
  AvatarFallback,
  AvatarImage,
} from "@gitru/ui/components/avatar";
import { Button } from "@gitru/ui/components/button";
import { useCanGoBack, useRouterState } from "@tanstack/react-router";
import {
  ArrowLeft,
  ArrowRight,
  GitBranch,
  GitPullRequestArrow,
  Plus,
  X,
} from "lucide-react";
import { GithubIcon } from "./components/svgs/githubIcon";

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
      className="h-(--main-custom-header-height) flex items-center _justify-between relative pl-4 mr-1 select-none _border-b"
      data-tauri-drag-region
      style={{
        // @ts-expect-error - ¯\_(ツ)_/¯
        WebkitAppRegion: "drag",
      }}
    >
      {restrictedPaths.includes(pathname) ? null : (
        <>
          <div
            className="flex items-center absolute w-fit"
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

            <div className="-translate-x-2 flex w-fit items-center h-[calc(var(--main-custom-header-height)-0px)] pt-1">
              <div className="flex items-end h-full">
                <svg
                  width="15"
                  height="15"
                  viewBox="0 0 15 15"
                  fill="none"
                  xmlns="http://www.w3.org/2000/svg"
                  className="filter-[drop-shadow(-1px_-1px_1px_#00000011)]"
                >
                  <path
                    d="M15 15H0C8.28427 15 15 8.28427 15 0V15Z"
                    fill="var(--background)"
                  />
                </svg>

                <div className="bg-background flex items-center pl-2.5 pr-1 h-full rounded-t-[16px] [box-shadow:-1px_-1px_1px_0px_#00000011,1px_-1px_1px_0px_#00000011]">
                  {/* <div className="flex items-center -translate-y-0.5">
                    <GitPullRequestArrow size={16} className="text-green-600" />
                    <span className="px-2 space-x-0.5">
                      <span className="text-sm text-muted-foreground font-normal">
                        ruru-m07
                      </span>
                      <span className="text-sm text-muted-foreground font-normal">
                        /
                      </span>
                      <span className="text-sm font-[450]">gitru</span>
                      <span className="text-sm font-[450] ml-1">#69</span>
                    </span>
                  </div> */}
                  <div className="flex items-center -translate-y-0.5">
                    <div className="relative">
                      <Avatar className="size-5 rounded-sm">
                        <AvatarImage
                          alt="User"
                          src="https://github.com/ruru-m07.png"
                        />
                        <AvatarFallback>LT</AvatarFallback>
                      </Avatar>
                      <span className="absolute ring ring-background -end-0.5 -bottom-0.5 bg-background rounded-full">
                        <span className="sr-only">Verified</span>
                        <GithubIcon className="size-3" />
                      </span>
                    </div>
                    <span className="px-2 space-x-0.5 flex items-center">
                      <span className="text-sm text-muted-foreground font-normal">
                        ruru-m07
                      </span>
                      <span className="text-sm text-muted-foreground font-normal">
                        /
                      </span>
                      <span className="text-sm font-[450]">gitru</span>
                      <span className="text-sm mx-2 text-muted-foreground font-normal">
                        {" > "}
                      </span>
                      <span className="text-sm font-[450] flex items-center">
                        dev
                      </span>
                    </span>
                  </div>
                  <Button
                    size={"icon-xs"}
                    variant={"ghost"}
                    className="rounded-full -translate-y-0.5"
                  >
                    <X />
                  </Button>
                </div>

                <svg
                  width="15"
                  height="15"
                  viewBox="0 0 15 15"
                  fill="none"
                  xmlns="http://www.w3.org/2000/svg"
                  className="filter-[drop-shadow(1px_-1px_1px_#00000011)]"
                >
                  <path
                    d="M0 15L6.5568e-07 0C2.93563e-07 8.28427 6.71573 15 15 15L0 15Z"
                    fill="var(--background)"
                  />
                </svg>
              </div>

              <Button
                size={"icon-sm"}
                variant={"ghost"}
                className="-translate-x-1.5 -translate-y-0.5"
              >
                <Plus size={16} aria-hidden="true" />
              </Button>
            </div>
          </div>
        </>
      )}
    </div>
  );
};

export default CustomTitleBar;

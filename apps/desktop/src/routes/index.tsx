import { Button } from "@gitru/ui/components/button";
import { createFileRoute } from "@tanstack/react-router";
import { ArrowRight } from "lucide-react";
import Logo from "@/components/logo";

export const Route = createFileRoute("/")({
  component: App,
});

function App() {
  return (
    <div className="h-screen w-screen flex flex-col">
      <div
        className="h-(--main-custom-header-height) flex items-center justify-between relative px-4 select-none _border-b"
        data-tauri-drag-region
        style={{
          // @ts-expect-error - ¯\_(ツ)_/¯
          WebkitAppRegion: "drag",
        }}
      ></div>
      <div className="flex flex-col justify-center items-center relative m-auto">
        {/* <div>
          <GitPullRequestArrow
            className="text-green-500 absolute rotate-3 -top-10 -left-14"
            size={52}
          />
          <GitCompareIcon
            className="absolute rotate-3 -top-10 -left-14"
            size={52}
          />
          <GitPullRequestCreateArrowIcon
            className="absolute rotate-3 -top-10 -left-14"
            size={52}
          />
          <CircleDotDashed
            className="absolute rotate-3 -top-10 -left-14"
            size={52}
          />
          <span className="text-green-500 absolute -rotate-3 -top-10 -right-14">
            {getStatusIcon(["IndexModified"], 42)}
          </span>
          <span className="text-green-500 absolute -rotate-3 -top-10 -right-14">
            {getStatusIcon(["IndexDeleted"], 42)}
          </span>
          <span className="text-green-500 absolute -rotate-3 -top-10 -right-14">
            {getStatusIcon(["IndexNew"], 42)}
          </span>
          <span className="text-green-500 absolute -rotate-3 -top-10 -right-14">
            {getStatusIcon(["IndexRenamed"], 42)}
          </span>
        </div> */}
        <div className="-rotate-3">
          <Logo size={250} />
        </div>
        <Button
          onClick={() => {
            window.location.href = "/auth/onboarding";
          }}
          variant={"secondary"}
          className="group"
        >
          Onboard
          <ArrowRight className="group-hover:translate-x-0.5 transition-transform" />
        </Button>
      </div>
    </div>
  );
}

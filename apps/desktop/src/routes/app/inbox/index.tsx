import {
  Avatar,
  AvatarFallback,
  AvatarImage,
} from "@gitru/ui/components/avatar";
import { Button } from "@gitru/ui/components/button";
import { createFileRoute } from "@tanstack/react-router";
import { GitPullRequestArrow, ListFilterPlus, Settings2 } from "lucide-react";
import PageLayout from "@/components/pageLayout";
import { ResizableLayout } from "@/components/resizableLayout";

export const Route = createFileRoute("/app/inbox/")({
  component: RouteComponent,
});

function RouteComponent() {
  return (
    <PageLayout className="overflow-y-auto">
      <ResizableLayout
        rightPannelClassName="h-full"
        id="inbox-layout"
        minWidth={350}
        maxWidth={800}
      >
        <div className="p-2 w-full">
          <header className="flex items-center justify-between w-full">
            <span>Inbox</span>
            <div className="flex">
              <Button size={"icon-sm"} variant={"ghost"}>
                <ListFilterPlus />
              </Button>
              <Button size={"icon-sm"} variant={"ghost"}>
                <Settings2 />
              </Button>
            </div>
          </header>
          <div>
            <div className="bg-muted rounded-lg w-full p-2 flex gap-2">
              {/* <div className="relative w-fit">
                <Avatar className={"rounded-sm size-7"}>
                  <AvatarImage
                    alt="User"
                    src="https://images.unsplash.com/photo-1543610892-0b1f7e6d8ac1?w=128&h=128&dpr=2&q=80"
                  />
                  <AvatarFallback>LT</AvatarFallback>
                </Avatar>
                <span className="absolute -end-1.5 -bottom-1.5 bg-muted">
                  <GitPullRequestArrow size={22} className="text-green-600" />
                </span>
              </div> */}
              <div className="relative w-fit">
                <GitPullRequestArrow size={22} className="text-green-600" />
                <span className="absolute -inset-e-1.5 -bottom-1.5">
                  <Avatar className={"rounded-sm size-4 ring ring-muted"}>
                    <AvatarImage
                      alt="User"
                      src="https://github.com/shadcn.png"
                    />
                    <AvatarFallback>LT</AvatarFallback>
                  </Avatar>
                </span>
              </div>
              <div>
                <span className="text-sm font-[450]">gitru</span>
                <span className="text-sm text-muted-foreground font-normal">
                  {" "}
                  / ruru-m07
                </span>
              </div>
            </div>
            {/*  */}
          </div>
        </div>
        <div className="p-2">Right</div>
      </ResizableLayout>
    </PageLayout>
  );
}

import { Git, Inbox, Issue, PullRequest } from "@gitru/icon";
import {
  Avatar,
  AvatarFallback,
  AvatarImage,
} from "@gitru/ui/components/avatar";
import { Button } from "@gitru/ui/components/button";
import { ScrollArea } from "@gitru/ui/components/scroll-area";
import { TooltipProvider } from "@gitru/ui/components/tooltip";
import { Plus } from "lucide-react";
import SideBarItems from "./items";

const Sidebar = () => {
  return (
    <div className="w-[--sidebar-width] flex flex-col justify-between items-center -mr-2 ml-0.5">
      <div className="w-full _m-2 _mt-4 flex flex-col items-center">
        <TooltipProvider>
          <div className="flex flex-col items-center gap-1">
            <SideBarItems
              items={[
                {
                  icon: Inbox,
                  name: "Inbox",
                  href: "/app/inbox",
                },
                {
                  icon: PullRequest,
                  name: "pull requests",
                  href: "/app/pulls",
                },
                {
                  icon: Issue,
                  name: "Issues",
                  href: "/app/issues",
                },
                {
                  icon: Git,
                  name: "Local Git",
                  href: "/app/git",
                },
              ]}
            />
            <div className="my-1 h-px w-full bg-border" />
            {/* // TODO: we will forward to do some kindof sortcuts */}
            <ScrollArea className="w-full max-h-[calc(100vh-3rem-4rem)]">
              <div className="flex flex-col items-center gap-1">
                {[
                  "legions-developer",
                  "shadcn",
                  "manuarora700",
                  "vercel",
                  "supabase",
                  "aceternity",
                  "shadcn-ui",
                  "oraczen",
                ].map((v) => (
                  <Button
                    className="size-8 p-0"
                    key={v}
                    variant="ghost"
                    size={"icon"}
                  >
                    <Avatar className="rounded-md size-7">
                      <AvatarImage
                        src={`https://github.com/${v}.png`}
                        alt={v}
                      />
                      <AvatarFallback></AvatarFallback>
                    </Avatar>
                  </Button>
                ))}
                <Button
                  variant="secondary"
                  size={"icon"}
                  className="size-8 p-0"
                >
                  <Plus
                    className="opacity-60"
                    size={14}
                    strokeWidth={2}
                    aria-hidden="true"
                  />
                </Button>
              </div>
            </ScrollArea>

            {/* <div className="my-1 h-px w-full bg-border" /> */}
            {/* <SideBarItems
              items={[
                {
                  icon: Star,
                  href: "/starred",
                  name: "Starred",
                },
                {
                  icon: BookMarked,
                  href: "/repositories",
                  name: "Repositories",
                },
                {
                  icon: Settings,
                  href: "/settings",
                  name: "Settings",
                },
              ]}
            /> */}
          </div>
        </TooltipProvider>
      </div>

      <div>
        <Avatar className="rounded-md size-7">
          <AvatarImage alt="User" src="https://github.com/ruru-m07.png" />
          <AvatarFallback>AV</AvatarFallback>
        </Avatar>
        {/* <AvatarDropdown rateLimit={rateLimit} user={session?.user} /> */}
      </div>
    </div>
  );
};

export default Sidebar;

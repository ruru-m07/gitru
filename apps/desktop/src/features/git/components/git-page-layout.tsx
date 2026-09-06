import { Mascot } from "@gitru/mascot";
import {
  Avatar,
  AvatarFallback,
  AvatarImage,
} from "@gitru/ui/components/avatar";
import { Badge } from "@gitru/ui/components/badge";
import { Button } from "@gitru/ui/components/button";
import { useCommandNavigation } from "@gitru/ui/components/command";
import { Separator } from "@gitru/ui/components/separator";
import {
  Tooltip,
  TooltipPopup,
  TooltipTrigger,
} from "@gitru/ui/components/tooltip";
import { open } from "@tauri-apps/plugin-dialog";
import { BookCopy, CircleDashed, CopyPlus, GitBranch } from "lucide-react";
import { toast } from "sonner";
import PageLayout from "@/components/page-layout";
import StatusBar from "@/components/status-bar";
import { useRepositories } from "@/hooks/use-repositories";
import { getAvatarByProvider } from "@/lib/get-avatar-by-git-provider";
import { openExternalUrlSafely } from "@/lib/open-external-url";
import { parseOrigin } from "@/lib/parse-origin";
import { selectActiveRepository, useAppStore } from "@/store/use-app-store";
import { ResizableArea } from "./resizable-area";
export function GitPageLayout() {
  const activeRepository = useAppStore(selectActiveRepository);
  const setSelectedRepository = useAppStore(
    (state) => state.setSelectedRepository,
  );
  const setRepoSelectIsOpen = useAppStore((state) => state.setRepoSelectIsOpen);
  const { repositories, addRepo } = useRepositories();
  const navigation = useCommandNavigation();

  if (!activeRepository) {
    return (
      <PageLayout className="flex-col flex justify-center items-center gap-4">
        <div className="flex flex-col gap-4 justify-center">
          <span className="flex items-center gap-3 px-[calc(--spacing(3)-1px)]">
            <div className="relative cursor-pointer **:data-[name='mascot-svg']:size-8 **:data-[name='heart-svg']:scale-50">
              <Mascot
                particles={{
                  offset: {
                    x: -0.1,
                    y: -0.5,
                  },
                }}
                transition={{
                  duration: 0.3,
                }}
              />
            </div>
            {/* <h1 className="text-3xl font-[350]">Add your first repository</h1> */}
            <span className="text-3xl">Gitru</span>
          </span>
          <div className="flex justify-between items-end px-[calc(--spacing(3)-1px)]">
            <h1 className="text-muted-foreground font-normal">
              {repositories?.length === 0 ? "Add" : "Select"} repositorys to get
              start!
            </h1>
            <button
              type="button"
              className="text-sm font-normal text-muted-foreground hover:underline opacity-70 hover:opacity-100 transition-opacity"
              onClick={() =>
                void openExternalUrlSafely("https://gitru.app/docs")
              }
            >
              Learn more ↗
            </button>
          </div>
          <div className="grid grid-cols-3 gap-4 px-[calc(--spacing(3)-1px)]">
            <Button
              onClick={async () => {
                const folder = await open({
                  directory: true,
                  multiple: false,
                });

                if (folder) {
                  if (repositories.find((r) => r.path === folder)) {
                    toast.error("Repository already added");
                    return;
                  }

                  try {
                    const repo = await addRepo(folder);
                    if (repo) {
                      setSelectedRepository(repo);
                      setRepoSelectIsOpen(false);
                      toast.success("Repository added successfully!");
                    }
                  } catch (error) {
                    // Error already handled by the hook
                  }
                }
              }}
              className="flex-col group h-fit! items-start pt-4 pb-2 gap-2 w-64"
            >
              <BookCopy className="size-5.5" />
              <span className="text-lg font-[450]">
                Import Local Repository
              </span>
            </Button>
            <Button
              variant="secondary"
              className="flex-col group h-fit! items-start pt-4 pb-2 gap-2 w-64"
              onClick={() => {
                navigation.setOpen(true);
                navigation.push("clone-repository");
              }}
            >
              <span className="flex items-center gap-2 pl-1">
                <span className="-rotate-20 -mr-2">
                  {getAvatarByProvider("bitbucket", "size-5.5")}
                </span>

                <span className="z-10">
                  {getAvatarByProvider("github", "size-5.5")}
                </span>

                <span className="rotate-20 -ml-2">
                  {getAvatarByProvider("gitlab", "size-5.5")}
                </span>
              </span>
              <span className="text-lg font-normal">Clone from Remote</span>
            </Button>
            <Button
              variant={"secondary"}
              className="flex-col group h-fit! items-start pt-4 pb-2 gap-2 w-64"
              onClick={() => {
                navigation.setOpen(true);
                navigation.push("init-repository");
              }}
            >
              <CopyPlus className="size-5.5" />
              <span className="text-lg font-normal">Create New Repository</span>
            </Button>
          </div>
          {repositories && repositories.length > 0 ? (
            <div className="flex flex-col w-full">
              <div className="px-[calc(--spacing(3)-1px)] w-full">
                <div className="relative my-4">
                  <Separator />
                  <span className="font-normal absolute -top-3 left-6 -translate-x-1/2 bg-background px-2 text-sm text-muted-foreground">
                    Recent
                  </span>
                </div>
              </div>
              <div className="flex flex-col w-full">
                {repositories.map((repo) => {
                  const origin = parseOrigin(repo.origin || "");

                  return (
                    <Button
                      variant={"ghost"}
                      className={`py-4 px-[calc(--spacing(3)-1px)]`}
                      size={"lg"}
                      onClick={() => {
                        setSelectedRepository(repo);
                        setRepoSelectIsOpen(false);
                      }}
                    >
                      <div className="flex w-full justify-between items-center gap-2 overflow-hidden">
                        <div className="flex items-center gap-1 flex-1">
                          <div className="text-muted-foreground flex items-center">
                            {origin ? (
                              <div>
                                <Avatar className="rounded-sm size-4 -translate-y-px">
                                  <AvatarImage
                                    alt="User"
                                    src={origin.avatarUrl}
                                  />
                                  <AvatarFallback>{repo.origin}</AvatarFallback>
                                </Avatar>
                                <span className="ml-1.5">{origin?.owner}</span>
                                <span className="ml-1">/</span>
                              </div>
                            ) : (
                              <div>
                                <span className="text-foreground">
                                  {repo.origin}
                                </span>
                              </div>
                            )}
                          </div>
                          <span className="font-medium text-sm text-left text-nowrap leading-none">
                            {repo.name}
                          </span>
                          {repo?.has_uncommitted_changes && (
                            <Tooltip>
                              <TooltipTrigger
                                render={
                                  <Badge variant={"warning"} className="ml-1" />
                                }
                              >
                                <CircleDashed className="size-3" />
                              </TooltipTrigger>
                              <TooltipPopup className={"dark"}>
                                Uncommitted changes
                              </TooltipPopup>
                            </Tooltip>
                          )}
                        </div>
                        <div className="flex items-center gap-1 ml-1 min-w-0">
                          {(repo.ahead_behind?.[0] || 0) > 0 ? (
                            <Badge
                              variant={"error"}
                              className="ml-1 font-normal tabular-nums"
                            >
                              <span className="translate-y-px">
                                <span className="ml-0.5 mr-0.75 h-fit">↑</span>
                                {repo.ahead_behind?.[0] || 0}
                              </span>
                            </Badge>
                          ) : null}
                          {(repo.ahead_behind?.[1] || 0) > 0 ? (
                            <Badge
                              variant={"warning"}
                              className="ml-1 font-normal flex items-center tabular-nums"
                            >
                              <span className="translate-y-px">
                                <span className="ml-0.5 mr-0.75 h-fit">↓</span>
                                {repo.ahead_behind?.[1] || 0}
                              </span>
                            </Badge>
                          ) : null}
                          <Badge
                            variant={"info"}
                            className="flex items-center min-w-0 flex-1"
                          >
                            <span className="ml-0.5 mr-px h-fit">
                              <GitBranch strokeWidth={2.5} className="size-3" />
                            </span>
                            <span className="truncate max-w-full min-w-0 font-[450]">
                              {repo.current_branch}
                            </span>
                          </Badge>
                        </div>
                      </div>
                    </Button>
                  );
                })}
              </div>
            </div>
          ) : null}
        </div>
      </PageLayout>
    );
  }

  return (
    <PageLayout className="flex-col flex justify-between">
      <ResizableArea />
      <StatusBar />
    </PageLayout>
  );
}

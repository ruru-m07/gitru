import { Button } from "@gitru/ui/components/button";
import { CommandPanel, CommandViewConfig } from "@gitru/ui/components/command";
import { Group, GroupSeparator, GroupText } from "@gitru/ui/components/group";
import { Input } from "@gitru/ui/components/input";
import { Kbd } from "@gitru/ui/components/kbd";
import { Label } from "@gitru/ui/components/label";
import { open } from "@tauri-apps/plugin-dialog";
import { FolderOpenIcon, Loader2 } from "lucide-react";
import { useEffect, useState } from "react";
import { toast } from "sonner";
import { useRepositories } from "@/hooks/use-repositories";
import { useAppStore } from "@/store/use-app-store";

export function useInitRepositoryView(): CommandViewConfig<
  "init-repository",
  undefined
> {
  const { initRepo, repositories } = useRepositories();
  const setSelectedRepository = useAppStore(
    (state) => state.setSelectedRepository,
  );
  const setRepoSelectIsOpen = useAppStore((state) => state.setRepoSelectIsOpen);
  const [parentPath, setParentPath] = useState("");
  const [folderName, setFolderName] = useState("");
  const [didAttemptSubmit, setDidAttemptSubmit] = useState(false);
  const [isInitializing, setIsInitializing] = useState(false);

  const normalizedParentPath = parentPath.trim().replace(/\/$/, "");
  const normalizedFolderName = folderName.trim();
  const resolvedInitPath = normalizedFolderName
    ? `${normalizedParentPath}/${normalizedFolderName}`
    : normalizedParentPath;

  useEffect(() => {
    if (!normalizedParentPath) {
      setFolderName("");
    }
  }, [normalizedParentPath]);

  const validationError = (() => {
    if (!normalizedParentPath) return "Parent folder is required";
    if (/[\\:*?"<>|]/.test(normalizedFolderName)) {
      return "Folder name contains invalid characters";
    }
    if (
      resolvedInitPath &&
      repositories.find((repository) => repository.path === resolvedInitPath)
    ) {
      return "Repository already added";
    }
    return null;
  })();

  const handleInitialize = async (close: () => void) => {
    setDidAttemptSubmit(true);

    if (validationError) {
      toast.error(validationError);
      return;
    }

    setIsInitializing(true);
    try {
      const repository = await initRepo(resolvedInitPath);
      await setSelectedRepository(repository);
      setRepoSelectIsOpen(false);
      close();
      toast.success("Repository initialized successfully!");
    } catch {
      // Error already handled by the hook
    } finally {
      setIsInitializing(false);
    }
  };

  const triggerInitializeFromEnter = async (
    e: React.KeyboardEvent<HTMLInputElement>,
    close: () => void,
  ) => {
    if (e.key !== "Enter") return;
    e.preventDefault();
    await handleInitialize(close);
  };

  return {
    id: "init-repository",
    input: {
      placeholder: "Initialize repository",
      autoFocus: false,
    },
    header() {
      return <div />;
    },
    footer(context) {
      return (
        <>
          <Button
            onClick={context.navigate.back}
            variant={"secondary"}
            className="flex items-center gap-2 cursor-pointer"
          >
            <span className="text-muted-foreground">Back</span>
            <Kbd className="bg-background">Esc</Kbd>
          </Button>
          <Button
            disabled={isInitializing}
            onClick={async () => {
              await handleInitialize(context.close);
            }}
          >
            {isInitializing ? (
              <>
                <Loader2 className="size-4 animate-spin" />
                Initializing...
              </>
            ) : (
              <>
                <span>Initialize</span>
                <Kbd className="bg-primary-foreground/20 text-primary-foreground">
                  Enter
                </Kbd>
              </>
            )}
          </Button>
        </>
      );
    },
    render: (context) => {
      return (
        <CommandPanel className="p-4">
          <div className="space-y-4">
            <div className="flex flex-col w-full">
              <Label className="mb-1.5">Parent Folder</Label>
              {!normalizedParentPath ? (
                <Button
                  variant="outline"
                  className="w-fit"
                  onClick={async () => {
                    const folder = await open({
                      directory: true,
                      multiple: false,
                    });
                    if (typeof folder === "string") {
                      setParentPath(folder);
                    }
                  }}
                >
                  <FolderOpenIcon className="size-4" />
                  Select parent folder
                </Button>
              ) : (
                <Group className="w-full">
                  <GroupText render={<Label aria-label="Parent folder" />}>
                    {normalizedParentPath}/
                  </GroupText>
                  <GroupSeparator />
                  <Input
                    placeholder="existing-folder or new-folder-name"
                    type="text"
                    value={folderName}
                    onKeyDown={(e) =>
                      triggerInitializeFromEnter(e, context.close)
                    }
                    onChange={(e) => setFolderName(e.target.value)}
                    className="*:[input]:px-0! *:[input]:pl-2! w-full"
                  />
                  <GroupSeparator />
                  <GroupText
                    render={
                      <Button
                        type="button"
                        size={"icon"}
                        variant={"secondary"}
                        onClick={async () => {
                          const folder = await open({
                            directory: true,
                            multiple: false,
                          });
                          if (typeof folder === "string") {
                            setParentPath(folder);
                          }
                        }}
                      />
                    }
                  >
                    <FolderOpenIcon className="size-3.5" />
                  </GroupText>
                </Group>
              )}

              {resolvedInitPath && (
                <p className="mt-1.5 text-xs text-muted-foreground">
                  Will initialize in{" "}
                  <span className="font-mono text-foreground">
                    {normalizedParentPath}
                    {normalizedFolderName ? "/" : ""}
                  </span>
                  <span className="font-mono font-semibold text-primary">
                    {normalizedFolderName}
                  </span>
                </p>
              )}

              {didAttemptSubmit && validationError && (
                <p className="mt-1.5 text-xs text-destructive">
                  {validationError}
                </p>
              )}
            </div>
          </div>
        </CommandPanel>
      );
    },
  };
}

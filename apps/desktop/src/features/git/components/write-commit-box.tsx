import type { CommitMessage } from "@gitru/commands";
import { Button } from "@gitru/ui/components/button";
import { Group, GroupSeparator } from "@gitru/ui/components/group";
import {
  InputGroup,
  InputGroupAddon,
  InputGroupInput,
  InputGroupTextarea,
} from "@gitru/ui/components/input-group";
import {
  Menu,
  MenuItem,
  MenuPopup,
  MenuTrigger,
} from "@gitru/ui/components/menu";
import { ChevronDownIcon, Loader2, Sparkles, UserPlus } from "lucide-react";
import { memo, useCallback, useState } from "react";
import { toast } from "sonner";
import {
  useCreateCommit,
  useGetCurrentBranch,
  useGetStatus,
  useGitAdd,
} from "@/hooks";

export const WriteCommitBox = memo(function WriteCommitBox({
  visibleAddablePaths,
}: {
  visibleAddablePaths: string[];
}) {
  const [title, setTitle] = useState("");
  const [description, setDescription] = useState("");
  const [co_authors, setCoAuthors] = useState<CommitMessage["co_authors"]>([]);

  const { data: currentBranch } = useGetCurrentBranch();
  const { data: status } = useGetStatus();
  const { mutateAsync: gitAdd, isPending: isAdding } = useGitAdd();
  const { mutateAsync: createCommit, isPending: isCreatingCommit } =
    useCreateCommit();

  const nothingToCommit =
    status?.files.filter((file) =>
      file.status.some((s) => s.startsWith("Index")),
    ).length === 0;

  const handelCommit = useCallback(async () => {
    if (nothingToCommit) {
      if (visibleAddablePaths.length === 0) {
        toast.error("No visible changes to add");
        return;
      }

      await gitAdd(visibleAddablePaths);
    }

    const data = await createCommit({
      commitMeta: {
        title,
        description,
        co_authors,
      },
      allowEmpty: false,
    });
    if (data) {
      setTitle("");
      setDescription("");
      setCoAuthors([]);
      toast.success("Commit created successfully");
    }
  }, [
    co_authors,
    createCommit,
    description,
    gitAdd,
    nothingToCommit,
    title,
    visibleAddablePaths,
  ]);

  return (
    <div>
      <div className="shrink-0 flex flex-col gap-2 justify-between items-center border-t px-2 py-2 ">
        <InputGroup>
          <InputGroupInput
            placeholder="Summary (required)"
            className="h-8"
            value={title}
            onChange={(e) => setTitle(e.target.value)}
          />
          <InputGroupAddon align="inline-end">
            <Button
              aria-label="Password requirements"
              size="icon-xs"
              variant="ghost"
              className="opacity-50 hover:opacity-100"
            >
              <Sparkles size={16} />
            </Button>
          </InputGroupAddon>
        </InputGroup>
        <InputGroup>
          <InputGroupTextarea
            placeholder="Description"
            value={description}
            onChange={(e) => setDescription(e.target.value)}
          />
          <InputGroupAddon align="block-end">
            <Button
              variant="ghost"
              size="icon-xs"
              className="opacity-50 hover:opacity-100"
              aria-label="Add Co Authors"
            >
              <UserPlus size={16} />
            </Button>
          </InputGroupAddon>
        </InputGroup>
        <div className="w-full flex items-center gap-2">
          <Group aria-label="Subscription actions" className="w-full">
            <Button
              onClick={handelCommit}
              className="flex-1 truncate"
              disabled={isAdding || isCreatingCommit}
            >
              {isAdding || isCreatingCommit ? (
                <>
                  <Loader2 className="size-4 animate-spin" />
                  Committing...
                </>
              ) : (
                <>
                  {nothingToCommit ? (
                    <span>Add visible & Commit</span>
                  ) : (
                    <span className="truncate">
                      Commit to <span>{currentBranch?.name}</span>
                    </span>
                  )}
                </>
              )}
            </Button>
            <GroupSeparator className="bg-primary/72" />
            <Menu>
              <MenuTrigger
                render={
                  <Button
                    aria-label="Copy options"
                    size="icon"
                    className="rounded-r-lg!"
                  />
                }
              >
                <ChevronDownIcon className="size-4" />
              </MenuTrigger>
              <MenuPopup align="end" className={"w-full"}>
                <MenuItem>Empty Commit</MenuItem>
                <MenuItem>Amend Last Commit</MenuItem>
              </MenuPopup>
            </Menu>
          </Group>
        </div>
      </div>
    </div>
  );
});

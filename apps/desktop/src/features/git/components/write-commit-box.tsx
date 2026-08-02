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
import { memo, useCallback, useEffect, useState } from "react";
import { toast } from "sonner";
import {
  useCreateCommit,
  useGetCurrentBranch,
  useGetRepoOperation,
  useGetStatus,
  useGitAdd,
} from "@/hooks";
import {
  splitCommitMessage,
  useCommitDraftStore,
} from "@/store/use-commit-draft-store";

export const WriteCommitBox = memo(function WriteCommitBox({
  visibleAddablePaths,
}: {
  visibleAddablePaths: string[];
}) {
  const [co_authors, setCoAuthors] = useState<CommitMessage["co_authors"]>([]);

  const title = useCommitDraftStore((s) => s.title);
  const description = useCommitDraftStore((s) => s.description);
  const setTitle = useCommitDraftStore((s) => s.setTitle);
  const setDescription = useCommitDraftStore((s) => s.setDescription);
  const applyAutofill = useCommitDraftStore((s) => s.applyAutofill);
  const clearDraft = useCommitDraftStore((s) => s.clear);

  const { data: currentBranch } = useGetCurrentBranch();
  const { data: status } = useGetStatus();
  const { data: operation } = useGetRepoOperation();
  const { mutateAsync: gitAdd, isPending: isAdding } = useGitAdd();
  const { mutateAsync: createCommit, isPending: isCreatingCommit } =
    useCreateCommit();

  // Prefill Summary/Description from the paused rebase commit — Continue reads
  // the same draft store. Keyed so refetch doesn't clobber user edits.
  // Autofill for reword and edit (edit is optional to change, but helpful).
  const shouldAutofillRebaseMessage =
    !!operation?.isRebasing &&
    !!operation.commitMessage?.trim() &&
    (operation.pauseReason === "reword" ||
      operation.pauseReason === "edit" ||
      operation.pauseReason === "conflict");
  const rebaseAutofillKey = shouldAutofillRebaseMessage
    ? `rebase:${operation.pausedAt ?? ""}:${operation.current ?? ""}:${operation.commitMessage}`
    : null;
  const rebaseMessage = shouldAutofillRebaseMessage
    ? operation?.commitMessage
    : undefined;
  const isRebasing = !!operation?.isRebasing;

  useEffect(() => {
    if (rebaseAutofillKey && rebaseMessage) {
      const parts = splitCommitMessage(rebaseMessage);
      applyAutofill(rebaseAutofillKey, parts.title, parts.description);
      return;
    }
    if (!isRebasing) {
      const key = useCommitDraftStore.getState().autofillKey;
      if (key?.startsWith("rebase:")) clearDraft();
    }
  }, [applyAutofill, clearDraft, isRebasing, rebaseAutofillKey, rebaseMessage]);

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
      clearDraft();
      setCoAuthors([]);
      toast.success("Commit created successfully");
    }
  }, [
    clearDraft,
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

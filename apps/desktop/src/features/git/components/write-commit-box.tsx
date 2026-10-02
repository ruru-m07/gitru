import type { Author, RepoOperation } from "@gitru/commands";
import {
  AlertDialog,
  AlertDialogClose,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogPopup,
  AlertDialogTitle,
} from "@gitru/ui/components/alert-dialog";
import { Badge } from "@gitru/ui/components/badge";
import { Button } from "@gitru/ui/components/button";
import {
  Combobox,
  ComboboxChips,
  ComboboxEmpty,
  ComboboxInput,
  ComboboxItem,
  ComboboxList,
} from "@gitru/ui/components/combobox";
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
import {
  Popover,
  PopoverDescription,
  PopoverPopup,
  PopoverTitle,
  PopoverTrigger,
} from "@gitru/ui/components/popover";
import {
  ChevronDownIcon,
  Loader2,
  SearchIcon,
  UserPlus,
  XIcon,
} from "lucide-react";
import { memo, useEffect, useMemo, useRef, useState } from "react";
import { toast } from "sonner";
import {
  useCreateCommit,
  useGetCommitAuthors,
  useGetCurrentBranch,
  useGetLastCommit,
  useGetRepoOperation,
  useGetStatus,
  useGitAdd,
} from "@/hooks";
import { useActiveRepositoryState } from "@/state/use-active-repository-state";
import {
  splitCommitMessage,
  stripTrailingCoAuthorTrailers,
  useCommitDraftStore,
} from "@/store/use-commit-draft-store";

type CommitAction = "create" | "empty" | "amend";

const REBASE_OPERATION_KINDS = new Set<RepoOperation["kind"]>([
  "rebase",
  "rebaseInteractive",
  "rebaseMerge",
]);
const NORMAL_COMMIT_OPERATION_KINDS = new Set<RepoOperation["kind"]>([
  "clean",
  "merge",
  "revert",
  "cherryPick",
  "bisect",
]);

export function getCommitActionAvailability(
  operation: RepoOperation | null | undefined,
) {
  if (!operation) {
    return { create: false, empty: false, amend: false, isRebaseLike: false };
  }

  const isRebaseLike =
    operation.isRebasing || REBASE_OPERATION_KINDS.has(operation.kind);
  const hasConflicts = operation.conflictPaths.length > 0;
  const clean = operation.kind === "clean" && !isRebaseLike && !hasConflicts;
  const editableRebasePause =
    isRebaseLike && operation.pauseReason === "edit" && !hasConflicts;

  return {
    create:
      !isRebaseLike &&
      !hasConflicts &&
      NORMAL_COMMIT_OPERATION_KINDS.has(operation.kind),
    empty: clean,
    amend: clean || editableRebasePause,
    isRebaseLike,
  };
}

function authorKey(author: Author | readonly [string, string]) {
  const email = "name" in author ? author.email : author[1];
  return email.trim().toLowerCase();
}

function authorTuple(author: Author): [string, string] {
  return [author.name, author.email];
}

export const WriteCommitBox = memo(function WriteCommitBox({
  visibleAddablePaths,
}: {
  visibleAddablePaths: string[];
}) {
  const summaryRef = useRef<HTMLInputElement>(null);
  const [emptyCommitOpen, setEmptyCommitOpen] = useState(false);
  const [coAuthorPickerOpen, setCoAuthorPickerOpen] = useState(false);
  const [enteringAmend, setEnteringAmend] = useState(false);

  const repo = useActiveRepositoryState();
  const title = useCommitDraftStore((state) => state.title);
  const description = useCommitDraftStore((state) => state.description);
  const coAuthors = useCommitDraftStore((state) => state.coAuthors);
  const mode = useCommitDraftStore((state) => state.mode);
  const amendCommitId = useCommitDraftStore((state) => state.amendCommitId);
  const setTitle = useCommitDraftStore((state) => state.setTitle);
  const setDescription = useCommitDraftStore((state) => state.setDescription);
  const setCoAuthors = useCommitDraftStore((state) => state.setCoAuthors);
  const applyAutofill = useCommitDraftStore((state) => state.applyAutofill);
  const beginAmend = useCommitDraftStore((state) => state.beginAmend);
  const cancelAmend = useCommitDraftStore((state) => state.cancelAmend);
  const switchRepo = useCommitDraftStore((state) => state.switchRepo);
  const clearDraft = useCommitDraftStore((state) => state.clear);

  const { data: currentBranch } = useGetCurrentBranch();
  const { data: status, isLoading: statusLoading } = useGetStatus();
  const { data: operation, isLoading: operationLoading } =
    useGetRepoOperation();
  const {
    data: lastCommit,
    isLoading: lastCommitLoading,
    refetch: refetchLastCommit,
  } = useGetLastCommit();
  const { data: suggestedAuthors = [], isLoading: authorsLoading } =
    useGetCommitAuthors();
  const { mutateAsync: gitAdd, isPending: isAdding } = useGitAdd();
  const { mutateAsync: createCommit, isPending: isCreatingCommit } =
    useCreateCommit();

  useEffect(() => {
    switchRepo(repo?.contextId ?? null);
  }, [repo?.contextId, switchRepo]);

  // Prefill Summary/Description from the paused rebase commit — Continue reads
  // the same draft store. Keyed so refetch doesn't clobber user edits.
  const shouldAutofillRebaseMessage =
    !!operation?.isRebasing &&
    !!operation.commitMessage?.trim() &&
    (operation.pauseReason === "reword" ||
      operation.pauseReason === "edit" ||
      operation.pauseReason === "conflict");
  const rebaseAutofillKey = shouldAutofillRebaseMessage
    ? `rebase:${operation?.pausedAt ?? ""}:${operation?.current ?? ""}:${operation?.commitMessage}`
    : null;
  const rebaseMessage = shouldAutofillRebaseMessage
    ? operation?.commitMessage
    : undefined;
  const isRebasing = !!operation?.isRebasing;

  useEffect(() => {
    if (rebaseAutofillKey && rebaseMessage) {
      const parts = splitCommitMessage(rebaseMessage);
      applyAutofill(
        rebaseAutofillKey,
        parts.title,
        parts.description,
        parts.coAuthors,
      );
      return;
    }
    if (!operationLoading && !isRebasing) {
      const draft = useCommitDraftStore.getState();
      const rebaseScopedAmend =
        draft.autofillKey?.startsWith("amend:") &&
        draft.draftBeforeAmend?.autofillKey?.startsWith("rebase:");
      if (draft.autofillKey?.startsWith("rebase:") || rebaseScopedAmend) {
        clearDraft();
      }
    }
  }, [
    applyAutofill,
    clearDraft,
    isRebasing,
    operationLoading,
    rebaseAutofillKey,
    rebaseMessage,
  ]);

  const selectedAuthors = useMemo<Author[]>(() => {
    const authors = coAuthors.map(([name, email]) => ({ name, email }));
    return authors.filter(
      (author, index) =>
        authors.findIndex(
          (candidate) => authorKey(candidate) === authorKey(author),
        ) === index,
    );
  }, [coAuthors]);
  const coAuthorOptions = useMemo(() => {
    const authors = [...selectedAuthors, ...(suggestedAuthors ?? [])];
    return authors.filter(
      (author, index) =>
        author.name.trim().length > 0 &&
        author.email.trim().length > 0 &&
        authors.findIndex(
          (candidate) => authorKey(candidate) === authorKey(author),
        ) === index,
    );
  }, [selectedAuthors, suggestedAuthors]);

  const hasStagedChanges =
    status?.files.some((file) =>
      file.status.some((fileStatus) => fileStatus.startsWith("Index")),
    ) ?? false;
  const statusReady = !statusLoading && status != null;
  const availability = operationLoading
    ? { create: false, empty: false, amend: false, isRebaseLike: false }
    : getCommitActionAvailability(operation);
  const summaryValid = title.trim().length > 0;
  const busy = isAdding || isCreatingCommit || enteringAmend;
  const canSubmitCreate =
    availability.create &&
    statusReady &&
    (hasStagedChanges || visibleAddablePaths.length > 0);
  const canSubmitEmpty = availability.empty && statusReady && !hasStagedChanges;
  const canSubmitAmend =
    availability.amend && statusReady && amendCommitId !== null;
  const submitDisabled =
    busy ||
    !summaryValid ||
    (mode === "amend" ? !canSubmitAmend : !canSubmitCreate);

  const submitCommit = async (action: CommitAction): Promise<boolean> => {
    if (!summaryValid) {
      summaryRef.current?.focus();
      return false;
    }
    if (action === "create" && !canSubmitCreate) return false;
    if (action === "empty" && !canSubmitEmpty) return false;
    if (action === "amend" && (!canSubmitAmend || !amendCommitId)) return false;

    const submissionRepoKey = repo?.contextId ?? null;
    try {
      if (action === "create" && !hasStagedChanges) {
        await gitAdd(visibleAddablePaths);
      }

      await createCommit({
        commitMeta: {
          title,
          description,
          co_authors: selectedAuthors.map(authorTuple),
        },
        allowEmpty: action === "empty",
        amend: action === "amend",
        expectedHead:
          action === "amend" ? (amendCommitId ?? undefined) : undefined,
      });

      if (useCommitDraftStore.getState().repoKey === submissionRepoKey) {
        clearDraft();
      }
      setEmptyCommitOpen(false);
      setCoAuthorPickerOpen(false);
      toast.success(
        action === "amend"
          ? "Last commit amended"
          : action === "empty"
            ? "Empty commit created"
            : "Commit created successfully",
      );
      return true;
    } catch {
      // Mutation hooks report the error. Keep every draft field and the active
      // mode intact so the user can fix the problem and retry.
      return false;
    }
  };

  const enterAmendMode = async () => {
    if (!availability.amend || !lastCommit || mode === "amend") return;
    const amendRepoKey = repo?.contextId ?? null;
    setEnteringAmend(true);
    try {
      const { data: freshLastCommit, error } = await refetchLastCommit();
      if (useCommitDraftStore.getState().repoKey !== amendRepoKey) return;
      if (error || !freshLastCommit) {
        toast.error(error?.message || "Could not load the latest commit");
        return;
      }
      beginAmend(
        freshLastCommit.id,
        freshLastCommit.summary,
        stripTrailingCoAuthorTrailers(freshLastCommit.body),
        freshLastCommit.authors.co_authors.map((author) => [
          author.name,
          author.email,
        ]),
      );
      requestAnimationFrame(() => summaryRef.current?.focus());
    } finally {
      setEnteringAmend(false);
    }
  };

  const removeCoAuthor = (key: string) => {
    setCoAuthors(coAuthors.filter((author) => authorKey(author) !== key));
  };

  return (
    <div className="shrink-0 flex flex-col gap-2 justify-between items-center border-t px-2 py-2">
      <InputGroup>
        <InputGroupInput
          ref={summaryRef}
          aria-invalid={!summaryValid && title.length > 0}
          aria-label="Commit summary"
          className="h-8"
          placeholder="Summary (required)"
          required
          type="text"
          value={title}
          onChange={(event) => setTitle(event.target.value)}
        />
      </InputGroup>

      <InputGroup>
        <InputGroupTextarea
          aria-label="Commit description"
          placeholder="Description"
          value={description}
          onChange={(event) => setDescription(event.target.value)}
        />
        <InputGroupAddon align="block-end" className="flex-wrap gap-1">
          {selectedAuthors.map((author) => {
            const key = authorKey(author);
            return (
              <Badge
                key={key}
                variant="secondary"
                render={
                  <button
                    type="button"
                    aria-label={`Remove co-author ${author.name}`}
                    disabled={busy}
                    onClick={() => removeCoAuthor(key)}
                  />
                }
              >
                <span className="max-w-28 truncate">{author.name}</span>
                <XIcon aria-hidden="true" />
              </Badge>
            );
          })}

          <Popover
            open={coAuthorPickerOpen}
            onOpenChange={(open) => {
              if (!busy) setCoAuthorPickerOpen(open);
            }}
          >
            <PopoverTrigger
              render={
                <Button
                  type="button"
                  aria-label={
                    selectedAuthors.length > 0
                      ? `Manage co-authors, ${selectedAuthors.length} selected`
                      : "Add co-authors"
                  }
                  variant="ghost"
                  size="xs"
                  disabled={busy}
                />
              }
            >
              <UserPlus aria-hidden="true" />
              Co-authors
              {selectedAuthors.length > 0 ? (
                <span className="tabular-nums text-muted-foreground">
                  {selectedAuthors.length}
                </span>
              ) : null}
            </PopoverTrigger>
            <PopoverPopup
              align="start"
              className="w-80 flex-col gap-2 p-2"
              side="top"
              viewport={false}
            >
              <PopoverTitle className="sr-only">Choose co-authors</PopoverTitle>
              <PopoverDescription className="px-1 text-xs">
                Search Git configuration and recent commit authors.
              </PopoverDescription>
              <Combobox<Author, Author, true>
                autoHighlight
                inline
                items={coAuthorOptions}
                itemToStringLabel={(author: Author) =>
                  `${author.name} ${author.email}`
                }
                itemToStringValue={(author: Author) => author.email}
                isItemEqualToValue={(author: Author, selected: Author) =>
                  authorKey(author) === authorKey(selected)
                }
                multiple
                value={selectedAuthors}
                onValueChange={(authors: Author[]) =>
                  setCoAuthors(authors.map(authorTuple))
                }
              >
                <ComboboxChips>
                  <SearchIcon
                    aria-hidden="true"
                    className="ml-1 size-4 opacity-50"
                  />
                  <ComboboxInput
                    aria-label="Search co-authors"
                    autoFocus
                    placeholder="Search by name or email…"
                    showTrigger={false}
                  />
                </ComboboxChips>
                <ComboboxEmpty>
                  {authorsLoading ? "Loading authors…" : "No authors found."}
                </ComboboxEmpty>
                <ComboboxList>
                  {(author) => (
                    <ComboboxItem key={authorKey(author)} value={author}>
                      <span className="flex min-w-0 flex-col">
                        <span className="truncate">{author.name}</span>
                        <span className="truncate text-xs text-muted-foreground">
                          {author.email}
                        </span>
                      </span>
                    </ComboboxItem>
                  )}
                </ComboboxList>
              </Combobox>
            </PopoverPopup>
          </Popover>
        </InputGroupAddon>
      </InputGroup>

      {mode === "amend" ? (
        <div
          className="flex w-full items-center justify-between gap-2 rounded-md border border-info/24 bg-info/8 px-2 py-1.5 text-xs"
          role="status"
        >
          <span className="min-w-0 truncate">
            Amending {amendCommitId?.slice(0, 7)}
            {hasStagedChanges ? " with staged changes" : " (message only)"}
          </span>
          <Button
            type="button"
            className="shrink-0"
            disabled={busy}
            onClick={cancelAmend}
            size="xs"
            variant="ghost"
          >
            Cancel amend
          </Button>
        </div>
      ) : availability.isRebaseLike ? (
        <p className="w-full text-xs text-muted-foreground" role="status">
          Use the rebase controls to continue. Commit creation is unavailable.
        </p>
      ) : null}

      <Group aria-label="Commit actions" className="w-full">
        <Button
          type="button"
          onClick={() =>
            void submitCommit(mode === "amend" ? "amend" : "create")
          }
          className="flex-1 truncate"
          disabled={submitDisabled}
        >
          {busy ? (
            <>
              <Loader2 aria-hidden="true" className="size-4 animate-spin" />
              {mode === "amend" ? "Amending…" : "Committing…"}
            </>
          ) : mode === "amend" ? (
            <span>Amend last commit</span>
          ) : !hasStagedChanges ? (
            <span>Add visible &amp; Commit</span>
          ) : (
            <span className="truncate">
              Commit to <span>{currentBranch?.name}</span>
            </span>
          )}
        </Button>
        <GroupSeparator className="bg-primary/72" />
        <Menu>
          <MenuTrigger
            render={
              <Button
                type="button"
                aria-label="Commit options"
                size="icon"
                className="rounded-r-lg!"
                disabled={busy}
              />
            }
          >
            <ChevronDownIcon aria-hidden="true" className="size-4" />
          </MenuTrigger>
          <MenuPopup align="end" className="w-56">
            <MenuItem
              closeOnClick
              disabled={!canSubmitEmpty || !summaryValid || mode === "amend"}
              onClick={() => setEmptyCommitOpen(true)}
            >
              Empty Commit…
            </MenuItem>
            <MenuItem
              closeOnClick
              disabled={
                !availability.amend ||
                lastCommitLoading ||
                !lastCommit ||
                mode === "amend"
              }
              onClick={() => void enterAmendMode()}
            >
              Amend Last Commit
            </MenuItem>
          </MenuPopup>
        </Menu>
      </Group>

      <AlertDialog
        open={emptyCommitOpen}
        onOpenChange={(open) => {
          if (!busy) setEmptyCommitOpen(open);
        }}
      >
        <AlertDialogPopup>
          <AlertDialogHeader>
            <AlertDialogTitle>Create an empty commit?</AlertDialogTitle>
            <AlertDialogDescription>
              This creates a commit on{" "}
              {currentBranch?.name ?? "the current branch"}
              without including file changes. Your working tree will be left
              unchanged.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogClose
              render={
                <Button type="button" variant="outline" disabled={busy} />
              }
            >
              Cancel
            </AlertDialogClose>
            <Button
              type="button"
              disabled={busy || !canSubmitEmpty || !summaryValid}
              onClick={() => void submitCommit("empty")}
            >
              {isCreatingCommit ? (
                <Loader2 aria-hidden="true" className="animate-spin" />
              ) : null}
              Create empty commit
            </Button>
          </AlertDialogFooter>
        </AlertDialogPopup>
      </AlertDialog>
    </div>
  );
});

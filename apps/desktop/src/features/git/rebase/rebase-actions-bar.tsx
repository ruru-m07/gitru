import type { RepoOperation } from "@gitru/commands";
import { Button } from "@gitru/ui/components/button";
import {
  Dialog,
  DialogClose,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogPopup,
  DialogTitle,
} from "@gitru/ui/components/dialog";
import { Group, GroupSeparator } from "@gitru/ui/components/group";
import {
  ChevronDownIcon,
  CircleSlash,
  Loader2,
  SkipForward,
  StepForward,
} from "lucide-react";
import { useState } from "react";
import { toast } from "sonner";
import {
  useRebaseAbort,
  useRebaseAbortPreview,
  useRebaseContinue,
  useRebaseSkip,
} from "@/hooks";
import {
  joinCommitMessage,
  useCommitDraftStore,
} from "@/store/use-commit-draft-store";

type RebaseDraftIdentity = {
  repoKey: string | null;
  autofillKey: string | null;
};

function getRebaseDraftIdentity(): RebaseDraftIdentity {
  const { repoKey, autofillKey } = useCommitDraftStore.getState();
  return { repoKey, autofillKey };
}

function clearRebaseDraftIfUnchanged(identity: RebaseDraftIdentity) {
  const draft = useCommitDraftStore.getState();
  if (
    draft.repoKey === identity.repoKey &&
    draft.autofillKey === identity.autofillKey
  ) {
    draft.clear();
  }
}

export function RebaseActionsBar({ operation }: { operation: RepoOperation }) {
  const { mutateAsync: continueRebase, isPending: continuing } =
    useRebaseContinue();
  const { mutateAsync: skip, isPending: skipping } = useRebaseSkip();
  const { mutateAsync: abort, isPending: aborting } = useRebaseAbort();
  const { mutateAsync: loadPreview } = useRebaseAbortPreview();
  const [abortOpen, setAbortOpen] = useState(false);
  const [abortWarning, setAbortWarning] = useState("");

  const title = useCommitDraftStore((s) => s.title);
  const description = useCommitDraftStore((s) => s.description);
  const coAuthors = useCommitDraftStore((s) => s.coAuthors);

  const busy = continuing || skipping || aborting;
  const hasConflicts = operation.conflictPaths.length > 0;
  const draftMessage = joinCommitMessage(title, description, coAuthors);
  // Only reword requires a message. Edit pauses just need `rebase --continue`
  // (amend is optional). Fall back to the server-provided commit message when
  // the draft is empty so Continue isn't blocked by a missed autofill.
  const needsMessage = operation.pauseReason === "reword";
  const continueMessage =
    draftMessage.trim() ||
    (needsMessage ? (operation.commitMessage?.trim() ?? "") : "");

  return (
    <div className="shrink-0 flex flex-col gap-2">
      <div className="flex items-center gap-2 shrink-0">
        <Button
          variant="destructive-outline"
          disabled={busy}
          size="sm"
          onClick={async () => {
            try {
              const preview = await loadPreview();
              setAbortWarning(preview.warning);
              setAbortOpen(true);
            } catch (error) {
              toast.error(
                error instanceof Error ? error.message : "Abort preview failed",
              );
            }
          }}
        >
          Abort
          <CircleSlash />
        </Button>
        <Button
          variant="outline"
          disabled={busy}
          size="sm"
          onClick={() => {
            const draftIdentity = getRebaseDraftIdentity();
            toast.promise(
              skip().then((op) => {
                clearRebaseDraftIfUnchanged(draftIdentity);
                return op;
              }),
              {
                loading: "Skipping…",
                success: "Skipped commit",
                error: (e) => e.message || "Skip failed",
              },
            );
          }}
        >
          Skip
          {skipping ? <Loader2 className="animate-spin" /> : <SkipForward />}
        </Button>
        <Group aria-label="Rebase continue actions">
          <Button
            disabled={
              busy || hasConflicts || (needsMessage && !continueMessage)
            }
            size="sm"
            onClick={() => {
              const draftIdentity = getRebaseDraftIdentity();
              toast.promise(
                continueRebase(continueMessage).then((op) => {
                  clearRebaseDraftIfUnchanged(draftIdentity);
                  return op;
                }),
                {
                  loading: "Continuing rebase…",
                  success: (op) =>
                    op.isRebasing ? "Continued" : "Rebase finished",
                  error: (e) => e.message || "Continue failed",
                },
              );
            }}
          >
            Continue
            {continuing ? (
              <Loader2 className="animate-spin" />
            ) : (
              <StepForward />
            )}
          </Button>
          <GroupSeparator className="bg-primary/72" />
          <Button aria-label="Continue options" size="icon-sm">
            <ChevronDownIcon aria-hidden="true" className="size-4" />
          </Button>
        </Group>
      </div>

      <Dialog open={abortOpen} onOpenChange={setAbortOpen}>
        <DialogPopup>
          <DialogHeader>
            <DialogTitle>Abort rebase?</DialogTitle>
            <DialogDescription>{abortWarning}</DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <DialogClose render={<Button variant="secondary" />}>
              Cancel
            </DialogClose>
            <Button
              variant="destructive"
              disabled={aborting}
              onClick={async () => {
                const draftIdentity = getRebaseDraftIdentity();
                await abort();
                clearRebaseDraftIfUnchanged(draftIdentity);
                setAbortOpen(false);
              }}
            >
              Abort rebase
            </Button>
          </DialogFooter>
        </DialogPopup>
      </Dialog>
    </div>
  );
}

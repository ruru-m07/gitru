import { collaborationErrorMessage } from "@gitru/collaboration-client";
import {
  type CollaborationRecoveryPreview,
  collaborationBackup,
  collaborationCancelRecovery,
  collaborationConfirmRecovery,
  collaborationInspectInterruptedRecovery,
  collaborationPrepareRestore,
} from "@gitru/commands";
import {
  AlertDialog,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogPopup,
  AlertDialogTitle,
} from "@gitru/ui/components/alert-dialog";
import { Button } from "@gitru/ui/components/button";
import {
  Dialog,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogPanel,
  DialogPopup,
  DialogTitle,
  DialogTrigger,
} from "@gitru/ui/components/dialog";
import { ArchiveRestore } from "lucide-react";
import { useEffect, useRef, useState } from "react";

/** Available even when the normal account/storage query cannot start. */
export function BackupRecoveryButton() {
  const [open, setOpen] = useState(false);
  const [busy, setBusy] = useState(false);
  const [preview, setPreview] = useState<CollaborationRecoveryPreview | null>(
    null,
  );
  const [error, setError] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const active = useRef(true);
  const session = useRef<string | null>(null);
  // This effect owns the native preview lifetime, including navigation/unmount.
  useEffect(() => {
    active.current = true;
    return () => {
      active.current = false;
      if (session.current) {
        void collaborationCancelRecovery({ sessionId: session.current }).catch(
          () => {},
        );
        session.current = null;
      }
    };
  }, []);

  async function run(action: () => Promise<void>) {
    if (busy) return;
    setBusy(true);
    setError(null);
    setMessage(null);
    try {
      await action();
    } catch (failure) {
      if (active.current) setError(collaborationErrorMessage(failure));
    } finally {
      if (active.current) setBusy(false);
    }
  }

  async function acceptPreview(next: CollaborationRecoveryPreview | null) {
    if (!next) return;
    if (!active.current) {
      await collaborationCancelRecovery({ sessionId: next.session_id });
      return;
    }
    session.current = next.session_id;
    setPreview(next);
  }

  async function cancelPreview() {
    if (!preview) return;
    let result;
    try {
      result = await collaborationCancelRecovery({
        sessionId: preview.session_id,
      });
    } catch (failure) {
      if (
        typeof failure !== "object" ||
        failure === null ||
        !("code" in failure) ||
        failure.code !== "stale_view"
      )
        throw failure;
      // The native deadline already retired this preview and resumed storage.
      result = { runtime_ready: true };
    }
    session.current = null;
    setPreview(null);
    if (!result.runtime_ready)
      setMessage(
        "Recovery was cancelled. Saved data remains preserved; storage still needs recovery.",
      );
  }

  async function confirm() {
    if (!preview) return;
    const confirmation =
      preview.restore?.confirmation_id ?? preview.interrupted?.confirmation_id;
    if (!confirmation) return;
    // After confirmation the native task owns completion even if this view goes away.
    session.current = null;
    try {
      const result = await collaborationConfirmRecovery({
        request: {
          session_id: preview.session_id,
          confirmation_id: confirmation,
          action: preview.restore
            ? "replace_current_data"
            : "keep_original_data",
        },
      });
      if (!active.current) return;
      setPreview(null);
      setMessage(
        result.runtime_ready
          ? "Recovery finished. Original data remains preserved on this device. Reconnect restored accounts to sync again."
          : "The recovery files were preserved, but storage could not restart. Inspect interrupted recovery or choose a verified backup.",
      );
    } catch (failure) {
      await collaborationCancelRecovery({
        sessionId: preview.session_id,
      }).catch(() => {});
      if (active.current) setPreview(null);
      throw failure;
    }
  }

  function changeOpen(next: boolean) {
    if (busy) return;
    if (!next && preview) {
      void run(async () => {
        await cancelPreview();
        setOpen(false);
      });
    } else {
      setOpen(next);
    }
  }

  return (
    <>
      <Dialog open={open} onOpenChange={changeOpen}>
        <DialogTrigger
          render={<Button type="button" variant="ghost" size="sm" />}
        >
          <ArchiveRestore aria-hidden="true" /> Backups
        </DialogTrigger>
        <DialogPopup showCloseButton={!busy}>
          <DialogHeader>
            <DialogTitle>Back up and recover collaboration data</DialogTitle>
            <DialogDescription>
              Save your private drafts, connected account records and local
              collaboration state. Tokens and passwords are excluded. Local Git
              repositories stay separate.
            </DialogDescription>
          </DialogHeader>
          <DialogPanel className="space-y-4">
            <div className="space-y-2">
              <Button
                type="button"
                variant="outline"
                disabled={busy || preview !== null}
                onClick={() =>
                  void run(async () => {
                    const result = await collaborationBackup({});
                    if (result && active.current)
                      setMessage(
                        `Backup verified and saved: ${result.drafts} drafts, ${result.accounts} accounts and ${result.commands} commands.`,
                      );
                  })
                }
              >
                Save a backup
              </Button>
              <p className="text-xs text-muted-foreground">
                Choose a new file. Existing backups are never overwritten.
              </p>
            </div>
            <div className="space-y-2">
              <Button
                type="button"
                variant="outline"
                disabled={busy || preview !== null}
                onClick={() =>
                  void run(async () =>
                    acceptPreview(await collaborationPrepareRestore({})),
                  )
                }
              >
                Choose backup to restore
              </Button>
              <p className="text-xs text-muted-foreground">
                You can review the backup before replacing current collaboration
                data. Collaboration pauses while the preview is open.
              </p>
            </div>
            <div className="space-y-2">
              <Button
                type="button"
                variant="ghost"
                disabled={busy || preview !== null}
                onClick={() =>
                  void run(async () =>
                    acceptPreview(
                      await collaborationInspectInterruptedRecovery({}),
                    ),
                  )
                }
              >
                Inspect interrupted recovery
              </Button>
              <p className="text-xs text-muted-foreground">
                Recover preserved originals if a previous restore was
                interrupted.
              </p>
            </div>
            {busy ? (
              <p role="status" className="text-sm text-muted-foreground">
                Working with the selected local files…
              </p>
            ) : null}
            {!preview && error ? (
              <p role="alert" className="text-sm text-destructive-foreground">
                {error}
              </p>
            ) : null}
            {message ? (
              <p role="status" className="text-sm">
                {message}
              </p>
            ) : null}
          </DialogPanel>
          <DialogFooter>
            <Button
              type="button"
              variant="ghost"
              disabled={busy}
              onClick={() => changeOpen(false)}
            >
              Close
            </Button>
          </DialogFooter>
        </DialogPopup>
      </Dialog>
      <AlertDialog
        open={preview !== null}
        onOpenChange={(next) => {
          if (!next && !busy) void run(cancelPreview);
        }}
      >
        <AlertDialogPopup className="max-w-xl">
          <AlertDialogHeader>
            <AlertDialogTitle>
              {preview?.restore
                ? "Replace collaboration data with this backup?"
                : "Recover the preserved originals?"}
            </AlertDialogTitle>
            <AlertDialogDescription>
              {preview?.restore
                ? "This replaces the active collaboration database. Current data, including newer drafts, remains in a recovery archive on this device. Drafts are not merged automatically."
                : "This restores the original files preserved before the interrupted recovery. The present files remain archived too."}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <div className="min-h-0 space-y-3 overflow-y-auto px-6 pb-6 text-sm">
            {preview?.restore ? (
              <>
                <dl className="grid grid-cols-2 gap-x-4 gap-y-1">
                  <dt>Backup drafts</dt>
                  <dd>{preview.restore.incoming.drafts}</dd>
                  <dt>Current drafts</dt>
                  <dd>{preview.restore.current_drafts ?? "Unavailable"}</dd>
                  <dt>Backup accounts</dt>
                  <dd>{preview.restore.incoming.accounts}</dd>
                  <dt>Commands held for review</dt>
                  <dd>{preview.restore.quarantined_commands}</dd>
                </dl>
                <p>
                  Restored accounts require reconnection. Cached provider
                  content will be rebuilt. Restored commands cannot send
                  automatically.
                </p>
                <details className="text-xs text-muted-foreground">
                  <summary className="cursor-pointer">
                    Backup verification
                  </summary>
                  <p>
                    Schema {preview.restore.incoming.schema_version} · revision{" "}
                    {preview.restore.incoming.revision}
                  </p>
                  <p className="break-all">
                    SHA-256: {preview.restore.incoming.sha256}
                  </p>
                </details>
              </>
            ) : (
              <p>
                {preview?.interrupted?.original_files ?? 0} verified original
                files are available.
              </p>
            )}
            <p className="text-xs text-muted-foreground">
              This preview expires after ten minutes. Cancel to resume without
              replacing data.
            </p>
            {error ? (
              <p role="alert" className="text-destructive-foreground">
                {error}
              </p>
            ) : null}
          </div>
          <AlertDialogFooter>
            <Button
              type="button"
              variant="ghost"
              disabled={busy}
              onClick={() => void run(cancelPreview)}
            >
              Cancel recovery
            </Button>
            <Button
              type="button"
              variant="destructive"
              disabled={busy || !preview}
              onClick={() => void run(confirm)}
            >
              {busy
                ? "Recovering…"
                : preview?.restore
                  ? "Replace collaboration data"
                  : "Keep original data"}
            </Button>
          </AlertDialogFooter>
        </AlertDialogPopup>
      </AlertDialog>
    </>
  );
}

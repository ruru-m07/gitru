import {
  Dialog,
  DialogDescription,
  DialogHeader,
  DialogPopup,
  DialogTitle,
} from "@gitru/ui/components/dialog";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { useEffect, useRef, useState } from "react";
import { setTabWebviewsSuspended } from "@/components/webview-tab-host";
import { ACCOUNT_SETTINGS_OPEN_EVENT } from "./account-dialog-events";
import { AccountManager, isTrustedAccountWindow } from "./account-manager";
import { LocalTransportSettings } from "./local-transport-settings";

// Effect owners prevent an old listener/unmount cleanup from releasing a newer
// acquisition during StrictMode, HMR or a close/reopen race.
const suspensionOwners = new Set<object>();
let suspensionWork: Promise<void> = Promise.resolve();

function setSuspensionLease(owner: object, held: boolean): Promise<void> {
  if (held) suspensionOwners.add(owner);
  else if (!suspensionOwners.delete(owner)) return Promise.resolve();
  const work = suspensionWork
    .catch(() => {})
    .then(() => setTabWebviewsSuspended(suspensionOwners.size > 0));
  suspensionWork = work;
  return work;
}

type DialogController = {
  open(): void;
  close(): void;
  closed(): void;
};

/** One host per main webview; child tab surfaces never mount credential controls. */
export function AccountDialogHost() {
  return isTrustedAccountWindow() ? <MainAccountDialogHost /> : null;
}

function MainAccountDialogHost() {
  const [open, setOpen] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const controller = useRef<DialogController | null>(null);

  useEffect(() => {
    const owner = {};
    const view = getCurrentWebview();
    let disposed = false;
    let desiredOpen = false;
    let intent = 0;
    let unlisten: (() => void) | undefined;

    async function release() {
      try {
        await setSuspensionLease(owner, false);
      } catch {
        if (!disposed)
          setError("Could not restore the selected tab. Try again.");
      }
    }

    async function show(version: number) {
      try {
        // Native child views cover the main DOM regardless of its CSS z-index.
        // Do not mount a credential form until every child has been hidden.
        await setSuspensionLease(owner, true);
        if (disposed || version !== intent || !desiredOpen) {
          if (!desiredOpen) await release();
          return;
        }
        await view.setFocus();
        if (disposed || version !== intent || !desiredOpen) {
          if (!desiredOpen) await release();
          return;
        }
        setOpen(true);
      } catch {
        if (disposed || version !== intent) return;
        desiredOpen = false;
        intent += 1;
        setOpen(false);
        setError("Could not open account settings. Try again.");
        await release();
      }
    }

    const current: DialogController = {
      open() {
        if (disposed || desiredOpen) return;
        desiredOpen = true;
        intent += 1;
        setError(null);
        void show(intent);
      },
      close() {
        if (disposed) return;
        desiredOpen = false;
        intent += 1;
        setOpen(false);
      },
      closed() {
        // Ignore completion of an older closing transition after reopening.
        if (!disposed && !desiredOpen) void release();
      },
    };
    controller.current = current;

    void view
      .listen(ACCOUNT_SETTINGS_OPEN_EVENT, () => {
        // Event payloads are deliberately ignored; this grants no credentials,
        // connection choice, or permission to perform a management operation.
        current.open();
      })
      .then((remove) => {
        if (disposed) remove();
        else unlisten = remove;
      })
      .catch(() => {
        if (!disposed)
          setError("Account settings could not start. Restart Gitru to retry.");
      });

    return () => {
      disposed = true;
      desiredOpen = false;
      intent += 1;
      unlisten?.();
      if (controller.current === current) controller.current = null;
      void release();
    };
  }, []);

  return (
    <>
      {error ? (
        <p role="alert" className="text-xs text-destructive-foreground">
          {error}
        </p>
      ) : null}
      <Dialog
        open={open}
        onOpenChange={(next: boolean) => {
          if (next) controller.current?.open();
          else controller.current?.close();
        }}
        onOpenChangeComplete={(next: boolean) => {
          if (!next) controller.current?.closed();
        }}
      >
        <DialogPopup
          className="sm:max-w-xl grid-rows-[auto_minmax(0,1fr)] max-h-[calc(100dvh-3rem)] overflow-hidden sm:max-h-[calc(80dvh-2rem)] motion-reduce:transition-none"
          finalFocus={false}
        >
          <DialogHeader>
            <DialogTitle>Connected accounts</DialogTitle>
            <DialogDescription>
              Connect your provider accounts to bring pull requests, issues, and
              notifications into Gitru.
            </DialogDescription>
          </DialogHeader>
          {open ? (
            <div
              className="min-h-0 overflow-y-auto overscroll-contain"
              role="region"
              aria-label="Account and repository settings"
            >
              <AccountManager />
              <LocalTransportSettings />
            </div>
          ) : null}
        </DialogPopup>
      </Dialog>
    </>
  );
}

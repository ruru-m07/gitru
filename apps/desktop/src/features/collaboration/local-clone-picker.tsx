import {
  collaboration,
  collaborationErrorMessage,
  type LocalCloneRecord,
  type RemoteAccount,
} from "@gitru/collaboration-client";
import { useLocalClones } from "@gitru/collaboration-client/react";
import { listRepositories } from "@gitru/commands";
import { Button } from "@gitru/ui/components/button";
import {
  Dialog,
  DialogDescription,
  DialogHeader,
  DialogPopup,
  DialogTitle,
  DialogTrigger,
} from "@gitru/ui/components/dialog";
import { useRouter } from "@tanstack/react-router";
import { useId, useState } from "react";
import { useAppStore } from "@/store/use-app-store";
import {
  localLinkStateLabel,
  useLocalLinkIntent,
} from "./local-repository-links";

export function OpenLocalCloneButton({
  account,
  instanceId,
  repositoryId,
}: {
  account: RemoteAccount;
  instanceId: string;
  repositoryId: string;
}) {
  const [open, setOpen] = useState(false);
  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger render={<Button size="sm" variant="outline" />}>
        Open local clone
      </DialogTrigger>
      <DialogPopup>
        <DialogHeader>
          <DialogTitle>Choose a local clone</DialogTitle>
          <DialogDescription>
            Open a registered clone through its saved link. No clone, fetch or
            checkout is performed.
          </DialogDescription>
        </DialogHeader>
        <div>
          {open ? (
            <LocalClonePicker
              key={`${account.id}:${account.actor_id}:${account.authorization_epoch}:${instanceId}:${repositoryId}`}
              account={account}
              instanceId={instanceId}
              repositoryId={repositoryId}
            />
          ) : null}
        </div>
      </DialogPopup>
    </Dialog>
  );
}
export function LocalClonePicker({
  account,
  instanceId,
  repositoryId,
  onNavigate,
}: {
  account: RemoteAccount;
  instanceId: string;
  repositoryId: string;
  onNavigate?: (localRepositoryId: string) => Promise<void>;
}) {
  const query = useLocalClones(account, instanceId, repositoryId);
  const registrations = useAppStore((state) => state.repositories);
  const registeredById = new Map(
    registrations.map((value) => [value.id, value]),
  );
  const pickerId = useId();
  const router = useRouter({ warn: false });
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const begin = useLocalLinkIntent();
  async function choose(clone: LocalCloneRecord) {
    const current = begin();
    setBusy(true);
    setError(null);
    try {
      const receipt = await collaboration.validateLocalNavigation({
        local_repository_id: clone.local_repository_id,
        link_id: clone.link_id,
        generation: clone.generation,
        direction: "git",
      });
      if (!current()) return;
      if (
        receipt.account_id !== account.id ||
        receipt.authorization_epoch !== account.authorization_epoch ||
        receipt.instance_id !== instanceId ||
        receipt.repository_id !== repositoryId ||
        receipt.local_repository_id !== clone.local_repository_id
      )
        throw { code: "stale_view" };
      if (onNavigate) await onNavigate(receipt.local_repository_id);
      else {
        const registered = await listRepositories({ refreshStale: false });
        if (!current()) return;
        const repository = registered.find(
          (value) => value.id === receipt.local_repository_id,
        );
        if (!repository || !router) throw { code: "not_ready" };
        const store = useAppStore.getState();
        store.setRepositories(registered);
        store.syncActiveTab({
          repositoryId: repository.id,
          routePath: "/app/git",
          title: repository.name,
        });
        await router.navigate({ to: "/app/git" });
      }
    } catch (failure) {
      if (current()) setError(collaborationErrorMessage(failure));
    } finally {
      if (current()) setBusy(false);
    }
  }
  return (
    <section className="space-y-3" aria-label="Registered local clones">
      {query.isPending ? (
        <p role="status">Inspecting linked local clones…</p>
      ) : null}
      {query.isError ? (
        <p role="alert">{collaborationErrorMessage(query.error)}</p>
      ) : null}
      {error ? (
        <p role="alert">{error} Inspect again before opening a changed link.</p>
      ) : null}
      {!query.isError
        ? query.data?.clones.map((clone, index) => {
            const registration =
              clone.state === "local_repository_missing"
                ? undefined
                : registeredById.get(clone.local_repository_id);
            const descriptionId = `${pickerId}-${index}`;
            return (
              <article
                key={clone.link_id}
                className="space-y-2 rounded-md border p-3"
                aria-label={`Local clone ${clone.local_repository_id}`}
              >
                <p className="break-all text-sm">
                  {clone.local_repository_name ?? "Missing local registration"}
                </p>
                <p
                  id={descriptionId}
                  className="break-all text-xs text-muted-foreground"
                >
                  {registration
                    ? `Local path: ${registration.path}`
                    : `Registration: ${clone.local_repository_id}`}
                </p>
                <p className="text-xs text-muted-foreground">
                  {localLinkStateLabel[clone.state]}
                </p>
                <Button
                  size="sm"
                  aria-describedby={descriptionId}
                  disabled={busy || clone.state !== "linked"}
                  onClick={() => {
                    void choose(clone);
                  }}
                >
                  Open {clone.local_repository_name ?? "local clone"}
                </Button>
              </article>
            );
          })
        : null}
      {query.data && !query.data.clones.length ? (
        <p className="text-sm text-muted-foreground">
          No saved local clone is linked. Open the local Git repository and
          choose Linked collaboration.
        </p>
      ) : null}
      <Button
        size="sm"
        variant="outline"
        disabled={busy || query.isFetching}
        onClick={() => {
          void query.refetch();
        }}
      >
        Inspect clones again
      </Button>
    </section>
  );
}

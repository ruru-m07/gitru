import {
  collaboration,
  collaborationErrorMessage,
  type LocalLinkCandidate,
  type LocalLinkState,
  type LocalNavigationReceipt,
  type LocalRepositoryLink,
} from "@gitru/collaboration-client";
import {
  useCollaborationAccounts,
  useLocalRepositoryLinks,
} from "@gitru/collaboration-client/react";
import { Badge } from "@gitru/ui/components/badge";
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
import { Link2 } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { useAppStore } from "@/store/use-app-store";
import { requestAccountSettings } from "./account-dialog-events";
import {
  type LocalLinkRouteTarget,
  localLinkRoutePath,
} from "./local-link-navigation";

export const localLinkStateLabel: Record<LocalLinkState, string> = {
  linked: "Linked",
  unresolved: "Not found in saved repositories",
  ambiguous: "Ambiguous repository",
  unconfigured_instance: "Transport mapping needed",
  unsupported_transport: "Unsupported transport",
  remote_changed: "Git remote changed",
  local_repository_missing: "Local repository missing",
  unavailable: "Saved repository unavailable",
};
export function useLocalLinkIntent() {
  const token = useRef(0);
  useEffect(
    () => () => {
      token.current += 1;
    },
    [],
  );
  return () => {
    const current = ++token.current;
    return () => current === token.current;
  };
}

export function LocalRepositoryLinksButton() {
  const repository = useAppStore((state) => {
    const id =
      state.sessionsById[state.activeSessionId ?? state.activeTabId ?? ""]
        ?.repositoryId;
    return state.repositories.find((repo) => repo.id === id);
  });
  const router = useRouter({ warn: false });
  const [open, setOpen] = useState(false);
  if (!repository) return null;
  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger
        render={<Button variant="ghost" size="sm" className="shrink-0" />}
      >
        <Link2 aria-hidden="true" />
        Linked collaboration
      </DialogTrigger>
      <DialogPopup className="sm:max-w-2xl">
        <DialogHeader>
          <DialogTitle>Linked collaboration</DialogTitle>
          <DialogDescription>
            Choose a saved provider repository for {repository.name}. Git
            remotes are unchanged.
          </DialogDescription>
        </DialogHeader>
        <div className="max-h-[70vh] overflow-y-auto">
          {open ? (
            <LocalRepositoryLinksPanel
              key={repository.id}
              localRepositoryId={repository.id}
              onNavigate={async (receipt, link, kind) => {
                if (!router) throw { code: "not_ready" };
                const target: LocalLinkRouteTarget = {
                  ...receipt,
                  link_id: link.id,
                  generation: link.generation,
                };
                const routePath = localLinkRoutePath(kind, target);
                useAppStore.getState().syncActiveTab({
                  routePath,
                  repositoryId: receipt.local_repository_id,
                });
                setOpen(false);
                await router.navigate({ to: routePath });
              }}
            />
          ) : null}
        </div>
      </DialogPopup>
    </Dialog>
  );
}

export function LocalRepositoryLinksPanel({
  localRepositoryId,
  onNavigate,
}: {
  localRepositoryId: string;
  onNavigate(
    receipt: LocalNavigationReceipt,
    link: LocalRepositoryLink,
    kind: "pull_request" | "issue",
  ): void | Promise<void>;
}) {
  const query = useLocalRepositoryLinks(localRepositoryId);
  const accounts = useCollaborationAccounts();
  const [replaceId, setReplaceId] = useState<string | null>(null);
  const [usedPreview, setUsedPreview] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const begin = useLocalLinkIntent();
  const inspection = !query.isError ? query.data : undefined;
  async function act(action: () => Promise<unknown>) {
    const current = begin();
    setBusy(true);
    setError(null);
    try {
      await action();
    } catch (failure) {
      if (current()) setError(collaborationErrorMessage(failure));
    } finally {
      if (current()) setBusy(false);
    }
  }
  async function choose(candidate: LocalLinkCandidate) {
    const previewId = inspection?.preview_id;
    if (!previewId || previewId === usedPreview) return;
    setUsedPreview(previewId);
    await act(async () => {
      await collaboration.confirmLocalLink({
        preview_id: previewId,
        candidate_id: candidate.id,
        replace_link_id: replaceId,
      });
      setReplaceId(null);
    });
  }
  async function navigate(
    link: LocalRepositoryLink,
    kind: "pull_request" | "issue",
  ) {
    const current = begin();
    setBusy(true);
    setError(null);
    try {
      const receipt = await collaboration.validateLocalNavigation({
        local_repository_id: localRepositoryId,
        link_id: link.id,
        generation: link.generation,
        direction: "collaboration",
      });
      if (current()) await onNavigate(receipt, link, kind);
    } catch (failure) {
      if (current()) setError(collaborationErrorMessage(failure));
    } finally {
      if (current()) setBusy(false);
    }
  }
  return (
    <section className="space-y-4" aria-label="Local repository links">
      <div className="flex flex-wrap items-center gap-2">
        <Button
          size="sm"
          variant="outline"
          disabled={busy || query.isFetching}
          onClick={() => {
            setError(null);
            void query.refetch();
          }}
        >
          Inspect remotes again
        </Button>
        <Button
          size="sm"
          variant="ghost"
          onClick={() => {
            void requestAccountSettings().catch(() =>
              setError("Could not open transport settings."),
            );
          }}
        >
          Configure transport mapping…
        </Button>
      </div>
      <p className="text-xs text-muted-foreground">
        Matches use saved repository identities. Choose each account and
        endpoint explicitly.
      </p>
      {query.isPending ? (
        <p role="status">Inspecting local Git remotes…</p>
      ) : null}
      {query.isError ? (
        <p role="alert">{collaborationErrorMessage(query.error)}</p>
      ) : null}
      {error ? (
        <p role="alert">
          {error} Inspect remotes again before retrying a changed or expired
          choice.
        </p>
      ) : null}
      {inspection?.observation_error ? (
        <p role="status">
          Local remote inspection:{" "}
          {inspection.observation_error.replace(/_/g, " ")}. Existing links can
          still be removed.
        </p>
      ) : null}
      {inspection?.snapshot.links.map((link) => (
        <article
          key={link.id}
          className="space-y-2 rounded-lg border p-3"
          aria-label={`Saved link ${link.endpoint.remote_name} ${link.endpoint.direction}`}
        >
          <div className="flex flex-wrap gap-2">
            <strong className="break-all text-sm">
              {link.repository?.full_name ?? "Saved local association"}
            </strong>
            <Badge variant="outline">{localLinkStateLabel[link.state]}</Badge>
          </div>
          <p className="break-all text-xs text-muted-foreground">
            {link.endpoint.remote_name} · {link.endpoint.direction} ·{" "}
            {link.endpoint.host}/{link.endpoint.path}
          </p>
          <div className="flex flex-wrap gap-2">
            <Button
              size="sm"
              variant="outline"
              disabled={busy}
              onClick={() => setReplaceId(link.id)}
            >
              Change link
            </Button>
            <Button
              size="sm"
              variant="ghost"
              disabled={busy}
              onClick={() => {
                void act(() =>
                  collaboration.removeLocalLink({
                    id: link.id,
                    generation: link.generation,
                  }),
                );
              }}
            >
              Remove link
            </Button>
            {link.state === "linked" && link.repository ? (
              <>
                <Button
                  size="sm"
                  disabled={busy}
                  onClick={() => {
                    void navigate(link, "pull_request");
                  }}
                >
                  Open pull requests
                </Button>
                <Button
                  size="sm"
                  variant="outline"
                  disabled={busy}
                  onClick={() => {
                    void navigate(link, "issue");
                  }}
                >
                  Open issues
                </Button>
              </>
            ) : null}
          </div>
        </article>
      ))}
      {replaceId ? (
        <p className="text-sm">
          Choose a replacement below.{" "}
          <Button size="sm" variant="ghost" onClick={() => setReplaceId(null)}>
            Cancel change
          </Button>
        </p>
      ) : null}
      {inspection?.snapshot.resolutions.map((resolution, index) => (
        <article
          key={`${resolution.endpoint.remote_name}:${resolution.endpoint.direction}:${resolution.endpoint.ordinal}:${index}`}
          className="space-y-2 border-t pt-3"
        >
          <p className="break-all text-sm">
            {resolution.endpoint.remote_name} · {resolution.endpoint.direction}{" "}
            {resolution.endpoint.ordinal + 1} · {resolution.endpoint.host}/
            {resolution.endpoint.path}
          </p>
          <p className="text-xs text-muted-foreground">
            {localLinkStateLabel[resolution.state]}
          </p>
          {resolution.state === "ambiguous" && resolution.candidates.length ? (
            <p className="text-xs text-muted-foreground">
              Some saved identities conflict. Only unambiguous account choices
              are listed below.
            </p>
          ) : null}
          <div className="flex flex-col items-start gap-2">
            {resolution.candidates.map((candidate) => (
              <Button
                key={candidate.id}
                size="sm"
                variant="outline"
                className="h-auto whitespace-normal text-left break-all"
                disabled={
                  busy ||
                  !inspection.preview_id ||
                  inspection.preview_id === usedPreview
                }
                onClick={() => {
                  void choose(candidate);
                }}
              >
                {replaceId ? "Replace with" : "Link"} @
                {accounts.data?.accounts.find(
                  (account) => account.id === candidate.account_id,
                )?.login ?? candidate.account_id}{" "}
                · {candidate.repository.full_name} ·{" "}
                {candidate.endpoint.direction}
              </Button>
            ))}
          </div>
        </article>
      ))}
      {inspection &&
      !inspection.snapshot.links.length &&
      !inspection.snapshot.resolutions.length ? (
        <p className="text-sm text-muted-foreground">
          No supported Git remote endpoints are available. Configure a Git
          remote in your existing Git workflow, then inspect again.
        </p>
      ) : null}
      {inspection?.remotes?.remotes.map((remote) => (
        <details key={remote.name} className="text-xs text-muted-foreground">
          <summary>Observed {remote.name} URLs</summary>
          {(["fetch_urls", "push_urls"] as const).flatMap((direction) =>
            remote[direction].map((url, index) => (
              <p key={`${direction}:${index}`} className="break-all">
                {direction === "fetch_urls" ? "Fetch" : "Push"}{" "}
                {url.ordinal + 1}: {url.sanitized_url ?? "Unsupported URL"}
                {url.redacted ? " (credentials removed)" : ""}
              </p>
            )),
          )}
        </details>
      ))}
    </section>
  );
}

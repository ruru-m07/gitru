import {
  type CommandFieldResolution,
  type CommandFieldValue,
  type CommandRecoveryActionRequest,
  type CommandRecoveryDetail,
  type CommandRecoveryReplaceRequest,
  collaboration,
  collaborationErrorMessage,
  type RemoteAccount,
} from "@gitru/collaboration-client";
import {
  commandRecoveryDetailQueryOptions,
  commandRecoveryQueryOptions,
  useCollaborationVersion,
} from "@gitru/collaboration-client/react";
import { Button } from "@gitru/ui/components/button";
import {
  Dialog,
  DialogDescription,
  DialogHeader,
  DialogPanel,
  DialogPopup,
  DialogTitle,
  DialogTrigger,
} from "@gitru/ui/components/dialog";
import {
  Select,
  SelectItem,
  SelectPopup,
  SelectTrigger,
  SelectValue,
} from "@gitru/ui/components/select";
import { Textarea } from "@gitru/ui/components/textarea";
import { useQuery } from "@tanstack/react-query";
import { useEffect, useState } from "react";

type Resolution = {
  review: CommandRecoveryDetail;
  fields: CommandFieldResolution[];
};
type Resolutions = Record<string, Resolution>;
type LocalAction =
  | { kind: "action"; request: CommandRecoveryActionRequest }
  | { kind: "replace"; request: CommandRecoveryReplaceRequest };

const comparisons = {
  unchanged: "Unchanged",
  independent: "Can apply independently",
  converged: "Provider already has this value",
  conflict: "Overlapping change",
  unknown: "Comparison unavailable",
  guard_changed: "Required context changed",
};
const fieldLabels = {
  title: "Title",
  body: "Description",
  state: "State",
  unread: "Unread",
  head: "Inspected commit",
};

function valueText(value: CommandFieldValue) {
  return value.known ? (value.value ?? "Empty value") : "Not available";
}
function changeLabel(kind: string) {
  return kind === "pull_request"
    ? "Pull request change"
    : kind === "issue"
      ? "Issue change"
      : kind === "notification"
        ? "Inbox change"
        : "Saved change";
}
function initialResolution(review: CommandRecoveryDetail): Resolution {
  return {
    review,
    fields: review.fields
      .filter((field) => field.editable)
      .map((field) => ({
        field: field.field,
        choice: "keep_desired",
        value: null,
      })),
  };
}

/** Draft resolutions live above the dialog portal, so closing/reopening or
 * switching an account cannot replace edited text with a background snapshot. */
export function CommandRecoveryButton({
  accounts,
}: {
  accounts: RemoteAccount[];
}) {
  const [open, setOpen] = useState(false);
  const [accountId, setAccountId] = useState<string | null>(null);
  const [resolutions, setResolutions] = useState<Resolutions>({});
  const [busy, setBusy] = useState(false);
  const account = accounts.find((item) => item.id === accountId) ?? accounts[0];
  const items = accounts.map((item) => ({
    value: item.id,
    label: `@${item.login} · ${item.host}`,
  }));
  return (
    <Dialog
      open={open}
      onOpenChange={(next) => {
        if (!busy) setOpen(next);
      }}
    >
      <DialogTrigger
        render={<Button type="button" variant="ghost" size="sm" />}
      >
        Saved changes
      </DialogTrigger>
      <DialogPopup className="sm:max-w-4xl" showCloseButton={!busy}>
        <DialogHeader>
          <DialogTitle>Saved changes</DialogTitle>
          <DialogDescription>
            Review queued changes and their delivery status. Your original text
            and action history stay saved locally.
          </DialogDescription>
        </DialogHeader>
        <DialogPanel className="space-y-4">
          {account ? (
            <>
              <Select
                items={items}
                value={account.id}
                onValueChange={setAccountId}
                disabled={busy}
              >
                <SelectTrigger aria-label="Saved changes account">
                  <SelectValue />
                </SelectTrigger>
                <SelectPopup>
                  {items.map((item) => (
                    <SelectItem key={item.value} value={item.value}>
                      {item.label}
                    </SelectItem>
                  ))}
                </SelectPopup>
              </Select>
              <AccountChanges
                key={`${account.id}:${account.authorization_epoch}`}
                account={account}
                enabled={open}
                busy={busy}
                setBusy={setBusy}
                resolutions={resolutions}
                setResolutions={setResolutions}
              />
            </>
          ) : (
            <p className="text-sm text-muted-foreground">No saved accounts.</p>
          )}
        </DialogPanel>
      </DialogPopup>
    </Dialog>
  );
}

type ResolutionProps = {
  account: RemoteAccount;
  busy: boolean;
  setBusy: (value: boolean) => void;
  resolutions: Resolutions;
  setResolutions: React.Dispatch<React.SetStateAction<Resolutions>>;
};

function AccountChanges(props: ResolutionProps & { enabled: boolean }) {
  const { account, enabled, busy } = props;
  const [commandId, setCommandId] = useState<string | null>(null);
  const [cursors, setCursors] = useState<Array<string | null>>([null]);
  const [includeTerminal, setIncludeTerminal] = useState(false);
  const version = useCollaborationVersion();
  const [observedVersion, setObservedVersion] = useState(version);
  if (observedVersion !== version) {
    setObservedVersion(version);
    if (cursors.length > 1) setCursors([null]);
  }
  useEffect(
    () =>
      collaboration.subscribeChanges((change) => {
        if (change.account_id === account.id && change.scope === "commands")
          setCursors((current) => (current.length > 1 ? [null] : current));
      }),
    [account.id],
  );
  const query = useQuery({
    ...commandRecoveryQueryOptions(account, {
      target_id: null,
      include_terminal: includeTerminal,
      cursor: cursors.at(-1) ?? null,
      limit: 50,
    }),
    enabled,
  });
  return (
    <div className="space-y-4">
      <Button
        type="button"
        size="sm"
        variant="outline"
        aria-pressed={includeTerminal}
        disabled={busy}
        onClick={() => {
          setIncludeTerminal((value) => !value);
          setCursors([null]);
        }}
      >
        {includeTerminal ? "Hide finished changes" : "Include finished changes"}
      </Button>
      <div className="grid gap-4 md:grid-cols-[minmax(0,1fr)_minmax(0,2fr)]">
        <div className="min-w-0 space-y-2">
          {query.isPending ? (
            <p role="status">Loading saved changes…</p>
          ) : query.isError ? (
            <div>
              <p role="alert">{collaborationErrorMessage(query.error)}</p>
              <Button
                type="button"
                onClick={() => {
                  setCursors([null]);
                  void query.refetch();
                }}
              >
                Reload saved changes
              </Button>
            </div>
          ) : !query.data.commands.length ? (
            <p className="text-sm text-muted-foreground">
              No saved changes in this view.
            </p>
          ) : (
            <ul
              className="max-h-80 space-y-2 overflow-y-auto"
              aria-label="Saved command list"
            >
              {query.data.commands.map((command) => (
                <li key={command.command_id}>
                  <Button
                    type="button"
                    variant={
                      commandId === command.command_id ? "secondary" : "outline"
                    }
                    disabled={busy}
                    className="h-auto w-full justify-start whitespace-normal text-left"
                    onClick={() => setCommandId(command.command_id)}
                    aria-pressed={commandId === command.command_id}
                  >
                    <span className="min-w-0 break-all">
                      <span className="block">
                        {changeLabel(command.target_kind)}
                      </span>
                      <span className="block text-xs text-muted-foreground">
                        {command.state.replace(/_/g, " ")}
                        {command.paused ? " · paused" : ""}
                        {command.quarantined ? " · restored" : ""}
                      </span>
                    </span>
                  </Button>
                </li>
              ))}
            </ul>
          )}
          <div className="flex gap-2">
            <Button
              type="button"
              size="sm"
              variant="ghost"
              disabled={busy || cursors.length < 2}
              onClick={() => setCursors((current) => current.slice(0, -1))}
            >
              Previous
            </Button>
            <Button
              type="button"
              size="sm"
              variant="ghost"
              disabled={busy || !query.data?.next_cursor}
              onClick={() => {
                if (query.data?.next_cursor)
                  setCursors((current) => [...current, query.data.next_cursor]);
              }}
            >
              Next
            </Button>
          </div>
        </div>
        {commandId ? (
          <CommandReview
            key={commandId}
            {...props}
            commandId={commandId}
            reloadList={() => {
              void query.refetch();
            }}
          />
        ) : (
          <p className="text-sm text-muted-foreground">
            Choose a saved change to inspect its original and current values.
          </p>
        )}
      </div>
    </div>
  );
}

function CommandReview({
  account,
  commandId,
  enabled,
  busy,
  setBusy,
  resolutions,
  setResolutions,
  reloadList,
}: ResolutionProps & {
  commandId: string;
  enabled: boolean;
  reloadList: () => void;
}) {
  const query = useQuery({
    ...commandRecoveryDetailQueryOptions(account, commandId),
    enabled,
  });
  const [error, setError] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [retry, setRetry] = useState<LocalAction | null>(null);
  const resolutionKey = JSON.stringify([account.id, commandId]);
  const detail = query.data;
  const saved = resolutions[resolutionKey];
  const resolution = saved ?? (detail ? initialResolution(detail) : undefined);
  const outdated =
    !!detail &&
    !!resolution &&
    resolution.review.context.review_token !== detail.context.review_token;
  const client = collaboration.forAccount(account);
  function retain(next: Resolution) {
    setResolutions((current) => ({ ...current, [resolutionKey]: next }));
  }
  function forget() {
    setResolutions((current) => {
      const next = { ...current };
      delete next[resolutionKey];
      return next;
    });
  }
  async function submit(action: LocalAction) {
    if (busy) return;
    setBusy(true);
    setError(null);
    setMessage(null);
    try {
      const result =
        action.kind === "replace"
          ? await client.commandRecoveryReplace(action.request)
          : await client.commandRecoveryAction(action.request);
      setRetry(null);
      if (action.kind === "replace") forget();
      setMessage(
        action.kind === "replace"
          ? "Resolution saved as a new change. The original history is retained."
          : result.paused
            ? "Future delivery is paused. A prior provider action may already have happened."
            : result.state === "cancelled"
              ? "Cancelled before sending. The original text remains saved."
              : "Delivery resumed within its existing limits.",
      );
      reloadList();
      await query.refetch();
      void collaboration.wake();
    } catch (failure) {
      setRetry(action);
      setError(collaborationErrorMessage(failure));
      void query.refetch();
    } finally {
      setBusy(false);
    }
  }
  async function exportOriginal() {
    if (!detail || busy) return;
    setBusy(true);
    setError(null);
    try {
      if (await client.commandRecoveryExport(detail.context))
        setMessage("Saved change exported.");
    } catch (failure) {
      setError(collaborationErrorMessage(failure));
    } finally {
      setBusy(false);
    }
  }
  async function copyResolution() {
    if (!resolution) return;
    try {
      await navigator.clipboard.writeText(
        JSON.stringify(resolution.fields, null, 2),
      );
      setMessage("Resolution copied.");
    } catch {
      setError("Could not copy the resolution. Your text is still here.");
    }
  }
  if (!detail || !resolution)
    return (
      <div>
        {query.isError ? (
          <>
            <p role="alert">{collaborationErrorMessage(query.error)}</p>
            <Button
              type="button"
              onClick={() => {
                void query.refetch();
              }}
            >
              Reload change
            </Button>
          </>
        ) : (
          <p role="status">Loading saved change…</p>
        )}
      </div>
    );
  const review = resolution.review;
  return (
    <section className="min-w-0 space-y-4" aria-label="Saved change review">
      {query.isError ? (
        <div className="space-y-2">
          <p role="alert">{collaborationErrorMessage(query.error)}</p>
          <Button
            type="button"
            variant="outline"
            disabled={busy}
            onClick={() => {
              void query.refetch();
            }}
          >
            Reload change
          </Button>
        </div>
      ) : null}
      <div className="space-y-1 text-sm">
        <p className="break-all font-medium">
          {changeLabel(detail.command.target_kind)}
        </p>
        <p>
          Status: {detail.command.state.replace(/_/g, " ")}
          {detail.command.paused ? " · paused" : ""}
        </p>
        {detail.command.attempt_count > 0 ? (
          <p>
            A provider action may already have happened. Pausing future delivery
            does not undo it.
          </p>
        ) : null}
        {detail.command.quarantined ? (
          <p>
            Restored history is preserved. It cannot resume delivery
            automatically.
          </p>
        ) : null}
        {detail.reason ? <p>{detail.reason}</p> : null}
        {detail.command.blocked_reason ? (
          <p>{detail.command.blocked_reason}</p>
        ) : null}
      </div>
      {outdated ? (
        <div className="space-y-2 rounded border p-3 text-sm">
          <p>
            Provider data or delivery status changed. Your edited resolution is
            preserved; review the latest values before saving.
          </p>
          <Button
            type="button"
            disabled={busy || query.isError}
            variant="outline"
            onClick={() =>
              retain({
                review: detail,
                fields: initialResolution(detail).fields.map(
                  (field) =>
                    resolution.fields.find(
                      (previous) => previous.field === field.field,
                    ) ?? field,
                ),
              })
            }
          >
            Review latest values
          </Button>
        </div>
      ) : null}
      <div className="max-h-[45vh] space-y-4 overflow-y-auto">
        {review.fields.map((field) => {
          const choice = resolution.fields.find(
            (item) => item.field === field.field,
          );
          const change = (next: CommandFieldResolution) =>
            retain({
              ...resolution,
              fields: resolution.fields.map((item) =>
                item.field === next.field ? next : item,
              ),
            });
          return (
            <fieldset
              key={field.field}
              className="space-y-2 rounded border p-3"
              disabled={busy}
            >
              <legend className="px-1 text-sm font-medium">
                {fieldLabels[field.field]}
              </legend>
              <p className="text-xs text-muted-foreground">
                {comparisons[field.comparison]}
              </p>
              <dl className="grid gap-2 text-xs sm:grid-cols-3">
                {(
                  [
                    ["Original", field.base],
                    ["Provider", field.remote],
                    ["Saved intent", field.desired],
                  ] as const
                ).map(([label, value]) => (
                  <div key={label} className="min-w-0">
                    <dt className="font-medium">{label}</dt>
                    <dd className="max-h-32 overflow-auto whitespace-pre-wrap break-all">
                      {valueText(value)}
                    </dd>
                  </div>
                ))}
              </dl>
              {field.editable && choice ? (
                <>
                  <div className="flex flex-wrap gap-2">
                    <Button
                      type="button"
                      size="sm"
                      variant="outline"
                      aria-pressed={choice.choice === "keep_desired"}
                      disabled={!field.desired.known}
                      onClick={() =>
                        change({
                          field: field.field,
                          choice: "keep_desired",
                          value: null,
                        })
                      }
                    >
                      Keep saved value
                    </Button>
                    <Button
                      type="button"
                      size="sm"
                      variant="outline"
                      aria-pressed={choice.choice === "use_remote"}
                      disabled={!field.remote.known}
                      onClick={() =>
                        change({
                          field: field.field,
                          choice: "use_remote",
                          value: null,
                        })
                      }
                    >
                      Use provider value
                    </Button>
                    {field.field === "title" || field.field === "body" ? (
                      <Button
                        type="button"
                        size="sm"
                        variant="outline"
                        aria-pressed={choice.choice === "edited"}
                        onClick={() =>
                          change({
                            field: field.field,
                            choice: "edited",
                            value: choice.value ?? field.desired.value ?? "",
                          })
                        }
                      >
                        Edit resolution
                      </Button>
                    ) : null}
                  </div>
                  {choice.choice === "edited" ? (
                    <Textarea
                      aria-label={`${fieldLabels[field.field]} resolution`}
                      value={choice.value ?? ""}
                      maxLength={field.field === "title" ? 4096 : 65536}
                      onChange={(event) =>
                        change({ ...choice, value: event.target.value })
                      }
                    />
                  ) : null}
                </>
              ) : null}
            </fieldset>
          );
        })}
      </div>
      {error ? (
        <p role="alert" className="text-sm text-destructive-foreground">
          {error}
        </p>
      ) : null}
      {message ? (
        <p role="status" className="text-sm">
          {message}
        </p>
      ) : null}
      {retry ? (
        <Button
          type="button"
          variant="outline"
          disabled={busy}
          onClick={() => {
            void submit(retry);
          }}
        >
          Retry previous local action
        </Button>
      ) : null}
      <div className="flex flex-wrap gap-2">
        {detail.can_replace ? (
          <Button
            type="button"
            disabled={busy || outdated || query.isError}
            onClick={() => {
              void submit({
                kind: "replace",
                request: {
                  context: review.context,
                  action_id: crypto.randomUUID(),
                  new_command_id: crypto.randomUUID(),
                  fields: resolution.fields,
                },
              });
            }}
          >
            Save new resolution
          </Button>
        ) : null}
        {detail.can_cancel ? (
          <Button
            type="button"
            variant="outline"
            disabled={busy || query.isError}
            onClick={() => {
              void submit({
                kind: "action",
                request: {
                  context: detail.context,
                  action_id: crypto.randomUUID(),
                  action: "cancel",
                },
              });
            }}
          >
            Cancel before sending
          </Button>
        ) : null}
        {detail.can_pause ? (
          <Button
            type="button"
            variant="outline"
            disabled={busy || query.isError}
            onClick={() => {
              void submit({
                kind: "action",
                request: {
                  context: detail.context,
                  action_id: crypto.randomUUID(),
                  action: "pause",
                },
              });
            }}
          >
            Pause future delivery
          </Button>
        ) : null}
        {detail.can_retry ? (
          <Button
            type="button"
            variant="outline"
            disabled={busy || query.isError}
            onClick={() => {
              void submit({
                kind: "action",
                request: {
                  context: detail.context,
                  action_id: crypto.randomUUID(),
                  action: "resume",
                },
              });
            }}
          >
            Resume delivery
          </Button>
        ) : null}
        <Button
          type="button"
          variant="outline"
          disabled={busy}
          onClick={() => {
            void exportOriginal();
          }}
        >
          Export original change
        </Button>
        {saved ? (
          <Button
            type="button"
            variant="ghost"
            disabled={busy}
            onClick={() => {
              void copyResolution();
            }}
          >
            Copy edited resolution
          </Button>
        ) : null}
      </div>
    </section>
  );
}

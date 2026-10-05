import {
  type ContextFacetCapability,
  collaboration,
  collaborationErrorMessage,
  type DetailEntry,
  type DetailField,
  type RemoteAccount,
  type TaskActor,
  type TaskV1,
} from "@gitru/collaboration-client";
import {
  detailQueryOptions,
  useVisibleDemand,
} from "@gitru/collaboration-client/react";
import { Badge } from "@gitru/ui/components/badge";
import { Button } from "@gitru/ui/components/button";
import {
  Collapsible,
  CollapsiblePanel,
  CollapsibleTrigger,
} from "@gitru/ui/components/collapsible";
import { useQuery } from "@tanstack/react-query";
import { ChevronDown } from "lucide-react";
import { useState } from "react";
import {
  CapabilityBoundary,
  ReadOnlyCapability,
  SynchronizationAvailability,
} from "./capability-boundary";
import {
  canMaintainDemand,
  canReadSaved,
  canSynchronize,
  dispatchCapabilityIntent,
} from "./capability-policy";

type Props = {
  account: RemoteAccount;
  subjectId: string;
  policy: ContextFacetCapability | undefined;
};
const PAGE_LIMIT = 50;
const CURSOR_LIMIT = 100;

/** Opening mounts only this facet; the independently keyed private editor stays mounted. */
export function NativeTasksPanel({ account, subjectId, policy }: Props) {
  const [open, setOpen] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  async function synchronize(recheck = false) {
    setBusy(true);
    setError(null);
    try {
      await dispatchCapabilityIntent(
        policy,
        recheck ? "recheck_access" : "synchronize",
        () =>
          collaboration
            .forAccount(account)
            .hydrateDetail({ subject_id: subjectId, facet: "tasks" }),
      );
    } catch (failure) {
      setError(collaborationErrorMessage(failure));
    } finally {
      setBusy(false);
    }
  }
  return (
    <section className="space-y-2 border-t pt-4" aria-label="Tasks">
      <Collapsible open={open} onOpenChange={setOpen}>
        <div className="flex flex-wrap items-center justify-between gap-2">
          <CollapsibleTrigger
            type="button"
            className="flex items-center gap-2 text-sm font-medium"
          >
            <ChevronDown
              aria-hidden="true"
              className={open ? "size-4 rotate-180" : "size-4"}
            />
            Tasks
          </CollapsibleTrigger>
          <ReadOnlyCapability policy={policy} />
        </div>
        <CollapsiblePanel className="motion-reduce:transition-none">
          {open ? (
            <div className="space-y-3 pt-3">
              <SynchronizationAvailability policy={policy} />
              <CapabilityBoundary
                policy={policy}
                busy={busy}
                recheck={() => {
                  void synchronize(true);
                }}
              >
                {canReadSaved(policy) ? (
                  <>
                    <Button
                      size="sm"
                      variant="ghost"
                      disabled={busy || !canSynchronize(policy)}
                      onClick={() => {
                        void synchronize();
                      }}
                    >
                      Sync tasks
                    </Button>
                    <ShownTasks
                      account={account}
                      subjectId={subjectId}
                      policy={policy}
                    />
                  </>
                ) : null}
              </CapabilityBoundary>
              {error ? (
                <p role="alert" className="text-xs text-destructive-foreground">
                  {error}
                </p>
              ) : null}
            </div>
          ) : null}
        </CollapsiblePanel>
      </Collapsible>
    </section>
  );
}

function ShownTasks({ account, subjectId, policy }: Props) {
  const [pages, setPages] = useState<{
    cursors: Array<string | null>;
    position: number;
  }>({ cursors: [null], position: 0 });
  const cursor = pages.cursors[pages.position];
  const query = useQuery(
    detailQueryOptions(account, {
      subject_id: subjectId,
      facet: "tasks",
      cursor,
      limit: PAGE_LIMIT,
    }),
  );
  const data =
    query.data?.evidence.availability !== "unavailable"
      ? query.data
      : undefined;
  const demandError = useVisibleDemand({
    account,
    target: {
      kind: "detail",
      repository_id: null,
      subject_id: subjectId,
      facet: "tasks",
    },
    enabled:
      account.state === "active" &&
      canMaintainDemand(policy) &&
      query.data?.evidence.availability !== "unavailable",
  });
  const next = data?.next_cursor ?? null;
  const capReached = pages.position + 1 >= CURSOR_LIMIT;
  const repeatedCursor =
    next !== null &&
    (next === cursor ||
      (pages.cursors.includes(next) &&
        pages.cursors[pages.position + 1] !== next));
  const canNext =
    !query.isPending &&
    !query.isError &&
    next !== null &&
    !capReached &&
    !repeatedCursor;
  function advance() {
    if (!canNext || next === null) return;
    setPages({
      cursors: [...pages.cursors.slice(0, pages.position + 1), next],
      position: pages.position + 1,
    });
  }
  return (
    <div className="space-y-3">
      <p className="text-xs text-muted-foreground">
        These are saved provider task observations. Task states and resolution
        dates do not establish review approval of the current commit.
      </p>
      {query.isPending ? (
        <p role="status" className="text-xs text-muted-foreground">
          Loading saved tasks…
        </p>
      ) : query.isError ? (
        <p role="alert" className="text-xs text-destructive-foreground">
          {collaborationErrorMessage(query.error)}
        </p>
      ) : query.data?.evidence.availability === "unavailable" ? (
        <p className="text-xs text-muted-foreground">
          Saved tasks are unavailable in the current authorized view.
        </p>
      ) : data ? (
        <>
          <div className="flex flex-wrap gap-2">
            {data.evidence.freshness === "stale" ? (
              <Badge variant="outline" size="sm">
                Saved data may be stale
              </Badge>
            ) : null}
            {data.evidence.coverage.state === "partial" ? (
              <Badge variant="outline" size="sm">
                Partial task set
              </Badge>
            ) : null}
          </div>
          {data.evidence.sync.error ? (
            <p role="alert" className="text-xs text-destructive-foreground">
              {collaborationErrorMessage(data.evidence.sync.error)}
            </p>
          ) : null}
          {data.evidence.sync.next_retry_at ? (
            <p role="status" className="text-xs text-muted-foreground">
              Sync can resume after{" "}
              <time dateTime={data.evidence.sync.next_retry_at}>
                {new Date(data.evidence.sync.next_retry_at).toLocaleString()}
              </time>
              . Saved observations remain available.
            </p>
          ) : null}
          {data.evidence.availability === "missing" ? (
            <p className="text-xs text-muted-foreground">
              Tasks have not been saved on this device yet.
            </p>
          ) : data.entries.length ? (
            <ul className="space-y-4" aria-label="Saved tasks">
              {data.entries.map((entry) => (
                <TaskRow key={entry.id} entry={entry} />
              ))}
            </ul>
          ) : (
            <p className="text-xs text-muted-foreground">
              {data.evidence.coverage.state === "complete"
                ? "No tasks were returned in the saved observation."
                : "No tasks are saved in this partial view."}
            </p>
          )}
          <nav
            aria-label="Saved task pages"
            className="flex flex-wrap items-center gap-2"
          >
            <Button
              size="sm"
              variant="outline"
              disabled={pages.position === 0}
              onClick={() =>
                setPages({ ...pages, position: pages.position - 1 })
              }
            >
              Previous saved tasks
            </Button>
            <p className="text-xs text-muted-foreground">
              Saved page {pages.position + 1}
            </p>
            <Button
              size="sm"
              variant="outline"
              disabled={!canNext}
              onClick={advance}
            >
              Next saved tasks
            </Button>
          </nav>
          {next && capReached ? (
            <p className="text-xs text-muted-foreground">
              This view can browse up to 100 saved pages. Close and reopen Tasks
              to browse from the beginning.
            </p>
          ) : null}
          {repeatedCursor ? (
            <p role="alert" className="text-xs text-destructive-foreground">
              The saved continuation did not advance. Reopen Tasks to read the
              current local view.
            </p>
          ) : null}
        </>
      ) : null}
      {demandError ? (
        <p role="alert" className="text-xs text-destructive-foreground">
          {collaborationErrorMessage(demandError)}
        </p>
      ) : null}
    </div>
  );
}

const fields = [
  ["task_content", "Content"],
  ["task_creator_login", "Creator nickname"],
  ["task_creator_display_name", "Creator display name"],
  ["task_state", "Provider state"],
  ["task_created_at", "Created time"],
  ["task_updated_at", "Updated time"],
  ["task_pending", "Pending flag"],
  ["task_resolved_at", "Resolved time"],
  ["task_resolver", "Resolver"],
  ["task_resolver_login", "Resolver nickname"],
  ["task_resolver_display_name", "Resolver display name"],
  ["task_comment_id", "Comment association"],
] as const satisfies ReadonlyArray<readonly [DetailField, string]>;

function actorIdentity(actor: TaskActor) {
  return `${actor.kind} · ${actor.provider_id}`;
}
function actorPresentation(
  actor: TaskActor,
  entry: DetailEntry,
  login: DetailField,
  displayName: DetailField,
) {
  const known = (field: DetailField) =>
    entry.field_validations.some((validation) => validation.field === field);
  return (
    (known(displayName) && actor.display_name) ||
    (known(login) && actor.login) ||
    actorIdentity(actor)
  );
}
function taskHeading(task: TaskV1, entry: DetailEntry) {
  if (
    task.content.state !== "known" ||
    !entry.field_validations.some((field) => field.field === "task_content")
  )
    return "Saved task";
  const line = task.content.text
    ?.split(/\r?\n/)
    .find((line) => line.trim())
    ?.trim();
  if (!line) return "Saved task";
  const characters = Array.from(line);
  return `Task: ${characters.slice(0, 120).join("")}${characters.length > 120 ? "…" : ""}`;
}
function valueFor(task: TaskV1, field: DetailField, entry: DetailEntry) {
  switch (field) {
    case "task_content":
      return task.content.state === "known"
        ? task.content.text === ""
          ? "This task’s content is empty."
          : task.content.text
        : "Unknown";
    case "task_creator_login":
      return task.creator.login;
    case "task_creator_display_name":
      return task.creator.display_name;
    case "task_state":
      return task.state;
    case "task_created_at":
      return task.created_at;
    case "task_updated_at":
      return task.updated_at;
    case "task_pending":
      return task.pending === null ? "Unknown" : task.pending ? "Yes" : "No";
    case "task_resolved_at":
      return task.resolved_at;
    case "task_resolver":
      return task.resolved_by
        ? actorPresentation(
            task.resolved_by,
            entry,
            "task_resolver_login",
            "task_resolver_display_name",
          )
        : null;
    case "task_resolver_login":
      return task.resolved_by?.login ?? null;
    case "task_resolver_display_name":
      return task.resolved_by?.display_name ?? null;
    case "task_comment_id":
      return task.comment_id === null ? null : `#${task.comment_id}`;
    default:
      return null;
  }
}

function TaskRow({ entry }: { entry: DetailEntry }) {
  if (entry.native?.kind !== "task.v1")
    return (
      <li className="text-xs text-muted-foreground">
        Saved task data could not be displayed.
      </li>
    );
  const task = entry.native.value;
  return (
    <li className="space-y-2 break-words text-sm">
      <p className="font-medium">{taskHeading(task, entry)}</p>
      <p className="text-xs text-muted-foreground">
        Creator:{" "}
        {actorPresentation(
          task.creator,
          entry,
          "task_creator_login",
          "task_creator_display_name",
        )}
      </p>
      {task.observed_content_state === "omitted" ||
      task.observed_content_state === "oversized" ? (
        <Badge variant="outline" size="sm">
          {task.observed_content_state === "oversized"
            ? "Latest task content exceeds the text limit"
            : "Latest task content was omitted"}
        </Badge>
      ) : null}
      <dl className="grid grid-cols-[auto_minmax(0,1fr)] gap-x-4 gap-y-2 text-xs">
        {fields.map(([field, label]) => {
          const validation = entry.field_validations.find(
            (item) => item.field === field,
          );
          const value = valueFor(task, field, entry);
          const current =
            entry.field_mask.includes(field) &&
            (field !== "task_content" ||
              task.observed_content_state === "known");
          return (
            <div key={field} className="contents">
              <dt className="text-muted-foreground">{label}</dt>
              <dd className="space-y-1">
                <p
                  className={
                    field === "task_content" ? "whitespace-pre-wrap" : undefined
                  }
                >
                  {validation ? (value ?? "None") : "Unknown"}
                </p>
                {validation ? (
                  <p className="text-muted-foreground">
                    {current
                      ? "Observed "
                      : "Retained from an earlier observation · last observed "}
                    <time dateTime={validation.validated_at}>
                      {new Date(validation.validated_at).toLocaleString()}
                    </time>
                  </p>
                ) : null}
              </dd>
            </div>
          );
        })}
      </dl>
    </li>
  );
}

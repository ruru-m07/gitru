import {
  type ContextFacetCapability,
  collaboration,
  collaborationErrorMessage,
  type DetailEntry,
  type DetailField,
  type ParticipantV1,
  type RemoteAccount,
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

/** Disclosure owns only this facet's mount; the independent draft stays mounted. */
export function NativeParticipantsPanel({ account, subjectId, policy }: Props) {
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
            .hydrateDetail({ subject_id: subjectId, facet: "participants" }),
      );
    } catch (failure) {
      setError(collaborationErrorMessage(failure));
    } finally {
      setBusy(false);
    }
  }
  return (
    <section className="space-y-2 border-t pt-4" aria-label="Participants">
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
            Participants
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
                      Sync participants
                    </Button>
                    <ShownParticipants
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

/** Mounted only for an explicitly opened, authorized saved facet. */
function ShownParticipants({ account, subjectId, policy }: Props) {
  const query = useQuery(
    detailQueryOptions(account, {
      subject_id: subjectId,
      facet: "participants",
      cursor: null,
      limit: 100,
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
      facet: "participants",
    },
    enabled:
      account.state === "active" &&
      canMaintainDemand(policy) &&
      query.data?.evidence.availability !== "unavailable",
  });
  return (
    <div className="space-y-3">
      <p className="text-xs text-muted-foreground">
        Provider approval flags and participation times are saved observations.
        They do not establish approval of the current commit.
      </p>
      {query.isPending ? (
        <p role="status" className="text-xs text-muted-foreground">
          Loading saved participants…
        </p>
      ) : query.isError ? (
        <p role="alert" className="text-xs text-destructive-foreground">
          {collaborationErrorMessage(query.error)}
        </p>
      ) : query.data?.evidence.availability === "unavailable" ? (
        <p className="text-xs text-muted-foreground">
          Saved participants are unavailable in the current authorized view.
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
                Partial participant set
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
              Participants have not been saved on this device yet.
            </p>
          ) : data.entries.length ? (
            <ul className="space-y-4" aria-label="Saved participants">
              {data.entries.map((entry) => (
                <ParticipantRow key={entry.id} entry={entry} />
              ))}
            </ul>
          ) : (
            <p className="text-xs text-muted-foreground">
              {data.evidence.coverage.state === "complete"
                ? "No participants were returned in the saved observation."
                : "No participants are saved in this partial view."}
            </p>
          )}
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
  ["participant_login", "Nickname"],
  ["participant_display_name", "Display name"],
  ["participant_role", "Provider role"],
  ["participant_approved", "Approval flag"],
  ["participant_state", "Provider state"],
  ["participant_participated_at", "Participation time"],
] as const satisfies ReadonlyArray<readonly [DetailField, string]>;

function valueFor(participant: ParticipantV1, field: DetailField) {
  switch (field) {
    case "participant_login":
      return participant.user.login;
    case "participant_display_name":
      return participant.user.display_name;
    case "participant_role":
      return participant.role;
    case "participant_approved":
      return participant.approved === null
        ? "Unknown"
        : participant.approved
          ? "Yes"
          : "No";
    case "participant_state":
      return participant.state;
    case "participant_participated_at":
      return participant.participated_at;
    default:
      return null;
  }
}

function ParticipantRow({ entry }: { entry: DetailEntry }) {
  if (entry.native?.kind !== "participant.v1")
    return (
      <li className="text-xs text-muted-foreground">
        Saved participant data could not be displayed.
      </li>
    );
  const participant = entry.native.value;
  const known = (field: DetailField) =>
    entry.field_validations.some((validation) => validation.field === field);
  const presentation =
    (known("participant_display_name") && participant.user.display_name) ||
    (known("participant_login") && participant.user.login) ||
    participant.user.provider_id;
  return (
    <li className="space-y-2 break-words text-sm">
      <p className="font-medium">{presentation}</p>
      <p className="text-xs text-muted-foreground">
        {participant.user.provider_id}
      </p>
      <dl className="grid grid-cols-[auto_minmax(0,1fr)] gap-x-4 gap-y-2 text-xs">
        {fields.map(([field, label]) => {
          const validation = entry.field_validations.find(
            (item) => item.field === field,
          );
          const value = valueFor(participant, field);
          return (
            <div key={field} className="contents">
              <dt className="text-muted-foreground">{label}</dt>
              <dd className="space-y-1">
                <p>{validation ? (value ?? "None") : "Unknown"}</p>
                {validation ? (
                  <p className="text-muted-foreground">
                    {entry.field_mask.includes(field)
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

import type {
  MetadataFieldEvidence,
  RemoteItem,
  ResourceMetadataSnapshot,
  ResourceMetadataValues,
} from "@gitru/collaboration-client";
import { Badge } from "@gitru/ui/components/badge";
import {
  Collapsible,
  CollapsiblePanel,
  CollapsibleTrigger,
} from "@gitru/ui/components/collapsible";
import type { ReactNode } from "react";
import { ProviderLink } from "./provider-link";

type MetadataField = MetadataFieldEvidence["field"];

function evidence(
  metadata: ResourceMetadataSnapshot | null,
  field: MetadataField,
) {
  return metadata?.fields.find((entry) => entry.field === field);
}

/** Known null is authoritative absence and cannot fall back to a summary. */
function project<K extends keyof ResourceMetadataValues>(
  metadata: ResourceMetadataSnapshot | null,
  field: MetadataField,
  key: K,
  fallback: ResourceMetadataValues[K],
): ResourceMetadataValues[K] {
  return evidence(metadata, field)?.saved_state === "known" && metadata
    ? metadata.values[key]
    : fallback;
}

function date(value: string | null) {
  if (value === null) return "Unknown";
  const parsed = new Date(value);
  return Number.isNaN(parsed.getTime()) ? "Unknown" : parsed.toLocaleString();
}

function FieldStatus({ value }: { value: MetadataFieldEvidence | undefined }) {
  if (!value)
    return <span className="text-xs text-muted-foreground">Not saved yet</span>;
  const observation =
    value.observed_state === "omitted"
      ? "Latest provider value omitted"
      : value.observed_state === "oversized"
        ? "Latest provider value exceeds the local limit"
        : null;
  return (
    <span className="inline-flex flex-wrap gap-x-2 text-xs text-muted-foreground">
      {observation ? <span>{observation}</span> : null}
      {value.saved_state !== "known" ? (
        <span>No saved value</span>
      ) : value.validated_at ? (
        <span>Validated {date(value.validated_at)}</span>
      ) : (
        <span>Validation unknown</span>
      )}
      {value.stale_at && Date.parse(value.stale_at) <= Date.now() ? (
        <Badge variant="outline" size="sm">
          May be stale
        </Badge>
      ) : null}
    </span>
  );
}

function MetadataRow({
  label,
  field,
  metadata,
  children,
}: {
  label: string;
  field: MetadataField;
  metadata: ResourceMetadataSnapshot | null;
  children: ReactNode;
}) {
  const value = evidence(metadata, field);
  return (
    <div className="min-w-0 space-y-1">
      <dt className="text-xs font-medium">{label}</dt>
      <dd className="break-words text-sm">
        {value?.saved_state === "known" ? children : "No saved value"}
      </dd>
      <dd>
        <FieldStatus value={value} />
      </dd>
    </div>
  );
}

export function SelectedResourceHeader({
  item,
  metadata,
}: {
  item: RemoteItem;
  metadata: ResourceMetadataSnapshot | null;
}) {
  const values = metadata?.values;
  const title = project(metadata, "title", "title", item.title);
  const state = project(metadata, "state", "state", item.state);
  const authorKnown = evidence(metadata, "author")?.saved_state === "known";
  const author = authorKnown ? values?.author?.login : item.author;
  const updated = project(
    metadata,
    "updated_at",
    "updated_at",
    item.updated_at,
  );
  const webUrl = project(metadata, "web_url", "web_url", item.web_url);
  const draft = project(metadata, "is_draft", "is_draft", item.is_draft);
  return (
    <header className="space-y-3" aria-label="Selected resource metadata">
      <div className="flex flex-wrap gap-2">
        <Badge variant="outline">{state ?? "State unavailable"}</Badge>
        {item.number ? (
          <span className="text-xs text-muted-foreground">#{item.number}</span>
        ) : null}
        {item.kind === "pull_request" ? (
          draft === true ? (
            <Badge variant="outline">Draft</Badge>
          ) : evidence(metadata, "is_draft")?.saved_state === "known" ? (
            <Badge variant="outline">
              {draft === false
                ? state === "open"
                  ? "Ready for review"
                  : "Not a draft"
                : "Draft status unavailable"}
            </Badge>
          ) : null
        ) : null}
      </div>
      <h2 className="break-words text-lg font-semibold leading-snug">
        {title === ""
          ? "This resource has an empty title."
          : (title ?? "Untitled resource")}
      </h2>
      <p className="break-words text-xs text-muted-foreground">
        {author ? `@${author}` : "Author unavailable"} · Updated {date(updated)}
      </p>
      <ProviderLink url={webUrl} />
      {metadata ? (
        <>
          <dl className="grid gap-3 sm:grid-cols-2">
            <MetadataRow label="Labels" field="labels" metadata={metadata}>
              {values?.labels.length
                ? values.labels.map((label, index) => (
                    <Badge
                      key={`${label.provider_id ?? "name-only"}:${label.name}:${index}`}
                      variant="outline"
                      size="sm"
                      className="mr-1 mb-1 h-auto min-h-5 max-w-full whitespace-normal py-px align-top sm:h-auto sm:min-h-4 [overflow-wrap:anywhere]"
                    >
                      {label.name}
                    </Badge>
                  ))
                : "No labels"}
            </MetadataRow>
            <MetadataRow
              label="Assignees"
              field="assignees"
              metadata={metadata}
            >
              {values?.assignees.length
                ? values.assignees.map((actor) => `@${actor.login}`).join(", ")
                : "No assignees"}
            </MetadataRow>
            <MetadataRow
              label="Milestone"
              field="milestone"
              metadata={metadata}
            >
              {values?.milestone?.title ?? "No milestone"}
            </MetadataRow>
            {item.kind === "issue" ||
            evidence(metadata, "state_reason")?.saved_state === "known" ? (
              <MetadataRow
                label="State reason"
                field="state_reason"
                metadata={metadata}
              >
                {values?.state_reason ?? "No state reason"}
              </MetadataRow>
            ) : null}
            {item.kind === "pull_request" ? (
              <>
                {(["head", "base"] as const).map((field) => (
                  <MetadataRow
                    key={field}
                    label={
                      field === "head"
                        ? "Head branch / SHA"
                        : "Base branch / SHA"
                    }
                    field={field}
                    metadata={metadata}
                  >
                    {values?.[field] ? (
                      <span>
                        {values[field].name}{" "}
                        <code className="block break-all text-xs">
                          {values[field].oid}
                        </code>
                      </span>
                    ) : (
                      "No branch reference"
                    )}
                  </MetadataRow>
                ))}
                <MetadataRow
                  label="Merged at"
                  field="merged_at"
                  metadata={metadata}
                >
                  {values?.merged_at ? date(values.merged_at) : "Not merged"}
                </MetadataRow>
              </>
            ) : null}
          </dl>
          <div
            className="space-y-1 text-xs text-muted-foreground"
            role="status"
          >
            {metadata.fields
              .filter((field) =>
                [
                  "title",
                  "state",
                  "author",
                  "updated_at",
                  "web_url",
                  "is_draft",
                ].includes(field.field),
              )
              .map((field) =>
                field.observed_state === "omitted" ||
                field.observed_state === "oversized" ||
                (field.stale_at && Date.parse(field.stale_at) <= Date.now()) ? (
                  <p key={field.field}>
                    {field.field.replace(/_/g, " ")}:{" "}
                    {field.observed_state === "oversized"
                      ? "latest value exceeds the local limit"
                      : field.observed_state === "omitted"
                        ? "latest value was omitted"
                        : "saved value may be stale"}
                    .
                  </p>
                ) : null,
              )}
          </div>
          <Collapsible>
            <CollapsibleTrigger className="text-xs text-muted-foreground underline underline-offset-4">
              Saved header details
            </CollapsibleTrigger>
            <CollapsiblePanel>
              <dl
                className="space-y-1 pt-2 text-xs text-muted-foreground"
                aria-label="Header field freshness"
              >
                {(
                  [
                    "title",
                    "state",
                    "author",
                    "updated_at",
                    "web_url",
                    ...(item.kind === "pull_request"
                      ? ["is_draft" as const]
                      : []),
                  ] as const
                ).map((field) => (
                  <div key={field} className="flex flex-wrap gap-x-2">
                    <dt>{field.replace(/_/g, " ")}</dt>
                    <dd>
                      <FieldStatus value={evidence(metadata, field)} />
                    </dd>
                  </div>
                ))}
              </dl>
            </CollapsiblePanel>
          </Collapsible>
        </>
      ) : null}
    </header>
  );
}

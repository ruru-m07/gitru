import type {
  DetailSnapshot,
  ResourceMetadataSnapshot,
} from "@gitru/collaboration-client";
import { fixtureAccount, fixtureItem } from "./collaboration";

export function fixtureMetadata(
  kind = fixtureItem.kind,
): ResourceMetadataSnapshot {
  const fields = [
    "title",
    "state",
    "state_reason",
    "author",
    "web_url",
    "updated_at",
    "labels",
    "assignees",
    "milestone",
    ...(kind === "pull_request"
      ? ["is_draft", "head", "base", "merged_at"]
      : []),
  ] as Array<ResourceMetadataSnapshot["fields"][number]["field"]>;
  return {
    kind,
    values: {
      title: "Authoritative detail title",
      state: "closed",
      state_reason: "completed",
      author: {
        provider_id: "9007199254740993",
        login: "detail-author",
        web_url: null,
      },
      web_url: fixtureItem.web_url,
      updated_at: "2026-10-03T12:10:00Z",
      labels: [
        { provider_id: "9007199254740994", name: "detail-label", color: null },
      ],
      assignees: [
        {
          provider_id: "9007199254740995",
          login: "detail-assignee",
          web_url: null,
        },
      ],
      milestone: {
        provider_id: "9007199254740996",
        number: "2",
        title: "Detail milestone",
        state: "open",
        web_url: null,
      },
      is_draft: false,
      head: { name: "feature", oid: "a".repeat(40), repository: null },
      base: {
        name: "main",
        oid: "b".repeat(40),
        repository: {
          provider_id: "345",
          full_name: "example-org/engine",
          web_url: null,
        },
      },
      merged_at: null,
      merge_base_oid: null,
    },
    fields: fields.map((field) => ({
      field,
      saved_state: "known",
      observed_state: "known",
      validated_at: "2026-10-03T12:10:00Z",
      stale_at: "2099-10-03T12:10:00Z",
      source: {
        source: "fixture/resource/v1",
        adapter_version: 1,
        provider_updated_at: "2026-10-03T12:10:00Z",
        observed_at: "2026-10-03T12:10:00Z",
      },
    })),
  };
}

export function fixtureBody(
  overrides: Partial<DetailSnapshot> = {},
): DetailSnapshot {
  return {
    subject_id: fixtureItem.id,
    body: { state: "known", text: "Full cached resource description" },
    metadata: fixtureMetadata(),
    entries: [],
    next_cursor: null,
    revision: "10",
    authorization_view: "1",
    evidence: {
      facet: "body",
      availability: "ready",
      coverage: {
        state: "complete",
        validated_at: "2026-10-03T12:10:00Z",
        remote_has_more: false,
      },
      freshness: "fresh",
      stale_at: "2099-10-03T12:10:00Z",
      facet_revision: "10",
      authorization_epoch: fixtureAccount.authorization_epoch,
      access_reason: null,
      source: null,
      value_source: null,
      saved_empty: false,
      observed_state: "known",
      sync: {
        state: "idle",
        last_success_at: "2026-10-03T12:10:00Z",
        next_retry_at: null,
        error: null,
      },
    },
    ...overrides,
  };
}

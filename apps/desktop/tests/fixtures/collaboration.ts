/** Sanitized deterministic test fixtures. Never imported by production code. */

import type { InboxSemantics } from "@gitru/collaboration-client";
import type {
  AccountSnapshot,
  CapabilitySnapshot,
  CapabilityTarget,
  ContextCapabilityRequest,
  ContextFacetCapability,
  ContextualCapabilitySnapshot,
  GithubCliDiscovery,
  InboxPage,
  ItemPage,
  LocalInboxState,
  RemoteAccount,
  RemoteItem,
  RepositorySnapshot,
} from "@gitru/commands";
import { mockIPC, mockWindows } from "@tauri-apps/api/mocks";

export const fixtureAccount = {
  id: "fixture-account",
  provider: "github" as const,
  host: "https://github.com",
  actor_id: "123",
  login: "example-user",
  display_name: "Example User",
  authorization_epoch: "1",
  state: "active" as const,
  notifications_supported: true,
};
export const fixtureAccounts: AccountSnapshot = {
  accounts: [fixtureAccount],
  revision: "10",
  authorization_view: "1",
};

export const fixtureGitlabAccount: RemoteAccount = {
  ...fixtureAccount,
  id: "fixture-gitlab-account",
  provider: "gitlab",
  host: "gitlab.com",
  actor_id: "9007199254740993",
  login: "gitlab-user",
  display_name: "GitLab User",
  notifications_supported: false,
};
export const fixtureGitlabCapabilities: CapabilitySnapshot = {
  account_id: fixtureGitlabAccount.id,
  instance: {
    id: "gitlab:https://gitlab.com/",
    provider: "gitlab",
    base_url: "https://gitlab.com/",
  },
  inbox_semantics: "none",
  revision: "10",
  authorization_view: "1",
  facets: (
    [
      "repositories",
      "pull_requests",
      "issues",
      "inbox",
      "pull_details",
      "issue_details",
      "comments",
      "reviews",
      "checks",
      "merge",
    ] as const
  ).map((facet): CapabilitySnapshot["facets"][number] => ({
    facet,
    state: facet === "repositories" ? "supported" : "unsupported",
    reason:
      facet === "repositories"
        ? null
        : facet === "inbox"
          ? "provider_semantics"
          : "not_implemented",
  })),
};

export function fixtureContextualCapabilities(
  account: RemoteAccount = fixtureAccount,
  target: CapabilityTarget = {
    kind: "account",
    instance_id: null,
    repository_id: null,
    resource_id: null,
    resource_kind: null,
  },
  semantics: InboxSemantics = "native_notifications",
): ContextualCapabilitySnapshot {
  const facets = [
    "repositories",
    "pull_requests",
    "issues",
    "inbox",
    "pull_details",
    "issue_details",
    "comments",
    "reviews",
    "checks",
    "merge",
  ] as const;
  return {
    account_id: account.id,
    authorization_epoch: account.authorization_epoch,
    instance: {
      id: `${account.provider}:${new URL(account.host.includes("://") ? account.host : `https://${account.host}`).href}`,
      provider: account.provider,
      base_url: new URL(
        account.host.includes("://") ? account.host : `https://${account.host}`,
      ).href,
    },
    target,
    inbox_semantics: semantics,
    revision: "10",
    authorization_view: "1",
    facets: facets.map((facet): ContextFacetCapability => {
      const implemented =
        ["repositories", "pull_requests", "issues", "inbox"].includes(facet) &&
        (facet !== "inbox" || semantics !== "none");
      const access = {
        state: implemented ? ("supported" as const) : ("unsupported" as const),
        reason: implemented
          ? null
          : facet === "inbox"
            ? ("provider_semantics" as const)
            : ("not_implemented" as const),
      };
      return {
        facet,
        saved_read: access,
        synchronize: access,
        remote_write: { state: "unsupported", reason: "not_implemented" },
        observation: implemented ? "partial" : "unknown",
        sync: {
          state: "idle",
          last_success_at: null,
          next_retry_at: null,
          error: null,
        },
        can_recheck_access: false,
      };
    }),
  };
}
export const fixtureGithubCli: GithubCliDiscovery = {
  status: "available",
  accounts: [
    {
      id: "fixture-cli-example",
      login: "example-user",
      host: "github.com",
      active: true,
      availability: "ready",
    },
    {
      id: "fixture-cli-second",
      login: "second-user",
      host: "github.com",
      active: false,
      availability: "ready",
    },
  ],
};
export const fixtureRepositories: RepositorySnapshot = {
  repositories: [
    {
      id: "fixture-repository",
      account_id: fixtureAccount.id,
      provider_id: "345",
      full_name: "example-org/engine",
      name: "engine",
      web_url: "https://github.com/example-org/engine",
      description: "An example repository for tests",
      default_branch: "main",
      selected: true,
    },
  ],
  revision: "10",
  authorization_view: "1",
  coverage: {
    state: "partial",
    validated_at: "2026-10-02T05:00:00Z",
    remote_has_more: true,
  },
  sync: {
    state: "idle",
    last_success_at: "2026-10-02T05:00:00Z",
    next_retry_at: null,
    error: null,
  },
};
export const fixtureItem: RemoteItem = {
  id: "fixture-item",
  account_id: fixtureAccount.id,
  repository_id: "fixture-repository",
  provider_id: "678",
  kind: "pull_request",
  number: "42",
  title: "Keep collaboration data available offline",
  body: "This description is saved locally.\n\nIt remains readable when the provider cannot be reached.",
  body_omitted: false,
  author: "example-user",
  web_url: "https://github.com/example-org/engine/pull/42",
  state: "open",
  updated_at: "2026-10-02T05:00:00Z",
  head_oid: "abc123",
  is_draft: false,
  reason: null,
  unread: null,
  native_inbox: null,
};
export const fixturePage: ItemPage = {
  items: [fixtureItem],
  revision: "10",
  authorization_view: "1",
  next_cursor: null,
  coverage: {
    state: "partial",
    validated_at: "2026-10-02T05:00:00Z",
    remote_has_more: true,
  },
  sync: {
    state: "offline",
    last_success_at: "2026-10-02T05:00:00Z",
    next_retry_at: null,
    error: null,
  },
};

export function fixtureLocalInboxState(
  item: RemoteItem = fixtureItem,
): LocalInboxState {
  return {
    disposition: "inbox",
    effective_disposition: "inbox",
    bookmarked: false,
    snoozed_until: null,
    activity_updated_at: item.updated_at,
    superseded_by_activity: false,
    generation: "0",
  };
}

export function fixtureInboxPage(
  items: RemoteItem[] = [fixtureItem],
): InboxPage {
  return {
    entries: items.map((item) => ({
      item,
      local: fixtureLocalInboxState(item),
    })),
    revision: fixturePage.revision,
    authorization_view: fixturePage.authorization_view,
    next_cursor: fixturePage.next_cursor,
    coverage: fixturePage.coverage,
    sync: fixturePage.sync,
    evaluated_at: "2026-10-02T05:00:00Z",
    next_local_change_at: null,
  };
}

/** Install only in a standalone browser QA harness, before rendering real features. */
export function installCollaborationPreviewBoundary() {
  mockWindows("main");
  mockIPC((command, payload) => {
    const input = payload as Record<string, unknown> | undefined;
    switch (command) {
      case "collaboration_accounts":
        return fixtureAccounts;
      case "collaboration_contextual_capabilities": {
        const request = input?.request as ContextCapabilityRequest;
        return fixtureContextualCapabilities(fixtureAccount, request.target);
      }
      case "collaboration_discover_github_cli":
        return fixtureGithubCli;
      case "collaboration_connect_github_cli":
        return input?.candidateId === "fixture-cli-second"
          ? {
              ...fixtureAccount,
              id: "fixture-second-account",
              actor_id: "124",
              login: "second-user",
              display_name: "Second User",
            }
          : fixtureAccount;
      case "collaboration_repositories":
        return fixtureRepositories;
      case "collaboration_items": {
        const query = input?.query as {
          kind: RemoteItem["kind"];
          search: string | null;
        };
        const item = {
          ...fixtureItem,
          kind: query.kind,
          ...(query.kind === "notification"
            ? { state: "unread", unread: true, reason: "review_requested" }
            : {}),
        };
        return {
          ...fixturePage,
          items:
            query.search &&
            !item.title.toLowerCase().includes(query.search.toLowerCase())
              ? []
              : [item],
        };
      }
      case "collaboration_inbox": {
        const query = input?.query as { search: string | null };
        const item = {
          ...fixtureItem,
          kind: "notification" as const,
          state: "unread",
          unread: true,
          reason: "review_requested",
        };
        return fixtureInboxPage(
          query.search &&
            !item.title.toLowerCase().includes(query.search.toLowerCase())
            ? []
            : [item],
        );
      }
      case "collaboration_item":
        return { item: fixtureItem, revision: "10", authorization_view: "1" };
      case "collaboration_draft":
        return null;
      case "collaboration_save_draft":
        return { ...(input?.draft as object), generation: "1" };
      case "collaboration_refresh":
        return { job_id: "fixture-job" };
      case "collaboration_select_repository":
        return "10";
      case "collaboration_changes_since":
        return {
          revision: "10",
          authorization_view: "1",
          reset_required: false,
          changes: [],
          has_more: false,
        };
      case "plugin:event|listen":
        return 1;
      case "plugin:event|unlisten":
        return undefined;
      default:
        throw new Error(
          `Unexpected command in collaboration preview: ${command}`,
        );
    }
  });
}

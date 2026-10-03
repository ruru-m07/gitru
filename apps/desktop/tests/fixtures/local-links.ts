import type {
  LocalLinkInspection,
  LocalNavigationReceipt,
  LocalRepositoryLink,
} from "@gitru/commands";
import { fixtureAccount, fixtureRepositories } from "./collaboration";
export const fixtureInstanceId = "github:https://github.com/";
export const fixtureEndpoint = {
  remote_name: "origin",
  direction: "fetch" as const,
  ordinal: 0,
  transport: "scp" as const,
  host: "github.com",
  port: 22,
  path: "example-org/engine",
};
export const fixtureLink: LocalRepositoryLink = {
  id: "link-a",
  local_repository_id: "registered-a",
  endpoint: fixtureEndpoint,
  account_id: fixtureAccount.id,
  actor_id: fixtureAccount.actor_id,
  instance_id: fixtureInstanceId,
  repository_provider_id: "345",
  repository_id: "fixture-repository",
  generation: "9007199254740993",
  state: "linked",
  repository: fixtureRepositories.repositories[0],
};
export const fixtureNavigation: LocalNavigationReceipt = {
  local_repository_id: "registered-a",
  account_id: fixtureAccount.id,
  instance_id: fixtureInstanceId,
  repository_id: "fixture-repository",
  authorization_epoch: fixtureAccount.authorization_epoch,
  selected: true,
};
export function fixtureInspection(
  overrides: Partial<LocalLinkInspection> = {},
): LocalLinkInspection {
  return {
    local_repository_id: "registered-a",
    remotes: {
      semantic_digest: "sanitized-proof",
      remotes: [
        {
          name: "origin",
          fetch_urls: [
            {
              ordinal: 0,
              sanitized_url: "github.com:example-org/engine.git",
              endpoint: {
                transport: "scp",
                host: "github.com",
                port: 22,
                path: "example-org/engine",
              },
              redacted: true,
            },
          ],
          push_urls: [],
        },
      ],
    },
    observation_error: null,
    preview_id: "native-preview-1",
    snapshot: {
      links: [fixtureLink],
      resolutions: [
        {
          endpoint: fixtureEndpoint,
          state: "linked",
          candidates: [
            {
              id: "candidate-a",
              endpoint: fixtureEndpoint,
              account_id: fixtureAccount.id,
              actor_id: fixtureAccount.actor_id,
              authorization_epoch: fixtureAccount.authorization_epoch,
              instance_id: fixtureInstanceId,
              repository: fixtureRepositories.repositories[0],
            },
          ],
        },
      ],
      bindings: [],
      bindings_generation: "9007199254740993",
      revision: "10",
      authorization_view: "1",
    },
    ...overrides,
  };
}

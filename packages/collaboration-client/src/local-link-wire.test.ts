import {
  collaborationConfirmLocalLink,
  collaborationLocalClones,
  collaborationLocalLinks,
  collaborationRemoveLocalLink,
  collaborationRemoveTransportBinding,
  collaborationSaveTransportBinding,
  collaborationValidateLocalNavigation,
  LocalLinkInspectionSchema,
  LocalLinkStateSchema,
} from "@gitru/commands";
import { afterEach, describe, expect, it, vi } from "vitest";

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
afterEach(() => invoke.mockReset());
const endpoint = {
  remote_name: "upstream.fork",
  direction: "push",
  ordinal: 1,
  transport: "ssh",
  host: "git.example",
  port: 2222,
  path: "team/subgroup/project",
};
const link = {
  id: "link",
  local_repository_id: "durable-registration",
  endpoint,
  account_id: "account",
  actor_id: "9007199254740993",
  instance_id: "gitlab:https://example:8443/base/",
  repository_provider_id: "9007199254740994",
  repository_id: "repository",
  generation: "9007199254740995",
  state: "unavailable",
  repository: null,
};
describe("generated local-link IPC wire", () => {
  it("preserves safe transport evidence, null provider metadata and all unresolved states", async () => {
    const inspection = LocalLinkInspectionSchema.parse({
      local_repository_id: link.local_repository_id,
      preview_id: null,
      observation_error: "unsupported_legacy_configuration",
      remotes: null,
      snapshot: {
        links: [link],
        resolutions: [{ endpoint, state: "ambiguous", candidates: [] }],
        bindings: [],
        bindings_generation: "9007199254740996",
        revision: "9007199254740997",
        authorization_view: "9007199254740998",
      },
    });
    expect(inspection.snapshot.links[0].repository).toBeNull();
    expect(inspection.snapshot.links[0].repository_provider_id).toBe(
      "9007199254740994",
    );
    for (const state of [
      "linked",
      "unresolved",
      "ambiguous",
      "unconfigured_instance",
      "unsupported_transport",
      "remote_changed",
      "local_repository_missing",
      "unavailable",
    ])
      expect(LocalLinkStateSchema.parse(state)).toBe(state);
    invoke.mockResolvedValue(inspection);
    await collaborationLocalLinks({
      localRepositoryId: link.local_repository_id,
    });
    expect(invoke).toHaveBeenCalledExactlyOnceWith(
      "collaboration_local_links",
      { localRepositoryId: link.local_repository_id },
    );
  });
  it("uses distinct opaque-preview/CAS/binding/clone/navigation commands without caller proof or paths", async () => {
    const request = {
      preview_id: "opaque-native-preview",
      candidate_id: "native-candidate",
      replace_link_id: null,
    };
    invoke.mockResolvedValueOnce({
      link,
      revision: "2",
      authorization_view: "1",
    });
    await collaborationConfirmLocalLink({ request });
    invoke.mockResolvedValueOnce("3");
    await collaborationRemoveLocalLink({
      id: link.id,
      generation: link.generation,
    });
    const binding = {
      instance_id: link.instance_id,
      transport: "scp" as const,
      host: "git-alias",
      port: 22,
      path_prefix: "prefix",
      layout: "subgroups" as const,
      expected_bindings_generation: "9007199254740996",
      replace: { id: "binding", generation: "9007199254740997" },
    };
    invoke.mockResolvedValueOnce({
      id: "binding",
      instance_id: binding.instance_id,
      transport: binding.transport,
      host: binding.host,
      port: binding.port,
      path_prefix: binding.path_prefix,
      layout: binding.layout,
      generation: "9007199254740998",
    });
    await collaborationSaveTransportBinding({ request: binding });
    invoke.mockResolvedValueOnce("4");
    await collaborationRemoveTransportBinding({
      id: "binding",
      generation: "9007199254740998",
      expectedBindingsGeneration: "9007199254740999",
    });
    const scope = {
      account_id: link.account_id,
      instance_id: link.instance_id,
      repository_id: link.repository_id,
      authorization_epoch: "9007199254740993",
    };
    invoke.mockResolvedValueOnce({
      clones: [
        {
          local_repository_id: link.local_repository_id,
          local_repository_name: null,
          link_id: link.id,
          generation: link.generation,
          state: "local_repository_missing",
        },
      ],
    });
    await collaborationLocalClones({ request: scope });
    for (const direction of ["git", "collaboration"] as const) {
      invoke.mockResolvedValueOnce({
        ...scope,
        local_repository_id: link.local_repository_id,
        selected: false,
      });
      await collaborationValidateLocalNavigation({
        request: {
          local_repository_id: link.local_repository_id,
          link_id: link.id,
          generation: link.generation,
          direction,
        },
      });
    }
    expect(invoke.mock.calls.map(([name]) => name)).toEqual([
      "collaboration_confirm_local_link",
      "collaboration_remove_local_link",
      "collaboration_save_transport_binding",
      "collaboration_remove_transport_binding",
      "collaboration_local_clones",
      "collaboration_validate_local_navigation",
      "collaboration_validate_local_navigation",
    ]);
    expect(invoke.mock.calls[0][1]).toEqual({ request });
    expect(invoke.mock.calls[4][1]).toEqual({ request: scope });
    for (const [, payload] of invoke.mock.calls) {
      expect(JSON.stringify(payload)).not.toContain("webview");
      expect(JSON.stringify(payload)).not.toContain("filesystem_path");
    }
  });
});

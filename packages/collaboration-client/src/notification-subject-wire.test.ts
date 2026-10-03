import {
  collaborationDiscoverNotificationSubject,
  collaborationNotificationSubject,
  DiscoverNotificationSubjectRequestSchema,
  NotificationSubjectSnapshotSchema,
  NotificationSubjectStateSchema,
} from "@gitru/commands";
import { afterEach, describe, expect, it, vi } from "vitest";

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
afterEach(() => invoke.mockReset());

const query = {
  account_id: "actor-bound-account",
  authorization_epoch: "9007199254740993",
  notification_id: "thread:9007199254740994",
};
const snapshot = {
  revision: "9007199254740995",
  authorization_view: "9007199254740996",
  authorization_epoch: query.authorization_epoch,
  state: "resolved",
  reason: null,
  selector_generation: "9007199254740997",
  subject: {
    account_id: query.account_id,
    instance_id: "gitlab:https://git.example:8443/base/",
    kind: "issue",
    id: "canonical:9007199254740998",
    provider_id: "9007199254740999",
  },
  fallback_web_url: null,
  discovery: {
    support: "supported",
    admission: true,
    paused: true,
    retry_at: "2099-10-03T12:00:00.000Z",
    attempts: 2,
    sync: {
      state: "rate_limited",
      next_retry_at: "2099-10-03T12:00:00.000Z",
      last_success_at: null,
      error: null,
    },
  },
};

describe("generated notification subject wire", () => {
  it("retains canonical identity, string generations, nullable values and independent paused admission", async () => {
    const result = NotificationSubjectSnapshotSchema.parse(snapshot);
    expect(result.subject?.provider_id).toBe("9007199254740999");
    expect(result.selector_generation).toBe("9007199254740997");
    expect(result.reason).toBeNull();
    expect(result.fallback_web_url).toBeNull();
    expect(result.discovery.admission).toBe(true);
    expect(result.discovery.paused).toBe(true);
    invoke.mockResolvedValue(result);
    expect(await collaborationNotificationSubject({ query })).toEqual(result);
    expect(invoke).toHaveBeenCalledExactlyOnceWith(
      "collaboration_notification_subject",
      { query },
    );
  });

  it("represents every unresolved state without inventing a canonical resource or selector", () => {
    for (const state of [
      "resolved",
      "not_cached",
      "unsupported",
      "ambiguous",
      "unavailable",
      "identity_unverified",
    ]) {
      expect(NotificationSubjectStateSchema.parse(state)).toBe(state);
      const result = NotificationSubjectSnapshotSchema.parse({
        ...snapshot,
        state,
        subject: null,
        selector_generation: null,
        fallback_web_url:
          state === "unavailable"
            ? null
            : "https://git.example/base/team/project/issues/67",
        discovery: {
          ...snapshot.discovery,
          support: "unsupported",
          admission: false,
          paused: false,
          retry_at: null,
        },
      });
      expect(result.subject).toBeNull();
      expect(result.selector_generation).toBeNull();
    }
  });

  it("sends only native-inspected thread and selector generation on explicit discovery", async () => {
    const request = DiscoverNotificationSubjectRequestSchema.parse({
      ...query,
      selector_generation: snapshot.selector_generation,
      subject_url: "https://untrusted.invalid/renderer-must-not-authorize-http",
      provider_id: "renderer-must-not-pick-identity",
      repository_path: "renderer-must-not-pick-path",
    });
    expect(request).toEqual({
      ...query,
      selector_generation: snapshot.selector_generation,
    });
    invoke.mockResolvedValue({ job_id: "finite-point-discovery" });
    await collaborationDiscoverNotificationSubject({ request });
    expect(invoke).toHaveBeenCalledExactlyOnceWith(
      "collaboration_discover_notification_subject",
      { request },
    );
  });

  it("rejects missing captured generation and numeric identity coercion", () => {
    expect(
      DiscoverNotificationSubjectRequestSchema.safeParse(query).success,
    ).toBe(false);
    expect(
      DiscoverNotificationSubjectRequestSchema.safeParse({
        ...query,
        selector_generation: 9007199254740992,
      }).success,
    ).toBe(false);
    expect(
      NotificationSubjectSnapshotSchema.safeParse({
        ...snapshot,
        subject: { ...snapshot.subject, provider_id: 9007199254740992 },
      }).success,
    ).toBe(false);
  });
});

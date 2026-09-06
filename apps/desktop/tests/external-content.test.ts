import { describe, expect, test } from "vitest";
import {
  githubAccountAvatarUrl,
  githubCommitterAvatarUrl,
  normalizeExternalHttpsUrl,
  normalizeRemoteImageUrl,
} from "../src/lib/external-content";
import { telemetryConsentFromValue } from "../src/lib/telemetry-preference";

describe("external URL validation", () => {
  test("accepts HTTPS links and normalizes them", () => {
    expect(normalizeExternalHttpsUrl("https://gitru.app/docs")).toBe(
      "https://gitru.app/docs",
    );
  });

  test("rejects executable, local-file, insecure, and credential URLs", () => {
    for (const value of [
      "javascript:alert(1)",
      "file:///etc/passwd",
      "http://gitru.app/docs",
      "https://token@example.com/private",
    ]) {
      expect(normalizeExternalHttpsUrl(value)).toBeNull();
    }
  });
});

describe("remote image validation", () => {
  test("allows only exact configured avatar hosts", () => {
    expect(normalizeRemoteImageUrl("https://github.com/gitru-app.png")).toBe(
      "https://github.com/gitru-app.png",
    );
    expect(
      normalizeRemoteImageUrl("https://evilgithub.com/gitru-app.png"),
    ).toBeUndefined();
    expect(
      normalizeRemoteImageUrl("https://example.com/avatar.png"),
    ).toBeUndefined();
    expect(
      normalizeRemoteImageUrl("https://github.com/gitru-app/private"),
    ).toBeUndefined();
    expect(
      normalizeRemoteImageUrl(
        "https://avatars.githubusercontent.com/u/e?email=a%40b.com&redirect=evil",
      ),
    ).toBeUndefined();
  });

  test("rejects malformed account names and safely encodes commit emails", () => {
    expect(githubAccountAvatarUrl("../admin")).toBeUndefined();
    expect(githubCommitterAvatarUrl("a+b@example.com&size=huge")).toContain(
      "email=a%2Bb%40example.com%26size%3Dhuge",
    );
  });
});

describe("telemetry consent", () => {
  test("defaults to disabled and requires an explicit grant", () => {
    expect(telemetryConsentFromValue(null)).toBe(false);
    expect(telemetryConsentFromValue("denied")).toBe(false);
    expect(telemetryConsentFromValue("granted")).toBe(true);
  });
});

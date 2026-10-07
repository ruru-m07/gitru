import { describe, expect, it } from "vitest";
import { parseProcessMemory } from "./process-observation";

describe("native process memory attribution", () => {
  it("attributes only recursively proven WebKit descendants", () => {
    const observed = parseProcessMemory(
      [
        " 100 1 1024 /tmp/gitru",
        " 101 100 2048 WebKit.WebContent",
        " 102 101 512 WebKit.Networking",
        " 900 1 9999 WebKit.WebContent",
      ].join("\n"),
      100,
    );
    expect(observed.rust.rss_bytes).toBe(1024 * 1024);
    expect(observed.webview.state).toBe("observed_descendants");
    expect(observed.webview.aggregate_rss_bytes).toBe(2560 * 1024);
  });

  it("reports missing rather than zero without a proven descendant", () => {
    const observed = parseProcessMemory("100 1 1024 /tmp/gitru\n", 100);
    expect(observed.webview).toMatchObject({
      state: "missing",
      aggregate_rss_bytes: null,
    });
  });
});

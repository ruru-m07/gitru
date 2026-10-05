import { mkdtempSync, realpathSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, win32 } from "node:path";
import { describe, expect, it, vi } from "vitest";
import { canonicalHarnessPath, nativeCanonicalSpelling } from "./harness-paths";

describe("Rust-compatible canonical path spelling", () => {
  it.each([
    ["C:\\Users\\runner\\fixture", "\\\\?\\C:\\Users\\runner\\fixture"],
    ["\\\\server\\share\\fixture", "\\\\?\\UNC\\server\\share\\fixture"],
    ["\\\\?\\C:\\Users\\runner\\fixture", "\\\\?\\C:\\Users\\runner\\fixture"],
    [
      "\\\\?\\UNC\\server\\share\\fixture",
      "\\\\?\\UNC\\server\\share\\fixture",
    ],
  ])("uses actual win32 namespace conversion for %s", (canonical, expected) => {
    const result = nativeCanonicalSpelling(canonical, "win32");
    expect(result).toBe(expected);
    expect(result).toBe(win32.toNamespacedPath(canonical));
    expect(win32.resolve(result)).toBe(result);
    expect(win32.join(result, "run.json")).toBe(`${result}\\run.json`);
    expect(win32.relative(result, win32.join(result, "artifacts"))).toBe(
      "artifacts",
    );
    expect(nativeCanonicalSpelling(result, "win32")).toBe(result);
  });
  it.each([
    "darwin",
    "linux",
  ] as const)("preserves the canonical POSIX spelling on %s", (platform) => {
    expect(nativeCanonicalSpelling("/private/tmp/fixture", platform)).toBe(
      "/private/tmp/fixture",
    );
  });
  it("resolves actual owned directory and file identities before conversion", () => {
    const root = mkdtempSync(join(tmpdir(), "gitru-path-test-"));
    try {
      const file = join(root, "run.json");
      writeFileSync(file, "{}", { flag: "wx" });
      for (const path of [root, file]) {
        const canonical = canonicalHarnessPath(path);
        expect(canonical).toBe(
          nativeCanonicalSpelling(realpathSync.native(path)),
        );
        expect(canonicalHarnessPath(canonical)).toBe(canonical);
      }
    } finally {
      rmSync(root, { recursive: true });
    }
  });
  it("uses the actual native identity resolver instead of the default path walker", () => {
    const root = mkdtempSync(join(tmpdir(), "gitru-native-path-test-"));
    const native = vi.spyOn(realpathSync, "native");
    try {
      const expected = nativeCanonicalSpelling(realpathSync.native(root));
      native.mockClear();
      expect(canonicalHarnessPath(root)).toBe(expected);
      expect(native).toHaveBeenCalledExactlyOnceWith(root);
    } finally {
      native.mockRestore();
      rmSync(root, { recursive: true });
    }
  });
});

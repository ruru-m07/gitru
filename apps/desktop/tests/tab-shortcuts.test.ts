import { describe, expect, test } from "vitest";
import { resolveTabManagementShortcut } from "../src/lib/tab-switching";

const keyboardEvent = (
  overrides: Partial<Parameters<typeof resolveTabManagementShortcut>[0]> = {},
) => ({
  altKey: false,
  ctrlKey: false,
  key: "",
  metaKey: false,
  repeat: false,
  shiftKey: false,
  ...overrides,
});

describe("resolveTabManagementShortcut", () => {
  test("creates a tab with Command+T or Control+T", () => {
    expect(
      resolveTabManagementShortcut(
        keyboardEvent({ key: "t", metaKey: true }),
        "Meta",
      ),
    ).toBe("create");
    expect(
      resolveTabManagementShortcut(
        keyboardEvent({ ctrlKey: true, key: "T" }),
        "Control",
      ),
    ).toBe("create");
  });

  test("closes a tab with Command+W or Control+W", () => {
    expect(
      resolveTabManagementShortcut(
        keyboardEvent({ key: "w", metaKey: true }),
        "Meta",
      ),
    ).toBe("close");
    expect(
      resolveTabManagementShortcut(
        keyboardEvent({ ctrlKey: true, key: "W" }),
        "Control",
      ),
    ).toBe("close");
  });

  test("uses Command on Apple platforms and Control elsewhere", () => {
    expect(
      resolveTabManagementShortcut(
        keyboardEvent({ ctrlKey: true, key: "w" }),
        "Meta",
      ),
    ).toBe(null);
    expect(
      resolveTabManagementShortcut(
        keyboardEvent({ key: "t", metaKey: true }),
        "Control",
      ),
    ).toBe(null);
  });

  test("ignores unmodified and unrelated keys", () => {
    expect(
      resolveTabManagementShortcut(keyboardEvent({ key: "t" }), "Meta"),
    ).toBe(null);
    expect(
      resolveTabManagementShortcut(
        keyboardEvent({ key: "p", metaKey: true }),
        "Meta",
      ),
    ).toBe(null);
  });

  test("preserves shifted and alternate shortcut combinations", () => {
    expect(
      resolveTabManagementShortcut(
        keyboardEvent({ key: "t", metaKey: true, shiftKey: true }),
        "Meta",
      ),
    ).toBe(null);
    expect(
      resolveTabManagementShortcut(
        keyboardEvent({ altKey: true, ctrlKey: true, key: "w" }),
        "Control",
      ),
    ).toBe(null);
  });

  test("ignores repeated keydown events", () => {
    expect(
      resolveTabManagementShortcut(
        keyboardEvent({ key: "t", metaKey: true, repeat: true }),
        "Meta",
      ),
    ).toBe(null);
    expect(
      resolveTabManagementShortcut(
        keyboardEvent({ ctrlKey: true, key: "w", repeat: true }),
        "Control",
      ),
    ).toBe(null);
  });
});

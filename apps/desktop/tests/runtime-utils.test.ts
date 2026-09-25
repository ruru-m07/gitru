import { describe, expect, test } from "vitest";
import { resolveRepositoryContextScope } from "../src/bootstrap/runtime-utils";

describe("repository context runtime scope", () => {
  test("does not create a repository context for the native host shell", () => {
    expect(
      resolveRepositoryContextScope(
        false,
        null,
        "active-runtime",
        "active-tab",
      ),
    ).toBeNull();
  });

  test("uses the embedded tab identity for child runtimes", () => {
    expect(
      resolveRepositoryContextScope(
        true,
        "embedded-tab",
        "active-runtime",
        "active-tab",
      ),
    ).toBe("embedded-tab");
  });

  test("keeps the active-tab fallback for browser embedded mode", () => {
    expect(resolveRepositoryContextScope(true, null, null, "active-tab")).toBe(
      "active-tab",
    );
  });
});

import { describe, expect, it } from "vitest";
import { branchDeleteBlockReason } from "@/features/git/lib/branch-action-availability";

describe("branchDeleteBlockReason", () => {
  it("blocks deleting the current local branch", () => {
    expect(
      branchDeleteBlockReason(
        { name: "feature/current", is_remote: false, is_protected: false },
        "feature/current",
      ),
    ).toContain("current branch");
  });

  it("blocks deleting the protected remote default branch", () => {
    expect(
      branchDeleteBlockReason({
        name: "origin/main",
        is_remote: true,
        is_protected: true,
      }),
    ).toContain("protected branch");
  });

  it("blocks deleting the current branch upstream", () => {
    expect(
      branchDeleteBlockReason(
        {
          name: "origin/feature/current",
          is_remote: true,
          is_protected: false,
        },
        "feature/current",
        "origin/feature/current",
      ),
    ).toContain("upstream");
  });

  it("allows deleting an unrelated branch", () => {
    expect(
      branchDeleteBlockReason(
        {
          name: "origin/feature/done",
          is_remote: true,
          is_protected: false,
        },
        "main",
        "origin/main",
      ),
    ).toBeNull();
  });
});

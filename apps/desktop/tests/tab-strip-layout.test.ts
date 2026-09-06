import { describe, expect, test } from "vitest";
import { resolveTabStripLayout } from "../src/components/custom-title-bar/tab-strip-layout";

describe("resolveTabStripLayout", () => {
  test("keeps sparse tabs at their preferred width", () => {
    expect(
      resolveTabStripLayout({
        containerWidth: 800,
        controlsWidth: 38,
        tabCount: 2,
      }),
    ).toEqual({ railWidth: 484, tabWidth: 240 });
  });

  test("shares constrained space evenly between tabs", () => {
    expect(
      resolveTabStripLayout({
        containerWidth: 642,
        controlsWidth: 38,
        tabCount: 4,
      }),
    ).toEqual({ railWidth: 600, tabWidth: 147 });
  });

  test("never produces negative widths in extremely tight space", () => {
    expect(
      resolveTabStripLayout({
        containerWidth: 20,
        controlsWidth: 38,
        tabCount: 12,
      }),
    ).toEqual({ railWidth: 0, tabWidth: 0 });
  });

  test("returns an empty rail when there are no tabs", () => {
    expect(
      resolveTabStripLayout({
        containerWidth: 800,
        controlsWidth: 38,
        tabCount: 0,
      }),
    ).toEqual({ railWidth: 0, tabWidth: 0 });
  });
});

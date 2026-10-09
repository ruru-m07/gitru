import { describe, expect, it } from "vitest";
import { distribution, nearestRank } from "./performance-metrics";

describe("native performance metrics", () => {
  it("uses deterministic nearest-rank percentiles without interpolation", () => {
    const values = Array.from({ length: 100 }, (_, index) => index + 1);
    expect(nearestRank([...values].reverse(), 0.5)).toBe(50);
    expect(nearestRank(values, 0.95)).toBe(95);
    expect(nearestRank(values, 0.99)).toBe(99);
    expect(distribution([4, 1, 3, 2])).toEqual({
      count: 4,
      minimum: 1,
      p50: 2,
      p95: 4,
      p99: 4,
      maximum: 4,
    });
  });

  it("rejects empty, negative and non-finite observations", () => {
    expect(() => distribution([])).toThrow();
    expect(() => distribution([1, -1])).toThrow();
    expect(() => distribution([Number.NaN])).toThrow();
    expect(() => nearestRank([1], 0)).toThrow();
  });
});

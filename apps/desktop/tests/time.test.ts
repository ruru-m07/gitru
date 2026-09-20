import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";
import { compactTimeAgoFromUnixSeconds } from "../src/lib/time";

const MINUTE = 60;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;
const WEEK = 7 * DAY;
const YEAR = 365 * DAY;
const NOW_SECONDS = Date.UTC(2026, 2, 10, 17, 15) / 1000;

describe("compactTimeAgoFromUnixSeconds", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(NOW_SECONDS * 1000);
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  test.each([
    [NOW_SECONDS, "now"],
    [NOW_SECONDS - 59, "now"],
    [NOW_SECONDS - MINUTE, "1m"],
    [NOW_SECONDS - 5 * MINUTE, "5m"],
    [NOW_SECONDS - 12 * HOUR, "12h"],
    [NOW_SECONDS - 5 * DAY, "5d"],
    [NOW_SECONDS - 12 * WEEK, "12w"],
    [NOW_SECONDS - 2 * YEAR, "2y"],
    [NOW_SECONDS + 5 * DAY, "in 5d"],
  ])("formats %i as %s", (timestamp, expected) => {
    expect(compactTimeAgoFromUnixSeconds(timestamp)).toBe(expected);
  });
});

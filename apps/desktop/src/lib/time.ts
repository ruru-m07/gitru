import { format } from "timeago.js";

const MINUTE_IN_SECONDS = 60;
const HOUR_IN_SECONDS = 60 * MINUTE_IN_SECONDS;
const DAY_IN_SECONDS = 24 * HOUR_IN_SECONDS;
const WEEK_IN_SECONDS = 7 * DAY_IN_SECONDS;
const YEAR_IN_SECONDS = 365 * DAY_IN_SECONDS;

const COMPACT_RELATIVE_TIME_UNITS = [
  { seconds: YEAR_IN_SECONDS, suffix: "y" },
  { seconds: WEEK_IN_SECONDS, suffix: "w" },
  { seconds: DAY_IN_SECONDS, suffix: "d" },
  { seconds: HOUR_IN_SECONDS, suffix: "h" },
  { seconds: MINUTE_IN_SECONDS, suffix: "m" },
] as const;

/**
 * Convert UNIX seconds -> "x days ago"
 */
export function timeAgoFromUnixSeconds(unixSeconds: number): string {
  const date = unixSeconds * 1000;
  return format(date);
}

/**
 * Convert UNIX seconds -> compact relative time such as "5d" or "12w".
 */
export function compactTimeAgoFromUnixSeconds(unixSeconds: number): string {
  if (!Number.isFinite(unixSeconds)) return "—";

  const elapsedSeconds = Date.now() / 1000 - unixSeconds;
  const absoluteSeconds = Math.floor(Math.abs(elapsedSeconds));
  if (absoluteSeconds < MINUTE_IN_SECONDS) return "now";

  const unit =
    COMPACT_RELATIVE_TIME_UNITS.find(
      ({ seconds }) => absoluteSeconds >= seconds,
    ) ?? COMPACT_RELATIVE_TIME_UNITS.at(-1);
  if (!unit) return "now";

  const value = Math.max(1, Math.floor(absoluteSeconds / unit.seconds));
  const compactTime = `${value}${unit.suffix}`;
  return elapsedSeconds < 0 ? `in ${compactTime}` : compactTime;
}

/**
 * Convert UNIX seconds -> "dd/mm/yyyy, hh:mm am/pm"
 */
export function formatUnixSecondsToDateTime(unixSeconds: number): string {
  const date = new Date(unixSeconds * 1000);
  return date.toLocaleString();
}

import { format } from "timeago.js";

/**
 * Convert UNIX seconds -> "x days ago"
 */
export function timeAgoFromUnixSeconds(unixSeconds: number): string {
  const date = unixSeconds * 1000;
  return format(date);
}

/**
 * Convert UNIX seconds -> "dd/mm/yyyy, hh:mm am/pm"
 */
export function formatUnixSecondsToDateTime(unixSeconds: number): string {
  const date = new Date(unixSeconds * 1000);
  return date.toLocaleString();
}

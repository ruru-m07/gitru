import { spawnSync } from "node:child_process";

type ProcessRow = {
  pid: number;
  parent_pid: number;
  rss_bytes: number;
  command: string;
};

export type ProcessMemoryObservation = {
  mechanism: "ps-rss-kib";
  rust: ProcessRow;
  webview:
    | {
        state: "observed_descendants";
        aggregate_rss_bytes: number;
        processes: ProcessRow[];
        per_webview_attribution: "unavailable_shared_runtime";
      }
    | {
        state: "missing";
        reason: "no_proven_webkit_descendant";
        aggregate_rss_bytes: null;
        processes: [];
        per_webview_attribution: "unavailable";
      };
};

export function parseProcessMemory(
  output: string,
  processId: number,
): ProcessMemoryObservation {
  const rows = output
    .split("\n")
    .map((line) => line.match(/^\s*(\d+)\s+(\d+)\s+(\d+)\s+(.+?)\s*$/))
    .filter((match): match is RegExpMatchArray => match !== null)
    .map((match) => ({
      pid: Number(match[1]),
      parent_pid: Number(match[2]),
      rss_bytes: Number(match[3]) * 1024,
      command: match[4],
    }));
  const rust = rows.find((row) => row.pid === processId);
  if (!rust) throw new Error("Owned native process was absent from ps output");
  const descendants = new Set([processId]);
  let changed = true;
  while (changed) {
    changed = false;
    for (const row of rows) {
      if (!descendants.has(row.pid) && descendants.has(row.parent_pid)) {
        descendants.add(row.pid);
        changed = true;
      }
    }
  }
  const webviews = rows.filter(
    (row) =>
      descendants.has(row.pid) &&
      row.pid !== processId &&
      /(?:webkit|webcontent)/i.test(row.command),
  );
  return {
    mechanism: "ps-rss-kib",
    rust,
    webview: webviews.length
      ? {
          state: "observed_descendants",
          aggregate_rss_bytes: webviews.reduce(
            (total, row) => total + row.rss_bytes,
            0,
          ),
          processes: webviews,
          per_webview_attribution: "unavailable_shared_runtime",
        }
      : {
          state: "missing",
          reason: "no_proven_webkit_descendant",
          aggregate_rss_bytes: null,
          processes: [],
          per_webview_attribution: "unavailable",
        },
  };
}

export function observeProcessMemory(processId: number) {
  const result = spawnSync("ps", ["-axo", "pid=,ppid=,rss=,comm="], {
    encoding: "utf8",
    maxBuffer: 4 * 1024 * 1024,
  });
  if (result.status !== 0 || result.error)
    throw new Error("Could not read bounded process RSS observations");
  return parseProcessMemory(result.stdout, processId);
}

import type { PickaxeQuery } from "@gitru/commands";
import { cancelPickaxe, startPickaxe } from "@gitru/commands";
import { listen } from "@tauri-apps/api/event";
import { useCallback, useEffect, useRef, useState } from "react";
import type { PickaxeSearchOptions } from "@/lib/pickaxe-search-options";
import { useActiveRepositoryState } from "@/state/useActiveRepositoryState";

export type PickaxeHit = {
  commitHash: string;
  commitSubject: string;
  authorName: string;
  authorEmail: string;
  commitTime: number;
  filePath: string;
  fileNewPath?: string | null;
  matchLine?: number | null;
  patch?: string | null;
};

export type PickaxePhase =
  | "started"
  | "hit"
  | "progress"
  | "finished"
  | "error"
  | "cancelled";

export type PickaxeProgressEvent = {
  operationId: string;
  phase: PickaxePhase;
  hit?: PickaxeHit | null;
  commitsScanned: number;
  hitsFound: number;
  status?: string | null;
  error?: string | null;
};

export type PickaxeStatus =
  | "idle"
  | "running"
  | "finished"
  | "error"
  | "cancelled";

export type PickaxeFilters = PickaxeSearchOptions & {
  query: string;
  author: string;
  since: string;
  until: string;
  filePatterns: string;
};

function createOperationId() {
  if (typeof crypto !== "undefined" && "randomUUID" in crypto) {
    return crypto.randomUUID();
  }

  return `pickaxe-${Date.now()}`;
}

function parseFilePatterns(input: string) {
  return input
    .split(",")
    .map((pattern) => pattern.trim())
    .filter(Boolean);
}

function hitDedupeKey(hit: PickaxeHit) {
  return `${hit.commitHash}:${hit.filePath}`;
}

export function usePickaxe() {
  const repo = useActiveRepositoryState();
  const [hits, setHits] = useState<PickaxeHit[]>([]);
  const [commitsScanned, setCommitsScanned] = useState(0);
  const [hitsFound, setHitsFound] = useState(0);
  const [status, setStatus] = useState<PickaxeStatus>("idle");
  const [statusMessage, setStatusMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const operationIdRef = useRef<string | null>(null);

  useEffect(() => {
    let unlisten: (() => void) | undefined;

    void listen<PickaxeProgressEvent>("git://pickaxe-progress", (incoming) => {
      const payload = incoming.payload;
      const activeOperationId = operationIdRef.current;
      if (!activeOperationId || payload.operationId !== activeOperationId) {
        return;
      }

      setCommitsScanned(payload.commitsScanned);
      setHitsFound(payload.hitsFound);

      if (payload.status) {
        setStatusMessage(payload.status);
      }

      switch (payload.phase) {
        case "started":
          setStatus("running");
          setError(null);
          break;
        case "hit":
          if (payload.hit) {
            setHits((previous) => {
              const key = hitDedupeKey(payload.hit!);
              if (previous.some((hit) => hitDedupeKey(hit) === key)) {
                return previous;
              }

              return [...previous, payload.hit!];
            });
          }
          break;
        case "progress":
          setStatus("running");
          break;
        case "finished":
          setStatus("finished");
          break;
        case "error":
          setStatus("error");
          setError(payload.error ?? "Pickaxe failed");
          break;
        case "cancelled":
          setStatus("cancelled");
          break;
      }
    }).then((dispose) => {
      unlisten = dispose;
    });

    return () => {
      if (unlisten) {
        unlisten();
      }
    };
  }, []);

  const cancelSearch = useCallback(async () => {
    const operationId = operationIdRef.current;
    if (!operationId) {
      return;
    }

    try {
      await cancelPickaxe({ operationId });
    } catch {
      // Ignore cancellation errors when the backend already finished.
    }
  }, []);

  const startSearch = useCallback(
    async (filters: PickaxeFilters) => {
      if (!repo) {
        return;
      }

      const trimmedQuery = filters.query.trim();
      if (!trimmedQuery) {
        return;
      }

      await cancelSearch();

      const operationId = createOperationId();
      operationIdRef.current = operationId;
      setHits([]);
      setCommitsScanned(0);
      setHitsFound(0);
      setStatus("running");
      setStatusMessage("Starting pickaxe...");
      setError(null);

      const query: PickaxeQuery = {
        query: trimmedQuery,
        isRegex: filters.isRegex,
        matchCase: filters.matchCase,
        matchWholeWord: filters.matchWholeWord,
        author: filters.author.trim() || undefined,
        since: filters.since.trim() || undefined,
        until: filters.until.trim() || undefined,
        filePatterns: parseFilePatterns(filters.filePatterns),
        limit: 500,
        operationId,
      };

      try {
        await startPickaxe({
          contextId: repo.contextId,
          query,
        });
      } catch (searchError) {
        setStatus("error");
        setError(
          searchError instanceof Error
            ? searchError.message
            : "Failed to start pickaxe",
        );
      }
    },
    [cancelSearch, repo],
  );

  return {
    hits,
    commitsScanned,
    hitsFound,
    status,
    statusMessage,
    error,
    startSearch,
    cancelSearch,
  };
}
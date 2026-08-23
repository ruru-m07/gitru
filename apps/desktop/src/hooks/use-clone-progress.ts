import { listen } from "@tauri-apps/api/event";
import { useEffect, useMemo, useRef, useState } from "react";

export type CloneProgressEvent = {
  operationId: string;
  phase:
    | "Preparing"
    | "Started"
    | "Sideband"
    | "Transfer"
    | "Pack"
    | "RefUpdate"
    | "Message"
    | "Finished"
    | "Error"
    | "Cancelled";
  status?: string | null;
  line?: string | null;
  percent?: number | null;
  current?: number | null;
  total?: number | null;
  transfer?: {
    totalObjects: number;
    receivedObjects: number;
    indexedObjects: number;
    localObjects: number;
    totalDeltas: number;
    indexedDeltas: number;
    receivedBytes: number;
  } | null;
  pack?: {
    stage: string;
    current: number;
    total: number;
  } | null;
  refUpdate?: {
    refname: string;
    oldOid: string;
    newOid: string;
  } | null;
  errorKind?: string | null;
};

function mergeCloneEvent(
  previous: CloneProgressEvent | null,
  incoming: CloneProgressEvent,
): CloneProgressEvent {
  if (!previous || previous.operationId !== incoming.operationId) {
    return incoming;
  }

  // Preserve last known numeric progress when message events omit progress fields.
  return {
    ...previous,
    ...incoming,
    percent: incoming.percent ?? previous.percent,
    current: incoming.current ?? previous.current,
    total: incoming.total ?? previous.total,
    transfer: incoming.transfer ?? previous.transfer,
    pack: incoming.pack ?? previous.pack,
    refUpdate: incoming.refUpdate ?? previous.refUpdate,
  };
}

export function useCloneProgress(operationId?: string) {
  const [event, setEvent] = useState<CloneProgressEvent | null>(null);
  const operationIdRef = useRef<string | undefined>(operationId);

  useEffect(() => {
    operationIdRef.current = operationId;

    setEvent((previous) => {
      if (!operationId) {
        return previous;
      }

      if (previous?.operationId === operationId) {
        return previous;
      }

      return null;
    });
  }, [operationId]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;

    void listen<CloneProgressEvent>("git://clone-progress", (incoming) => {
      const payload = incoming.payload;
      const currentOperationId = operationIdRef.current;
      if (currentOperationId && payload.operationId !== currentOperationId) {
        return;
      }
      setEvent((previous) => mergeCloneEvent(previous, payload));
    }).then((dispose) => {
      unlisten = dispose;
    });

    return () => {
      if (unlisten) {
        unlisten();
      }
    };
  }, []);

  const status = useMemo(() => {
    if (!event) return "idle" as const;
    if (event.phase === "Error" || event.phase === "Cancelled") {
      return "error" as const;
    }
    if (event.phase === "Finished") return "finished" as const;
    return "running" as const;
  }, [event]);

  return {
    event,
    status,
  };
}

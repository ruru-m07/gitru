import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useEffect, useRef } from "react";

type DiffEvent = string;

export async function onDiffEvent(
  handler: (payload: DiffEvent) => void
): Promise<UnlistenFn> {
  return listen<DiffEvent>("diff_event", (event) => {
    handler(event.payload);
  });
}

export function useTauriEvent<T>(
  event: string,
  handler: (payload: T) => void
) {
  const handlerRef = useRef(handler);

  handlerRef.current = handler;

  useEffect(() => {
    let unlisten: (() => void) | null = null;

    listen<T>(event, (e) => {
      handlerRef.current(e.payload);
    }).then((dispose) => {
      unlisten = dispose;
    });

    return () => {
      unlisten?.();
    };
  }, [event]);
}

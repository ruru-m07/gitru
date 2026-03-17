import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useEffect } from "react";

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
  useEffect(() => {
    let unlisten: (() => void) | undefined;

    listen<T>(event, (e) => handler(e.payload)).then((dispose) => {
      unlisten = dispose;
    });

    return () => {
      unlisten?.();
    };
  }, [event, handler]);
}
import type { Event } from "@tauri-apps/api/event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { collaboration } from "./index";

const { invoke, transformCallback, unregisterListener } = vi.hoisted(() => ({
  invoke: vi.fn(),
  transformCallback: vi.fn((_callback: unknown) => 42),
  unregisterListener: vi.fn(),
}));
// Use the installed Webview/event/core APIs. Only their native command boundary
// and callback registration are doubled; this does not model native routing.

let metadata: {
  currentWindow: { label: string };
  currentWebview: { label: string };
};
beforeEach(() => {
  metadata = {
    currentWindow: { label: "fixture-window" },
    currentWebview: { label: "main" },
  };
  vi.stubGlobal("window", {
    __TAURI_INTERNALS__: { metadata, invoke, transformCallback },
    __TAURI_EVENT_PLUGIN_INTERNALS__: { unregisterListener },
  });
  invoke.mockImplementation(async (command: string) => {
    if (command === "plugin:event|listen") return 17;
    if (command === "plugin:event|unlisten") return;
    throw new Error("Unexpected native command in the event boundary test");
  });
});
afterEach(() => {
  invoke.mockReset();
  transformCallback.mockClear();
  unregisterListener.mockClear();
  vi.unstubAllGlobals();
});

describe("ordinary collaboration revision event boundary", () => {
  it.each([
    "main",
    "tab-webview:fixture-child",
  ])("registers only the actual %s webview and preserves callback/unsubscribe", async (label) => {
    metadata.currentWebview.label = label;
    const changed = vi.fn();
    const stop = await collaboration.transport.listen(changed);
    expect(invoke).toHaveBeenCalledExactlyOnceWith(
      "plugin:event|listen",
      {
        event: "gitru:collaboration-change",
        target: { kind: "Webview", label },
        handler: 42,
      },
      undefined,
    );
    expect(transformCallback).toHaveBeenCalledExactlyOnceWith(changed, false);
    // Forward the actual API callback unchanged; durable catchup interprets
    // local snapshots, rather than trusting the wake event's revision.
    const event: Event<{ revision: string }> = {
      event: "gitru:collaboration-change",
      id: 17,
      payload: { revision: "9007199254740993" },
    };
    const handler = transformCallback.mock.calls[0][0] as (
      event: Event<{ revision: string }>,
    ) => void;
    handler(event);
    expect(changed).toHaveBeenCalledExactlyOnceWith(event);
    await stop();
    expect(unregisterListener).toHaveBeenCalledExactlyOnceWith(
      "gitru:collaboration-change",
      17,
    );
    expect(invoke).toHaveBeenLastCalledWith(
      "plugin:event|unlisten",
      {
        event: "gitru:collaboration-change",
        eventId: 17,
      },
      undefined,
    );
    expect(invoke).toHaveBeenCalledTimes(2);
  });

  it("propagates listener registration failure without a global fallback", async () => {
    const failure = { code: "permission_denied" };
    invoke.mockRejectedValueOnce(failure);
    await expect(collaboration.transport.listen(vi.fn())).rejects.toEqual(
      failure,
    );
    expect(invoke).toHaveBeenCalledTimes(1);
    expect(unregisterListener).not.toHaveBeenCalled();
  });
});

import { describe, expect, it } from "vitest";
import {
  type CdpTarget,
  classifyGitruTargets,
  diagnosticFromCdpEvent,
} from "./cdp";

const target = (id: string, url: string, type = "page"): CdpTarget => ({
  id,
  title: id,
  type,
  url,
  webSocketDebuggerUrl: `ws://127.0.0.1/devtools/page/${id}`,
});

describe("classifyGitruTargets", () => {
  it("separates the host shell from every embedded child webview", () => {
    const host = target("host", "http://tauri.localhost/app/git");
    const childOne = target(
      "child-one",
      "http://tauri.localhost/app/git?embedded=1",
    );
    const childTwo = target(
      "child-two",
      "http://tauri.localhost/app/inbox?embedded=true",
    );

    expect(classifyGitruTargets([childTwo, host, childOne])).toEqual({
      children: [childTwo, childOne],
      host,
      other: [],
    });
  });

  it("does not mistake DevTools or unrelated pages for Gitru webviews", () => {
    const devtools = target("devtools", "devtools://devtools/bundled/");
    const external = target("external", "https://example.com/app/git");
    const worker = target("worker", "http://tauri.localhost/app/git", "worker");

    expect(classifyGitruTargets([devtools, external, worker])).toEqual({
      children: [],
      host: undefined,
      other: [devtools, external, worker],
    });
  });
});

describe("diagnosticFromCdpEvent", () => {
  it("preserves exception descriptions and their source location", () => {
    expect(
      diagnosticFromCdpEvent(
        {
          method: "Runtime.exceptionThrown",
          params: {
            exceptionDetails: {
              columnNumber: 8,
              exception: {
                description:
                  "Error: failed to refresh repository\n    at refresh",
                type: "object",
              },
              lineNumber: 20,
              text: "Uncaught (in promise)",
              timestamp: 123,
              url: "http://tauri.localhost/assets/app.js",
            },
          },
        },
        "http://tauri.localhost/app/git?embedded=1",
      ),
    ).toEqual({
      level: "exception",
      message: "Error: failed to refresh repository\n    at refresh",
      source: "http://tauri.localhost/assets/app.js:21:9",
      timestamp: 123,
    });
  });

  it("serializes console arguments and maps failed assertions to errors", () => {
    expect(
      diagnosticFromCdpEvent(
        {
          method: "Runtime.consoleAPICalled",
          params: {
            args: [
              { type: "string", value: "request failed" },
              { type: "object", value: { status: 500 } },
              { type: "number", unserializableValue: "NaN" },
              { description: "Error: unavailable", type: "object" },
            ],
            stackTrace: {
              callFrames: [
                {
                  columnNumber: 6,
                  lineNumber: 4,
                  url: "http://tauri.localhost/assets/app.js",
                },
              ],
            },
            timestamp: 456,
            type: "assert",
          },
        },
        "http://tauri.localhost/app/git?embedded=1",
      ),
    ).toEqual({
      level: "error",
      message: 'request failed {"status":500} NaN Error: unavailable',
      source: "http://tauri.localhost/assets/app.js:5:7",
      timestamp: 456,
    });
  });

  it("uses the failing resource URL for log entries", () => {
    expect(
      diagnosticFromCdpEvent(
        {
          method: "Log.entryAdded",
          params: {
            entry: {
              level: "error",
              source: "network",
              text: "Failed to load resource",
              timestamp: 789,
              url: "http://127.0.0.1:9/latest.json",
            },
          },
        },
        "http://tauri.localhost/app",
      ),
    ).toEqual({
      level: "error",
      message: "Failed to load resource",
      source: "http://127.0.0.1:9/latest.json",
      timestamp: 789,
    });
  });
});

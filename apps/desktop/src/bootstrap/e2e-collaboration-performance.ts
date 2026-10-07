import { collaboration } from "@gitru/collaboration-client";
import {
  itemQueryOptions,
  itemsQueryOptions,
} from "@gitru/collaboration-client/react";
import type { HarnessViewManifest, RemoteAccount } from "@gitru/commands";
import type { QueryClient } from "@tanstack/react-query";
import type { HarnessPerformanceView } from "../../e2e/protocol/collaboration-harness";

const ACCOUNT_ID = "ruru103:primary";
const ITEM_ID = "github:pull:ruru125:primary:4999";
const FIRST_TITLE = "RURU-125 cached pull 4999 primary repository 4";
const STEP_TIMEOUT_MS = 10_000;

type Sample = { duration_ms: number; payload_bytes: number | null };

function payloadBytes(value: unknown) {
  return new TextEncoder().encode(JSON.stringify(value)).byteLength;
}

async function painted() {
  await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
  await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
}

async function waitFor<T>(read: () => T | null): Promise<T> {
  const current = read();
  if (current !== null) {
    await painted();
    return current;
  }
  return new Promise<T>((resolve, reject) => {
    const timeout = window.setTimeout(() => {
      observer.disconnect();
      reject(new Error("Bounded performance content did not render"));
    }, STEP_TIMEOUT_MS);
    const observer = new MutationObserver(() => {
      const value = read();
      if (value === null) return;
      observer.disconnect();
      window.clearTimeout(timeout);
      void painted().then(() => resolve(value), reject);
    });
    observer.observe(document.body, {
      childList: true,
      subtree: true,
      characterData: true,
      attributes: true,
    });
  });
}

function buttonWithText(container: HTMLElement, text: string) {
  return (
    Array.from(container.querySelectorAll<HTMLButtonElement>("button")).find(
      (button) => button.textContent?.includes(text),
    ) ?? null
  );
}

function usefulTitle(container: HTMLElement, title: string) {
  const button = buttonWithText(container, title);
  return button && !button.hidden ? button : null;
}

function searchInput(container: HTMLElement) {
  return container.querySelector<HTMLInputElement>(
    'input[aria-label="Search saved pull requests"]',
  );
}

function setInput(input: HTMLInputElement, value: string) {
  const setter = Object.getOwnPropertyDescriptor(
    HTMLInputElement.prototype,
    "value",
  )?.set;
  if (!setter) throw new Error("Native input setter is unavailable");
  setter.call(input, value);
  input.dispatchEvent(new InputEvent("input", { bubbles: true, data: value }));
}

function elapsed(started: number): Sample {
  return { duration_ms: performance.now() - started, payload_bytes: null };
}

function searchTitle(index: number) {
  return `RURU-125 cached pull ${4900 + index} primary repository 4 needle${index
    .toString()
    .padStart(2, "0")}`;
}

function listQuery(search: string | null = null) {
  return {
    kind: "pull_request" as const,
    repository_id: null,
    state: "open",
    search,
    cursor: null,
    limit: 50,
  };
}

async function directSdkSamples(account: RemoteAccount, sampleCount: 10 | 30) {
  const lists: Sample[] = [];
  const searches: Sample[] = [];
  const details: Sample[] = [];
  for (let index = 0; index < sampleCount; index += 1) {
    let started = performance.now();
    const page = await collaboration.forAccount(account).items(listQuery());
    lists.push({
      duration_ms: performance.now() - started,
      payload_bytes: payloadBytes(page),
    });
    started = performance.now();
    const search = await collaboration
      .forAccount(account)
      .items(listQuery(`needle${index.toString().padStart(2, "0")}`));
    searches.push({
      duration_ms: performance.now() - started,
      payload_bytes: payloadBytes(search),
    });
    started = performance.now();
    const detail = await collaboration.forAccount(account).item(ITEM_ID);
    details.push({
      duration_ms: performance.now() - started,
      payload_bytes: payloadBytes(detail),
    });
  }
  return { lists, searches, details };
}

async function memorySamples(
  client: QueryClient,
  account: RemoteAccount,
  sampleCount: 10 | 30,
) {
  const list = itemsQueryOptions(account, listQuery());
  const detail = itemQueryOptions(account, ITEM_ID);
  await client.fetchQuery(list);
  await client.fetchQuery(detail);
  const searches = Array.from({ length: sampleCount }, (_, index) =>
    itemsQueryOptions(
      account,
      listQuery(`needle${index.toString().padStart(2, "0")}`),
    ),
  );
  await Promise.all(searches.map((query) => client.fetchQuery(query)));
  const lists: Sample[] = [];
  const searchSamples: Sample[] = [];
  const details: Sample[] = [];
  for (let index = 0; index < sampleCount; index += 1) {
    let started = performance.now();
    const page = await client.fetchQuery(list);
    lists.push({
      duration_ms: performance.now() - started,
      payload_bytes: payloadBytes(page),
    });
    started = performance.now();
    const search = await client.fetchQuery(searches[index]);
    searchSamples.push({
      duration_ms: performance.now() - started,
      payload_bytes: payloadBytes(search),
    });
    started = performance.now();
    const item = await client.fetchQuery(detail);
    details.push({
      duration_ms: performance.now() - started,
      payload_bytes: payloadBytes(item),
    });
  }
  return { lists, searches: searchSamples, details };
}

export async function runCollaborationPerformance({
  client,
  container,
  manifest,
  phase,
  sampleCount,
  showWorkspace,
  hideWorkspace,
}: {
  client: QueryClient;
  container: HTMLElement;
  manifest: HarnessViewManifest;
  phase: "warm" | "restart";
  sampleCount: 10 | 30;
  showWorkspace: () => void;
  hideWorkspace: () => void;
}): Promise<HarnessPerformanceView> {
  const account = (await collaboration.accounts()).accounts.find(
    (candidate) => candidate.id === ACCOUNT_ID,
  );
  if (
    !account ||
    account.authorization_epoch !== "1" ||
    account.state !== "active"
  )
    throw new Error("Performance account does not match the fixed fixture");

  const listUi: Sample[] = [];
  hideWorkspace();
  await painted();
  client.removeQueries({ queryKey: ["collaboration"] });
  let started = performance.now();
  showWorkspace();
  await waitFor(() => usefulTitle(container, FIRST_TITLE));
  listUi.push(elapsed(started));
  const firstUsefulEpochMs = performance.timeOrigin + performance.now();
  for (let index = 1; index < sampleCount; index += 1) {
    hideWorkspace();
    await painted();
    client.removeQueries({ queryKey: ["collaboration"] });
    started = performance.now();
    showWorkspace();
    await waitFor(() => usefulTitle(container, FIRST_TITLE));
    listUi.push(elapsed(started));
  }

  const input = await waitFor(() => searchInput(container));
  const searchUi: Sample[] = [];
  for (let index = 0; index < sampleCount; index += 1) {
    const term = `needle${index.toString().padStart(2, "0")}`;
    started = performance.now();
    setInput(input, term);
    await waitFor(() => usefulTitle(container, searchTitle(index)));
    searchUi.push(elapsed(started));
  }
  setInput(input, "");
  await waitFor(() => usefulTitle(container, FIRST_TITLE));

  const detailUi: Sample[] = [];
  for (let index = 0; index < sampleCount; index += 1) {
    client.removeQueries({
      predicate: (query) =>
        query.queryKey[0] === "collaboration" &&
        JSON.stringify(query.queryKey).includes(ITEM_ID),
    });
    const item = await waitFor(() => usefulTitle(container, FIRST_TITLE));
    started = performance.now();
    item.click();
    await waitFor(() => {
      const detail = container.querySelector<HTMLElement>(
        'article[aria-label="Saved item detail"]',
      );
      return detail?.textContent?.includes(FIRST_TITLE) ? detail : null;
    });
    detailUi.push(elapsed(started));
    const back = buttonWithText(container, "Back to list");
    if (!back) throw new Error("Saved item detail did not expose list return");
    back.click();
    await waitFor(() => usefulTitle(container, FIRST_TITLE));
  }

  const ipc = await directSdkSamples(account, sampleCount);
  const memory = await memorySamples(client, account, sampleCount);
  const role = manifest.role;
  if (role !== "main" && role !== "concurrent_child")
    throw new Error("Performance view is not an issued retained surface");
  return {
    phase,
    webview_label: manifest.webview_label,
    role,
    sample_count: sampleCount,
    account_id: ACCOUNT_ID,
    first_useful_epoch_ms: firstUsefulEpochMs,
    navigation_to_first_useful_ms: firstUsefulEpochMs - performance.timeOrigin,
    exact_first_title: FIRST_TITLE,
    cases: [
      {
        name: "list_local_ipc",
        boundary: "react_useful_content",
        samples: listUi,
      },
      {
        name: "search_local_ipc",
        boundary: "react_useful_content",
        samples: searchUi,
      },
      {
        name: "detail_local_ipc",
        boundary: "react_useful_content",
        samples: detailUi,
      },
      { name: "list_local_ipc", boundary: "sdk_ipc", samples: ipc.lists },
      { name: "search_local_ipc", boundary: "sdk_ipc", samples: ipc.searches },
      { name: "detail_local_ipc", boundary: "sdk_ipc", samples: ipc.details },
      {
        name: "list_memory_hit",
        boundary: "query_memory",
        samples: memory.lists,
      },
      {
        name: "search_memory_hit",
        boundary: "query_memory",
        samples: memory.searches,
      },
      {
        name: "detail_memory_hit",
        boundary: "query_memory",
        samples: memory.details,
      },
    ],
  };
}

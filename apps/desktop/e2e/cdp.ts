import { writeFileSync } from "node:fs";

type CdpError = {
  code: number;
  message: string;
};

type CdpResponse = {
  id?: number;
  method?: string;
  params?: unknown;
  result?: unknown;
  error?: CdpError;
};

type CdpRemoteObject = {
  description?: string;
  type?: string;
  unserializableValue?: string;
  value?: unknown;
};

type CdpCallFrame = {
  columnNumber?: number;
  lineNumber?: number;
  url?: string;
};

type PendingRequest = {
  method: string;
  reject: (error: Error) => void;
  resolve: (result: unknown) => void;
  timeout: ReturnType<typeof setTimeout>;
};

export type CdpTarget = {
  description?: string;
  devtoolsFrontendUrl?: string;
  id: string;
  title: string;
  type: string;
  url: string;
  webSocketDebuggerUrl?: string;
};

export type CdpVersion = {
  Browser?: string;
  "Protocol-Version"?: string;
  "User-Agent"?: string;
  "V8-Version"?: string;
  webSocketDebuggerUrl?: string;
};

export type GitruTargets = {
  children: CdpTarget[];
  host: CdpTarget | undefined;
  other: CdpTarget[];
};

export type FrontendDiagnostic = {
  level: string;
  message: string;
  source: string;
  timestamp: number;
};

const serializeRemoteObject = (object: CdpRemoteObject): string => {
  if (Object.hasOwn(object, "value")) {
    if (typeof object.value === "string") return object.value;
    try {
      const serialized = JSON.stringify(object.value);
      if (serialized !== undefined) return serialized;
    } catch {
      // Fall through to the CDP description for non-serializable values.
    }
  }

  return (
    object.unserializableValue ??
    object.description ??
    `[${object.type ?? "unknown"}]`
  );
};

const diagnosticSource = (fallback: string, frame?: CdpCallFrame): string => {
  if (!frame?.url) return fallback;
  const line = frame.lineNumber;
  const column = frame.columnNumber;
  if (line === undefined) return frame.url;
  if (column === undefined) return `${frame.url}:${line + 1}`;
  return `${frame.url}:${line + 1}:${column + 1}`;
};

export function diagnosticFromCdpEvent(
  message: Pick<CdpResponse, "method" | "params">,
  targetUrl: string,
): FrontendDiagnostic | undefined {
  if (message.method === "Runtime.exceptionThrown") {
    const params = message.params as {
      exceptionDetails?: {
        columnNumber?: number;
        exception?: CdpRemoteObject;
        lineNumber?: number;
        stackTrace?: { callFrames?: CdpCallFrame[] };
        text?: string;
        timestamp?: number;
        url?: string;
      };
    };
    const details = params.exceptionDetails;
    const exception = details?.exception;
    const frame =
      details?.url || details?.lineNumber !== undefined
        ? {
            columnNumber: details.columnNumber,
            lineNumber: details.lineNumber,
            url: details.url,
          }
        : details?.stackTrace?.callFrames?.[0];
    return {
      level: "exception",
      message: exception
        ? serializeRemoteObject(exception)
        : (details?.text ?? "Uncaught frontend exception"),
      source: diagnosticSource(targetUrl, frame),
      timestamp: details?.timestamp ?? Date.now(),
    };
  }

  if (message.method === "Runtime.consoleAPICalled") {
    const params = message.params as {
      args?: CdpRemoteObject[];
      stackTrace?: { callFrames?: CdpCallFrame[] };
      timestamp?: number;
      type?: string;
    };
    const type = params.type ?? "log";
    const frame = params.stackTrace?.callFrames?.[0];
    return {
      level: type === "assert" ? "error" : type,
      message:
        params.args?.map(serializeRemoteObject).join(" ") ??
        `console.${type} called without arguments`,
      source: diagnosticSource(targetUrl, frame),
      timestamp: params.timestamp ?? Date.now(),
    };
  }

  if (message.method === "Log.entryAdded") {
    const params = message.params as {
      entry?: {
        level?: string;
        source?: string;
        text?: string;
        timestamp?: number;
        url?: string;
      };
    };
    const entry = params.entry;
    if (!entry) return undefined;
    return {
      level: entry.level ?? "unknown",
      message: entry.text ?? "",
      source: entry.url ?? entry.source ?? targetUrl,
      timestamp: entry.timestamp ?? Date.now(),
    };
  }

  return undefined;
}

const DEFAULT_REQUEST_TIMEOUT_MS = 20_000;

const isTauriAppUrl = (value: string): boolean => {
  try {
    const url = new URL(value);
    return (
      (url.protocol === "http:" || url.protocol === "https:") &&
      url.hostname === "tauri.localhost"
    );
  } catch {
    return false;
  }
};

export function classifyGitruTargets(targets: CdpTarget[]): GitruTargets {
  const result: GitruTargets = {
    children: [],
    host: undefined,
    other: [],
  };

  for (const target of targets) {
    if (target.type !== "page" || !isTauriAppUrl(target.url)) {
      result.other.push(target);
      continue;
    }

    const url = new URL(target.url);
    const embedded = url.searchParams.get("embedded");
    if (embedded === "1" || embedded === "true") {
      result.children.push(target);
    } else if (!result.host) {
      result.host = target;
    } else {
      result.other.push(target);
    }
  }

  return result;
}

async function responseJson<T>(
  response: Response,
  description: string,
): Promise<T> {
  if (!response.ok) {
    throw new Error(`${description} returned HTTP ${response.status}`);
  }
  return (await response.json()) as T;
}

export async function listCdpTargets(port: number): Promise<CdpTarget[]> {
  const response = await fetch(`http://127.0.0.1:${port}/json/list`);
  return responseJson<CdpTarget[]>(response, "CEF target discovery");
}

export async function readCdpVersion(port: number): Promise<CdpVersion> {
  const response = await fetch(`http://127.0.0.1:${port}/json/version`);
  return responseJson<CdpVersion>(response, "CEF version discovery");
}

export async function waitFor<T>(
  read: () => Promise<T>,
  predicate: (value: T) => boolean,
  description: string,
  options: { intervalMs?: number; timeoutMs?: number } = {},
): Promise<T> {
  const intervalMs = options.intervalMs ?? 150;
  const timeoutMs = options.timeoutMs ?? 30_000;
  const deadline = Date.now() + timeoutMs;
  let latest: T | undefined;
  let latestError: unknown;

  while (Date.now() < deadline) {
    try {
      latest = await read();
      if (predicate(latest)) return latest;
      latestError = undefined;
    } catch (error) {
      latestError = error;
    }
    await new Promise((resolve) => setTimeout(resolve, intervalMs));
  }

  const detail = latestError
    ? latestError instanceof Error
      ? latestError.message
      : String(latestError)
    : JSON.stringify(latest);
  throw new Error(
    `Timed out waiting for ${description}; latest value: ${detail}`,
  );
}

const messageText = async (data: unknown): Promise<string> => {
  if (typeof data === "string") return data;
  if (data instanceof Blob) return data.text();
  if (data instanceof ArrayBuffer) return new TextDecoder().decode(data);
  if (ArrayBuffer.isView(data)) {
    return new TextDecoder().decode(data);
  }
  return String(data);
};

export class CdpClient {
  readonly diagnostics: FrontendDiagnostic[] = [];
  readonly target: CdpTarget;

  private readonly pending = new Map<number, PendingRequest>();
  private readonly socket: WebSocket;
  private nextId = 1;

  private constructor(target: CdpTarget, socket: WebSocket) {
    this.target = target;
    this.socket = socket;
    socket.binaryType = "arraybuffer";
    socket.addEventListener("message", (event) => {
      void this.handleMessage(event.data);
    });
    socket.addEventListener("close", () => {
      this.rejectPending(
        new Error(`CEF closed the CDP connection for ${this.target.url}`),
      );
    });
  }

  static async connect(target: CdpTarget): Promise<CdpClient> {
    if (!target.webSocketDebuggerUrl) {
      throw new Error(`CEF target ${target.id} has no WebSocket debugger URL`);
    }

    const socket = new WebSocket(target.webSocketDebuggerUrl);
    await new Promise<void>((resolve, reject) => {
      const handleOpen = () => {
        socket.removeEventListener("error", handleError);
        resolve();
      };
      const handleError = () => {
        socket.removeEventListener("open", handleOpen);
        reject(new Error(`Could not connect to CEF target ${target.url}`));
      };
      socket.addEventListener("open", handleOpen, { once: true });
      socket.addEventListener("error", handleError, { once: true });
    });

    const client = new CdpClient(target, socket);
    await Promise.all([
      client.send("Runtime.enable"),
      client.send("Page.enable"),
      client.send("Log.enable"),
    ]);
    return client;
  }

  async close(): Promise<void> {
    if (
      this.socket.readyState === WebSocket.CLOSING ||
      this.socket.readyState === WebSocket.CLOSED
    ) {
      return;
    }

    await new Promise<void>((resolve) => {
      this.socket.addEventListener("close", () => resolve(), { once: true });
      this.socket.close();
      setTimeout(resolve, 1_000);
    });
  }

  async send<T = unknown>(
    method: string,
    params: Record<string, unknown> = {},
    timeoutMs = DEFAULT_REQUEST_TIMEOUT_MS,
  ): Promise<T> {
    if (this.socket.readyState !== WebSocket.OPEN) {
      throw new Error(`CDP target ${this.target.url} is not connected`);
    }

    const id = this.nextId;
    this.nextId += 1;

    return new Promise<T>((resolve, reject) => {
      const timeout = setTimeout(() => {
        this.pending.delete(id);
        reject(new Error(`CDP ${method} timed out after ${timeoutMs}ms`));
      }, timeoutMs);

      this.pending.set(id, {
        method,
        reject,
        resolve: resolve as (result: unknown) => void,
        timeout,
      });
      this.socket.send(JSON.stringify({ id, method, params }));
    });
  }

  async evaluate<T>(expression: string): Promise<T> {
    const response = await this.send<{
      exceptionDetails?: {
        text?: string;
        exception?: { description?: string };
      };
      result: { description?: string; value?: T };
    }>("Runtime.evaluate", {
      awaitPromise: true,
      expression,
      returnByValue: true,
      userGesture: true,
    });

    if (response.exceptionDetails) {
      throw new Error(
        response.exceptionDetails.exception?.description ??
          response.exceptionDetails.text ??
          "CEF Runtime.evaluate failed",
      );
    }
    return response.result.value as T;
  }

  async isVisibleCss(selector: string): Promise<boolean> {
    return this.evaluate<boolean>(`(() => {
      const element = document.querySelector(${JSON.stringify(selector)});
      if (!(element instanceof HTMLElement)) return false;
      const style = getComputedStyle(element);
      return element.getClientRects().length > 0 && style.visibility !== "hidden" && style.display !== "none";
    })()`);
  }

  async isVisibleXpath(xpath: string): Promise<boolean> {
    return this.evaluate<boolean>(`(() => {
      const element = document.evaluate(
        ${JSON.stringify(xpath)},
        document,
        null,
        XPathResult.FIRST_ORDERED_NODE_TYPE,
      ).singleNodeValue;
      if (!(element instanceof HTMLElement)) return false;
      const style = getComputedStyle(element);
      return element.getClientRects().length > 0 && style.visibility !== "hidden" && style.display !== "none";
    })()`);
  }

  async hasCssWithin(rootSelector: string, selector: string): Promise<boolean> {
    return this.evaluate<boolean>(`(() => {
      const root = document.querySelector(${JSON.stringify(rootSelector)});
      return root?.querySelector(${JSON.stringify(selector)}) !== null && root !== null;
    })()`);
  }

  async waitForVisibleCss(selector: string, timeoutMs = 30_000): Promise<void> {
    await waitFor(
      () => this.isVisibleCss(selector),
      Boolean,
      `${selector} to be visible in ${this.target.url}`,
      { timeoutMs },
    );
  }

  async waitForVisibleXpath(xpath: string, timeoutMs = 30_000): Promise<void> {
    await waitFor(
      () => this.isVisibleXpath(xpath),
      Boolean,
      `${xpath} to be visible in ${this.target.url}`,
      { timeoutMs },
    );
  }

  async waitForAbsentCss(selector: string, timeoutMs = 30_000): Promise<void> {
    await waitFor(
      () =>
        this.evaluate<boolean>(
          `document.querySelector(${JSON.stringify(selector)}) === null`,
        ),
      Boolean,
      `${selector} to be absent from ${this.target.url}`,
      { timeoutMs },
    );
  }

  async clickCss(selector: string): Promise<void> {
    await this.clickExpression(
      `document.querySelector(${JSON.stringify(selector)})`,
      selector,
    );
  }

  async clickXpath(xpath: string): Promise<void> {
    await this.clickExpression(
      `document.evaluate(${JSON.stringify(xpath)}, document, null, XPathResult.FIRST_ORDERED_NODE_TYPE).singleNodeValue`,
      xpath,
    );
  }

  async clickXpathWithin(rootSelector: string, xpath: string): Promise<void> {
    await this.clickExpression(
      `(() => {
        const root = document.querySelector(${JSON.stringify(rootSelector)});
        if (!root) return null;
        return document.evaluate(${JSON.stringify(xpath)}, root, null, XPathResult.FIRST_ORDERED_NODE_TYPE).singleNodeValue;
      })()`,
      `${xpath} within ${rootSelector}`,
    );
  }

  async setInputCss(selector: string, value: string): Promise<void> {
    await this.waitForVisibleCss(selector);
    const updated = await this.evaluate<boolean>(`(() => {
      const input = document.querySelector(${JSON.stringify(selector)});
      if (!(input instanceof HTMLInputElement) && !(input instanceof HTMLTextAreaElement)) return false;
      const prototype = input instanceof HTMLTextAreaElement ? HTMLTextAreaElement.prototype : HTMLInputElement.prototype;
      const setter = Object.getOwnPropertyDescriptor(prototype, "value")?.set;
      if (!setter) return false;
      input.focus();
      setter.call(input, ${JSON.stringify(value)});
      input.dispatchEvent(new Event("input", { bubbles: true }));
      input.dispatchEvent(new Event("change", { bubbles: true }));
      return input.value === ${JSON.stringify(value)};
    })()`);
    if (!updated)
      throw new Error(`Could not set ${selector} in ${this.target.url}`);
  }

  async textCss(selector: string): Promise<string | null> {
    return this.evaluate<string | null>(
      `document.querySelector(${JSON.stringify(selector)})?.textContent ?? null`,
    );
  }

  async pressKey(key: string, code = key): Promise<void> {
    await this.send("Input.dispatchKeyEvent", {
      code,
      key,
      type: "keyDown",
      windowsVirtualKeyCode: key === "Escape" ? 27 : undefined,
    });
    await this.send("Input.dispatchKeyEvent", {
      code,
      key,
      type: "keyUp",
      windowsVirtualKeyCode: key === "Escape" ? 27 : undefined,
    });
  }

  async captureScreenshot(path: string): Promise<void> {
    const response = await this.send<{ data: string }>(
      "Page.captureScreenshot",
      {
        captureBeyondViewport: false,
        format: "png",
        fromSurface: true,
      },
    );
    writeFileSync(path, Buffer.from(response.data, "base64"));
  }

  private async clickExpression(
    elementExpression: string,
    description: string,
  ): Promise<void> {
    const point = await this.evaluate<{ x: number; y: number } | null>(`(() => {
      const element = ${elementExpression};
      if (!(element instanceof HTMLElement)) return null;
      const rect = element.getBoundingClientRect();
      const style = getComputedStyle(element);
      if (rect.width <= 0 || rect.height <= 0 || style.visibility === "hidden" || style.display === "none") return null;
      element.scrollIntoView({ block: "center", inline: "center" });
      const updated = element.getBoundingClientRect();
      return { x: updated.left + updated.width / 2, y: updated.top + updated.height / 2 };
    })()`);
    if (!point) {
      throw new Error(`${description} is not clickable in ${this.target.url}`);
    }

    await this.send("Input.dispatchMouseEvent", {
      button: "left",
      clickCount: 1,
      type: "mousePressed",
      x: point.x,
      y: point.y,
    });
    await this.send("Input.dispatchMouseEvent", {
      button: "left",
      clickCount: 1,
      type: "mouseReleased",
      x: point.x,
      y: point.y,
    });
  }

  private async handleMessage(data: unknown): Promise<void> {
    const text = await messageText(data);
    const message = JSON.parse(text) as CdpResponse;

    if (typeof message.id === "number") {
      const pending = this.pending.get(message.id);
      if (!pending) return;
      clearTimeout(pending.timeout);
      this.pending.delete(message.id);
      if (message.error) {
        pending.reject(
          new Error(
            `CDP ${pending.method} failed (${message.error.code}): ${message.error.message}`,
          ),
        );
      } else {
        pending.resolve(message.result);
      }
      return;
    }

    const diagnostic = diagnosticFromCdpEvent(message, this.target.url);
    if (diagnostic) this.diagnostics.push(diagnostic);
  }

  private rejectPending(error: Error): void {
    for (const pending of this.pending.values()) {
      clearTimeout(pending.timeout);
      pending.reject(error);
    }
    this.pending.clear();
  }
}

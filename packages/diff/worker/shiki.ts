/// <reference lib="webworker" />

import { LRUMap } from "lru_map";

import {
  type BundledLanguage,
  bundledLanguages,
  createHighlighter,
  createJavaScriptRegexEngine,
  type HighlighterCore,
} from "shiki";
import type {
  HighlightErrorResponse,
  HighlightRequest,
  HighlightResponse,
} from "../types";

export const AttachedLanguages: Set<string> = new Set();
export const AttachedThemes: Set<string> = new Set();

class ShikiWorker {
  private ready = false;
  private highlighter!: HighlighterCore;

  private fileCache: LRUMap<string, string>;
  private diffCache: LRUMap<string, string>;

  constructor() {
    this.fileCache = new LRUMap(100);
    this.diffCache = new LRUMap(100);

    (async () => {
      await this.initializeHighlighter();
    })();

    this.ready = true;
  }

  private hash(code: string) {
    let h = 2166136261;
    for (let i = 0; i < code.length; i++) {
      h ^= code.charCodeAt(i);
      h += (h << 1) + (h << 4) + (h << 7) + (h << 8) + (h << 24);
    }
    return h >>> 0;
  }

  async highlight(req: HighlightRequest): Promise<HighlightResponse> {
    const key = `${this.hash(req.code)}:${req.language}:${req.theme}`;

    const cached = this.fileCache.get(key);

    if (cached) {
      return { id: req.id, html: cached };
    }

    if (req.language != null && req.theme != null) {
      await this.attachLanguages(req.language as BundledLanguage);
      await this.attachTheme(req.theme);
    }

    const html = this.highlighter.codeToHtml(req.code, {
      lang: req.language,
      theme: req.theme,
      //   themes: {
      //     light: "vesper-light",
      //     "dark-classic": "vesper",
      //   },
      defaultColor: "light",
      cssVariablePrefix: "--shiki-",
    });

    this.fileCache.set(key, html);

    return { id: req.id, html };
  }

  async attachLanguages(lang: BundledLanguage): Promise<void> {
    if (AttachedLanguages.has(lang)) {
      return;
    } else {
      const loadedLang = bundledLanguages[lang];

      if (!loadedLang) {
        throw new Error(`Language not found: ${lang}`);
      }

      await this.highlighter.loadLanguage(loadedLang);
      AttachedLanguages.add(lang);
    }
  }

  async attachTheme(theme: string): Promise<void> {
    if (AttachedThemes.has(theme)) {
      return;
    } else {
      await this.highlighter.loadTheme(theme as any);
      AttachedThemes.add(theme);
    }
  }

  async initializeHighlighter() {
    this.highlighter = await createHighlighter({
      themes: [],
      langs: [],
      engine: createJavaScriptRegexEngine(),
    });
  }
}

const worker = new ShikiWorker();

self.addEventListener("error", (event) => {
  console.error("[Shiki Worker] Unhandled error:", event.error);
});

self.onmessage = async (e: MessageEvent<HighlightRequest>) => {
  const req = e.data;

  try {
    const startTime = performance.now();
    if (!worker["ready"]) {
      const response: HighlightErrorResponse = {
        id: req.id,
        error: "Worker not ready",
      };
      postMessage(response);
    }

    const result = await worker.highlight(e.data);

    const response: HighlightResponse = {
      id: result.id,
      html: result.html,
    };

    postMessage(response);
    const endTime = performance.now();
    console.log(
      `[Shiki Worker] Highlighted (lang: ${req.language}, theme: ${req.theme}) in ${
        endTime - startTime
      } ms`,
    );
  } catch (error) {
    console.error("Worker error:", error);

    const response: HighlightErrorResponse = {
      id: req.id,
      error: error instanceof Error ? error.message : String(error),
      stack: error instanceof Error ? error.stack : undefined,
    };

    postMessage(response);
  }
};

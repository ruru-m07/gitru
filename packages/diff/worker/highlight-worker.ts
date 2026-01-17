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
  SupportedLanguage,
  SupportedTheme,
} from "../types";
import { createCacheKey } from "../utils/common";

const loadedLanguages = new Set<string>(["text", "plaintext"]);
const loadedThemes = new Set<string>();

const HTML_CACHE_SIZE = 500000;
const htmlCache = new LRUMap<string, string>(HTML_CACHE_SIZE);

class ShikiHighlightWorker {
  private highlighter: HighlighterCore | null = null;
  private initPromise: Promise<void> | null = null;
  private ready = false;

  constructor() {
    this.initPromise = this.initialize();
  }

  private async initialize(): Promise<void> {
    try {
      this.highlighter = await createHighlighter({
        themes: [],
        langs: [],
        engine: createJavaScriptRegexEngine(),
      });
      this.ready = true;
      console.log("[ShikiWorker] Initialized successfully");
    } catch (error) {
      console.error("[ShikiWorker] Failed to initialize:", error);
      throw error;
    }
  }

  async ensureReady(): Promise<void> {
    if (this.ready) return;
    if (this.initPromise) {
      await this.initPromise;
    }
  }

  async loadLanguage(lang: SupportedLanguage): Promise<void> {
    if (!this.highlighter) return;
    if (loadedLanguages.has(lang)) return;

    // Handle special cases
    if (lang === "text" || lang === "plaintext") {
      loadedLanguages.add(lang);
      return;
    }

    const langDef = bundledLanguages[lang as BundledLanguage];
    if (!langDef) {
      console.warn(
        `[ShikiWorker] Language not found: ${lang}, falling back to text`,
      );
      return;
    }

    try {
      await this.highlighter.loadLanguage(langDef);
      loadedLanguages.add(lang);
    } catch (error) {
      console.error(`[ShikiWorker] Failed to load language ${lang}:`, error);
    }
  }

  async loadTheme(theme: SupportedTheme): Promise<void> {
    if (!this.highlighter) return;
    if (loadedThemes.has(theme)) return;

    try {
      await this.highlighter.loadTheme(theme as any);
      loadedThemes.add(theme);
    } catch (error) {
      console.error(`[ShikiWorker] Failed to load theme ${theme}:`, error);
    }
  }

  async highlight(request: HighlightRequest): Promise<HighlightResponse> {
    await this.ensureReady();

    if (!this.highlighter) {
      throw new Error("Highlighter not initialized");
    }

    const { id, code, language, theme } = request;

    const cacheKey = createCacheKey(code, language, theme);
    const cached = htmlCache.get(cacheKey);
    if (cached) {
      return { id, html: cached };
    }

    // Load language and theme if needed
    await Promise.all([this.loadLanguage(language), this.loadTheme(theme)]);

    const actualLang = loadedLanguages.has(language) ? language : "text";

    const startTime = performance.now();

    const html = this.highlighter.codeToHtml(code, {
      lang: actualLang,
      theme: theme,
      defaultColor: false,
    });

    const timing = performance.now() - startTime;

    htmlCache.set(cacheKey, html);

    return { id, html, timing };
  }

  /**
   * Highlight multiple lines at once (batch operation)
   */
  async highlightLines(
    lines: string[],
    language: SupportedLanguage,
    theme: SupportedTheme,
  ): Promise<string[]> {
    await this.ensureReady();

    if (!this.highlighter) {
      throw new Error("Highlighter not initialized");
    }

    // Load language and theme if needed
    await Promise.all([this.loadLanguage(language), this.loadTheme(theme)]);

    const actualLang = loadedLanguages.has(language) ? language : "text";

    return lines.map((line) => {
      const cacheKey = createCacheKey(line, actualLang, theme);
      const cached = htmlCache.get(cacheKey);
      if (cached) return cached;

      const html = this.highlighter!.codeToHtml(line, {
        lang: actualLang,
        theme: theme,
        defaultColor: false,
      });

      htmlCache.set(cacheKey, html);
      return html;
    });
  }

  clearCache(): void {
    htmlCache.clear();
  }

  getCacheStats(): { size: number; maxSize: number } {
    return {
      size: htmlCache.size,
      maxSize: HTML_CACHE_SIZE,
    };
  }
}

const worker = new ShikiHighlightWorker();

self.onmessage = async (event: MessageEvent<HighlightRequest>) => {
  const request = event.data;

  try {
    const response = await worker.highlight(request);
    self.postMessage(response);
  } catch (error) {
    const errorResponse: HighlightErrorResponse = {
      id: request.id,
      error: error instanceof Error ? error.message : String(error),
      stack: error instanceof Error ? error.stack : undefined,
    };
    self.postMessage(errorResponse);
  }
};

self.onerror = (event) => {
  console.error("[ShikiWorker] Unhandled error:", event);
};

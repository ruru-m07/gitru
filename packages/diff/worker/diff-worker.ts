/// <reference lib="webworker" />

// ============================================================================
// Diff Worker - Handles highlighting, inline diff, and patch parsing
// All heavy computation runs here, streaming results back to main thread
// ============================================================================

import { LRUMap } from "lru_map";
import {
  type BundledLanguage,
  bundledLanguages,
  createHighlighter,
  createJavaScriptRegexEngine,
  type HighlighterCore,
} from "shiki";

import type {
  DiffHunk,
  DiffLine,
  FileDiff,
  InlineDiffSegment,
  SupportedLanguage,
  SupportedTheme,
} from "../types";
import { createCacheKey } from "../utils/common";
import type {
  HighlightBatchRequest,
  HighlightBatchResponse,
  HighlightRequest,
  HighlightResponse,
  InitializeRequest,
  InitializeResponse,
  ParsePatchRequest,
  ParsePatchResponse,
  ProcessDiffChunk,
  ProcessDiffComplete,
  ProcessDiffError,
  ProcessDiffRequest,
  WorkerRequest,
} from "./types";

// ============================================================================
// Constants
// ============================================================================

const HTML_CACHE_SIZE = 500_000;
const DEFAULT_CHUNK_SIZE = 25;

// ============================================================================
// State
// ============================================================================

const loadedLanguages = new Set<string>(["text", "plaintext"]);
const loadedThemes = new Set<string>();
const htmlCache = new LRUMap<string, string>(HTML_CACHE_SIZE);

let highlighter: HighlighterCore | null = null;
let initPromise: Promise<void> | null = null;
let isReady = false;

// ============================================================================
// Initialization
// ============================================================================

async function initialize(): Promise<void> {
  if (isReady) return;
  if (initPromise) {
    await initPromise;
    return;
  }

  initPromise = (async () => {
    try {
      highlighter = await createHighlighter({
        themes: [],
        langs: [],
        engine: createJavaScriptRegexEngine(),
      });
      isReady = true;

      // console.log("[DiffWorker] Initialized successfully");
    } catch (error) {
      console.error("[DiffWorker] Failed to initialize:", error);
      throw error;
    }
  })();

  await initPromise;
}

// ============================================================================
// Language & Theme Loading
// ============================================================================

async function loadLanguage(lang: SupportedLanguage): Promise<void> {
  if (!highlighter) return;
  if (loadedLanguages.has(lang)) return;

  if (lang === "text" || lang === "plaintext") {
    loadedLanguages.add(lang);
    return;
  }

  const langDef = bundledLanguages[lang as BundledLanguage];
  if (!langDef) {
    console.warn(
      `[DiffWorker] Language not found: ${lang}, falling back to text`,
    );
    return;
  }

  try {
    await highlighter.loadLanguage(langDef);
    loadedLanguages.add(lang);
  } catch (error) {
    console.error(`[DiffWorker] Failed to load language ${lang}:`, error);
  }
}

async function loadTheme(theme: SupportedTheme): Promise<void> {
  if (!highlighter) return;
  if (loadedThemes.has(theme)) return;

  try {
    // biome-ignore lint/suspicious/noExplicitAny: <shiki theme type>
    await highlighter.loadTheme(theme as any);
    loadedThemes.add(theme);
  } catch (error) {
    console.error(`[DiffWorker] Failed to load theme ${theme}:`, error);
  }
}

// ============================================================================
// Highlighting with Grammar State Continuity
// ============================================================================

import type { GrammarState } from "shiki";

// Cache for grammar states to enable incremental parsing
const grammarStateCache = new Map<string, GrammarState>();

/**
 * Highlight a single line, optionally using grammar state for continuity.
 * Grammar state allows us to continue parsing from a previous line's state,
 * which is essential for multi-line constructs (strings, comments, etc.)
 */
function highlightLineWithGrammar(
  content: string,
  language: string,
  theme: SupportedTheme,
  grammarState?: GrammarState,
): { html: string; grammarState?: GrammarState } {
  if (!highlighter) {
    return { html: escapeHtml(content) };
  }

  const cacheKey = createCacheKey(content, language, theme);
  const cached = htmlCache.get(cacheKey);
  if (cached) {
    // Even if cached, we need to update grammar state for continuity
    const result = highlighter.codeToTokens(content, {
      lang: loadedLanguages.has(language) ? language : "text",
      theme: theme,
      grammarState,
    });
    return { html: cached, grammarState: result.grammarState };
  }

  const actualLang = loadedLanguages.has(language) ? language : "text";

  // Use codeToTokens for grammar state continuity
  const result = highlighter.codeToTokens(content, {
    lang: actualLang,
    theme: theme,
    grammarState,
  });

  // Convert tokens to HTML
  const html = tokensToHtml(result.tokens[0] ?? []);
  htmlCache.set(cacheKey, html);

  return { html, grammarState: result.grammarState };
}

/**
 * Convert Shiki tokens to HTML string, merging adjacent same-color tokens
 */
function tokensToHtml(
  tokens: Array<{ content: string; color?: string }>,
): string {
  if (tokens.length === 0) return "";

  let result = "";
  let currentColor: string | undefined;
  let currentText = "";

  const flush = () => {
    if (currentText) {
      if (currentColor) {
        result += `<span style="color:${currentColor}">${escapeHtml(currentText)}</span>`;
      } else {
        result += escapeHtml(currentText);
      }
      currentText = "";
    }
  };

  for (const token of tokens) {
    if (token.color === currentColor) {
      currentText += token.content;
    } else {
      flush();
      currentColor = token.color;
      currentText = token.content;
    }
  }

  flush();
  return result;
}

// Legacy function for backward compatibility
function highlightLine(
  content: string,
  language: string,
  theme: SupportedTheme,
): string {
  return highlightLineWithGrammar(content, language, theme).html;
}

function extractInnerHtml(html: string): string {
  const codeStart = html.indexOf("<code>");
  const codeEnd = html.lastIndexOf("</code>");

  if (codeStart !== -1 && codeEnd !== -1) {
    return html.slice(codeStart + 6, codeEnd);
  }

  const spanMatch = html.match(/<span[^>]*>([\s\S]*)<\/span>/);
  if (spanMatch) {
    return spanMatch[0];
  }

  return html;
}

function escapeHtml(text: string): string {
  return text
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#039;");
}

// ============================================================================
// Inline Diff Computation (moved from main thread)
// ============================================================================

function computeInlineDiff(
  oldText: string,
  newText: string,
): InlineDiffSegment[] {
  if (oldText === newText) {
    return [{ text: oldText }];
  }

  if (!oldText) {
    return [{ text: newText, added: true }];
  }

  if (!newText) {
    return [{ text: oldText, removed: true }];
  }

  const oldWords = tokenize(oldText);
  const newWords = tokenize(newText);
  const lcs = computeWordLCS(oldWords, newWords);

  const segments: InlineDiffSegment[] = [];
  let oldIdx = 0;
  let newIdx = 0;
  let lcsIdx = 0;

  while (oldIdx < oldWords.length || newIdx < newWords.length) {
    const lcsItem = lcs[lcsIdx];

    const matchOld = lcsItem !== undefined && oldWords[oldIdx] === lcsItem.word;
    const matchNew = lcsItem !== undefined && newWords[newIdx] === lcsItem.word;

    if (matchOld && matchNew) {
      pushSegment(segments, { text: oldWords[oldIdx] ?? "" });
      oldIdx++;
      newIdx++;
      lcsIdx++;
    } else if (
      newIdx < newWords.length &&
      (lcsItem === undefined || newWords[newIdx] !== lcsItem.word)
    ) {
      pushSegment(segments, { text: newWords[newIdx] ?? "", added: true });
      newIdx++;
    } else if (oldIdx < oldWords.length) {
      pushSegment(segments, { text: oldWords[oldIdx] ?? "", removed: true });
      oldIdx++;
    }
  }

  return mergeAdjacentSegments(segments);
}

function tokenize(text: string): string[] {
  const tokens: string[] = [];
  let current = "";

  for (let i = 0; i < text.length; i++) {
    const char = text[i];
    if (char === undefined) continue;

    if (/\s/.test(char)) {
      if (current) {
        tokens.push(current);
        current = "";
      }
      let whitespace = char;
      while (i + 1 < text.length && /\s/.test(text[i + 1] ?? "")) {
        whitespace += text[++i] ?? "";
      }
      tokens.push(whitespace);
    } else if (/[^\w]/.test(char)) {
      if (current) {
        tokens.push(current);
        current = "";
      }
      tokens.push(char);
    } else {
      current += char;
    }
  }

  if (current) {
    tokens.push(current);
  }

  return tokens;
}

interface LCSWord {
  word: string;
  oldIdx: number;
  newIdx: number;
}

function computeWordLCS(oldWords: string[], newWords: string[]): LCSWord[] {
  const m = oldWords.length;
  const n = newWords.length;

  const dp: number[][] = Array(m + 1)
    .fill(null)
    .map(() => Array(n + 1).fill(0) as number[]);

  for (let i = 1; i <= m; i++) {
    for (let j = 1; j <= n; j++) {
      if (oldWords[i - 1] === newWords[j - 1]) {
        dp[i]![j] = (dp[i - 1]?.[j - 1] ?? 0) + 1;
      } else {
        dp[i]![j] = Math.max(dp[i - 1]?.[j] ?? 0, dp[i]?.[j - 1] ?? 0);
      }
    }
  }

  const lcs: LCSWord[] = [];
  let i = m;
  let j = n;

  while (i > 0 && j > 0) {
    if (oldWords[i - 1] === newWords[j - 1]) {
      lcs.unshift({
        word: oldWords[i - 1] ?? "",
        oldIdx: i - 1,
        newIdx: j - 1,
      });
      i--;
      j--;
    } else if ((dp[i - 1]?.[j] ?? 0) > (dp[i]?.[j - 1] ?? 0)) {
      i--;
    } else {
      j--;
    }
  }

  return lcs;
}

function pushSegment(
  segments: InlineDiffSegment[],
  segment: InlineDiffSegment,
): void {
  const prev = segments[segments.length - 1];

  if (
    prev &&
    prev.added === segment.added &&
    prev.removed === segment.removed
  ) {
    prev.text += segment.text;
  } else {
    segments.push(segment);
  }
}

function mergeAdjacentSegments(
  segments: InlineDiffSegment[],
): InlineDiffSegment[] {
  if (segments.length <= 1) return segments;

  const result: InlineDiffSegment[] = [];

  for (const segment of segments) {
    const prev = result[result.length - 1];

    if (
      prev &&
      prev.added === segment.added &&
      prev.removed === segment.removed
    ) {
      prev.text += segment.text;
    } else {
      result.push({ ...segment });
    }
  }

  return result;
}

// ============================================================================
// Apply Inline Diff to Highlighted HTML
// ============================================================================

function applyInlineDiffToHighlighted(
  plainText: string,
  highlightedHtml: string,
  segments: InlineDiffSegment[],
): string {
  const hasChanges = segments.some((s) => s.added || s.removed);
  if (!hasChanges) {
    return highlightedHtml;
  }

  const markedPositions = new Set<number>();
  let pos = 0;
  for (const segment of segments) {
    if (segment.added || segment.removed) {
      for (let k = 0; k < segment.text.length; k++) {
        markedPositions.add(pos + k);
      }
    }
    pos += segment.text.length;
  }

  let result = "";
  let textPos = 0;
  let inMarkedSpan = false;
  let i = 0;

  while (i < highlightedHtml.length) {
    if (highlightedHtml[i] === "<") {
      if (inMarkedSpan) {
        result += "</span>";
        inMarkedSpan = false;
      }

      const tagEnd = highlightedHtml.indexOf(">", i);
      if (tagEnd === -1) break;

      result += highlightedHtml.slice(i, tagEnd + 1);
      i = tagEnd + 1;
      continue;
    }

    if (highlightedHtml[i] === "&") {
      const entityEnd = highlightedHtml.indexOf(";", i);
      if (entityEnd !== -1 && entityEnd - i < 10) {
        const shouldMark = markedPositions.has(textPos);

        if (shouldMark && !inMarkedSpan) {
          result += `<span data-diff-inline="">`;
          inMarkedSpan = true;
        } else if (!shouldMark && inMarkedSpan) {
          result += "</span>";
          inMarkedSpan = false;
        }

        result += highlightedHtml.slice(i, entityEnd + 1);
        i = entityEnd + 1;
        textPos++;
        continue;
      }
    }

    const shouldMark = markedPositions.has(textPos);

    if (shouldMark && !inMarkedSpan) {
      result += `<span data-diff-inline="">`;
      inMarkedSpan = true;
    } else if (!shouldMark && inMarkedSpan) {
      result += "</span>";
      inMarkedSpan = false;
    }

    result += highlightedHtml[i];
    textPos++;
    i++;
  }

  if (inMarkedSpan) {
    result += "</span>";
  }

  return result;
}

// ============================================================================
// Process Diff (Main Entry Point for Streaming)
// ============================================================================

async function processDiff(request: ProcessDiffRequest): Promise<void> {
  const { id, instanceId, diff, theme, highlightInline } = request;
  const startTime = performance.now();

  try {
    await initialize();
    await Promise.all([loadLanguage(diff.language), loadTheme(theme)]);

    // Collect all lines in order for grammar state continuity
    // We need to process lines in the order they appear for correct parsing
    const allLines: Array<{
      content: string;
      hunkIdx: number;
      lineIdx: number;
    }> = [];
    const uniqueLines = new Map<string, string>(); // content -> highlighted html
    const linePairs: Array<{ deletion: DiffLine; addition: DiffLine }> = [];

    // First pass: collect all lines in order
    for (let hunkIdx = 0; hunkIdx < diff.hunks.length; hunkIdx++) {
      const hunk = diff.hunks[hunkIdx];
      if (!hunk) continue;

      const lines = hunk.lines;
      for (let lineIdx = 0; lineIdx < lines.length; lineIdx++) {
        const line = lines[lineIdx];
        if (!line || line.type === "metadata") continue;

        allLines.push({ content: line.content, hunkIdx, lineIdx });

        // Track for highlighting
        if (!uniqueLines.has(line.content)) {
          uniqueLines.set(line.content, "");
        }

        // Track pairs for inline diff
        if (highlightInline && line.type === "deletion") {
          const next = lines[lineIdx + 1];
          if (next && next.type === "addition") {
            linePairs.push({ deletion: line, addition: next });
            if (!uniqueLines.has(next.content)) {
              uniqueLines.set(next.content, "");
            }
          }
        }
      }
    }

    // Second pass: highlight with grammar state continuity
    // Process lines in order to maintain grammar state
    const totalLines = allLines.length;
    let processedCount = 0;
    let grammarState: GrammarState | undefined;

    const chunkSize = DEFAULT_CHUNK_SIZE;
    const processedContents = new Set<string>();

    for (let i = 0; i < allLines.length; i += chunkSize) {
      const chunk = allLines.slice(i, i + chunkSize);
      const entries: Array<[string, string]> = [];

      for (const { content } of chunk) {
        // Use grammar state continuity for better multi-line parsing
        const result = highlightLineWithGrammar(
          content,
          diff.language,
          theme,
          grammarState,
        );

        // Update grammar state for next line
        grammarState = result.grammarState;

        // Only add to entries if not already processed (for duplicates)
        if (!processedContents.has(content)) {
          processedContents.add(content);
          uniqueLines.set(content, result.html);
          entries.push([content, result.html]);
        }

        processedCount++;
      }

      // Send chunk (only if there are new entries)
      if (entries.length > 0) {
        const chunkResponse: ProcessDiffChunk = {
          type: "process-diff-chunk",
          id,
          instanceId,
          entries,
          progress: processedCount / totalLines,
        };
        self.postMessage(chunkResponse);
      }
    }

    // Third pass: compute inline diffs and send final updates
    if (highlightInline && linePairs.length > 0) {
      const inlineEntries: Array<[string, string]> = [];

      for (const pair of linePairs) {
        const segments = computeInlineDiff(
          pair.deletion.content,
          pair.addition.content,
        );

        // Apply to deletion
        const deletionSegments = segments.filter((s) => !s.added);
        const deletionHtml = applyInlineDiffToHighlighted(
          pair.deletion.content,
          uniqueLines.get(pair.deletion.content) ?? "",
          deletionSegments,
        );
        inlineEntries.push([pair.deletion.content, deletionHtml]);

        // Apply to addition
        const additionSegments = segments.filter((s) => !s.removed);
        const additionHtml = applyInlineDiffToHighlighted(
          pair.addition.content,
          uniqueLines.get(pair.addition.content) ?? "",
          additionSegments,
        );
        inlineEntries.push([pair.addition.content, additionHtml]);
      }

      // Send inline diff updates as final chunk
      if (inlineEntries.length > 0) {
        const inlineChunk: ProcessDiffChunk = {
          type: "process-diff-chunk",
          id,
          instanceId,
          entries: inlineEntries,
          progress: 1,
        };
        self.postMessage(inlineChunk);
      }
    }

    // Send completion
    const duration = performance.now() - startTime;
    const completeResponse: ProcessDiffComplete = {
      type: "process-diff-complete",
      id,
      instanceId,
      totalLines,
      duration,
    };
    self.postMessage(completeResponse);
  } catch (error) {
    const errorResponse: ProcessDiffError = {
      type: "process-diff-error",
      id,
      instanceId,
      error: error instanceof Error ? error.message : String(error),
    };
    self.postMessage(errorResponse);
  }
}

// ============================================================================
// Handle Single Highlight Request (backward compatibility)
// ============================================================================

async function handleHighlight(request: HighlightRequest): Promise<void> {
  try {
    await initialize();
    await Promise.all([
      loadLanguage(request.language as SupportedLanguage),
      loadTheme(request.theme),
    ]);

    const html = highlightLine(
      request.content,
      request.language,
      request.theme,
    );

    const response: HighlightResponse = {
      type: "highlight",
      id: request.id,
      html,
    };
    self.postMessage(response);
  } catch (error) {
    const response: HighlightResponse = {
      type: "highlight",
      id: request.id,
      html: escapeHtml(request.content),
      error: error instanceof Error ? error.message : String(error),
    };
    self.postMessage(response);
  }
}

// ============================================================================
// Handle Batch Highlight Request
// ============================================================================

async function handleHighlightBatch(
  request: HighlightBatchRequest,
): Promise<void> {
  try {
    await initialize();
    await Promise.all([
      loadLanguage(request.language as SupportedLanguage),
      loadTheme(request.theme),
    ]);

    const results: Array<{ content: string; html: string }> = [];

    for (const content of request.lines) {
      const html = highlightLine(content, request.language, request.theme);
      results.push({ content, html });
    }

    const response: HighlightBatchResponse = {
      type: "highlight-batch",
      id: request.id,
      results,
    };
    self.postMessage(response);
  } catch (error) {
    const response: HighlightBatchResponse = {
      type: "highlight-batch",
      id: request.id,
      results: [],
      error: error instanceof Error ? error.message : String(error),
    };

    self.postMessage(response);
  }
}

// ============================================================================
// Handle Initialize Request
// ============================================================================

async function handleInitialize(request: InitializeRequest): Promise<void> {
  try {
    await initialize();

    const response: InitializeResponse = {
      type: "initialize",
      id: request.id,
      success: true,
      loadedLanguages: Array.from(loadedLanguages),
    };
    self.postMessage(response);
  } catch (error) {
    const response: InitializeResponse = {
      type: "initialize",
      id: request.id,
      success: false,
      loadedLanguages: [],
    };
    self.postMessage(response);
  }
}

// ============================================================================
// Message Handler
// ============================================================================

self.onmessage = async (event: MessageEvent<WorkerRequest>) => {
  const request = event.data;

  switch (request.type) {
    case "initialize":
      await handleInitialize(request);
      break;

    case "highlight":
      await handleHighlight(request as HighlightRequest);
      break;

    case "highlight-batch":
      await handleHighlightBatch(request);
      break;

    case "process-diff":
      await processDiff(request);
      break;

    case "parse-patch":
      // Parse patch is lightweight, but we can handle it here too
      // Import dynamically to keep worker lean
      try {
        const { parsePatch } = await import("../utils/parse-patch");
        const result = parsePatch((request as ParsePatchRequest).patch);
        const response: ParsePatchResponse = {
          type: "parse-patch",
          id: request.id,
          files: result.files,
        };
        self.postMessage(response);
      } catch (error) {
        const response: ParsePatchResponse = {
          type: "parse-patch",
          id: request.id,
          files: [],
          error: error instanceof Error ? error.message : String(error),
        };
        self.postMessage(response);
      }
      break;
  }
};

self.onerror = (event) => {
  console.error("[DiffWorker] Unhandled error:", event);
};

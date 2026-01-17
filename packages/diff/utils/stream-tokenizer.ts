// ============================================================================
// Streaming Tokenizer with Grammar State Continuity
// Maintains parser state between lines for incremental parsing
// ============================================================================

import type { GrammarState, ThemedToken } from "shiki";

import type { SupportedLanguage, SupportedTheme } from "../types";
import type { HASTNode } from "./hast";
import { createSpan, createText } from "./hast";

// ============================================================================
// Types
// ============================================================================

export interface StreamTokenizerOptions {
  /** The Shiki highlighter instance */
  highlighter: {
    codeToTokens: (
      code: string,
      options: {
        lang: string;
        theme: string;
        grammarState?: GrammarState;
      },
    ) => {
      tokens: ThemedToken[][];
      grammarState?: GrammarState;
    };
  };
  /** Language to use for tokenization */
  lang: SupportedLanguage;
  /** Theme for token colors */
  theme: SupportedTheme;
}

export interface TokenizerEnqueueResult {
  /** Number of previously unstable tokens that should be recalled/replaced */
  recall: number;
  /** Tokens that are now stable (complete lines) */
  stable: ThemedToken[];
  /** Tokens that are unstable (incomplete last line) */
  unstable: ThemedToken[];
}

// ============================================================================
// Stream Tokenizer Class
// ============================================================================

/**
 * Tokenizer that maintains grammar state between chunks for incremental parsing.
 * This is key for performance - we don't re-parse from scratch each time.
 */
export class StreamTokenizer {
  private options: StreamTokenizerOptions;

  /** Tokens from stable (complete) lines */
  private tokensStable: ThemedToken[] = [];
  /** Tokens from the unstable (incomplete) last line */
  private tokensUnstable: ThemedToken[] = [];

  /** Buffer for incomplete last line */
  private lastUnstableCodeChunk = "";
  /** Grammar state at end of last stable line - KEY for incremental parsing */
  private lastStableGrammarState: GrammarState | undefined;

  constructor(options: StreamTokenizerOptions) {
    this.options = options;
  }

  /**
   * Enqueue a chunk of code for tokenization.
   * Returns stable tokens that won't change, and unstable tokens that might.
   */
  enqueue(chunk: string): TokenizerEnqueueResult {
    const { highlighter, lang, theme } = this.options;

    // Combine with leftover from last chunk
    const chunkLines = (this.lastUnstableCodeChunk + chunk).split("\n");

    const stable: ThemedToken[] = [];
    let unstable: ThemedToken[] = [];
    const recall = this.tokensUnstable.length;

    for (let i = 0; i < chunkLines.length; i++) {
      const line = chunkLines[i];
      if (line === undefined) continue;

      const isLastLine = i === chunkLines.length - 1;

      // Use grammar state from last stable line for incremental parsing
      const result = highlighter.codeToTokens(line, {
        lang,
        theme,
        grammarState: this.lastStableGrammarState,
      });

      const tokens = result.tokens[0] ?? []; // Only one line per call

      // Add newline token for non-last lines
      if (!isLastLine) {
        tokens.push({ content: "\n", offset: 0 });
      }

      if (!isLastLine) {
        // Line is complete - save grammar state for next iteration
        this.lastStableGrammarState = result.grammarState;
        stable.push(...tokens);
      } else {
        // Last line is unstable - might change with next chunk
        unstable = tokens;
        this.lastUnstableCodeChunk = line;
      }
    }

    this.tokensStable.push(...stable);
    this.tokensUnstable = unstable;

    return { recall, stable, unstable };
  }

  /**
   * Close the tokenizer and get remaining tokens.
   * Call this when no more chunks will be added.
   */
  close(): { stable: ThemedToken[] } {
    const stable = this.tokensUnstable;
    this.tokensUnstable = [];
    this.lastUnstableCodeChunk = "";
    this.lastStableGrammarState = undefined;
    return { stable };
  }

  /**
   * Clear all state.
   */
  clear(): void {
    this.tokensStable = [];
    this.tokensUnstable = [];
    this.lastUnstableCodeChunk = "";
    this.lastStableGrammarState = undefined;
  }

  /**
   * Clone this tokenizer's state.
   */
  clone(): StreamTokenizer {
    const clone = new StreamTokenizer(this.options);
    clone.lastUnstableCodeChunk = this.lastUnstableCodeChunk;
    clone.tokensUnstable = [...this.tokensUnstable];
    clone.tokensStable = [...this.tokensStable];
    clone.lastStableGrammarState = this.lastStableGrammarState;
    return clone;
  }

  /**
   * Get current grammar state (useful for continuing later)
   */
  getGrammarState(): GrammarState | undefined {
    return this.lastStableGrammarState;
  }

  /**
   * Set grammar state (useful for continuing from a known state)
   */
  setGrammarState(state: GrammarState | undefined): void {
    this.lastStableGrammarState = state;
  }
}

// ============================================================================
// Token to HAST conversion
// ============================================================================

/**
 * Convert Shiki tokens to HAST nodes.
 * This produces AST nodes that can be manipulated before final HTML conversion.
 */
export function tokensToHast(tokens: ThemedToken[]): HASTNode[] {
  const nodes: HASTNode[] = [];

  for (const token of tokens) {
    if (token.color) {
      // Colored token - wrap in span with style
      nodes.push(
        createSpan({ style: `color:${token.color}` }, [
          createText(token.content),
        ]),
      );
    } else {
      // No color - just text
      nodes.push(createText(token.content));
    }
  }

  return nodes;
}

/**
 * Convert tokens for a single line to HAST, merging adjacent same-color tokens
 */
export function lineTokensToHast(tokens: ThemedToken[]): HASTNode[] {
  if (tokens.length === 0) {
    return [createText("")];
  }

  const nodes: HASTNode[] = [];
  let currentColor: string | undefined;
  let currentText = "";

  const flush = () => {
    if (currentText) {
      if (currentColor) {
        nodes.push(
          createSpan({ style: `color:${currentColor}` }, [
            createText(currentText),
          ]),
        );
      } else {
        nodes.push(createText(currentText));
      }
      currentText = "";
    }
  };

  for (const token of tokens) {
    // Skip newline tokens in line output
    if (token.content === "\n") continue;

    if (token.color === currentColor) {
      currentText += token.content;
    } else {
      flush();
      currentColor = token.color;
      currentText = token.content;
    }
  }

  flush();

  return nodes.length > 0 ? nodes : [createText("")];
}

// ============================================================================
// Line-by-line tokenization with grammar state
// ============================================================================

export interface LineTokenizeResult {
  /** HAST nodes for this line */
  nodes: HASTNode[];
  /** Grammar state after this line (for continuing) */
  grammarState?: GrammarState;
}

/**
 * Tokenize a single line, optionally continuing from a previous grammar state.
 * Returns HAST nodes and the new grammar state.
 */
export function tokenizeLine(
  highlighter: StreamTokenizerOptions["highlighter"],
  line: string,
  lang: SupportedLanguage,
  theme: SupportedTheme,
  grammarState?: GrammarState,
): LineTokenizeResult {
  const result = highlighter.codeToTokens(line, {
    lang,
    theme,
    grammarState,
  });

  const tokens = result.tokens[0] ?? [];
  const nodes = lineTokensToHast(tokens);

  return {
    nodes,
    grammarState: result.grammarState,
  };
}

/**
 * Tokenize multiple lines with grammar state continuity.
 * Much more efficient than tokenizing each line independently!
 */
export function tokenizeLines(
  highlighter: StreamTokenizerOptions["highlighter"],
  lines: string[],
  lang: SupportedLanguage,
  theme: SupportedTheme,
): Map<string, HASTNode[]> {
  const results = new Map<string, HASTNode[]>();
  let grammarState: GrammarState | undefined;

  for (const line of lines) {
    // Skip if already processed (deduplication)
    if (results.has(line)) continue;

    const { nodes, grammarState: newState } = tokenizeLine(
      highlighter,
      line,
      lang,
      theme,
      grammarState,
    );

    results.set(line, nodes);
    grammarState = newState;
  }

  return results;
}

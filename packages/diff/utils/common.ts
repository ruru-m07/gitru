import { EXTENSION_TO_LANGUAGE, FILENAME_TO_LANGUAGE } from "../constants";
import type { SupportedLanguage } from "../types";

/**
 * Detect the programming language from a file path.
 * Returns the language identifier for Shiki.
 */
export function detectLanguage(filePath: string): SupportedLanguage {
  if (!filePath) return "text";

  const fileName = filePath.split("/").pop() ?? filePath;

  // Check for exact filename matches first
  if (fileName in FILENAME_TO_LANGUAGE) {
    return FILENAME_TO_LANGUAGE[
      fileName as keyof typeof FILENAME_TO_LANGUAGE
    ] as SupportedLanguage;
  }

  // Check for extension
  const lastDot = fileName.lastIndexOf(".");
  if (lastDot !== -1) {
    const ext = fileName.slice(lastDot + 1).toLowerCase();

    if (ext in EXTENSION_TO_LANGUAGE) {
      return EXTENSION_TO_LANGUAGE[
        ext as keyof typeof EXTENSION_TO_LANGUAGE
      ] as SupportedLanguage;
    }
  }

  return "text";
}

/**
 * Escape HTML special characters to prevent XSS
 */
export function escapeHtml(text: string): string {
  return text
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#039;");
}

/**
 * Split text into lines while preserving newlines
 */
export function splitLines(text: string): string[] {
  if (!text) return [];
  // Split but keep the newline characters
  return text.split(/(?<=\n)/);
}

/**
 * Remove trailing newline from a line
 */
export function trimTrailingNewline(line: string): string {
  return line.replace(/\r?\n$/, "");
}

/**
 * Generate a simple hash for cache keys
 */
export function hashCode(str: string): number {
  let hash = 2166136261; // FNV-1a offset basis
  for (let i = 0; i < str.length; i++) {
    hash ^= str.charCodeAt(i);
    hash +=
      (hash << 1) + (hash << 4) + (hash << 7) + (hash << 8) + (hash << 24);
  }
  return hash >>> 0;
}

/**
 * Create a cache key from code, language, and theme
 */
export function createCacheKey(
  code: string,
  language: string,
  theme: string,
): string {
  return `${hashCode(code)}:${language}:${theme}`;
}

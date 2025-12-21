import {
  BundledLanguage,
  createHighlighterCore,
  createOnigurumaEngine,
  HighlighterCore,
  loadWasm,
} from "shiki";
import { preloadedLangs } from "./preload/langs";
import { preloadedThemes } from "./preload/themes";

export const DEFAULT_LANGUAGE = "plaintext" as BundledLanguage;

export const EXTENSION_LANGUAGE_MAP: Record<
  string,
  BundledLanguage | undefined
> = {
  ts: "typescript",
  tsx: "tsx",
  js: "javascript",
  jsx: "jsx",
  py: "python",
  css: "css",
  html: "html",
  json: "json",
  txt: "plaintext" as BundledLanguage,
  lock: "json",
  sh: "bash",
  bash: "bash",
  rs: "rust",
  rust: "rust",
  java: "java",
  make: "makefile",
  docker: "dockerfile",
  dockerfile: "dockerfile",
  yaml: "yaml",
  yml: "yaml",
  csv: "csv",
  markdown: "markdown",
  md: "markdown",
  toml: "toml",
  mdx: "mdx",
};

export const SPECIAL_FILENAMES: Record<string, BundledLanguage | undefined> = {
  makefile: "makefile",
  dockerfile: "dockerfile",
  gitignore: "bash",
  gitattributes: "bash",
  jenkinsfile: "groovy",
  procfile: "bash",
  vagrantfile: "ruby",
  brewfile: "ruby",
};

let cancelled = false;

export let highlighter: null | HighlighterCore = null;

(async () => {
  try {
    if (cancelled) {
      return;
    }
    if (!highlighter) {
      await loadWasm(import("shiki/onig.wasm?init"));
      const hl = await createHighlighterCore({
        themes: preloadedThemes,
        langs: preloadedLangs,
        engine: createOnigurumaEngine(import("shiki/wasm")),
      });
      if (!cancelled) {
        highlighter = hl;
        cancelled = true;
      }
    }
  } catch (error) {
    console.error("Failed to initialize Shiki highlighter:", error);
  }
})();

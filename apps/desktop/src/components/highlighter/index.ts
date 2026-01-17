import { BundledLanguage, HighlighterCore } from "shiki";
import { createHighlighterCore } from "shiki/core";
import { createJavaScriptRegexEngine } from "shiki/engine/javascript";
import { bundledLanguages } from "shiki/langs";
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

export let highlighter: null | HighlighterCore = null;

(async () => {
  try {
    if (!highlighter) {
      const hl = await createHighlighterCore({
        // themes: preloadedThemes as any,
        // langs: preloadedLangs,
        langs: [],
        themes: [],
        engine: createJavaScriptRegexEngine(),
      });
      await hl.loadTheme(...(preloadedThemes as any));
      await hl.loadLanguage(bundledLanguages.ts);
      await hl.loadLanguage(bundledLanguages.tsx);
      // await hl.loadLanguage(...preloadedLangs);
      if (!highlighter) {
        console.log("getLoadedThemes", hl.getLoadedThemes());
        console.log("getLoadedLanguages", hl.getLoadedLanguages());
        highlighter = hl;
      }
    }
  } catch (error) {
    console.error("Failed to initialize Shiki highlighter:", error);
  }
})();

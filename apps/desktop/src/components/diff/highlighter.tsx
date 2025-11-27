import { BundledLanguage, createHighlighter, Highlighter } from "shiki";
import { vesperLight } from "./custome-themes";

export function HighlightedLine({
  html,
  className,
}: {
  html: string;
  className?: string;
}) {
  return (
    <span className={className} dangerouslySetInnerHTML={{ __html: html }} />
  );
}

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
  mdx: "mdx" as BundledLanguage,
};

export const SPECIAL_FILENAMES: Record<string, BundledLanguage | undefined> = {
  makefile: "makefile",
  dockerfile: "dockerfile",
  gitignore: "bash",
  gitattributes: "bash",
  jenkinsfile: "groovy" as BundledLanguage,
  procfile: "bash",
  vagrantfile: "ruby" as BundledLanguage,
  brewfile: "ruby" as BundledLanguage,
};

const ALWAYS_AVAILABLE_LANGS = [
  "typescript",
  "javascript",
  "tsx",
  "jsx",
  "json",
  "css",
  "html",
  "bash",
  "python",
  "diff",
  "plaintext" as BundledLanguage,
  "rs",
  "rust",
  "makefile",
  "make",
  "docker",
  "dockerfile",
  "java",
  "csv",
  "yaml",
  "markdown",
  "md",
  "mdx",
  "groovy",
  "ruby",
  "toml",
] as BundledLanguage[];

let cancelled = false;

export let highlighter: null | Highlighter = null;

(async () => {
  try {
    var hl = await createHighlighter({
      themes: ["vesper", vesperLight],
      langs: ALWAYS_AVAILABLE_LANGS,
    });
    if (!cancelled) {
      highlighter = hl;
      cancelled = true;
    }
  } catch (error) {
    console.error("Failed to initialize Shiki highlighter:", error);
  }
})();

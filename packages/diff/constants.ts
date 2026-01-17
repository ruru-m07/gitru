// Regex patterns for parsing unified diff format
export const PATTERNS = {
  /** Split patch into separate file diffs (git format) */
  GIT_DIFF_SPLIT: /(?=^diff --git)/gm,

  /** Split patch into separate file diffs (unified format) */
  UNIFIED_DIFF_SPLIT: /(?=^---\s+\S)/gm,

  /** Match hunk headers: @@ -start,count +start,count @@ context */
  HUNK_HEADER: /^@@ -(\d+)(?:,(\d+))? \+(\d+)(?:,(\d+))? @@(?: (.*))?/,

  /** Match file header in git diff format */
  GIT_FILE_HEADER:
    /^diff --git (?:"?a\/(.+?)"?|a\/(.+?)) (?:"?b\/(.+?)"?|b\/(.+?))$/,

  /** Match --- and +++ file headers */
  FILE_HEADER: /^(---|\+\+\+)\s+(?:[ab]\/)?([^\t\r\n]+)/,

  /** Split content by newlines, keeping newlines */
  SPLIT_LINES: /(?<=\n)/,

  /** Match new file mode */
  NEW_FILE_MODE: /^new file mode (\d+)/,

  /** Match deleted file mode */
  DELETED_FILE_MODE: /^deleted file mode (\d+)/,

  /** Match similarity index */
  SIMILARITY_INDEX: /^similarity index (\d+)%/,

  /** Match rename from */
  RENAME_FROM: /^rename from (.+)$/,

  /** Match rename to */
  RENAME_TO: /^rename to (.+)$/,

  /** Match index line with mode */
  INDEX_LINE: /^index [a-f0-9]+\.\.[a-f0-9]+(?:\s+(\d+))?/,

  /** Match binary file indicator */
  BINARY_FILES: /^Binary files? .+ differ$/,

  /** Match no newline at end of file */
  NO_NEWLINE: /^\\ No newline at end of file/,
} as const;

// Default themes
export const DEFAULT_THEME_CONFIG = {
  light: "github-light",
  dark: "github-dark",
} as const;

// Language detection mappings
export const EXTENSION_TO_LANGUAGE: Record<string, string> = {
  // JavaScript/TypeScript
  js: "javascript",
  mjs: "javascript",
  cjs: "javascript",
  jsx: "jsx",
  ts: "typescript",
  mts: "typescript",
  cts: "typescript",
  tsx: "tsx",

  // Web
  html: "html",
  htm: "html",
  css: "css",
  scss: "scss",
  sass: "sass",
  less: "less",
  vue: "vue",
  svelte: "svelte",

  // Data formats
  json: "json",
  jsonc: "jsonc",
  json5: "json5",
  yaml: "yaml",
  yml: "yaml",
  toml: "toml",
  xml: "xml",
  csv: "csv",

  // Config files
  gitignore: "gitignore",
  env: "dotenv",

  // Rust
  rs: "rust",

  // Python
  py: "python",
  pyi: "python",
  pyw: "python",

  // Go
  go: "go",

  // Ruby
  rb: "ruby",
  rake: "ruby",
  gemspec: "ruby",

  // Shell
  sh: "shellscript",
  bash: "shellscript",
  zsh: "shellscript",
  fish: "fish",

  // C/C++
  c: "c",
  h: "c",
  cpp: "cpp",
  cc: "cpp",
  cxx: "cpp",
  hpp: "cpp",

  // Java/Kotlin
  java: "java",
  kt: "kotlin",
  kts: "kotlin",

  // Other
  md: "markdown",
  mdx: "mdx",
  sql: "sql",
  graphql: "graphql",
  gql: "graphql",
  dockerfile: "dockerfile",
  makefile: "makefile",
  lua: "lua",
  php: "php",
  swift: "swift",
  r: "r",
  dart: "dart",
  zig: "zig",
  nim: "nim",
  elixir: "elixir",
  ex: "elixir",
  exs: "elixir",
  erl: "erlang",
  hrl: "erlang",
  clj: "clojure",
  cljs: "clojure",
  fs: "fsharp",
  fsi: "fsharp",
  fsx: "fsharp",
  scala: "scala",
  groovy: "groovy",
  pl: "perl",
  pm: "perl",
} as const;

// Special file name mappings
export const FILENAME_TO_LANGUAGE: Record<string, string> = {
  Dockerfile: "dockerfile",
  Makefile: "makefile",
  Rakefile: "ruby",
  Gemfile: "ruby",
  Vagrantfile: "ruby",
  ".gitignore": "gitignore",
  ".gitattributes": "gitattributes",
  ".editorconfig": "ini",
  ".env": "dotenv",
  ".env.local": "dotenv",
  ".env.development": "dotenv",
  ".env.production": "dotenv",
  "tsconfig.json": "jsonc",
  "jsconfig.json": "jsonc",
  ".prettierrc": "json",
  ".eslintrc": "json",
  "package.json": "json",
  "cargo.toml": "toml",
  "Cargo.toml": "toml",
} as const;

// CSS class names
export const CSS_CLASSES = {
  container: "diff-viewer",
  split: "diff-viewer--split",
  unified: "diff-viewer--unified",
  hunk: "diff-hunk",
  hunkHeader: "diff-hunk-header",
  line: "diff-line",
  lineNumber: "diff-line-number",
  lineContent: "diff-line-content",
  addition: "diff-line--addition",
  deletion: "diff-line--deletion",
  context: "diff-line--context",
  inlineAdded: "diff-inline--added",
  inlineRemoved: "diff-inline--removed",
  noNewline: "diff-no-newline",
  empty: "diff-line--empty",
} as const;

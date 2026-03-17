/// Language registry with comprehensive language support
/// Manages tree-sitter language loading and detection
use std::collections::HashMap;
use tree_sitter::Language;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LanguageId {
    TypeScript,
    TypeScriptReact,
    JavaScript,
    JavaScriptReact,
    Rust,
    Python,
    Go,
    C,
    Cpp,
    CSharp,
    Java,
    Kotlin,
    Swift,
    Ruby,
    PHP,
    SQL,
    Json,
    Yaml,
    Toml,
    Xml,
    Html,
    Css,
    Bash,
    Lua,
    Vim,
    Unknown,
}

impl LanguageId {
    /// Get tree-sitter language for this ID, if available
    pub fn get_language(&self) -> Option<Language> {
        match self {
            Self::TypeScript => Some(tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()),
            Self::TypeScriptReact => Some(tree_sitter_typescript::LANGUAGE_TSX.into()),
            Self::JavaScript => {
                // tree-sitter-javascript can be used for JS if available
                // For now, fallback to TypeScript as they're similar
                Some(tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into())
            }
            Self::JavaScriptReact => Some(tree_sitter_typescript::LANGUAGE_TSX.into()),
            Self::Rust => Some(tree_sitter_rust::LANGUAGE.into()),
            // All other languages would need their respective crates
            // Placeholder for future expansion
            _ => None,
        }
    }

    /// Get all file extensions for this language
    pub fn extensions(&self) -> &'static [&'static str] {
        match self {
            Self::TypeScript => &["ts"],
            Self::TypeScriptReact => &["tsx"],
            Self::JavaScript => &["js", "mjs", "cjs"],
            Self::JavaScriptReact => &["jsx"],
            Self::Rust => &["rs"],
            Self::Python => &["py", "pyw"],
            Self::Go => &["go"],
            Self::C => &["c", "h"],
            Self::Cpp => &["cpp", "cc", "cxx", "hpp", "hh"],
            Self::CSharp => &["cs"],
            Self::Java => &["java"],
            Self::Kotlin => &["kt", "kts"],
            Self::Swift => &["swift"],
            Self::Ruby => &["rb", "erb"],
            Self::PHP => &["php"],
            Self::SQL => &["sql"],
            Self::Json => &["json", "jsonc"],
            Self::Yaml => &["yaml", "yml"],
            Self::Toml => &["toml"],
            Self::Xml => &["xml"],
            Self::Html => &["html", "htm"],
            Self::Css => &["css"],
            Self::Bash => &["sh", "bash", "zsh"],
            Self::Lua => &["lua"],
            Self::Vim => &["vim"],
            Self::Unknown => &[],
        }
    }
}

/// Registry for managing language detection and metadata
pub struct LanguageRegistry {
    ext_map: HashMap<String, LanguageId>,
}

impl LanguageRegistry {
    pub fn new() -> Self {
        let mut ext_map = HashMap::new();

        // Build extension to language mapping
        for lang in &[
            LanguageId::TypeScript,
            LanguageId::TypeScriptReact,
            LanguageId::JavaScript,
            LanguageId::JavaScriptReact,
            LanguageId::Rust,
            LanguageId::Python,
            LanguageId::Go,
            LanguageId::C,
            LanguageId::Cpp,
            LanguageId::CSharp,
            LanguageId::Java,
            LanguageId::Kotlin,
            LanguageId::Swift,
            LanguageId::Ruby,
            LanguageId::PHP,
            LanguageId::SQL,
            LanguageId::Json,
            LanguageId::Yaml,
            LanguageId::Toml,
            LanguageId::Xml,
            LanguageId::Html,
            LanguageId::Css,
            LanguageId::Bash,
            LanguageId::Lua,
            LanguageId::Vim,
        ] {
            for ext in lang.extensions() {
                ext_map.insert(ext.to_string(), *lang);
            }
        }

        Self { ext_map }
    }

    /// Detect language from file path
    pub fn detect_language(&self, file_path: &str) -> LanguageId {
        use std::path::Path;

        let path = Path::new(file_path);

        // Check extension first
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            if let Some(&lang) = self.ext_map.get(ext) {
                return lang;
            }
        }

        // Check filename patterns
        if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
            match file_name {
                "Makefile" | "makefile" => return LanguageId::Bash,
                "Dockerfile" => return LanguageId::Bash,
                "Gemfile" => return LanguageId::Ruby,
                "Rakefile" => return LanguageId::Ruby,
                _ => {}
            }
        }

        LanguageId::Unknown
    }

    /// Get tree-sitter language for a file path
    pub fn get_language_for_path(&self, file_path: &str) -> Option<Language> {
        let lang_id = self.detect_language(file_path);
        lang_id.get_language()
    }
}

impl Default for LanguageRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_language_detection() {
        let registry = LanguageRegistry::new();

        assert_eq!(registry.detect_language("main.ts"), LanguageId::TypeScript);
        assert_eq!(registry.detect_language("App.tsx"), LanguageId::TypeScriptReact);
        assert_eq!(registry.detect_language("lib.rs"), LanguageId::Rust);
        assert_eq!(registry.detect_language("script.py"), LanguageId::Python);
        assert_eq!(registry.detect_language("Makefile"), LanguageId::Bash);
    }

    #[test]
    fn test_file_extensions() {
        let ts = LanguageId::TypeScript;
        assert!(ts.extensions().contains(&"ts"));
        assert!(!ts.extensions().contains(&"tsx"));
    }
}

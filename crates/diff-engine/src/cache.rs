/// Caching layer for syntax resolution and language detection
/// Prevents repeated parsing/detection of the same file types
use std::sync::Arc;

use lru::LruCache;
use parking_lot::RwLock;
use std::num::NonZeroUsize;
use syntect::parsing::SyntaxReference;

/// Cache for syntax resolution by file path
/// LRU with 512 entries (typical projects have < 512 unique file types)
pub struct SyntaxCache {
    cache: Arc<RwLock<LruCache<String, Arc<SyntaxReference>>>>,
}

impl SyntaxCache {
    pub fn new() -> Self {
        let cache_size = NonZeroUsize::new(512).unwrap();
        Self {
            cache: Arc::new(RwLock::new(LruCache::new(cache_size))),
        }
    }

    pub fn get(&self, key: &str) -> Option<Arc<SyntaxReference>> {
        self.cache.write().get(key).cloned()
    }

    pub fn insert(&self, key: String, value: Arc<SyntaxReference>) {
        self.cache.write().put(key, value);
    }

    pub fn stats(&self) -> (usize, usize) {
        let cache = self.cache.read();
        (cache.len(), cache.cap().get())
    }

    pub fn clear(&self) {
        self.cache.write().clear();
    }
}

impl Clone for SyntaxCache {
    fn clone(&self) -> Self {
        Self {
            cache: self.cache.clone(),
        }
    }
}

impl Default for SyntaxCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Cache for theme lookups
/// Since themes are loaded once per engine lifecycle, this is a simple store
pub struct ThemeCache {
    themes: Arc<RwLock<std::collections::HashMap<String, Arc<syntect::highlighting::Theme>>>>,
}

impl ThemeCache {
    pub fn new() -> Self {
        Self {
            themes: Arc::new(RwLock::new(std::collections::HashMap::new())),
        }
    }

    pub fn get(&self, name: &str) -> Option<Arc<syntect::highlighting::Theme>> {
        self.themes.read().get(name).cloned()
    }

    pub fn insert(&self, name: String, theme: Arc<syntect::highlighting::Theme>) {
        self.themes.write().insert(name, theme);
    }

    pub fn clear(&self) {
        self.themes.write().clear();
    }
}

impl Clone for ThemeCache {
    fn clone(&self) -> Self {
        Self {
            themes: self.themes.clone(),
        }
    }
}

impl Default for ThemeCache {
    fn default() -> Self {
        Self::new()
    }
}

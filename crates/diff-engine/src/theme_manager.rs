/// Theme management and customization
/// Supports multiple themes and theme configuration
use std::collections::HashMap;
use std::sync::Arc;
use syntect::highlighting::{Theme, ThemeSet};

/// Supported themes in the system
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ThemeName {
    Base16OceanDark,
    Base16OceanLight,
    Monokai,
    SolarizedDark,
    SolarizedLight,
    Dracula,
    OneHalfDark,
    OneHalfLight,
}

impl ThemeName {
    /// Map internal theme name to syntect theme name
    pub fn syntect_name(&self) -> &'static str {
        match self {
            Self::Base16OceanDark => "base16-ocean.dark",
            Self::Base16OceanLight => "base16-ocean.light",
            Self::Monokai => "Monokai Extended",
            Self::SolarizedDark => "Solarized (dark)",
            Self::SolarizedLight => "Solarized (light)",
            Self::Dracula => "Dracula",
            Self::OneHalfDark => "One Half Dark",
            Self::OneHalfLight => "One Half Light",
        }
    }

    /// Parse from string (e.g., from config)
    pub fn from_string(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "base16-ocean-dark" | "base16-ocean.dark" => Some(Self::Base16OceanDark),
            "base16-ocean-light" | "base16-ocean.light" => Some(Self::Base16OceanLight),
            "monokai" => Some(Self::Monokai),
            "solarized-dark" | "solarized (dark)" => Some(Self::SolarizedDark),
            "solarized-light" | "solarized (light)" => Some(Self::SolarizedLight),
            "dracula" => Some(Self::Dracula),
            "one-half-dark" | "one half dark" => Some(Self::OneHalfDark),
            "one-half-light" | "one half light" => Some(Self::OneHalfLight),
            _ => None,
        }
    }
}

impl Default for ThemeName {
    fn default() -> Self {
        Self::Base16OceanDark
    }
}

/// Theme manager with caching and lazy loading
pub struct ThemeManager {
    requested_theme: ThemeName,
    cached_themes: Arc<parking_lot::RwLock<HashMap<ThemeName, Arc<Theme>>>>,
    theme_set: Arc<ThemeSet>,
}

impl ThemeManager {
    pub fn new(theme: ThemeName) -> Result<Self, String> {
        let theme_set = Arc::new(ThemeSet::load_defaults());
        
        Ok(Self {
            requested_theme: theme,
            cached_themes: Arc::new(parking_lot::RwLock::new(HashMap::new())),
            theme_set,
        })
    }

    /// Get the current theme, loading from cache or disk if necessary
    pub fn get_theme(&self) -> Result<Arc<Theme>, String> {
        // Check cache first
        {
            let cache = self.cached_themes.read();
            if let Some(theme) = cache.get(&self.requested_theme) {
                return Ok(theme.clone());
            }
        }

        // Load from theme set
        let theme = self.theme_set
            .themes
            .get(self.requested_theme.syntect_name())
            .cloned()
            .or_else(|| {
                // Fallback to default if requested theme not found
                self.theme_set.themes.get("base16-ocean.dark").cloned()
            })
            .ok_or_else(|| "no themes available in theme set".to_string())?;

        let theme = Arc::new(theme);
        
        // Cache it
        self.cached_themes.write().insert(self.requested_theme, theme.clone());
        
        Ok(theme)
    }

    /// Get a different theme by name
    pub fn get_theme_by_name(&self, name: ThemeName) -> Result<Arc<Theme>, String> {
        // Check cache first
        {
            let cache = self.cached_themes.read();
            if let Some(theme) = cache.get(&name) {
                return Ok(theme.clone());
            }
        }

        // Load from theme set
        let theme = self.theme_set
            .themes
            .get(name.syntect_name())
            .cloned()
            .ok_or_else(|| format!("theme not found: {}", name.syntect_name()))?;

        let theme = Arc::new(theme);
        
        // Cache it
        self.cached_themes.write().insert(name, theme.clone());
        
        Ok(theme)
    }

    /// Change the current theme
    pub fn set_theme(&mut self, theme: ThemeName) -> Result<(), String> {
        // Validate that the theme exists before switching
        let _ = self.get_theme_by_name(theme)?;
        self.requested_theme = theme;
        Ok(())
    }

    /// Get available themes
    pub fn available_themes(&self) -> Vec<&'static str> {
        vec![
            "base16-ocean.dark",
            "base16-ocean.light",
            "Monokai Extended",
            "Solarized (dark)",
            "Solarized (light)",
            "Dracula",
            "One Half Dark",
            "One Half Light",
        ]
    }

    /// Clear theme cache (useful when system theme changes)
    pub fn clear_cache(&self) {
        self.cached_themes.write().clear();
    }
}

impl Clone for ThemeManager {
    fn clone(&self) -> Self {
        Self {
            requested_theme: self.requested_theme,
            cached_themes: self.cached_themes.clone(),
            theme_set: self.theme_set.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_theme_name_parsing() {
        assert_eq!(ThemeName::from_string("base16-ocean.dark"), Some(ThemeName::Base16OceanDark));
        assert_eq!(ThemeName::from_string("dracula"), Some(ThemeName::Dracula));
        assert_eq!(ThemeName::from_string("invalid"), None);
    }

    #[test]
    fn test_theme_manager_creation() {
        let manager = ThemeManager::new(ThemeName::Base16OceanDark);
        assert!(manager.is_ok());
    }

    #[test]
    fn test_theme_loading() {
        let manager = ThemeManager::new(ThemeName::Base16OceanDark).expect("create manager");
        let theme = manager.get_theme();
        assert!(theme.is_ok());
    }
}

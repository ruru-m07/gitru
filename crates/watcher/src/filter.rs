use crate::error::{WatcherError, WatcherResult};
use ignore::WalkBuilder;
use std::path::{Path, PathBuf};
use tracing::debug;

/// Filter for .gitignore rules
#[derive(Debug)]
pub struct GitignoreFilter {
    repo_path: PathBuf,
    extra_ignores: Vec<String>,
}

impl GitignoreFilter {
    /// Create a new gitignore filter
    pub fn new<P: AsRef<Path>>(repo_path: P, extra_ignores: Vec<String>) -> WatcherResult<Self> {
        let repo_path = repo_path.as_ref().to_path_buf();

        if !repo_path.exists() {
            return Err(WatcherError::PathNotFound(repo_path.display().to_string()));
        }

        debug!("Initializing GitignoreFilter for {:?}", repo_path);
        debug!("Extra ignore patterns: {:?}", extra_ignores);

        Ok(Self {
            repo_path,
            extra_ignores,
        })
    }

    /// Check if a path should be ignored
    pub fn is_ignored<P: AsRef<Path>>(&self, path: P) -> bool {
        let path = path.as_ref();

        // Quick check for common patterns
        if self.is_common_ignore(path) {
            return true;
        }

        // Use ignore crate for full .gitignore support
        // let mut builder = WalkBuilder::new(&self.repo_path);
        // builder.max_depth(Some(0)); // We just want the matcher, not to walk

        // // Add extra ignores
        // for pattern in &self.extra_ignores {
        //     builder.add_custom_ignore_filename(pattern);
        // }

        // // Build the matcher
        // let walker = builder.build();

        // // Check if path matches ignore rules
        // // We need to actually walk to get the ignore matcher
        // let overrides = builder.overrides();

        // For now, use a simpler approach with the ignore crate
        self.check_with_gitignore(path)
    }

    /// Check against .gitignore files
    fn check_with_gitignore(&self, path: &Path) -> bool {
        // Use WalkBuilder to check if path should be ignored
        let mut builder = WalkBuilder::new(path);
        builder.hidden(false); // Don't automatically ignore hidden files
        builder.parents(true); // Respect parent .gitignore files
        builder.git_ignore(true); // Enable .gitignore parsing
        builder.git_global(true); // Enable global gitignore
        builder.git_exclude(true); // Enable .git/info/exclude

        // Add custom ignores
        for pattern in &self.extra_ignores {
            // This is a simplified approach - in production, use proper override builder
            if path.to_string_lossy().contains(pattern) {
                return true;
            }
        }

        // Try to walk the path - if it's ignored, the walk will skip it
        let mut is_ignored = false;
        for result in builder.build() {
            match result {
                Ok(entry) => {
                    // If we get an entry, check if it's the path we're looking for
                    if entry.path() == path {
                        is_ignored = false;
                    }
                }
                Err(_) => {
                    // Error might indicate the path is ignored or doesn't exist
                    is_ignored = true;
                }
            }
        }

        is_ignored
    }

    /// Quick check for common ignore patterns
    fn is_common_ignore(&self, path: &Path) -> bool {
        let path_str = path.to_string_lossy();

        // Check extra ignore patterns
        for pattern in &self.extra_ignores {
            if pattern.starts_with('*') {
                // Wildcard pattern
                let suffix = &pattern[1..];
                if path_str.ends_with(suffix) {
                    debug!("Path {:?} ignored by pattern {}", path, pattern);
                    return true;
                }
            } else if path_str.contains(pattern) {
                debug!("Path {:?} ignored by pattern {}", path, pattern);
                return true;
            }
        }

        // Common patterns
        let common_ignores = [".git", "node_modules", "target", ".DS_Store"];
        for ignore in &common_ignores {
            if path_str.contains(ignore) {
                debug!("Path {:?} ignored by common pattern {}", path, ignore);
                return true;
            }
        }

        false
    }

    /// Check if path is inside repository
    pub fn is_in_repo<P: AsRef<Path>>(&self, path: P) -> bool {
        path.as_ref().starts_with(&self.repo_path)
    }

    /// Get relative path from repo root
    pub fn make_relative<P: AsRef<Path>>(&self, path: P) -> Option<PathBuf> {
        path.as_ref()
            .strip_prefix(&self.repo_path)
            .ok()
            .map(|p| p.to_path_buf())
    }

    /// Reload gitignore rules (call after .gitignore changes)
    pub fn reload(&mut self) -> WatcherResult<()> {
        debug!("Reloading gitignore rules for {:?}", self.repo_path);
        // The ignore crate automatically reloads on each check,
        // so we don't need to do anything special here
        Ok(())
    }

    /// Get repository path
    pub fn repo_path(&self) -> &Path {
        &self.repo_path
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_repo() -> (TempDir, PathBuf) {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().to_path_buf();

        // Create .gitignore
        let gitignore_content = "*.log\ntarget/\nnode_modules/\n";
        fs::write(repo_path.join(".gitignore"), gitignore_content).unwrap();

        // Create some files
        fs::create_dir_all(repo_path.join("src")).unwrap();
        fs::write(repo_path.join("src/main.rs"), "fn main() {}").unwrap();
        fs::create_dir_all(repo_path.join("target")).unwrap();
        fs::write(repo_path.join("test.log"), "log").unwrap();

        (temp_dir, repo_path)
    }

    #[test]
    fn test_filter_creation() {
        let (_temp, repo_path) = create_test_repo();
        let filter = GitignoreFilter::new(&repo_path, vec![]).unwrap();
        assert_eq!(filter.repo_path(), repo_path);
    }

    #[test]
    fn test_common_ignores() {
        let (_temp, repo_path) = create_test_repo();
        let filter = GitignoreFilter::new(&repo_path, vec![]).unwrap();

        // These should be ignored by default
        assert!(filter.is_ignored(repo_path.join(".git/config")));
        assert!(filter.is_ignored(repo_path.join("node_modules/package")));
        assert!(filter.is_ignored(repo_path.join("target/debug/app")));
    }

    #[test]
    fn test_extra_ignores() {
        let (_temp, repo_path) = create_test_repo();
        let filter =
            GitignoreFilter::new(&repo_path, vec!["*.tmp".to_string(), "cache".to_string()])
                .unwrap();

        assert!(filter.is_ignored(repo_path.join("file.tmp")));
        assert!(filter.is_ignored(repo_path.join("cache/data")));
    }

    #[test]
    fn test_relative_path() {
        let (_temp, repo_path) = create_test_repo();
        let filter = GitignoreFilter::new(&repo_path, vec![]).unwrap();

        let full_path = repo_path.join("src/main.rs");
        let relative = filter.make_relative(&full_path).unwrap();
        assert_eq!(relative, PathBuf::from("src/main.rs"));
    }

    #[test]
    fn test_is_in_repo() {
        let (_temp, repo_path) = create_test_repo();
        let filter = GitignoreFilter::new(&repo_path, vec![]).unwrap();

        assert!(filter.is_in_repo(repo_path.join("src/main.rs")));
        assert!(!filter.is_in_repo(PathBuf::from("/other/path")));
    }
}

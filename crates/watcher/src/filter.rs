use crate::error::{WatcherError, WatcherResult};
use ignore::WalkBuilder;
use std::path::{Path, PathBuf};
use tracing::debug;

#[derive(Debug)]
pub struct GitignoreFilter {
    repo_path: PathBuf,
    extra_ignores: Vec<String>,
}

impl GitignoreFilter {
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

    pub fn is_ignored<P: AsRef<Path>>(&self, path: P) -> bool {
        let path = path.as_ref();

        if self.is_common_ignore(path) {
            return true;
        }

        self.check_with_gitignore(path)
    }

    fn check_with_gitignore(&self, path: &Path) -> bool {
        let mut builder = WalkBuilder::new(path);
        builder.hidden(false); // Don't automatically ignore hidden files
        builder.parents(true); // Respect parent .gitignore files
        builder.git_ignore(true); // Enable .gitignore parsing
        builder.git_global(true); // Enable global gitignore
        builder.git_exclude(true); // Enable .git/info/exclude

        for pattern in &self.extra_ignores {
            if path.to_string_lossy().contains(pattern) {
                return true;
            }
        }

        let mut is_ignored = false;
        for result in builder.build() {
            match result {
                Ok(entry) => {
                    if entry.path() == path {
                        is_ignored = false;
                    }
                }
                Err(_) => {
                    is_ignored = true;
                }
            }
        }

        is_ignored
    }

    fn is_common_ignore(&self, path: &Path) -> bool {
        let path_str = path.to_string_lossy();

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

        let common_ignores = [".git", "node_modules", "target", ".DS_Store"];
        for ignore in &common_ignores {
            if path_str.contains(ignore) {
                debug!("Path {:?} ignored by common pattern {}", path, ignore);
                return true;
            }
        }

        false
    }

    pub fn is_in_repo<P: AsRef<Path>>(&self, path: P) -> bool {
        path.as_ref().starts_with(&self.repo_path)
    }

    pub fn make_relative<P: AsRef<Path>>(&self, path: P) -> Option<PathBuf> {
        path.as_ref()
            .strip_prefix(&self.repo_path)
            .ok()
            .map(|p| p.to_path_buf())
    }

    pub fn reload(&mut self) -> WatcherResult<()> {
        debug!("Reloading gitignore rules for {:?}", self.repo_path);
        Ok(())
    }

    pub fn repo_path(&self) -> &Path {
        &self.repo_path
    }
}

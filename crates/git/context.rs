use std::sync::Arc;

use crate::{cache::RepoCache, runner::GitCommandRunner};

pub struct RepoContext {
    pub repo_path: String,
    pub runner: GitCommandRunner,
    pub cache: Arc<RepoCache>,
}

impl RepoContext {
    pub fn new(repo_path: &str) -> Result<Self, String> {
        let runner = GitCommandRunner::new(repo_path)
            .map_err(|e| format!("Failed to create Git runner: {e}"))?;

        Ok(Self {
            repo_path: repo_path.to_string(),
            runner,
            cache: Arc::new(RepoCache::new()),
        })
    }
}

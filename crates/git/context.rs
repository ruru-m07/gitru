use crate::runner::GitCommandRunner;

pub struct RepoContext {
    pub repo_path: String,
    pub runner: GitCommandRunner,
}

impl RepoContext {
    pub fn new(repo_path: &str) -> Result<Self, String> {
        let runner = GitCommandRunner::new(repo_path)
            .map_err(|e| format!("Failed to create Git runner: {e}"))?;

        Ok(Self {
            repo_path: repo_path.to_string(),
            runner,
        })
    }
}

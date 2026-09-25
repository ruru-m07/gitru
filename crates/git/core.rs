use std::path::Path;
use std::sync::Arc;

use crate::{
    AppState,
    context::{RepoContext, RepositoryWatchPaths},
    runner::GitRunOptions,
    service::{
        actions::ActionService, branch::BranchService, commit::CommitService, diff::DiffService,
        history::HistoryService, operation::OperationService, origin::OriginService,
        pickaxe::PickaxeService, query::QueryService, rebase::RebaseService, stash::StashService,
    },
};

pub struct RepoServices {
    ctx: Arc<RepoContext>,
}

impl RepoServices {
    pub fn new(repo_path: &str) -> Result<Self, String> {
        Ok(Self {
            ctx: Arc::new(RepoContext::new(repo_path)?),
        })
    }

    pub fn diff(&self) -> DiffService {
        DiffService::new(self.ctx.clone())
    }

    pub fn branch(&self) -> BranchService {
        BranchService::new(self.ctx.clone())
    }

    pub fn history(&self) -> HistoryService {
        HistoryService::new(self.ctx.clone())
    }

    pub fn origin(&self) -> OriginService {
        OriginService::new(self.ctx.clone())
    }

    pub fn commit(&self) -> CommitService {
        CommitService::new(self.ctx.clone())
    }

    pub fn action(&self) -> ActionService {
        ActionService::new(self.ctx.clone())
    }

    pub fn query(&self) -> QueryService {
        QueryService::new(self.ctx.clone())
    }

    pub fn stash(&self) -> StashService {
        StashService::new(self.ctx.clone())
    }

    pub fn pickaxe(&self) -> PickaxeService {
        PickaxeService::new(self.ctx.clone())
    }

    pub fn operation(&self) -> OperationService {
        OperationService::new(self.ctx.clone())
    }

    pub fn rebase(&self) -> RebaseService {
        RebaseService::new(self.ctx.clone())
    }

    pub fn watch_paths(&self) -> Result<RepositoryWatchPaths, String> {
        RepositoryWatchPaths::discover(&self.ctx.repo_path)
    }

    pub async fn validate_worktree(&self) -> Result<(), String> {
        let top_level = self
            .ctx
            .runner
            .run_with_options(
                &["rev-parse", "--show-toplevel"],
                GitRunOptions::default_read(),
            )
            .await?;
        let selected_path = Path::new(&self.ctx.repo_path)
            .canonicalize()
            .map_err(|error| format!("Failed to resolve selected repository path: {error}"))?;
        let top_level_path = Path::new(top_level.trim())
            .canonicalize()
            .map_err(|error| format!("Failed to resolve Git worktree root: {error}"))?;

        if canonical_paths_equal(&selected_path, &top_level_path) {
            Ok(())
        } else {
            Err(format!(
                "Select the Git worktree root: {}",
                top_level_path.display()
            ))
        }
    }

    pub fn invalidate_cache_namespaces(&self, namespaces: &[&str]) {
        self.ctx.cache.invalidate_namespaces(namespaces);
    }

    pub fn invalidate_cache(&self) {
        self.ctx.cache.invalidate_all();
    }
}

#[cfg(not(windows))]
fn canonical_paths_equal(left: &Path, right: &Path) -> bool {
    left == right
}

#[cfg(windows)]
fn canonical_paths_equal(left: &Path, right: &Path) -> bool {
    fn normalized(path: &Path) -> String {
        path.to_string_lossy()
            .trim_start_matches(r"\\?\")
            .replace('/', r"\")
            .to_lowercase()
    }

    normalized(left) == normalized(right)
}

pub async fn get_services(
    state: tauri::State<'_, AppState>,
    context_id: &str,
) -> Result<Arc<RepoServices>, String> {
    let lock = state.services.read().await;
    lock.get(context_id)
        .cloned()
        .ok_or_else(|| "Context not initialized".to_string())
}

pub async fn insert_services(
    state: tauri::State<'_, AppState>,
    context_id: String,
    services: Arc<RepoServices>,
) {
    let mut lock = state.services.write().await;
    lock.insert(context_id, services);
}

pub async fn remove_services(state: tauri::State<'_, AppState>, context_id: &str) -> bool {
    let mut lock = state.services.write().await;
    lock.remove(context_id).is_some()
}

#[cfg(test)]
mod tests {
    use super::RepoServices;
    use std::process::Command;
    use tempfile::tempdir;

    #[tokio::test]
    async fn cli_supported_repository_formats_do_not_require_libgit2() {
        let directory = tempdir().expect("temp dir");
        let output = Command::new("git")
            .args(["init", "--object-format=sha256"])
            .arg(directory.path())
            .output()
            .expect("run git init");

        // Older Git versions cannot create SHA-256 repositories. On newer
        // versions this guards against making libgit2 watcher discovery a
        // prerequisite for otherwise CLI-compatible repository services.
        if !output.status.success() {
            return;
        }

        let services = RepoServices::new(directory.path().to_str().unwrap())
            .expect("construct CLI-backed services");
        services
            .validate_worktree()
            .await
            .expect("validate SHA-256 worktree with Git CLI");
    }

    #[tokio::test]
    async fn worktree_validation_rejects_a_plain_directory() {
        let directory = tempdir().expect("temp dir");
        let services = RepoServices::new(directory.path().to_str().unwrap())
            .expect("construct path-backed services");

        assert!(services.validate_worktree().await.is_err());
    }

    #[tokio::test]
    async fn worktree_validation_rejects_a_nested_directory() {
        let directory = tempdir().expect("temp dir");
        git2::Repository::init(directory.path()).expect("init repository");
        let nested = directory.path().join("nested");
        std::fs::create_dir(&nested).expect("create nested directory");
        let services =
            RepoServices::new(nested.to_str().unwrap()).expect("construct path-backed services");

        let error = services
            .validate_worktree()
            .await
            .expect_err("nested directory must not be stored as repository root");
        assert!(error.contains("Select the Git worktree root"));
    }
}

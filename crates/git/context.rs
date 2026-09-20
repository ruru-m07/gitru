use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::{cache::RepoCache, runner::GitCommandRunner};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RepositoryWatchPaths {
    pub worktree: PathBuf,
    pub git_dir: PathBuf,
    pub common_dir: PathBuf,
}

impl RepositoryWatchPaths {
    pub(crate) fn discover(repo_path: &str) -> Result<Self, String> {
        let repository = open_worktree_repository(repo_path)?;
        let worktree = repository
            .workdir()
            .expect("worktree repository validation must reject bare repositories");

        Ok(Self {
            worktree: canonicalize_watch_path(worktree)?,
            git_dir: canonicalize_watch_path(repository.path())?,
            common_dir: canonicalize_watch_path(repository.commondir())?,
        })
    }
}

fn open_worktree_repository(repo_path: &str) -> Result<git2::Repository, String> {
    let repository = git2::Repository::open(repo_path)
        .map_err(|error| format!("Failed to open Git repository: {error}"))?;
    if repository.workdir().is_none() {
        return Err("Bare Git repositories are not supported".to_string());
    }
    Ok(repository)
}

fn canonicalize_watch_path(path: &Path) -> Result<PathBuf, String> {
    path.canonicalize().map_err(|error| {
        format!(
            "Failed to resolve repository path {}: {error}",
            path.display()
        )
    })
}

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

#[cfg(test)]
mod tests {
    use super::{RepoContext, RepositoryWatchPaths};
    use tempfile::tempdir;

    #[test]
    fn discovers_normal_repository_watch_paths() {
        let directory = tempdir().expect("temp dir");
        git2::Repository::init(directory.path()).expect("init repository");

        let paths = RepositoryWatchPaths::discover(
            directory
                .path()
                .to_str()
                .expect("temporary path should be UTF-8"),
        )
        .expect("discover watch paths");

        assert_eq!(paths.worktree, directory.path().canonicalize().unwrap());
        assert_eq!(
            paths.git_dir,
            directory.path().join(".git").canonicalize().unwrap()
        );
        assert_eq!(paths.common_dir, paths.git_dir);
    }

    #[test]
    fn discovers_linked_worktree_private_and_common_git_dirs() {
        let directory = tempdir().expect("temp dir");
        let main_path = directory.path().join("main");
        let linked_path = directory.path().join("linked");
        let repository = git2::Repository::init(&main_path).expect("init repository");
        let tree_id = repository
            .index()
            .and_then(|mut index| index.write_tree())
            .expect("write empty tree");
        {
            let tree = repository.find_tree(tree_id).expect("find empty tree");
            let signature = git2::Signature::now("Gitru", "gitru@example.com").unwrap();
            repository
                .commit(
                    Some("HEAD"),
                    &signature,
                    &signature,
                    "Initial commit",
                    &tree,
                    &[],
                )
                .expect("create initial commit");
        }
        repository
            .worktree("linked", &linked_path, None)
            .expect("create linked worktree");

        let paths = RepositoryWatchPaths::discover(linked_path.to_str().unwrap())
            .expect("discover linked worktree paths");

        assert_eq!(paths.worktree, linked_path.canonicalize().unwrap());
        assert_ne!(paths.git_dir, paths.common_dir);
        assert_eq!(
            paths.common_dir,
            main_path.join(".git").canonicalize().unwrap()
        );
        assert!(
            paths
                .git_dir
                .starts_with(paths.common_dir.join("worktrees"))
        );
        assert!(linked_path.join(".git").is_file());
    }

    #[test]
    fn watcher_path_failure_does_not_prevent_context_construction() {
        let directory = tempdir().expect("temp dir");
        git2::Repository::init(directory.path()).expect("init repository");
        let repo_path = directory.path().to_str().unwrap();

        std::fs::rename(
            directory.path().join(".git"),
            directory.path().join(".git-away"),
        )
        .expect("make watcher path unavailable");

        let _context = RepoContext::new(repo_path).expect("construct repository context");
        assert!(RepositoryWatchPaths::discover(repo_path).is_err());
    }
}

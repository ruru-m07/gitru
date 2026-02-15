use std::fmt;

#[derive(Debug)]
pub enum GitError {
    RepositoryOpen(git2::Error),
    InvalidPath(String),
}

impl fmt::Display for GitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GitError::RepositoryOpen(e) => {
                write!(f, "Failed to open repository: {}", e.message())
            }
            GitError::InvalidPath(msg) => write!(f, "{}", msg),
        }
    }
}

impl std::error::Error for GitError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            GitError::RepositoryOpen(e) => Some(e),
            GitError::InvalidPath(_) => None,
        }
    }
}

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NavigationState {
    pub paths: Vec<String>,
    pub current_index: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionNavigationInfo {
    pub session_id: String,
    pub can_go_back: bool,
    pub can_go_forward: bool,
    pub current_path: Option<String>,
}

/// Manages navigation history for each session/tab
pub struct SessionManager {
    history_store: Arc<RwLock<HashMap<String, NavigationState>>>,
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            history_store: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Push a new path to the session's navigation history
    pub async fn push_to_history(
        &self,
        session_id: String,
        route_path: String,
    ) -> SessionNavigationInfo {
        let mut store: tokio::sync::RwLockWriteGuard<'_, HashMap<String, NavigationState>> =
            self.history_store.write().await;

        let current = store
            .entry(session_id.clone())
            .or_insert_with(|| NavigationState {
                paths: vec![],
                current_index: 0,
            });

        // Normalize the path to avoid duplicates
        let normalized_path = normalize_path(&route_path);

        // If we're at the end of history and the new path differs from the last,
        // add it and increment index
        if current.current_index == current.paths.len() {
            if current.paths.last() != Some(&normalized_path) {
                current.paths.push(normalized_path.clone());
                current.current_index = current.paths.len() - 1;
            }
        } else if current.current_index < current.paths.len() {
            // If we're not at the end, truncate future history and add new path
            if current.paths.get(current.current_index) != Some(&normalized_path) {
                current.paths.truncate(current.current_index + 1);
                current.paths.push(normalized_path.clone());
                current.current_index = current.paths.len() - 1;
            }
        }

        SessionNavigationInfo {
            session_id,
            can_go_back: current.current_index > 0,
            can_go_forward: current.current_index < current.paths.len() - 1,
            current_path: current.paths.get(current.current_index).cloned(),
        }
    }

    /// Get the next path by going back in history
    pub async fn go_back(&self, session_id: &str) -> Option<SessionNavigationInfo> {
        let mut store: tokio::sync::RwLockWriteGuard<'_, HashMap<String, NavigationState>> =
            self.history_store.write().await;

        if let Some(current) = store.get_mut(session_id) {
            if current.current_index > 0 {
                current.current_index -= 1;

                return Some(SessionNavigationInfo {
                    session_id: session_id.to_string(),
                    can_go_back: current.current_index > 0,
                    can_go_forward: current.current_index < current.paths.len() - 1,
                    current_path: current.paths.get(current.current_index).cloned(),
                });
            }
        }

        None
    }

    /// Get the next path by going forward in history
    pub async fn go_forward(&self, session_id: &str) -> Option<SessionNavigationInfo> {
        let mut store: tokio::sync::RwLockWriteGuard<'_, HashMap<String, NavigationState>> =
            self.history_store.write().await;

        if let Some(current) = store.get_mut(session_id) {
            if current.current_index < current.paths.len() - 1 {
                current.current_index += 1;

                return Some(SessionNavigationInfo {
                    session_id: session_id.to_string(),
                    can_go_back: current.current_index > 0,
                    can_go_forward: current.current_index < current.paths.len() - 1,
                    current_path: current.paths.get(current.current_index).cloned(),
                });
            }
        }

        None
    }

    /// Get the current navigation state for a session
    pub async fn get_navigation_state(&self, session_id: &str) -> SessionNavigationInfo {
        let store: tokio::sync::RwLockReadGuard<'_, HashMap<String, NavigationState>> =
            self.history_store.read().await;

        if let Some(current) = store.get(session_id) {
            SessionNavigationInfo {
                session_id: session_id.to_string(),
                can_go_back: current.current_index > 0,
                can_go_forward: current.current_index < current.paths.len() - 1,
                current_path: current.paths.get(current.current_index).cloned(),
            }
        } else {
            SessionNavigationInfo {
                session_id: session_id.to_string(),
                can_go_back: false,
                can_go_forward: false,
                current_path: None,
            }
        }
    }

    /// Clear history for a session (e.g., when tab is closed)
    pub async fn clear_session_history(&self, session_id: &str) {
        let mut store: tokio::sync::RwLockWriteGuard<'_, HashMap<String, NavigationState>> =
            self.history_store.write().await;
        store.remove(session_id);
    }
}

fn normalize_path(path: &str) -> String {
    // Remove trailing slashes and normalize the path
    path.trim_end_matches('/').to_string()
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_push_to_history() {
        let manager = SessionManager::new();
        let session_id = "session-1".to_string();

        let info = manager
            .push_to_history(session_id.clone(), "/app/git".to_string())
            .await;
        assert!(!info.can_go_back);
        assert!(!info.can_go_forward);
        assert_eq!(info.current_path, Some("/app/git".to_string()));

        let info = manager
            .push_to_history(session_id.clone(), "/app/git/branches".to_string())
            .await;
        assert!(info.can_go_back);
        assert!(!info.can_go_forward);
    }

    #[tokio::test]
    async fn test_go_back_forward() {
        let manager = SessionManager::new();
        let session_id = "session-1".to_string();

        manager
            .push_to_history(session_id.clone(), "/app/git".to_string())
            .await;
        manager
            .push_to_history(session_id.clone(), "/app/git/branches".to_string())
            .await;

        if let Some(info) = manager.go_back(&session_id).await {
            assert_eq!(info.current_path, Some("/app/git".to_string()));
            assert!(!info.can_go_back);
            assert!(info.can_go_forward);
        }
    }
}

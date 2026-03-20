use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use tokio::sync::RwLock;

/// Maximum number of concurrent diff operations to allow
const MAX_CONCURRENT_OPERATIONS: usize = 5;

/// Unique identifier for a diff request
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RequestId(u64);

impl RequestId {
    fn next() -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        Self(COUNTER.fetch_add(1, Ordering::SeqCst))
    }
}

/// Cancellation token that tracks if a request should continue or be cancelled
#[derive(Clone)]
pub struct CancellationToken {
    id: RequestId,
    scope_key: Arc<str>,
    cancelled: Arc<AtomicBool>,
}

impl CancellationToken {
    /// Check if this request has been cancelled
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }

    /// Get the request ID
    pub fn id(&self) -> RequestId {
        self.id
    }

    /// Scope key for this request (typically a file path)
    pub fn scope_key(&self) -> &str {
        &self.scope_key
    }
}

/// Manages a queue of diff requests with cancellation support
/// When a new request arrives, supersedes all previous requests
pub struct RequestQueueManager {
    /// Maps request IDs to their cancellation tokens
    active_requests: Arc<RwLock<HashMap<RequestId, Arc<AtomicBool>>>>,
    /// Latest request per scope key (e.g. per file path)
    latest_by_scope: Arc<RwLock<HashMap<Arc<str>, RequestId>>>,
    /// Scope key for each request ID
    request_scopes: Arc<RwLock<HashMap<RequestId, Arc<str>>>>,
    /// Number of currently active workers
    active_workers: Arc<AtomicU64>,
}

impl RequestQueueManager {
    pub fn new() -> Self {
        Self {
            active_requests: Arc::new(RwLock::new(HashMap::new())),
            latest_by_scope: Arc::new(RwLock::new(HashMap::new())),
            request_scopes: Arc::new(RwLock::new(HashMap::new())),
            active_workers: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Register a new request and cancel only previous requests in the same scope.
    /// Returns a cancellation token for tracking this request
    pub async fn register_request(&self, scope_key: &str) -> CancellationToken {
        let new_id = RequestId::next();
        let scope_key: Arc<str> = Arc::from(scope_key);

        // If there is an older request for this scope, cancel it.
        let previous_request = {
            let mut latest = self.latest_by_scope.write().await;
            latest.insert(scope_key.clone(), new_id)
        };

        if let Some(previous_id) = previous_request {
            let previous_flag = {
                let requests = self.active_requests.read().await;
                requests.get(&previous_id).cloned()
            };

            if let Some(flag) = previous_flag {
                flag.store(true, Ordering::Release);
            }
        }

        // Register the new request.
        let mut requests = self.active_requests.write().await;
        let cancelled_flag = Arc::new(AtomicBool::new(false));
        requests.insert(new_id, cancelled_flag.clone());
        drop(requests);

        let mut scopes = self.request_scopes.write().await;
        scopes.insert(new_id, scope_key.clone());

        CancellationToken {
            id: new_id,
            scope_key,
            cancelled: cancelled_flag,
        }
    }

    /// Wait until there's an available worker slot (up to MAX_CONCURRENT_OPERATIONS)
    /// Returns true if can proceed, false if cancelled
    pub async fn acquire_worker_slot(&self, token: &CancellationToken) -> bool {
        // Fast-path: avoid entering backoff loop for a superseded request.
        if token.is_cancelled() || !self.is_latest_request(token).await {
            return false;
        }

        let mut backoff_ms = 1u64; // Start with 1ms

        loop {
            if token.is_cancelled() || !self.is_latest_request(token).await {
                return false;
            }

            let current_workers = self.active_workers.load(Ordering::Acquire);
            if current_workers < MAX_CONCURRENT_OPERATIONS as u64 {
                if self
                    .active_workers
                    .compare_exchange(
                        current_workers,
                        current_workers + 1,
                        Ordering::Release,
                        Ordering::Acquire,
                    )
                    .is_ok()
                {
                    return true;
                }
                // Retry if CAS failed
                continue;
            }

            // Exponential backoff: 1ms -> 2ms -> 4ms -> 8ms (max)
            tokio::time::sleep(tokio::time::Duration::from_millis(backoff_ms)).await;
            backoff_ms = (backoff_ms * 2).min(8); // Cap at 8ms
        }
    }

    /// Release a worker slot when done
    pub fn release_worker_slot(&self) {
        let _ = self.active_workers.fetch_sub(1, Ordering::Release);
    }

    /// Clean up a request from tracking (called when done)
    /// This is fast - just removes from hashmap
    pub async fn cleanup_request(&self, token: &CancellationToken) {
        let mut requests = self.active_requests.write().await;
        requests.remove(&token.id);
        drop(requests);

        let mut scopes = self.request_scopes.write().await;
        scopes.remove(&token.id);
        drop(scopes);

        let mut latest = self.latest_by_scope.write().await;
        if latest
            .get(token.scope_key())
            .map(|id| *id == token.id)
            .unwrap_or(false)
        {
            latest.remove(token.scope_key());
        }
    }

    /// Check if a request is still the "latest" request
    pub async fn is_latest_request(&self, token: &CancellationToken) -> bool {
        let latest = self.latest_by_scope.read().await;
        latest
            .get(token.scope_key())
            .map(|id| *id == token.id)
            .unwrap_or(false)
    }
}

impl Default for RequestQueueManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn new_request_same_scope_cancels_previous() {
        let qm = RequestQueueManager::new();
        let token1 = qm.register_request("a.txt").await;
        let token2 = qm.register_request("a.txt").await;

        // token1 should be cancelled
        assert!(token1.is_cancelled());
        // token2 should be active
        assert!(!token2.is_cancelled());
    }

    #[tokio::test]
    async fn new_request_different_scope_does_not_cancel_previous() {
        let qm = RequestQueueManager::new();
        let token1 = qm.register_request("a.txt").await;
        let token2 = qm.register_request("b.txt").await;

        assert!(!token1.is_cancelled());
        assert!(!token2.is_cancelled());
    }

    #[tokio::test]
    async fn worker_slot_respects_max_concurrent() {
        let qm = RequestQueueManager::new();
        let token = qm.register_request("a.txt").await;

        // Acquire MAX_CONCURRENT_OPERATIONS slots
        let mut acquired = 0;
        for _ in 0..MAX_CONCURRENT_OPERATIONS {
            assert!(qm.acquire_worker_slot(&token).await);
            acquired += 1;
        }

        // Next acquisition should block (we'll abort after a timeout)
        let timeout = tokio::time::timeout(
            tokio::time::Duration::from_millis(100),
            qm.acquire_worker_slot(&token),
        )
        .await;
        assert!(timeout.is_err()); // Should timeout because slots are full

        // Release all and verify we can acquire again
        for _ in 0..acquired {
            qm.release_worker_slot();
        }

        assert!(qm.acquire_worker_slot(&token).await);
    }

    #[tokio::test]
    async fn cancelled_token_prevents_worker_acquisition() {
        let qm = RequestQueueManager::new();
        let token1 = qm.register_request("a.txt").await;
        let _token2 = qm.register_request("a.txt").await;

        // token1 is cancelled, should not be able to acquire
        let result = qm.acquire_worker_slot(&token1).await;
        assert!(!result);
    }
}

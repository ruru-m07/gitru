use std::any::Any;
use std::collections::HashMap;
use std::future::Future;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tokio::sync::Notify;

pub const TTL_CURRENT_BRANCH: Duration = Duration::from_millis(500);
pub const TTL_CURRENT_BRANCH_STASH: Duration = Duration::from_millis(1000);
pub const TTL_STATUS: Duration = Duration::from_millis(500);
pub const TTL_HAS_UNCOMMITTED: Duration = Duration::from_millis(500);
pub const TTL_LIST_BRANCHES: Duration = Duration::from_millis(1000);
pub const TTL_AHEAD_BEHIND: Duration = Duration::from_millis(100);
pub const TTL_BRANCH_INFO: Duration = Duration::from_millis(300);
pub const TTL_REPOSITORY_ORIGIN: Duration = Duration::from_millis(3000);
pub const TTL_LAST_COMMIT: Duration = Duration::from_millis(300);
pub const TTL_COMMIT_BY_ID: Duration = Duration::from_millis(300);
pub const TTL_PATCH_BY_FILE_PATH: Duration = Duration::from_millis(200);
pub const TTL_HISTORY: Duration = Duration::from_millis(100);

#[derive(Clone, Copy)]
pub struct CachePolicy {
    pub namespace: &'static str,
    pub ttl: Duration,
}

type CachedValue = Arc<dyn Any + Send + Sync>;

struct CacheEntry {
    value: CachedValue,
    inserted_at: Instant,
    ttl: Duration,
}

impl CacheEntry {
    fn is_fresh(&self, now: Instant) -> bool {
        now.duration_since(self.inserted_at) <= self.ttl
    }
}

struct InFlight {
    notify: Notify,
    result: Mutex<Option<Result<CachedValue, String>>>,
}

impl InFlight {
    fn new() -> Self {
        Self {
            notify: Notify::new(),
            result: Mutex::new(None),
        }
    }
}

#[derive(Default)]
struct RepoCacheState {
    entries: HashMap<String, CacheEntry>,
    inflight: HashMap<String, Arc<InFlight>>,
}

pub struct RepoCache {
    generation: AtomicU64,
    state: Mutex<RepoCacheState>,
}

impl Default for RepoCache {
    fn default() -> Self {
        Self::new()
    }
}

impl RepoCache {
    pub fn new() -> Self {
        Self {
            generation: AtomicU64::new(0),
            state: Mutex::new(RepoCacheState::default()),
        }
    }

    pub fn invalidate_all(&self) {
        self.generation.fetch_add(1, Ordering::SeqCst);
        if let Ok(mut state) = self.state.lock() {
            state.entries.clear();
            state.inflight.clear();
        }
    }

    pub async fn get_or_refresh<T, F, Fut>(
        &self,
        policy: CachePolicy,
        key: String,
        fetcher: F,
    ) -> Result<T, String>
    where
        T: Clone + Send + Sync + 'static,
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<T, String>>,
    {
        let storage_key = self.build_storage_key(policy.namespace, &key);
        let now = Instant::now();

        let (inflight, is_leader, stale) = {
            let mut state = self
                .state
                .lock()
                .map_err(|_| "Failed to lock cache state".to_string())?;

            let stale = state
                .entries
                .get(&storage_key)
                .and_then(|entry| self.clone_typed::<T>(&entry.value));

            if let Some(entry) = state.entries.get(&storage_key) {
                if entry.is_fresh(now) {
                    if let Some(value) = self.clone_typed::<T>(&entry.value) {
                        log::debug!("cache hit key='{}'", storage_key);
                        return Ok(value);
                    }
                    state.entries.remove(&storage_key);
                }
            }

            if let Some(inflight) = state.inflight.get(&storage_key) {
                log::debug!("cache wait-inflight key='{}'", storage_key);
                (inflight.clone(), false, stale)
            } else {
                let inflight = Arc::new(InFlight::new());
                state.inflight.insert(storage_key.clone(), inflight.clone());
                log::debug!("cache miss key='{}'", storage_key);
                (inflight, true, stale)
            }
        };

        if !is_leader {
            inflight.notify.notified().await;
            let result = inflight
                .result
                .lock()
                .map_err(|_| "Failed to lock in-flight result".to_string())?
                .clone()
                .ok_or_else(|| "In-flight result missing".to_string())?;

            return self.resolve_result(result, stale, &storage_key);
        }

        let refresh_result = fetcher().await;

        match refresh_result {
            Ok(value) => {
                let cached: CachedValue = Arc::new(value.clone());

                {
                    let mut result = inflight
                        .result
                        .lock()
                        .map_err(|_| "Failed to lock in-flight result".to_string())?;
                    *result = Some(Ok(cached.clone()));
                }

                {
                    let mut state = self
                        .state
                        .lock()
                        .map_err(|_| "Failed to lock cache state".to_string())?;
                    state.entries.insert(
                        storage_key.clone(),
                        CacheEntry {
                            value: cached,
                            inserted_at: Instant::now(),
                            ttl: policy.ttl,
                        },
                    );
                    state.inflight.remove(&storage_key);
                }

                inflight.notify.notify_waiters();
                log::debug!("cache refresh-success key='{}'", storage_key);
                Ok(value)
            }
            Err(err) => {
                {
                    let mut result = inflight
                        .result
                        .lock()
                        .map_err(|_| "Failed to lock in-flight result".to_string())?;
                    *result = Some(Err(err.clone()));
                }

                {
                    let mut state = self
                        .state
                        .lock()
                        .map_err(|_| "Failed to lock cache state".to_string())?;
                    state.inflight.remove(&storage_key);
                }

                inflight.notify.notify_waiters();

                if let Some(stale_value) = stale {
                    log::warn!(
                        "cache refresh failed for key '{}': {}; serving stale value",
                        storage_key,
                        err
                    );
                    return Ok(stale_value);
                }

                log::warn!("cache refresh-failed key='{}': {}", storage_key, err);
                Err(err)
            }
        }
    }

    fn resolve_result<T>(
        &self,
        result: Result<CachedValue, String>,
        stale: Option<T>,
        storage_key: &str,
    ) -> Result<T, String>
    where
        T: Clone + Send + Sync + 'static,
    {
        match result {
            Ok(value) => self
                .clone_typed::<T>(&value)
                .ok_or_else(|| format!("Cached value type mismatch for key '{}'", storage_key)),
            Err(err) => {
                if let Some(stale_value) = stale {
                    log::warn!(
                        "cache refresh failed for key '{}': {}; serving stale value",
                        storage_key,
                        err
                    );
                    Ok(stale_value)
                } else {
                    Err(err)
                }
            }
        }
    }

    fn clone_typed<T>(&self, value: &CachedValue) -> Option<T>
    where
        T: Clone + Send + Sync + 'static,
    {
        value.downcast_ref::<T>().cloned()
    }

    fn build_storage_key(&self, namespace: &str, key: &str) -> String {
        let generation = self.generation.load(Ordering::SeqCst);
        format!("{}:{}:{}", generation, namespace, key)
    }
}

#[cfg(test)]
mod tests {
    use super::{CachePolicy, RepoCache};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;
    use tokio::runtime::Builder;

    fn run_async<F>(f: F)
    where
        F: std::future::Future<Output = ()>,
    {
        let rt = Builder::new_current_thread()
            .enable_time()
            .build()
            .expect("runtime");
        rt.block_on(f);
    }

    #[test]
    fn cache_returns_fresh_entries() {
        run_async(async {
            let cache = RepoCache::new();
            let calls = Arc::new(AtomicUsize::new(0));
            let policy = CachePolicy {
                namespace: "fresh",
                ttl: Duration::from_millis(500),
            };

            let first = cache
                .get_or_refresh(policy, "k".to_string(), {
                    let calls = calls.clone();
                    move || async move {
                        calls.fetch_add(1, Ordering::SeqCst);
                        Ok::<usize, String>(42)
                    }
                })
                .await
                .expect("first value");

            let second = cache
                .get_or_refresh(policy, "k".to_string(), {
                    let calls = calls.clone();
                    move || async move {
                        calls.fetch_add(1, Ordering::SeqCst);
                        Ok::<usize, String>(7)
                    }
                })
                .await
                .expect("second value");

            assert_eq!(first, 42);
            assert_eq!(second, 42);
            assert_eq!(calls.load(Ordering::SeqCst), 1);
        });
    }

    #[test]
    fn cache_refreshes_after_ttl() {
        run_async(async {
            let cache = RepoCache::new();
            let calls = Arc::new(AtomicUsize::new(0));
            let policy = CachePolicy {
                namespace: "ttl",
                ttl: Duration::from_millis(1),
            };

            let _ = cache
                .get_or_refresh(policy, "k".to_string(), {
                    let calls = calls.clone();
                    move || async move {
                        calls.fetch_add(1, Ordering::SeqCst);
                        Ok::<usize, String>(1)
                    }
                })
                .await
                .expect("prime value");

            tokio::time::sleep(Duration::from_millis(5)).await;

            let refreshed = cache
                .get_or_refresh(policy, "k".to_string(), {
                    let calls = calls.clone();
                    move || async move {
                        calls.fetch_add(1, Ordering::SeqCst);
                        Ok::<usize, String>(2)
                    }
                })
                .await
                .expect("refreshed value");

            assert_eq!(refreshed, 2);
            assert_eq!(calls.load(Ordering::SeqCst), 2);
        });
    }

    #[test]
    fn cache_dedupes_concurrent_requests() {
        run_async(async {
            let cache = Arc::new(RepoCache::new());
            let calls = Arc::new(AtomicUsize::new(0));
            let policy = CachePolicy {
                namespace: "singleflight",
                ttl: Duration::from_millis(500),
            };

            let mut handles = Vec::new();
            for _ in 0..8 {
                let cache = cache.clone();
                let calls = calls.clone();
                handles.push(tokio::spawn(async move {
                    cache
                        .get_or_refresh(policy, "k".to_string(), move || async move {
                            calls.fetch_add(1, Ordering::SeqCst);
                            tokio::time::sleep(Duration::from_millis(20)).await;
                            Ok::<usize, String>(9)
                        })
                        .await
                }));
            }

            for handle in handles {
                let value = handle.await.expect("join").expect("value");
                assert_eq!(value, 9);
            }

            assert_eq!(calls.load(Ordering::SeqCst), 1);
        });
    }

    #[test]
    fn cache_returns_stale_on_refresh_error() {
        run_async(async {
            let cache = RepoCache::new();
            let policy = CachePolicy {
                namespace: "stale",
                ttl: Duration::from_millis(1),
            };

            let _ = cache
                .get_or_refresh(policy, "k".to_string(), || async {
                    Ok::<String, String>("old".to_string())
                })
                .await
                .expect("prime value");

            tokio::time::sleep(Duration::from_millis(5)).await;

            let stale = cache
                .get_or_refresh(policy, "k".to_string(), || async {
                    Err::<String, String>("boom".to_string())
                })
                .await
                .expect("stale value");

            assert_eq!(stale, "old");
        });
    }

    #[test]
    fn cache_returns_error_when_no_stale_entry_exists() {
        run_async(async {
            let cache = RepoCache::new();
            let policy = CachePolicy {
                namespace: "nostale",
                ttl: Duration::from_millis(10),
            };

            let err = cache
                .get_or_refresh::<String, _, _>(policy, "k".to_string(), || async {
                    Err::<String, String>("fail".to_string())
                })
                .await
                .expect_err("expected error");

            assert_eq!(err, "fail");
        });
    }

    #[test]
    fn cache_invalidate_all_forces_refresh() {
        run_async(async {
            let cache = RepoCache::new();
            let calls = Arc::new(AtomicUsize::new(0));
            let policy = CachePolicy {
                namespace: "invalidate",
                ttl: Duration::from_secs(5),
            };

            let _ = cache
                .get_or_refresh(policy, "k".to_string(), {
                    let calls = calls.clone();
                    move || async move {
                        calls.fetch_add(1, Ordering::SeqCst);
                        Ok::<usize, String>(1)
                    }
                })
                .await
                .expect("prime value");

            cache.invalidate_all();

            let _ = cache
                .get_or_refresh(policy, "k".to_string(), {
                    let calls = calls.clone();
                    move || async move {
                        calls.fetch_add(1, Ordering::SeqCst);
                        Ok::<usize, String>(2)
                    }
                })
                .await
                .expect("post-invalidation value");

            assert_eq!(calls.load(Ordering::SeqCst), 2);
        });
    }
}

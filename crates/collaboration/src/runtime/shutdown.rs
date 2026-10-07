//! Admission and owned-task draining for an entire runtime generation.
use super::*;
use std::sync::{Mutex as StdMutex, atomic::AtomicUsize};

pub(super) struct RuntimeLifetime {
    accepting: AtomicBool,
    closed: AtomicBool,
    active: AtomicUsize,
    drained: Notify,
    pub background: StdMutex<Option<tokio::task::JoinHandle<()>>>,
    shutdown: Mutex<()>,
    credentials: StdMutex<Vec<tokio::task::JoinHandle<()>>>,
}
impl Default for RuntimeLifetime {
    fn default() -> Self {
        Self {
            accepting: AtomicBool::new(true),
            closed: AtomicBool::new(false),
            active: AtomicUsize::new(0),
            drained: Notify::new(),
            background: StdMutex::new(None),
            shutdown: Mutex::new(()),
            credentials: StdMutex::new(Vec::new()),
        }
    }
}
/// A native operation's lease survives across all awaits and owned credential
/// completion. Stale runtime owners cannot create a new lease during recovery.
pub struct RuntimeOperation {
    lifetime: Arc<RuntimeLifetime>,
}
impl Drop for RuntimeOperation {
    fn drop(&mut self) {
        if self.lifetime.active.fetch_sub(1, Ordering::SeqCst) == 1 {
            self.lifetime.drained.notify_waiters();
        }
    }
}
impl CollaborationRuntime {
    /// Reuse native provider/vault configuration after the old generation has
    /// drained. The new Store and runtime receive fresh admission/demand state.
    pub fn replacement(&self, store: Arc<Store>) -> Result<Self, CollaborationError> {
        if !self.lifetime.closed.load(Ordering::Acquire) {
            return Err(stopped());
        }
        let mut runtime =
            Self::with_registry(store, self.vault.clone(), ProviderRegistry::default());
        runtime.registry = self.registry.clone();
        runtime.github_cli = self.github_cli.clone();
        runtime.demand_visibility = self.demand_visibility.clone();
        runtime.clock = self.clock.clone();
        Ok(runtime)
    }
    pub(super) async fn owned_operation<F, Fut, T>(
        &self,
        operation: F,
    ) -> Result<T, CollaborationError>
    where
        F: FnOnce(Self) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = Result<T, CollaborationError>> + Send + 'static,
        T: Send + 'static,
    {
        let lease = self.acquire_operation()?;
        let (send, receive) = tokio::sync::oneshot::channel();
        {
            let mut tasks = self
                .lifetime
                .credentials
                .lock()
                .map_err(|_| CollaborationError::storage())?;
            tasks.retain(|task| !task.is_finished());
            if tasks.len() >= 32 {
                return Err(CollaborationError::new(
                    ErrorCode::Busy,
                    "Collaboration credential operations are busy",
                ));
            }
            let runtime = self.clone();
            tasks.push(tokio::spawn(async move {
                let _operation = lease;
                let result = operation(runtime).await;
                let _ = send.send(result);
            }));
        }
        receive.await.map_err(|_| vault_error())?
    }
    pub fn acquire_operation(&self) -> Result<RuntimeOperation, CollaborationError> {
        if self.is_stopping() {
            return Err(stopped());
        }
        self.lifetime.active.fetch_add(1, Ordering::SeqCst);
        let operation = RuntimeOperation {
            lifetime: self.lifetime.clone(),
        };
        if self.is_stopping() {
            drop(operation);
            return Err(stopped());
        }
        Ok(operation)
    }
    pub fn is_stopping(&self) -> bool {
        !self.lifetime.accepting.load(Ordering::SeqCst)
    }
    pub fn stop_admission(&self) {
        self.lifetime.accepting.store(false, Ordering::SeqCst);
        self.notify.notify_waiters();
        self.notify.notify_one();
    }
    /// Owned completion is deliberate: abandoning a requester cannot interrupt
    /// a credential cutover or release the writer lease while work is running.
    pub async fn shutdown(&self) -> Result<(), CollaborationError> {
        self.stop_admission();
        let owned = self.clone();
        tokio::spawn(async move { owned.shutdown_owned().await })
            .await
            .map_err(|_| CollaborationError::storage())?
    }
    async fn shutdown_owned(&self) -> Result<(), CollaborationError> {
        let _shutdown = self.lifetime.shutdown.lock().await;
        let background = self
            .lifetime
            .background
            .lock()
            .map_err(|_| CollaborationError::storage())?
            .take();
        if let Some(background) = background {
            background
                .await
                .map_err(|_| CollaborationError::storage())?;
        }
        loop {
            let drained = self.lifetime.drained.notified();
            tokio::pin!(drained);
            drained.as_mut().enable();
            if self.lifetime.active.load(Ordering::SeqCst) == 0 {
                break;
            }
            drained.await;
        }
        let credentials = std::mem::take(
            &mut *self
                .lifetime
                .credentials
                .lock()
                .map_err(|_| CollaborationError::storage())?,
        );
        for task in credentials {
            task.await.map_err(|_| CollaborationError::storage())?;
        }
        // Dispatch paths acquire lifecycle after dispatch. Never reverse that
        // order while waiting, and never hold lifecycle during operation drain.
        let _dispatch = self.dispatch.lock().await;
        let _lifecycle = self.lifecycle.lock().await;
        *self.scheduler.lock().await = Scheduler::default();
        self.store.close().await?;
        self.lifetime.closed.store(true, Ordering::Release);
        Ok(())
    }
}
pub(super) fn stopped() -> CollaborationError {
    CollaborationError::new(
        ErrorCode::NotReady,
        "Collaboration is paused for storage recovery",
    )
}

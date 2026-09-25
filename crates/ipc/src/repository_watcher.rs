use git::context::RepositoryWatchPaths;
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use serde::Serialize;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

pub const REPOSITORY_CHANGED_EVENT: &str = "gitru://repository-changed";
const CHANGE_DEBOUNCE: Duration = Duration::from_millis(150);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum RepositoryChangeKind {
    Worktree,
    Index,
    Head,
    Refs,
    Stash,
    Operation,
    Config,
}

const ALL_REPOSITORY_CHANGE_KINDS: [RepositoryChangeKind; 7] = [
    RepositoryChangeKind::Worktree,
    RepositoryChangeKind::Index,
    RepositoryChangeKind::Head,
    RepositoryChangeKind::Refs,
    RepositoryChangeKind::Stash,
    RepositoryChangeKind::Operation,
    RepositoryChangeKind::Config,
];

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryChangedEvent {
    pub context_id: String,
    pub changes: Vec<RepositoryChangeKind>,
}

#[derive(Clone)]
struct ChangeSink {
    pending: Arc<Mutex<PendingChanges>>,
    signal: mpsc::Sender<()>,
    active: Arc<AtomicBool>,
}

#[derive(Default)]
struct PendingChanges {
    changes: BTreeSet<RepositoryChangeKind>,
    revision: u64,
}

impl ChangeSink {
    fn push<I>(&self, changes: I)
    where
        I: IntoIterator<Item = RepositoryChangeKind>,
    {
        if !self.active.load(Ordering::Acquire) {
            return;
        }

        let received_change = match self.pending.lock() {
            Ok(mut pending) => {
                let mut received_change = false;
                for change in changes {
                    received_change = true;
                    pending.changes.insert(change);
                }
                if received_change {
                    pending.revision = pending.revision.wrapping_add(1);
                }
                received_change
            }
            Err(error) => {
                log::error!("repository watcher pending set was poisoned: {error}");
                false
            }
        };

        if received_change {
            // Capacity one keeps wake-ups bounded while still letting repeated
            // events reset the trailing debounce once the worker receives them.
            let _ = self.signal.try_send(());
        }
    }
}

struct ChangeCoalescer {
    sink: ChangeSink,
    callback_gate: Arc<Mutex<()>>,
    task: Option<JoinHandle<()>>,
}

impl ChangeCoalescer {
    fn new<F>(delay: Duration, callback: F) -> Self
    where
        F: Fn(Vec<RepositoryChangeKind>) + Send + Sync + 'static,
    {
        let pending = Arc::new(Mutex::new(PendingChanges::default()));
        let active = Arc::new(AtomicBool::new(false));
        let callback_gate = Arc::new(Mutex::new(()));
        let (signal, mut receiver) = mpsc::channel(1);
        let callback = Arc::new(callback);

        let task_pending = pending.clone();
        let task_active = active.clone();
        let task_callback_gate = callback_gate.clone();
        let task = tokio::spawn(async move {
            while receiver.recv().await.is_some() {
                let mut observed_revision = match task_pending.lock() {
                    Ok(pending) => pending.revision,
                    Err(error) => {
                        log::error!("repository watcher pending set was poisoned: {error}");
                        return;
                    }
                };
                let quiet = tokio::time::sleep(delay);
                tokio::pin!(quiet);

                let changes = loop {
                    tokio::select! {
                        biased;
                        signal = receiver.recv() => match signal {
                            Some(()) => {
                                observed_revision = match task_pending.lock() {
                                    Ok(pending) => pending.revision,
                                    Err(error) => {
                                        log::error!("repository watcher pending set was poisoned: {error}");
                                        return;
                                    }
                                };
                                quiet.as_mut().reset(tokio::time::Instant::now() + delay);
                            }
                            None => return,
                        },
                        _ = &mut quiet => {
                            let settled = match task_pending.lock() {
                                Ok(mut pending) => {
                                    if pending.revision == observed_revision {
                                        Some(std::mem::take(&mut pending.changes))
                                    } else {
                                        observed_revision = pending.revision;
                                        None
                                    }
                                }
                                Err(error) => {
                                    log::error!("repository watcher pending set was poisoned: {error}");
                                    return;
                                }
                            };

                            if let Some(changes) = settled {
                                break changes;
                            }
                            quiet.as_mut().reset(tokio::time::Instant::now() + delay);
                        },
                    }
                };

                if changes.is_empty() {
                    continue;
                }

                let Ok(_callback_guard) = task_callback_gate.lock() else {
                    log::error!("repository watcher callback gate was poisoned");
                    continue;
                };
                if task_active.load(Ordering::Acquire) {
                    callback(changes.into_iter().collect());
                }
            }
        });

        Self {
            sink: ChangeSink {
                pending,
                signal,
                active,
            },
            callback_gate,
            task: Some(task),
        }
    }

    fn sink(&self) -> ChangeSink {
        self.sink.clone()
    }

    fn activate(&self) {
        self.sink.active.store(true, Ordering::Release);
    }

    fn stop(&mut self) {
        self.sink.active.store(false, Ordering::Release);
        if let Ok(mut pending) = self.sink.pending.lock() {
            pending.changes.clear();
        }

        // Waiting for this gate makes disposal a hard boundary: when stop
        // returns, no callback can still be emitting an event.
        if let Ok(callback_guard) = self.callback_gate.lock() {
            drop(callback_guard);
        }

        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}

impl Drop for ChangeCoalescer {
    fn drop(&mut self) {
        self.stop();
    }
}

pub struct RepositoryWatcher {
    watcher: Option<RecommendedWatcher>,
    coalescer: ChangeCoalescer,
}

impl RepositoryWatcher {
    pub fn new<F>(paths: RepositoryWatchPaths, callback: F) -> Result<Self, String>
    where
        F: Fn(Vec<RepositoryChangeKind>) + Send + Sync + 'static,
    {
        let coalescer = ChangeCoalescer::new(CHANGE_DEBOUNCE, callback);
        let sink = coalescer.sink();
        let event_paths = paths.clone();
        let mut watcher = notify::recommended_watcher(move |result: notify::Result<Event>| {
            let event = match result {
                Ok(event) => event,
                Err(error) => {
                    log::warn!("repository watcher error: {error}");
                    sink.push(ALL_REPOSITORY_CHANGE_KINDS);
                    return;
                }
            };

            if event.need_rescan() {
                sink.push(ALL_REPOSITORY_CHANGE_KINDS);
                return;
            }

            if matches!(event.kind, EventKind::Access(_)) {
                return;
            }

            let changes = event
                .paths
                .iter()
                .filter_map(|path| classify_path(path, &event_paths));
            sink.push(changes);
        })
        .map_err(|error| format!("Failed to create repository watcher: {error}"))?;

        for target in watch_targets(&paths) {
            watcher.watch(&target.path, target.mode).map_err(|error| {
                format!(
                    "Failed to watch repository path {}: {error}",
                    target.path.display()
                )
            })?;
        }

        Ok(Self {
            watcher: Some(watcher),
            coalescer,
        })
    }

    fn activate(&self) {
        self.coalescer.activate();
    }

    fn stop(&mut self) {
        self.coalescer.stop();
        self.watcher.take();
    }
}

impl Drop for RepositoryWatcher {
    fn drop(&mut self) {
        self.stop();
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct WatchTarget {
    path: PathBuf,
    mode: RecursiveMode,
}

fn watch_targets(paths: &RepositoryWatchPaths) -> Vec<WatchTarget> {
    let mut targets = Vec::new();
    push_target(
        &mut targets,
        paths.worktree.clone(),
        RecursiveMode::Recursive,
    );
    if paths.git_dir != paths.common_dir || !paths.git_dir.starts_with(&paths.worktree) {
        // A linked worktree's private gitdir is small and owns its index,
        // HEAD, and nested sequencer state, so it is safe to watch fully. A
        // separate external gitdir also needs recursion because the worktree
        // watch cannot observe nested operation state there.
        push_target(
            &mut targets,
            paths.git_dir.clone(),
            RecursiveMode::Recursive,
        );
    }
    // The shared common dir can contain a very large object database and
    // metadata for other worktrees. Watch only its direct files and ref trees.
    push_metadata_targets(&mut targets, &paths.common_dir);
    targets
}

fn push_metadata_targets(targets: &mut Vec<WatchTarget>, root: &Path) {
    push_target(targets, root.to_path_buf(), RecursiveMode::NonRecursive);
    for relative in [Path::new("refs"), Path::new("logs/refs"), Path::new("info")] {
        let path = root.join(relative);
        if path.is_dir() {
            push_target(targets, path, RecursiveMode::Recursive);
        }
    }
}

fn push_target(targets: &mut Vec<WatchTarget>, path: PathBuf, mode: RecursiveMode) {
    if targets.iter().any(|target| {
        target.path == path
            || (target.mode == RecursiveMode::Recursive && path.starts_with(&target.path))
    }) {
        return;
    }

    if mode == RecursiveMode::Recursive {
        targets.retain(|target| !target.path.starts_with(&path));
    }

    if !targets
        .iter()
        .any(|target| target.path == path && target.mode == mode)
    {
        targets.push(WatchTarget { path, mode });
    }
}

fn classify_path(path: &Path, paths: &RepositoryWatchPaths) -> Option<RepositoryChangeKind> {
    if let Ok(relative) = path.strip_prefix(&paths.git_dir) {
        return classify_git_path(relative);
    }
    if paths.common_dir != paths.git_dir
        && let Ok(relative) = path.strip_prefix(&paths.common_dir)
    {
        return classify_git_path(relative);
    }
    if path.strip_prefix(&paths.worktree).is_ok() {
        return Some(RepositoryChangeKind::Worktree);
    }
    None
}

fn classify_git_path(relative: &Path) -> Option<RepositoryChangeKind> {
    if relative == Path::new("index") || relative == Path::new("index.lock") {
        return Some(RepositoryChangeKind::Index);
    }

    if matches_path(relative, &["HEAD", "HEAD.lock", "ORIG_HEAD", "logs/HEAD"]) {
        return Some(RepositoryChangeKind::Head);
    }

    if matches_path(
        relative,
        &[
            "refs/stash",
            "refs/stash.lock",
            "logs/refs/stash",
            "logs/refs/stash.lock",
        ],
    ) {
        return Some(RepositoryChangeKind::Stash);
    }

    if relative.starts_with("refs")
        || relative.starts_with(Path::new("logs").join("refs"))
        || matches_path(relative, &["packed-refs", "packed-refs.lock", "FETCH_HEAD"])
    {
        return Some(RepositoryChangeKind::Refs);
    }

    if matches_path(
        relative,
        &[
            "config",
            "config.lock",
            "config.worktree",
            "info/exclude",
            "info/attributes",
        ],
    ) {
        return Some(RepositoryChangeKind::Config);
    }

    if is_operation_path(relative) {
        return Some(RepositoryChangeKind::Operation);
    }

    // Object storage, hook temp files, and other Git internals do not map to
    // UI query domains and are intentionally ignored.
    None
}

fn matches_path(path: &Path, candidates: &[&str]) -> bool {
    candidates
        .iter()
        .any(|candidate| path == Path::new(candidate))
}

fn is_operation_path(relative: &Path) -> bool {
    if relative.starts_with("rebase-merge")
        || relative.starts_with("rebase-apply")
        || relative.starts_with("sequencer")
        || relative.starts_with("gitru-rebase")
    {
        return true;
    }

    let file_name = relative
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    matches!(
        file_name,
        "MERGE_HEAD"
            | "MERGE_MSG"
            | "CHERRY_PICK_HEAD"
            | "REVERT_HEAD"
            | "REBASE_HEAD"
            | "SQUASH_MSG"
    ) || file_name.starts_with("BISECT_")
}

pub fn cache_namespaces_for_changes(changes: &[RepositoryChangeKind]) -> Vec<&'static str> {
    let mut namespaces = BTreeSet::new();
    for change in changes {
        let affected: &[&str] = match change {
            RepositoryChangeKind::Worktree | RepositoryChangeKind::Index => {
                &["status", "has_uncommitted_changes", "patch_by_file_path"]
            }
            RepositoryChangeKind::Head => &[
                "status",
                "has_uncommitted_changes",
                "patch_by_file_path",
                "current_branch",
                "list_branches",
                "ahead_behind_status",
                "branch_info",
                "last_commit",
                "history",
            ],
            RepositoryChangeKind::Refs => &[
                "status",
                "has_uncommitted_changes",
                "patch_by_file_path",
                "list_branches",
                "ahead_behind_status",
                "branch_info",
                "last_commit",
                "history",
            ],
            RepositoryChangeKind::Stash => &["stash_list", "patch_by_file_path"],
            RepositoryChangeKind::Operation => &[
                "status",
                "has_uncommitted_changes",
                "patch_by_file_path",
                "current_branch",
            ],
            RepositoryChangeKind::Config => &[
                "repository_origin",
                "list_branches",
                "ahead_behind_status",
                "branch_info",
                "status",
                "has_uncommitted_changes",
                "patch_by_file_path",
            ],
        };
        namespaces.extend(affected.iter().copied());
    }
    namespaces.into_iter().collect()
}

struct ContextRegistry<T> {
    contexts: HashMap<String, T>,
    context_owners: HashMap<String, String>,
    owner_contexts: HashMap<String, HashSet<String>>,
    owner_generations: HashMap<String, u64>,
}

impl<T> Default for ContextRegistry<T> {
    fn default() -> Self {
        Self {
            contexts: HashMap::new(),
            context_owners: HashMap::new(),
            owner_contexts: HashMap::new(),
            owner_generations: HashMap::new(),
        }
    }
}

impl<T> ContextRegistry<T> {
    fn begin_owner(&mut self, owner_id: &str) -> Result<(u64, Vec<String>, Vec<T>), String> {
        if owner_id.is_empty() {
            return Err("Repository context owner cannot be empty".to_string());
        }

        let generation = self
            .owner_generations
            .entry(owner_id.to_string())
            .or_default();
        *generation = generation.wrapping_add(1);
        let generation = *generation;
        let (context_ids, values) = self.remove_owner_contexts(owner_id);
        Ok((generation, context_ids, values))
    }

    fn can_register(
        &self,
        owner_id: &str,
        owner_generation: u64,
        context_id: &str,
    ) -> Result<(), String> {
        if self
            .owner_generations
            .get(owner_id)
            .copied()
            .unwrap_or_default()
            != owner_generation
        {
            return Err(format!(
                "Repository context owner '{owner_id}' changed while its context was being created"
            ));
        }
        if self.contexts.contains_key(context_id) {
            return Err(format!(
                "Repository context '{context_id}' is already registered"
            ));
        }
        Ok(())
    }

    fn insert(&mut self, owner_id: String, context_id: String, value: T) {
        self.context_owners
            .insert(context_id.clone(), owner_id.clone());
        self.owner_contexts
            .entry(owner_id)
            .or_default()
            .insert(context_id.clone());
        self.contexts.insert(context_id, value);
    }

    fn remove_context(&mut self, context_id: &str) -> Option<T> {
        if let Some(owner_id) = self.context_owners.remove(context_id)
            && let Some(contexts) = self.owner_contexts.get_mut(&owner_id)
        {
            contexts.remove(context_id);
            if contexts.is_empty() {
                self.owner_contexts.remove(&owner_id);
            }
        }
        self.contexts.remove(context_id)
    }

    fn dispose_owner(&mut self, owner_id: &str) -> (Vec<String>, Vec<T>) {
        let generation = self
            .owner_generations
            .entry(owner_id.to_string())
            .or_default();
        *generation = generation.wrapping_add(1);
        self.remove_owner_contexts(owner_id)
    }

    fn remove_owner_contexts(&mut self, owner_id: &str) -> (Vec<String>, Vec<T>) {
        let context_ids = self
            .owner_contexts
            .remove(owner_id)
            .unwrap_or_default()
            .into_iter()
            .collect::<Vec<_>>();
        let mut values = Vec::with_capacity(context_ids.len());
        for context_id in &context_ids {
            self.context_owners.remove(context_id);
            if let Some(value) = self.contexts.remove(context_id) {
                values.push(value);
            }
        }
        (context_ids, values)
    }
}

#[derive(Default)]
pub struct RepoContextRuntime {
    registry: Mutex<ContextRegistry<Option<RepositoryWatcher>>>,
}

impl RepoContextRuntime {
    pub fn begin_owner_context(&self, owner_id: &str) -> Result<(u64, Vec<String>), String> {
        let (generation, context_ids, watchers) = self
            .registry
            .lock()
            .map_err(|_| "Repository watcher registry was poisoned".to_string())?
            .begin_owner(owner_id)?;
        drop(watchers);
        Ok((generation, context_ids))
    }

    pub fn register(
        &self,
        owner_id: String,
        owner_generation: u64,
        context_id: String,
        watcher: Option<RepositoryWatcher>,
    ) -> Result<(), String> {
        let mut registry = self
            .registry
            .lock()
            .map_err(|_| "Repository watcher registry was poisoned".to_string())?;
        registry.can_register(&owner_id, owner_generation, &context_id)?;
        if let Some(watcher) = &watcher {
            watcher.activate();
        }
        registry.insert(owner_id, context_id, watcher);
        Ok(())
    }

    pub fn dispose_context(&self, context_id: &str) -> Result<bool, String> {
        let watcher = self
            .registry
            .lock()
            .map_err(|_| "Repository watcher registry was poisoned".to_string())?
            .remove_context(context_id);
        let found = watcher.is_some();
        drop(watcher);
        Ok(found)
    }

    pub fn dispose_owner(&self, owner_id: &str) -> Result<Vec<String>, String> {
        let (context_ids, watchers) = self
            .registry
            .lock()
            .map_err(|_| "Repository watcher registry was poisoned".to_string())?
            .dispose_owner(owner_id);
        drop(watchers);
        Ok(context_ids)
    }
}

#[cfg(test)]
mod tests {
    #[cfg(target_os = "linux")]
    use super::RepositoryWatcher;
    use super::{
        ALL_REPOSITORY_CHANGE_KINDS, ChangeCoalescer, ContextRegistry, RepositoryChangeKind,
        cache_namespaces_for_changes, classify_path, watch_targets,
    };
    use git::context::RepositoryWatchPaths;
    #[cfg(target_os = "linux")]
    use git::core::RepoServices;
    use notify::RecursiveMode;
    use std::collections::BTreeSet;
    use std::path::PathBuf;
    #[cfg(target_os = "linux")]
    use std::process::Command;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;
    use tempfile::tempdir;

    fn test_paths() -> RepositoryWatchPaths {
        RepositoryWatchPaths {
            worktree: PathBuf::from("/repo"),
            git_dir: PathBuf::from("/repo/.git/worktrees/feature"),
            common_dir: PathBuf::from("/repo/.git"),
        }
    }

    #[test]
    fn classifies_worktree_and_git_metadata_domains() {
        let paths = test_paths();
        let cases = [
            ("/repo/src/main.rs", RepositoryChangeKind::Worktree),
            (
                "/repo/.git/worktrees/feature/index.lock",
                RepositoryChangeKind::Index,
            ),
            (
                "/repo/.git/worktrees/feature/HEAD",
                RepositoryChangeKind::Head,
            ),
            ("/repo/.git/refs/heads/main", RepositoryChangeKind::Refs),
            ("/repo/.git/logs/refs/stash", RepositoryChangeKind::Stash),
            ("/repo/.git/refs/stash.lock", RepositoryChangeKind::Stash),
            ("/repo/.git/config", RepositoryChangeKind::Config),
            ("/repo/.git/info/exclude", RepositoryChangeKind::Config),
            ("/repo/.git/info/attributes", RepositoryChangeKind::Config),
            (
                "/repo/.git/worktrees/feature/rebase-merge/git-rebase-todo",
                RepositoryChangeKind::Operation,
            ),
            (
                "/repo/.git/worktrees/feature/gitru-rebase/state.json",
                RepositoryChangeKind::Operation,
            ),
        ];

        for (path, expected) in cases {
            assert_eq!(
                classify_path(PathBuf::from(path).as_path(), &paths),
                Some(expected)
            );
        }
        assert_eq!(
            classify_path(PathBuf::from("/repo/.git/objects/aa/bb").as_path(), &paths),
            None
        );
    }

    #[test]
    fn creates_targeted_metadata_watches_for_linked_worktrees() {
        let directory = tempdir().unwrap();
        let worktree = directory.path().join("worktree");
        let git_dir = directory.path().join("common/worktrees/feature");
        let common_dir = directory.path().join("common");
        for path in [
            &worktree,
            &git_dir,
            &common_dir,
            &common_dir.join("refs"),
            &common_dir.join("logs/refs"),
            &common_dir.join("info"),
        ] {
            std::fs::create_dir_all(path).unwrap();
        }
        let targets = watch_targets(&RepositoryWatchPaths {
            worktree: worktree.clone(),
            git_dir: git_dir.clone(),
            common_dir: common_dir.clone(),
        });

        assert!(
            targets.iter().any(|target| {
                target.path == worktree && target.mode == RecursiveMode::Recursive
            })
        );
        assert!(
            targets.iter().any(|target| {
                target.path == git_dir && target.mode == RecursiveMode::Recursive
            })
        );
        assert!(targets.iter().any(|target| {
            target.path == common_dir.join("refs") && target.mode == RecursiveMode::Recursive
        }));
        assert!(targets.iter().any(|target| {
            target.path == common_dir.join("info") && target.mode == RecursiveMode::Recursive
        }));
        assert!(!targets.iter().any(|target| {
            target.path == common_dir && target.mode == RecursiveMode::Recursive
        }));
    }

    #[test]
    fn recursive_worktree_watch_subsumes_normal_git_metadata_targets() {
        let directory = tempdir().unwrap();
        let worktree = directory.path().join("worktree");
        let git_dir = worktree.join(".git");
        std::fs::create_dir_all(git_dir.join("refs")).unwrap();
        std::fs::create_dir_all(git_dir.join("logs/refs")).unwrap();

        let targets = watch_targets(&RepositoryWatchPaths {
            worktree: worktree.clone(),
            git_dir: git_dir.clone(),
            common_dir: git_dir,
        });

        assert_eq!(
            targets,
            vec![super::WatchTarget {
                path: worktree,
                mode: RecursiveMode::Recursive,
            }]
        );
    }

    #[tokio::test(start_paused = true)]
    async fn coalesces_a_burst_into_one_bounded_change_set() {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let callback_seen = seen.clone();
        let mut coalescer = ChangeCoalescer::new(Duration::from_millis(100), move |changes| {
            callback_seen.lock().unwrap().push(changes);
        });
        coalescer.activate();
        let sink = coalescer.sink();

        sink.push([RepositoryChangeKind::Worktree, RepositoryChangeKind::Index]);
        sink.push([RepositoryChangeKind::Worktree, RepositoryChangeKind::Head]);
        tokio::task::yield_now().await;
        tokio::time::advance(Duration::from_millis(100)).await;
        tokio::task::yield_now().await;

        assert_eq!(
            *seen.lock().unwrap(),
            vec![vec![
                RepositoryChangeKind::Worktree,
                RepositoryChangeKind::Index,
                RepositoryChangeKind::Head,
            ]]
        );
        coalescer.stop();
    }

    #[tokio::test(start_paused = true)]
    async fn repeated_events_reset_the_quiet_period() {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let callback_seen = seen.clone();
        let mut coalescer = ChangeCoalescer::new(Duration::from_millis(100), move |changes| {
            callback_seen.lock().unwrap().push(changes);
        });
        coalescer.activate();
        let sink = coalescer.sink();

        sink.push([RepositoryChangeKind::Worktree]);
        tokio::task::yield_now().await;
        tokio::time::advance(Duration::from_millis(90)).await;
        sink.push([RepositoryChangeKind::Worktree]);
        tokio::task::yield_now().await;
        tokio::time::advance(Duration::from_millis(20)).await;
        tokio::task::yield_now().await;

        assert!(seen.lock().unwrap().is_empty());

        tokio::time::advance(Duration::from_millis(80)).await;
        tokio::task::yield_now().await;
        assert_eq!(
            *seen.lock().unwrap(),
            vec![vec![RepositoryChangeKind::Worktree]]
        );
        coalescer.stop();
    }

    #[tokio::test(start_paused = true)]
    async fn stopping_discards_a_pending_callback() {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let callback_seen = seen.clone();
        let mut coalescer = ChangeCoalescer::new(Duration::from_millis(100), move |changes| {
            callback_seen.lock().unwrap().push(changes);
        });
        coalescer.activate();
        coalescer.sink().push([RepositoryChangeKind::Worktree]);
        tokio::task::yield_now().await;

        coalescer.stop();
        tokio::time::advance(Duration::from_millis(100)).await;
        tokio::task::yield_now().await;

        assert!(seen.lock().unwrap().is_empty());
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn read_only_status_queries_do_not_feed_the_watcher() {
        let directory = tempdir().unwrap();
        let run_git = |args: &[&str]| {
            let output = Command::new("git")
                .current_dir(directory.path())
                .args(args)
                .output()
                .expect("run git command");
            assert!(
                output.status.success(),
                "git {} failed: {}",
                args.join(" "),
                String::from_utf8_lossy(&output.stderr)
            );
        };

        run_git(&["init", "-b", "main"]);
        run_git(&["config", "user.email", "test@example.com"]);
        run_git(&["config", "user.name", "Test User"]);
        let tracked_file = directory.path().join("tracked.txt");
        std::fs::write(&tracked_file, "unchanged\n").unwrap();
        run_git(&["add", "tracked.txt"]);
        run_git(&["commit", "-m", "initial"]);

        let index_path = directory.path().join(".git/index");
        let index_before = std::fs::read(&index_path).unwrap();
        std::thread::sleep(Duration::from_millis(1_100));
        std::fs::write(&tracked_file, "unchanged\n").unwrap();

        let services = RepoServices::new(directory.path().to_str().unwrap()).unwrap();
        let (change_tx, mut change_rx) = tokio::sync::mpsc::unbounded_channel();
        let mut watcher = RepositoryWatcher::new(services.watch_paths().unwrap(), move |changes| {
            let _ = change_tx.send(changes);
        })
        .unwrap();
        watcher.activate();

        assert!(
            services
                .action()
                .get_status()
                .await
                .unwrap()
                .files
                .is_empty()
        );
        assert!(!services.query().has_uncommitted_changes().await.unwrap());
        assert_eq!(std::fs::read(index_path).unwrap(), index_before);
        assert!(
            tokio::time::timeout(Duration::from_millis(500), change_rx.recv())
                .await
                .is_err(),
            "background status queries emitted a repository change"
        );

        watcher.stop();
    }

    #[test]
    fn registry_rejects_pre_dispose_create_but_allows_owner_reuse() {
        let mut registry = ContextRegistry::default();
        let (generation, _, _) = registry.begin_owner("tab-1").unwrap();
        registry
            .can_register("tab-1", generation, "context-1")
            .unwrap();
        registry.insert("tab-1".into(), "context-1".into(), ());

        assert!(
            registry
                .can_register("tab-1", generation, "context-1")
                .is_err()
        );
        let (context_ids, values) = registry.dispose_owner("tab-1");
        assert_eq!(context_ids, vec!["context-1"]);
        assert_eq!(values.len(), 1);
        assert!(
            registry
                .can_register("tab-1", generation, "context-late")
                .is_err()
        );

        let (new_generation, _, _) = registry.begin_owner("tab-1").unwrap();
        assert!(new_generation > generation);
        registry
            .can_register("tab-1", new_generation, "context-new")
            .unwrap();
    }

    #[test]
    fn beginning_a_new_context_retires_the_existing_owner_context() {
        let mut registry = ContextRegistry::default();
        let (generation, _, _) = registry.begin_owner("tab-1").unwrap();
        registry
            .can_register("tab-1", generation, "context-1")
            .unwrap();
        registry.insert("tab-1".into(), "context-1".into(), ());

        let (next_generation, context_ids, values) = registry.begin_owner("tab-1").unwrap();
        assert!(next_generation > generation);
        assert_eq!(context_ids, vec!["context-1"]);
        assert_eq!(values.len(), 1);
    }

    #[test]
    fn change_kinds_map_to_scoped_cache_namespaces() {
        let namespaces = cache_namespaces_for_changes(&[
            RepositoryChangeKind::Worktree,
            RepositoryChangeKind::Config,
        ]);
        assert!(namespaces.contains(&"status"));
        assert!(namespaces.contains(&"patch_by_file_path"));
        assert!(namespaces.contains(&"repository_origin"));
        assert!(!namespaces.contains(&"history"));
        assert!(!namespaces.contains(&"stash_list"));

        let config_namespaces = cache_namespaces_for_changes(&[RepositoryChangeKind::Config]);
        assert!(config_namespaces.contains(&"status"));
        assert!(config_namespaces.contains(&"has_uncommitted_changes"));
        assert!(config_namespaces.contains(&"patch_by_file_path"));

        let stash_namespaces = cache_namespaces_for_changes(&[RepositoryChangeKind::Stash]);
        assert!(stash_namespaces.contains(&"stash_list"));
        assert!(stash_namespaces.contains(&"patch_by_file_path"));

        let all_changes = ALL_REPOSITORY_CHANGE_KINDS
            .into_iter()
            .collect::<BTreeSet<_>>();
        assert_eq!(all_changes.len(), 7);
    }

    #[test]
    fn registry_tracks_contexts_even_when_native_watching_is_unavailable() {
        let mut registry = ContextRegistry::<Option<()>>::default();
        let (generation, _, _) = registry.begin_owner("tab-1").unwrap();
        registry
            .can_register("tab-1", generation, "context-1")
            .unwrap();
        registry.insert("tab-1".into(), "context-1".into(), None);

        assert!(registry.remove_context("context-1").is_some());
    }
}

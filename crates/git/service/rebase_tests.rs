//! Integration tests for rebase operation detection and the Gitru sequencer.
#![cfg(test)]

use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::Arc,
};

use serial_test::serial;
use tempfile::TempDir;

use crate::{
    context::RepoContext,
    models::rebase::{RebaseAction, RebasePlanEntry, RebaseStartRequest},
    service::{operation::OperationService, rebase::RebaseService},
};

fn git(cwd: &PathBuf, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("git failed to start");
    assert!(
        output.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

fn init_repo() -> (TempDir, PathBuf) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().to_path_buf();
    git(&path, &["init"]);
    git(&path, &["config", "user.name", "Gitru Test"]);
    git(&path, &["config", "user.email", "test@gitru.local"]);
    git(&path, &["checkout", "-b", "main"]);
    fs::write(path.join("file.txt"), "base\n").unwrap();
    git(&path, &["add", "."]);
    git(&path, &["commit", "-m", "base"]);
    (dir, path)
}

fn commit_file(path: &PathBuf, name: &str, content: &str, message: &str) -> String {
    fs::write(path.join(name), content).unwrap();
    git(path, &["add", "."]);
    git(path, &["commit", "-m", message]);
    git(path, &["rev-parse", "HEAD"])
}

#[test]
#[serial]
fn detects_clean_repo() {
    let (_dir, path) = init_repo();
    let ctx = Arc::new(RepoContext::new(path.to_str().unwrap()).unwrap());
    let op = OperationService::new(ctx).get_repo_operation().unwrap();
    assert!(!op.is_rebasing);
    assert!(matches!(
        op.kind,
        crate::models::operation::RepoOperationKind::Clean
    ));
}

#[test]
#[serial]
fn detects_external_rebase_merge() {
    let (_dir, path) = init_repo();
    // Create a branch with a commit, then rebase onto a divergent main tip.
    git(&path, &["checkout", "-b", "feature"]);
    commit_file(&path, "a.txt", "a\n", "feat a");

    git(&path, &["checkout", "main"]);
    commit_file(&path, "b.txt", "b\n", "feat b");

    git(&path, &["checkout", "feature"]);
    // Start a rebase that should succeed cleanly — then force a conflict path.
    // Create conflict: edit same file on both sides.
    git(&path, &["checkout", "main"]);
    commit_file(&path, "conflict.txt", "main side\n", "main conflict");
    git(&path, &["checkout", "feature"]);
    commit_file(&path, "conflict.txt", "feature side\n", "feature conflict");

    let status = Command::new("git")
        .args(["rebase", "main"])
        .current_dir(&path)
        .output()
        .unwrap();
    // Expect conflict / non-zero
    assert!(!status.status.success());

    let ctx = Arc::new(RepoContext::new(path.to_str().unwrap()).unwrap());
    let op = OperationService::new(ctx).get_repo_operation().unwrap();
    assert!(op.is_rebasing);
    assert!(!op.conflict_paths.is_empty() || op.pause_reason.is_some());

    // Abort via CLI through service
    let rebase = RebaseService::new(Arc::new(RepoContext::new(path.to_str().unwrap()).unwrap()));
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let after = rt
        .block_on(async { rebase.abort(None).await })
        .unwrap();
    assert!(!after.is_rebasing);
}

#[test]
#[serial]
fn plain_git2_rebase_succeeds() {
    let (_dir, path) = init_repo();
    git(&path, &["checkout", "-b", "feature"]);
    commit_file(&path, "a.txt", "a\n", "feat a");
    commit_file(&path, "c.txt", "c\n", "feat c");

    git(&path, &["checkout", "main"]);
    let onto = commit_file(&path, "b.txt", "b\n", "feat b");

    git(&path, &["checkout", "feature"]);

    let rebase = RebaseService::new(Arc::new(RepoContext::new(path.to_str().unwrap()).unwrap()));
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let result = rt.block_on(async {
        rebase
            .start(
                RebaseStartRequest {
                    onto,
                    upstream: None,
                    entries: None,
                    autostash: false,
                },
                None,
            )
            .await
    });
    assert!(result.is_ok(), "{result:?}");
    let op = result.unwrap();
    assert!(!op.is_rebasing, "expected finished rebase, got {op:?}");
}

#[test]
#[serial]
fn interactive_gitru_sequencer_with_drop() {
    let (_dir, path) = init_repo();
    git(&path, &["checkout", "-b", "feature"]);
    let c1 = commit_file(&path, "a.txt", "a\n", "feat a");
    let c2 = commit_file(&path, "b.txt", "b\n", "feat b");
    let c3 = commit_file(&path, "c.txt", "c\n", "feat c");

    git(&path, &["checkout", "main"]);
    let onto = git(&path, &["rev-parse", "HEAD"]);
    git(&path, &["checkout", "feature"]);

    let entries = vec![
        RebasePlanEntry {
            action: RebaseAction::Pick,
            commit: c1,
            message: Some("feat a".into()),
        },
        RebasePlanEntry {
            action: RebaseAction::Drop,
            commit: c2,
            message: Some("feat b".into()),
        },
        RebasePlanEntry {
            action: RebaseAction::Pick,
            commit: c3,
            message: Some("feat c".into()),
        },
    ];

    let rebase = RebaseService::new(Arc::new(RepoContext::new(path.to_str().unwrap()).unwrap()));
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let result = rt.block_on(async {
        rebase
            .start(
                RebaseStartRequest {
                    onto,
                    upstream: None,
                    entries: Some(entries),
                    autostash: false,
                },
                None,
            )
            .await
    });
    assert!(result.is_ok(), "{result:?}");
    assert!(!result.unwrap().is_rebasing);
    assert!(!path.join("b.txt").exists(), "dropped commit file should be gone");
    assert!(path.join("a.txt").exists());
    assert!(path.join("c.txt").exists());
}

#[test]
#[serial]
fn abort_preview_when_rebasing() {
    let (_dir, path) = init_repo();
    git(&path, &["checkout", "-b", "feature"]);
    commit_file(&path, "a.txt", "a\n", "feat a");
    git(&path, &["checkout", "main"]);
    commit_file(&path, "conflict.txt", "main\n", "main");
    git(&path, &["checkout", "feature"]);
    commit_file(&path, "conflict.txt", "feature\n", "feature");

    let _ = Command::new("git")
        .args(["rebase", "main"])
        .current_dir(&path)
        .output();

    let rebase = RebaseService::new(Arc::new(RepoContext::new(path.to_str().unwrap()).unwrap()));
    let preview = rebase.abort_preview().unwrap();
    assert!(preview.warning.contains("Aborting"));
}

#[test]
#[serial]
fn continue_native_interactive_reword_without_editor_hang() {
    // Reproduces the GUI hang: `git rebase --continue` after reword must not wait on an editor.
    let (_dir, path) = init_repo();
    git(&path, &["checkout", "-b", "feature"]);
    commit_file(&path, "a.txt", "a\n", "feat a");
    commit_file(&path, "b.txt", "b\n", "feat b");
    git(&path, &["checkout", "main"]);
    commit_file(&path, "m.txt", "m\n", "main tip");
    git(&path, &["checkout", "feature"]);

    let seq_editor = path.join("rewrite-todo.sh");
    fs::write(
        &seq_editor,
        "#!/bin/sh\nsed -i.bak '1s/^pick/reword/' \"$1\"\n",
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&seq_editor).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&seq_editor, perms).unwrap();
    }

    // Fail the reword editor on purpose so the rebase stays paused at the reword step.
    let _ = Command::new("git")
        .args(["rebase", "-i", "main"])
        .current_dir(&path)
        .env("GIT_SEQUENCE_EDITOR", &seq_editor)
        .env("GIT_EDITOR", "false")
        .output();
    assert!(
        path.join(".git/rebase-merge").is_dir(),
        "expected rebase-merge dir after reword pause"
    );

    let rebase = RebaseService::new(Arc::new(RepoContext::new(path.to_str().unwrap()).unwrap()));
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let result = rt.block_on(async {
        tokio::time::timeout(
            std::time::Duration::from_secs(15),
            rebase.continue_rebase(Some("feat a reworded\n".into()), None),
        )
        .await
    });
    assert!(
        matches!(result, Ok(Ok(_))),
        "continue should finish quickly without waiting for an editor: {result:?}"
    );
}

#[test]
#[serial]
fn detects_native_edit_pause_not_reword() {
    let (_dir, path) = init_repo();
    git(&path, &["checkout", "-b", "feature"]);
    commit_file(&path, "a.txt", "a\n", "feat a");
    commit_file(&path, "b.txt", "b\n", "feat b");
    git(&path, &["checkout", "main"]);
    commit_file(&path, "m.txt", "m\n", "main tip");
    git(&path, &["checkout", "feature"]);

    let seq_editor = path.join("rewrite-todo.sh");
    fs::write(
        &seq_editor,
        "#!/bin/sh\nsed -i.bak '1s/^pick/edit/' \"$1\"\n",
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&seq_editor).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&seq_editor, perms).unwrap();
    }

    let _ = Command::new("git")
        .args(["rebase", "-i", "main"])
        .current_dir(&path)
        .env("GIT_SEQUENCE_EDITOR", &seq_editor)
        .env("GIT_EDITOR", "true")
        .output();
    assert!(path.join(".git/rebase-merge").is_dir());
    assert!(
        path.join(".git/rebase-merge/amend").is_file(),
        "edit pause should create amend file"
    );

    let op = OperationService::new(Arc::new(RepoContext::new(path.to_str().unwrap()).unwrap()))
        .get_repo_operation()
        .unwrap();
    assert!(op.is_rebasing);
    assert_eq!(
        op.pause_reason,
        Some(crate::models::rebase::RebasePauseReason::Edit),
        "edit stop must not be reported as reword: {op:?}"
    );
}

#[test]
#[serial]
fn update_native_interactive_todo_actions() {
    let (_dir, path) = init_repo();
    git(&path, &["checkout", "-b", "feature"]);
    commit_file(&path, "a.txt", "a\n", "feat a");
    commit_file(&path, "b.txt", "b\n", "feat b");
    git(&path, &["checkout", "main"]);
    commit_file(&path, "m.txt", "m\n", "main tip");
    git(&path, &["checkout", "feature"]);

    let seq_editor = path.join("rewrite-todo.sh");
    fs::write(
        &seq_editor,
        "#!/bin/sh\nsed -i.bak '1s/^pick/edit/' \"$1\"\n",
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&seq_editor).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&seq_editor, perms).unwrap();
    }

    let _ = Command::new("git")
        .args(["rebase", "-i", "main"])
        .current_dir(&path)
        .env("GIT_SEQUENCE_EDITOR", &seq_editor)
        .env("GIT_EDITOR", "true")
        .output();
    assert!(path.join(".git/rebase-merge").is_dir());

    let rebase = RebaseService::new(Arc::new(RepoContext::new(path.to_str().unwrap()).unwrap()));
    let op = rebase.get_repo_operation().unwrap();
    assert!(op.is_rebasing);
    let pending: Vec<_> = op
        .todo
        .iter()
        .filter(|e| matches!(e.status, crate::models::rebase::RebaseTodoStatus::Pending))
        .collect();
    assert!(!pending.is_empty(), "expected pending todo entries");

    let mut entries: Vec<RebasePlanEntry> = op
        .todo
        .iter()
        .map(|e| RebasePlanEntry {
            action: e.action.clone(),
            commit: e.commit.clone(),
            message: Some(e.message.clone()),
        })
        .collect();
    // Drop the last pending commit via todo edit.
    if let Some(last) = entries.last_mut() {
        last.action = RebaseAction::Drop;
    }
    let updated = rebase.update_todo(entries).unwrap();
    assert!(updated
        .todo
        .iter()
        .any(|e| matches!(e.action, RebaseAction::Drop)));
}

#[test]
#[serial]
fn plan_rebase_lists_commits() {
    let (_dir, path) = init_repo();
    git(&path, &["checkout", "-b", "feature"]);
    commit_file(&path, "a.txt", "a\n", "feat a");
    commit_file(&path, "b.txt", "b\n", "feat b");
    git(&path, &["checkout", "main"]);
    let onto = git(&path, &["rev-parse", "HEAD"]);
    git(&path, &["checkout", "feature"]);

    let rebase = RebaseService::new(Arc::new(RepoContext::new(path.to_str().unwrap()).unwrap()));
    let plan = rebase.plan_rebase(&onto, None).unwrap();
    assert_eq!(plan.entries.len(), 2);
    assert!(matches!(plan.entries[0].action, RebaseAction::Pick));
}

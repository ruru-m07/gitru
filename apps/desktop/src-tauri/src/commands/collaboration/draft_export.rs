use std::{
    future::Future,
    io::Write,
    path::{Path, PathBuf},
};

use collaboration::{CollaborationError, ErrorCode, LocalDraft, Store};

pub(super) async fn saved(
    store: &Store,
    account_id: &str,
    subject_id: &str,
    generation: &str,
) -> Result<LocalDraft, CollaborationError> {
    let draft = store
        .draft(account_id, subject_id)
        .await?
        .ok_or_else(|| CollaborationError::invalid("The saved draft does not exist"))?;
    if draft.generation != generation {
        return Err(CollaborationError::new(
            ErrorCode::StaleView,
            "The saved draft changed",
        ));
    }
    Ok(draft)
}

pub(super) async fn export(
    draft: LocalDraft,
    choice: impl Future<Output = Result<Option<PathBuf>, CollaborationError>>,
) -> Result<bool, CollaborationError> {
    let Some(path) = choice.await? else {
        return Ok(false);
    };
    // Capture the inspected saved generation before opening the dialog. A
    // concurrent edit cannot silently substitute different text for this export.
    tokio::task::spawn_blocking(move || write(&path, &draft.body))
        .await
        .map_err(|_| CollaborationError::storage())??;
    Ok(true)
}

/// Write beside the explicitly selected destination, then replace atomically.
/// This avoids truncating an existing export when a write fails and does not
/// follow a destination symlink. Temporary files are private and self-cleaning.
pub(super) fn write(path: &Path, body: &str) -> Result<(), CollaborationError> {
    let parent = path.parent().ok_or_else(CollaborationError::storage)?;
    let mut temporary =
        tempfile::NamedTempFile::new_in(parent).map_err(|_| CollaborationError::storage())?;
    temporary
        .write_all(body.as_bytes())
        .map_err(|_| CollaborationError::storage())?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|_| CollaborationError::storage())?;
    temporary
        .persist(path)
        .map_err(|_| CollaborationError::storage())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{export, saved, write};
    use collaboration::{AccountState, ErrorCode, LocalDraft, ProviderKind, RemoteAccount, Store};

    #[tokio::test]
    async fn dialog_cancellation_writes_nothing_and_concurrent_edits_keep_the_inspected_generation()
    {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(dir.path().join("drafts.db")).await.unwrap();
        store
            .upsert_account(RemoteAccount {
                id: "actor".into(),
                provider: ProviderKind::Github,
                host: "github.com".into(),
                actor_id: "1".into(),
                login: "fixture".into(),
                display_name: None,
                authorization_epoch: "1".into(),
                state: AccountState::Disconnected,
                notifications_supported: false,
            })
            .await
            .unwrap();
        let draft = store
            .save_draft(LocalDraft {
                account_id: "actor".into(),
                subject_id: "missing".into(),
                body: "inspected text".into(),
                generation: "0".into(),
            })
            .await
            .unwrap();
        let snapshot = saved(&store, "actor", "missing", "1").await.unwrap();
        assert!(!export(snapshot.clone(), async { Ok(None) }).await.unwrap());
        let chosen = dir.path().join("user-selected.txt");
        assert!(!chosen.exists());
        let (send, receive) = tokio::sync::oneshot::channel();
        let task = tokio::spawn(export(snapshot, async { Ok(receive.await.unwrap()) }));
        store
            .save_draft(LocalDraft {
                body: "newer concurrent text".into(),
                ..draft
            })
            .await
            .unwrap();
        send.send(Some(chosen.clone())).unwrap();
        assert!(task.await.unwrap().unwrap());
        assert_eq!(std::fs::read_to_string(chosen).unwrap(), "inspected text");
        assert_eq!(
            saved(&store, "actor", "missing", "1")
                .await
                .unwrap_err()
                .code,
            ErrorCode::StaleView
        );
        assert_eq!(
            saved(&store, "actor", "missing", "2").await.unwrap().body,
            "newer concurrent text"
        );
        assert!(saved(&store, "another-actor", "missing", "2")
            .await
            .is_err());
    }

    #[test]
    fn export_replaces_only_the_chosen_file_with_exact_authored_utf8() {
        let dir = tempfile::tempdir().unwrap();
        let chosen = dir.path().join("chosen.txt");
        let other = dir.path().join("other.txt");
        std::fs::write(&chosen, "old export").unwrap();
        std::fs::write(&other, "keep").unwrap();
        write(&chosen, "Private draft\nこんにちは\n").unwrap();
        assert_eq!(
            std::fs::read_to_string(chosen).unwrap(),
            "Private draft\nこんにちは\n"
        );
        assert_eq!(std::fs::read_to_string(other).unwrap(), "keep");
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 2);
    }

    #[test]
    fn failed_destination_preserves_existing_files_and_removes_temporary_file() {
        let dir = tempfile::tempdir().unwrap();
        let chosen = dir.path().join("directory");
        std::fs::create_dir(&chosen).unwrap();
        let inside = chosen.join("keep.txt");
        std::fs::write(&inside, "keep").unwrap();
        assert!(write(&chosen, "draft").is_err());
        assert_eq!(std::fs::read_to_string(inside).unwrap(), "keep");
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[cfg(unix)]
    #[test]
    fn export_does_not_follow_a_destination_symlink_and_is_private() {
        use std::os::unix::{fs::symlink, fs::PermissionsExt};
        let dir = tempfile::tempdir().unwrap();
        let actual = dir.path().join("actual.txt");
        let selected = dir.path().join("selected.txt");
        std::fs::write(&actual, "keep").unwrap();
        symlink(&actual, &selected).unwrap();
        write(&selected, "draft").unwrap();
        assert_eq!(std::fs::read_to_string(actual).unwrap(), "keep");
        assert_eq!(std::fs::read_to_string(&selected).unwrap(), "draft");
        assert_eq!(
            std::fs::metadata(selected).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
}

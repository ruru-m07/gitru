//! Actual OS descriptor ownership, independent of scheduling or provider timing.
use super::*;

#[test]
fn closing_only_the_original_descriptor_keeps_the_raw_os_lock() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("lease.lock");
    let original = std::fs::File::create(&path).unwrap();
    original.try_lock().unwrap();
    let inherited_description = original.try_clone().unwrap();
    let independent = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&path)
        .unwrap();

    drop(original);
    assert!(matches!(
        independent.try_lock(),
        Err(std::fs::TryLockError::WouldBlock)
    ));
    inherited_description.unlock().unwrap();
    independent.try_lock().unwrap();
    drop(inherited_description);
    independent.unlock().unwrap();
}

#[tokio::test]
async fn actual_close_releases_lease_despite_clones_and_an_inherited_descriptor() {
    use std::os::unix::fs::MetadataExt;

    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("cache.sqlite");
    let owner = Store::open(&path).await.unwrap();
    let clone = owner.clone();
    let weak_owner = Arc::downgrade(&owner.inner);
    // dup and fork share the same open file description on Unix. Keep it
    // alive deterministically instead of racing an unrelated fork/exec.
    let inherited_description = owner
        .inner
        .writer_lease
        .lock()
        .await
        .as_ref()
        .unwrap()
        .file
        .try_clone()
        .unwrap();
    let lease_path = path.with_file_name("cache.sqlite.lock");
    let identity = inherited_description.metadata().unwrap();

    owner.close().await.unwrap();
    assert!(
        clone.revision().await.is_err(),
        "old readers are closed on every clone"
    );
    assert!(
        clone.inner.writer.acquire().await.is_err(),
        "old writers are closed on every clone"
    );
    drop(owner);
    assert_eq!(Arc::strong_count(&clone.inner), 1);
    clone.close().await.unwrap(); // repeated close is idempotent
    drop(clone);
    assert!(weak_owner.upgrade().is_none());

    let reopened = Store::open(&path)
        .await
        .expect("final Store drop releases the lease before inherited descriptor closure");
    assert_eq!(reopened.revision().await.unwrap(), "0");
    let retained_identity = std::fs::metadata(&lease_path).unwrap();
    assert_eq!(identity.dev(), retained_identity.dev());
    assert_eq!(identity.ino(), retained_identity.ino());
    // Closing the old descriptor cannot release the new owner's independent
    // lock or replace the stable lease inode.
    drop(inherited_description);
    assert_eq!(
        Store::open(&path).await.err().unwrap().code,
        ErrorCode::Busy
    );
    reopened.close().await.unwrap();
    drop(reopened);
    assert!(Store::open(&path).await.is_ok());
}

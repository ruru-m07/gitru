//! Native database-key bootstrap, deliberately independent of provider tokens.
//!
//! Not wired to Store::open until the cipher and all keyed connection paths are
//! qualified. Call synchronous preparation on an owned blocking task. Keep this
//! session alive until every connection using its key has actually closed.
use std::{
    fmt,
    path::{Path, PathBuf},
};
use subtle::ConstantTimeEq;
use zeroize::Zeroize;

mod files;
#[cfg(test)]
mod tests;

pub struct DatabaseKey([u8; 32]);
impl DatabaseKey {
    /// Native vault/codec boundary only. No text/IPC representation is provided.
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
    pub fn expose(&self) -> &[u8; 32] {
        &self.0
    }
    fn generate() -> Result<Self, DatabaseKeyError> {
        let mut key = Self([0; 32]);
        getrandom::fill(&mut key.0).map_err(|_| DatabaseKeyError::EntropyUnavailable)?;
        Ok(key)
    }
    fn same(&self, other: &Self) -> bool {
        bool::from(self.0.ct_eq(&other.0))
    }
}
impl fmt::Debug for DatabaseKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("DatabaseKey([REDACTED])")
    }
}
impl Drop for DatabaseKey {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatabaseKeyIdentity {
    database_id: String,
    generation: u64,
}
impl DatabaseKeyIdentity {
    pub fn database_id(&self) -> &str {
        &self.database_id
    }
    pub fn generation(&self) -> u64 {
        self.generation
    }
    /// Database-only namespace; never pass this to provider CredentialVault.
    pub fn vault_reference(&self) -> String {
        format!(
            "gitru.collaboration.database.v1.{}.{}",
            self.database_id, self.generation
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DatabaseKeyError {
    Busy,
    Storage,
    InvalidMetadata,
    InterruptedMetadata,
    InterruptedRestore,
    CreationNotAuthorized,
    PlaintextMigrationRequired,
    MissingKeyMetadata,
    MissingKey,
    MissingDatabase,
    VaultLocked,
    VaultUnavailable,
    VaultWriteUncertain,
    VaultKeyMismatch,
    EntropyUnavailable,
    WrongKeyOrCorrupt,
    StaleFilesystem,
}
impl fmt::Display for DatabaseKeyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Busy => "Database storage is already owned by another session",
            Self::Storage => "Database key metadata could not be stored safely",
            Self::InvalidMetadata => "Database key metadata is invalid; files were preserved",
            Self::InterruptedMetadata => {
                "Database key metadata publication was interrupted; files were preserved"
            }
            Self::InterruptedRestore => "Database recovery is pending; files were preserved",
            Self::CreationNotAuthorized => "A new database key reservation was not requested",
            Self::PlaintextMigrationRequired => {
                "The existing database requires verified encrypted migration"
            }
            Self::MissingKeyMetadata => {
                "Existing database data has no key metadata; files were preserved"
            }
            Self::MissingKey => "The database key is missing; files were preserved",
            Self::MissingDatabase => "The expected database is missing; key metadata was preserved",
            Self::VaultLocked => "Unlock the operating system database-key vault and retry",
            Self::VaultUnavailable => "The operating system database-key vault is unavailable",
            Self::VaultWriteUncertain => {
                "Database key persistence could not be verified; retry the saved reservation"
            }
            Self::VaultKeyMismatch => {
                "Database key readback differs; the saved key was not replaced"
            }
            Self::EntropyUnavailable => "Secure database key generation is unavailable",
            Self::WrongKeyOrCorrupt => {
                "The database key or encrypted data could not be verified; files were preserved"
            }
            Self::StaleFilesystem => {
                "Database files changed during key startup; retry without replacing them"
            }
        })
    }
}
impl std::error::Error for DatabaseKeyError {}

/// OS adapter operations are blocking. A failed store can still have persisted
/// the key. Never replace an existing entry: preparation always loads first and
/// verifies readback after store. There is intentionally no deletion operation.
pub trait DatabaseKeyVault: Send + Sync {
    fn load(&self, identity: &DatabaseKeyIdentity)
    -> Result<Option<DatabaseKey>, DatabaseKeyError>;
    fn store_new(
        &self,
        identity: &DatabaseKeyIdentity,
        key: &DatabaseKey,
    ) -> Result<(), DatabaseKeyError>;
}

/// Trusted native codec seam. Must authenticate an existing database read-only,
/// prove this identity from encrypted content, reject plaintext/unkeyed handles,
/// and close its handle before returning. No default or production adapter exists.
#[async_trait::async_trait]
pub trait DatabaseKeyVerifier: Send + Sync {
    async fn verify(
        &self,
        path: &Path,
        identity: &DatabaseKeyIdentity,
        key: &DatabaseKey,
    ) -> Result<(), DatabaseKeyError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DatabaseCreation {
    ExistingOnly,
    AllowNew,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DatabaseKeyMode {
    CreateNew,
    VerifyInterruptedCreation,
    VerifyExisting,
    Verified,
}

pub struct DatabaseKeySession {
    path: PathBuf,
    journal: files::Journal,
    key: DatabaseKey,
    mode: DatabaseKeyMode,
    _lease: crate::storage::WriterLease,
}
impl fmt::Debug for DatabaseKeySession {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DatabaseKeySession")
            .field("identity", &self.journal.identity())
            .field("mode", &self.mode)
            .finish_non_exhaustive()
    }
}
impl DatabaseKeySession {
    /// Native-only bootstrap. Existing DB/sidecar evidence can never authorize
    /// generation, even when the journal says Reserved or the file is empty.
    pub fn prepare(
        path: &Path,
        vault: &dyn DatabaseKeyVault,
        creation: DatabaseCreation,
    ) -> Result<Self, DatabaseKeyError> {
        let path = files::canonical_path(path)?;
        let lease = crate::storage::acquire_writer_lease(&path).map_err(|e| {
            if e.code == crate::ErrorCode::Busy {
                DatabaseKeyError::Busy
            } else {
                DatabaseKeyError::Storage
            }
        })?;
        crate::recovery::require_no_pending_restore(&path)
            .map_err(|_| DatabaseKeyError::InterruptedRestore)?;
        files::require_no_pending(&path)?;
        let mut before = files::observe(&path)?;
        let journal = match files::read(&path)? {
            Some(journal) => journal,
            None if before.any() => return Err(files::unmanaged_error(&path, &before)?),
            None if creation == DatabaseCreation::ExistingOnly => {
                return Err(DatabaseKeyError::CreationNotAuthorized);
            }
            None => {
                let journal = files::Journal::reserved();
                files::publish(&path, &journal, false)?;
                before = files::observe(&path)?;
                journal
            }
        };
        if journal.ready && before.database.is_none() {
            return Err(DatabaseKeyError::MissingDatabase);
        }
        let identity = journal.identity();
        let key = match vault.load(&identity)? {
            Some(key) => key,
            None if journal.ready || before.any() => return Err(DatabaseKeyError::MissingKey),
            None => {
                if files::observe(&path)? != before {
                    return Err(DatabaseKeyError::StaleFilesystem);
                }
                let generated = DatabaseKey::generate()?;
                let stored = vault.store_new(&identity, &generated);
                match vault.load(&identity) {
                    Ok(Some(saved)) if generated.same(&saved) => saved,
                    Ok(Some(_)) => return Err(DatabaseKeyError::VaultKeyMismatch),
                    Ok(None) => {
                        return Err(stored
                            .err()
                            .unwrap_or(DatabaseKeyError::VaultWriteUncertain));
                    }
                    Err(error) => return Err(error),
                }
            }
        };
        files::require_no_pending(&path)?;
        if files::observe(&path)? != before || files::read(&path)?.as_ref() != Some(&journal) {
            return Err(DatabaseKeyError::StaleFilesystem);
        }
        let mode = if journal.ready {
            DatabaseKeyMode::VerifyExisting
        } else if before.database.is_some() {
            DatabaseKeyMode::VerifyInterruptedCreation
        } else if before.any() {
            return Err(DatabaseKeyError::MissingDatabase);
        } else {
            DatabaseKeyMode::CreateNew
        };
        Ok(Self {
            path,
            journal,
            key,
            mode,
            _lease: lease,
        })
    }
    pub fn identity(&self) -> DatabaseKeyIdentity {
        self.journal.identity()
    }
    pub fn mode(&self) -> DatabaseKeyMode {
        self.mode
    }
    /// Only a native keyed connection factory may consume these bytes. The
    /// factory must retain this session/lease until its last SQLite handle closes.
    pub fn key(&self) -> &DatabaseKey {
        &self.key
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub async fn verify(
        &mut self,
        verifier: &dyn DatabaseKeyVerifier,
    ) -> Result<(), DatabaseKeyError> {
        // A prior success is not authority for this new attempt. This reset also
        // survives failure or cancellation while the native verifier is held.
        self.mode = if self.journal.ready {
            DatabaseKeyMode::VerifyExisting
        } else {
            DatabaseKeyMode::VerifyInterruptedCreation
        };
        files::require_no_pending(&self.path)?;
        if files::read(&self.path)?.as_ref() != Some(&self.journal) {
            return Err(DatabaseKeyError::StaleFilesystem);
        }
        let before = files::observe(&self.path)?;
        if before.database.is_none() {
            return Err(DatabaseKeyError::MissingDatabase);
        }
        verifier
            .verify(&self.path, &self.identity(), &self.key)
            .await?;
        files::require_no_pending(&self.path)?;
        if files::observe(&self.path)? != before
            || files::read(&self.path)?.as_ref() != Some(&self.journal)
        {
            return Err(DatabaseKeyError::StaleFilesystem);
        }
        if !self.journal.ready {
            let mut ready = self.journal.clone();
            ready.ready = true;
            files::publish(&self.path, &ready, true)?;
            self.journal = ready;
        }
        self.mode = DatabaseKeyMode::Verified;
        Ok(())
    }
}

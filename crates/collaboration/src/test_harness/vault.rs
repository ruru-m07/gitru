//! A durable allowlisted synthetic vault. Native keyring is never constructed.
use super::{HarnessActorSlot, files::OwnedRoot};
use crate::credentials::{CredentialError, CredentialVault, SecretToken};
use sha2::{Digest, Sha256};
use std::sync::atomic::{AtomicU64, Ordering};

pub(super) const PRIMARY_TOKEN: &str = "ruru103-synthetic-primary-only";
pub(super) const ALTERNATE_TOKEN: &str = "ruru103-synthetic-alternate-only";

pub(super) fn token(slot: HarnessActorSlot) -> SecretToken {
    SecretToken::new(match slot {
        HarnessActorSlot::Primary => PRIMARY_TOKEN.into(),
        HarnessActorSlot::Alternate => ALTERNATE_TOKEN.into(),
    })
    .expect("compiled ASCII synthetic token")
}

pub(super) fn token_slot(value: &SecretToken) -> Result<HarnessActorSlot, CredentialError> {
    match value.expose() {
        PRIMARY_TOKEN => Ok(HarnessActorSlot::Primary),
        ALTERNATE_TOKEN => Ok(HarnessActorSlot::Alternate),
        _ => Err(CredentialError::InvalidToken),
    }
}

pub(super) struct FixtureVault {
    root: OwnedRoot,
    files: OwnedRoot,
    primary: String,
    alternate: String,
    pub loads: AtomicU64,
    pub stores: AtomicU64,
    pub deletes: AtomicU64,
}

impl FixtureVault {
    pub fn new(root: OwnedRoot, nonce: &str) -> Result<Self, crate::CollaborationError> {
        let files = root.vault()?;
        Ok(Self {
            root,
            files,
            primary: format!("ruru103:{nonce}:primary"),
            alternate: format!("ruru103:{nonce}:alternate"),
            loads: AtomicU64::new(0),
            stores: AtomicU64::new(0),
            deletes: AtomicU64::new(0),
        })
    }

    pub fn reference(&self, slot: HarnessActorSlot) -> &str {
        match slot {
            HarnessActorSlot::Primary => &self.primary,
            HarnessActorSlot::Alternate => &self.alternate,
        }
    }

    fn key(&self, reference: &str) -> Result<(String, HarnessActorSlot), CredentialError> {
        self.root
            .check()
            .map_err(|_| CredentialError::Unavailable)?;
        let slot = if reference == self.primary {
            HarnessActorSlot::Primary
        } else if reference == self.alternate {
            HarnessActorSlot::Alternate
        } else {
            return Err(CredentialError::InvalidToken);
        };
        // Never use references as filesystem names, even in a synthetic vault.
        Ok((
            format!("{:x}.token", Sha256::digest(reference.as_bytes())),
            slot,
        ))
    }
}

impl CredentialVault for FixtureVault {
    fn store(&self, reference: &str, value: &SecretToken) -> Result<(), CredentialError> {
        let (key, slot) = self.key(reference)?;
        if token_slot(value)? != slot {
            return Err(CredentialError::InvalidToken);
        }
        self.files
            .write_bytes(&key, value.expose().as_bytes())
            .map_err(|_| CredentialError::Unavailable)?;
        self.stores.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    fn load(&self, reference: &str) -> Result<Option<SecretToken>, CredentialError> {
        let (key, slot) = self.key(reference)?;
        let Some(bytes) = self
            .files
            .read(&key, 256)
            .map_err(|_| CredentialError::Unavailable)?
        else {
            self.loads.fetch_add(1, Ordering::SeqCst);
            return Ok(None);
        };
        let value =
            SecretToken::new(String::from_utf8(bytes).map_err(|_| CredentialError::InvalidToken)?)?;
        if token_slot(&value)? != slot {
            return Err(CredentialError::InvalidToken);
        }
        self.loads.fetch_add(1, Ordering::SeqCst);
        Ok(Some(value))
    }

    fn delete(&self, reference: &str) -> Result<(), CredentialError> {
        let (key, _) = self.key(reference)?;
        self.files
            .remove(&key)
            .map_err(|_| CredentialError::Unavailable)?;
        self.deletes.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

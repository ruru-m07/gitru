//! Explicit-handle macOS file-keychain adapter, never selected by app startup.
//!
//! File keychains are a legacy SecItem shim. Calls are blocking and can require
//! UI if the keychain locks after the status check. The qualified fixture uses
//! a dedicated noninteractive child process. Do not wire this adapter into app
//! startup without separately qualifying that owned execution boundary.
use super::{DatabaseKey, DatabaseKeyError as Error, DatabaseKeyIdentity, DatabaseKeyVault};
use core_foundation::{
    array::CFArray,
    base::{CFType, TCFType},
    boolean::CFBoolean,
    data::CFData,
    dictionary::CFDictionary,
    number::CFNumber,
    string::CFString,
};
use security_framework::os::macos::keychain::SecKeychain;
use security_framework_sys::{
    base::*,
    item::*,
    keychain_item::{SecItemAdd, SecItemCopyMatching},
};
use std::{fmt, ptr, sync::Arc};
use zeroize::Zeroizing;

const SERVICE: &str = "com.gitru.collaboration.database-key.v1";
const UNLOCKED: u32 = 1;
#[link(name = "Security", kind = "framework")]
unsafe extern "C" {
    fn SecKeychainGetStatus(keychain: SecKeychainRef, status: *mut u32) -> i32;
}

/// An exact native keychain location, with no default/search-list fallback.
/// No update/delete operation is exposed, and no provider credential namespace
/// is reachable through the typed database identity.
pub struct FileKeychainDatabaseVault {
    keychain: SecKeychain,
}
impl fmt::Debug for FileKeychainDatabaseVault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("FileKeychainDatabaseVault([NATIVE HANDLE])")
    }
}
impl FileKeychainDatabaseVault {
    pub fn from_keychain(keychain: SecKeychain) -> Self {
        Self { keychain }
    }
    fn unlocked(&self) -> Result<(), Error> {
        let mut status = 0;
        // The retained CF handle is valid through the synchronous OS call.
        let result =
            unsafe { SecKeychainGetStatus(self.keychain.as_concrete_TypeRef(), &mut status) };
        if result != errSecSuccess {
            return Err(Error::VaultUnavailable);
        }
        if status & UNLOCKED == 0 {
            return Err(Error::VaultLocked);
        }
        Ok(())
    }
    fn attributes(&self, identity: &DatabaseKeyIdentity) -> Vec<(CFType, CFType)> {
        // Security constants are immortal framework-owned CFStrings.
        unsafe {
            vec![
                (
                    CFString::wrap_under_get_rule(kSecClass).into_CFType(),
                    CFString::wrap_under_get_rule(kSecClassGenericPassword).into_CFType(),
                ),
                (
                    CFString::wrap_under_get_rule(kSecAttrService).into_CFType(),
                    CFString::new(SERVICE).into_CFType(),
                ),
                (
                    CFString::wrap_under_get_rule(kSecAttrAccount).into_CFType(),
                    CFString::new(&identity.vault_reference()).into_CFType(),
                ),
            ]
        }
    }
    fn add(&self, identity: &DatabaseKeyIdentity, bytes: &[u8]) -> Result<(), Error> {
        self.unlocked()?;
        let mut attributes = self.attributes(identity);
        // CFData retains this Arc until all framework references release it;
        // the only owned input copy zeroizes then, including failed additions.
        let data = CFData::from_arc(Arc::new(SecretBytes(Zeroizing::new(bytes.to_vec()))));
        unsafe {
            attributes.push((
                CFString::wrap_under_get_rule(kSecUseKeychain).into_CFType(),
                self.keychain.as_CFType(),
            ));
            attributes.push((
                CFString::wrap_under_get_rule(kSecValueData).into_CFType(),
                data.into_CFType(),
            ));
        }
        let query = CFDictionary::from_CFType_pairs(&attributes);
        let status = unsafe { SecItemAdd(query.as_concrete_TypeRef(), ptr::null_mut()) };
        match status {
            value if value == errSecSuccess => Ok(()),
            value if value == errSecDuplicateItem => Err(Error::VaultAlreadyExists),
            _ => {
                self.unlocked()?;
                Err(Error::VaultWriteUncertain)
            }
        }
    }
}
struct SecretBytes(Zeroizing<Vec<u8>>);
impl AsRef<[u8]> for SecretBytes {
    fn as_ref(&self) -> &[u8] {
        self.0.as_slice()
    }
}
impl DatabaseKeyVault for FileKeychainDatabaseVault {
    fn load(&self, identity: &DatabaseKeyIdentity) -> Result<Option<DatabaseKey>, Error> {
        self.unlocked()?;
        let mut attributes = self.attributes(identity);
        let locations = CFArray::from_CFTypes(std::slice::from_ref(&self.keychain));
        unsafe {
            attributes.push((
                CFString::wrap_under_get_rule(kSecMatchSearchList).into_CFType(),
                locations.into_CFType(),
            ));
            attributes.push((
                CFString::wrap_under_get_rule(kSecMatchLimit).into_CFType(),
                CFNumber::from(1i32).into_CFType(),
            ));
            attributes.push((
                CFString::wrap_under_get_rule(kSecReturnData).into_CFType(),
                CFBoolean::true_value().into_CFType(),
            ));
        }
        let query = CFDictionary::from_CFType_pairs(&attributes);
        let mut result = ptr::null();
        let status = unsafe { SecItemCopyMatching(query.as_concrete_TypeRef(), &mut result) };
        // A successful copy returns one owned CF value. Keep it type-checked and
        // release the framework-owned buffer; never stringify it or OS errors.
        let value = if result.is_null() {
            None
        } else {
            Some(unsafe { CFType::wrap_under_create_rule(result) })
        };
        match status {
            value if value == errSecItemNotFound => Ok(None),
            status if status == errSecSuccess => {
                let value = value.ok_or(Error::VaultInvalidKey)?;
                let data = value.downcast::<CFData>().ok_or(Error::VaultInvalidKey)?;
                if data.len() != 32 {
                    return Err(Error::VaultInvalidKey);
                }
                // Copy directly into the zeroizing owner, with no additional
                // plaintext array that would outlive this scope unwiped.
                let mut key = DatabaseKey([0; 32]);
                key.0.copy_from_slice(data.bytes());
                Ok(Some(key))
            }
            _ => {
                self.unlocked()?;
                Err(Error::VaultUnavailable)
            }
        }
    }
    fn store_new(&self, identity: &DatabaseKeyIdentity, key: &DatabaseKey) -> Result<(), Error> {
        self.add(identity, key.expose())
    }
}

#[cfg(test)]
mod tests;

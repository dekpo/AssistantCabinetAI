//! The per-workstation key that stands between a personal identifier and the knowledge base.
//!
//! An e-mail address, a social security number and an IBAN each point at one person. The knowledge
//! base needs only to know that two documents carry the **same** one, so it stores a keyed hash of
//! the reduced value and never the value (`docs/DECISIONS.md`, "Owner answers after KB lot 3").
//!
//! ```text
//! digest = HMAC-SHA256(key, "acai.identifier.v1" 0x00 scheme 0x00 reduced value), as 64 hex digits
//! ```
//!
//! The key is 32 random bytes made by the operating system's random source the first time it is
//! needed. It lives in a file of its own in the application's local data folder, **outside**
//! `index.sqlite3`, so a copy of the index (a backup, a sync client, a support request) does not
//! carry it. It is never logged, never printed (`Debug` hides it), never sent to a model and never
//! part of a prompt. A fingerprint of it, which reveals nothing usable, is kept in `kb_meta` so a
//! lost or replaced key is noticed and the documents are read again with the new one.
//!
//! What this protects and what it does not is written in `docs/PRIVACY-AND-SECURITY.md`: it keeps the
//! structured directory of identifiers from existing in the index; it does not hide the text of the
//! documents, which the index holds anyway for retrieval, and a short number can be guessed by
//! someone who holds both the index and the key.

use std::io::Write;
use std::path::Path;

use hmac::{Hmac, Mac};
use sha2::Sha256;

use crate::error::AppError;

/// The file in the application's local data folder that holds the key.
pub const KEY_FILE_NAME: &str = "knowledge-identifier.key";

const KEY_BYTES: usize = 32;
const DIGEST_DOMAIN: &[u8] = b"acai.identifier.v1";
const FINGERPRINT_DOMAIN: &[u8] = b"acai.identifier.key-check.v1";

type HmacSha256 = Hmac<Sha256>;

pub struct IdentifierKey {
    bytes: [u8; KEY_BYTES],
}

// Never derived: a derived `Debug` would print the key into a failed assertion or a log line.
impl std::fmt::Debug for IdentifierKey {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("IdentifierKey(..)")
    }
}

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn from_hex(text: &str) -> Option<[u8; KEY_BYTES]> {
    let text = text.trim();
    if text.len() != KEY_BYTES * 2 || !text.is_ascii() {
        return None;
    }
    let mut bytes = [0u8; KEY_BYTES];
    for (position, slot) in bytes.iter_mut().enumerate() {
        *slot = u8::from_str_radix(&text[position * 2..position * 2 + 2], 16).ok()?;
    }
    Some(bytes)
}

impl IdentifierKey {
    /// A key from known bytes: for tests, and for nothing else.
    pub fn from_bytes(bytes: [u8; KEY_BYTES]) -> Self {
        Self { bytes }
    }

    /// The key in `directory`, made the first time it is asked for. A file that does not hold a key
    /// (empty, truncated, edited by hand) is replaced: it can never be right again, and the
    /// fingerprint tells the next pass that the identifiers must be hashed again. A file that
    /// cannot be read at all is an error and is left alone, so a permission problem never costs a
    /// good key.
    pub fn load_or_create(directory: &Path) -> Result<Self, AppError> {
        let path = directory.join(KEY_FILE_NAME);
        match std::fs::read_to_string(&path) {
            Ok(text) => match from_hex(&text) {
                Some(bytes) => Ok(Self { bytes }),
                None => Self::replace(directory, &path),
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                std::fs::create_dir_all(directory).map_err(|_| AppError::KnowledgeUnavailable)?;
                let bytes = Self::random()?;
                match std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&path)
                {
                    Ok(mut file) => {
                        file.write_all(format!("{}\n", to_hex(&bytes)).as_bytes())
                            .map_err(|_| AppError::KnowledgeUnavailable)?;
                        Ok(Self { bytes })
                    }
                    // Another pass made it between the read and the create: use theirs.
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                        let text = std::fs::read_to_string(&path)
                            .map_err(|_| AppError::KnowledgeUnavailable)?;
                        from_hex(&text)
                            .map(|bytes| Self { bytes })
                            .ok_or(AppError::KnowledgeUnavailable)
                    }
                    Err(_) => Err(AppError::KnowledgeUnavailable),
                }
            }
            Err(_) => Err(AppError::KnowledgeUnavailable),
        }
    }

    fn replace(directory: &Path, path: &Path) -> Result<Self, AppError> {
        let bytes = Self::random()?;
        let staging = directory.join(format!("{KEY_FILE_NAME}.new"));
        std::fs::write(&staging, format!("{}\n", to_hex(&bytes)))
            .and_then(|()| std::fs::rename(&staging, path))
            .map_err(|_| AppError::KnowledgeUnavailable)?;
        Ok(Self { bytes })
    }

    fn random() -> Result<[u8; KEY_BYTES], AppError> {
        let mut bytes = [0u8; KEY_BYTES];
        getrandom::getrandom(&mut bytes).map_err(|_| AppError::KnowledgeUnavailable)?;
        Ok(bytes)
    }

    fn mac(&self) -> HmacSha256 {
        HmacSha256::new_from_slice(&self.bytes).expect("HMAC accepts a key of any length")
    }

    /// The stand-in for one identifier: 64 hex digits that are equal for two occurrences of the
    /// same reduced value under the same scheme, and unrelated to the value for anyone without the
    /// key. The scheme is part of the message, so the same digits under two schemes do not meet.
    pub fn digest(&self, scheme: &str, reduced_value: &str) -> String {
        let mut mac = self.mac();
        mac.update(DIGEST_DOMAIN);
        mac.update(&[0]);
        mac.update(scheme.as_bytes());
        mac.update(&[0]);
        mac.update(reduced_value.as_bytes());
        to_hex(&mac.finalize().into_bytes())
    }

    /// A short label of this key, recorded in `kb_meta`. Two different keys give two different
    /// labels; the label tells nothing about the key or about any identifier.
    pub fn fingerprint(&self) -> String {
        let mut mac = self.mac();
        mac.update(FINGERPRINT_DOMAIN);
        to_hex(&mac.finalize().into_bytes()[..8])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(seed: u8) -> IdentifierKey {
        IdentifierKey::from_bytes([seed; KEY_BYTES])
    }

    #[test]
    fn the_same_value_gives_the_same_digest_and_a_different_value_a_different_one() {
        let key = key(7);
        let first = key.digest("email", "ALICE@EXAMPLE.ORG");
        assert_eq!(first, key.digest("email", "ALICE@EXAMPLE.ORG"));
        assert_ne!(first, key.digest("email", "BOB@EXAMPLE.ORG"));
        assert_eq!(first.len(), 64);
        assert!(first.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn the_scheme_and_the_key_are_part_of_the_digest() {
        let value = "123456789";
        assert_ne!(
            key(1).digest("phone", value),
            key(1).digest("social", value)
        );
        assert_ne!(key(1).digest("phone", value), key(2).digest("phone", value));
    }

    #[test]
    fn the_digest_does_not_contain_the_value() {
        let digest = key(3).digest("iban", "FR7630006000011234567890189");
        assert!(!digest.contains("FR76"));
        assert!(!digest.to_uppercase().contains("1234567890"));
    }

    #[test]
    fn the_fingerprint_changes_with_the_key_and_is_not_the_key() {
        assert_ne!(key(1).fingerprint(), key(2).fingerprint());
        assert_eq!(key(1).fingerprint(), key(1).fingerprint());
        assert_eq!(key(1).fingerprint().len(), 16);
        assert!(!key(1).fingerprint().contains(&to_hex(&[1u8; 8])));
    }

    #[test]
    fn debug_never_prints_the_key() {
        let shown = format!("{:?}", key(0xab));
        assert_eq!(shown, "IdentifierKey(..)");
        assert!(!shown.contains("ab"));
    }

    #[test]
    fn the_key_is_made_once_and_then_read_back() {
        let directory = tempfile::tempdir().unwrap();
        let first = IdentifierKey::load_or_create(directory.path()).unwrap();
        let second = IdentifierKey::load_or_create(directory.path()).unwrap();
        assert_eq!(first.fingerprint(), second.fingerprint());
        assert_eq!(first.digest("email", "A"), second.digest("email", "A"));

        let stored = std::fs::read_to_string(directory.path().join(KEY_FILE_NAME)).unwrap();
        assert_eq!(stored.trim().len(), 64);
    }

    #[test]
    fn two_workstations_do_not_share_a_key() {
        let one = tempfile::tempdir().unwrap();
        let two = tempfile::tempdir().unwrap();
        let a = IdentifierKey::load_or_create(one.path()).unwrap();
        let b = IdentifierKey::load_or_create(two.path()).unwrap();
        assert_ne!(a.fingerprint(), b.fingerprint());
    }

    #[test]
    fn a_file_that_holds_no_key_is_replaced_and_a_new_fingerprint_says_so() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join(KEY_FILE_NAME);
        let made = IdentifierKey::load_or_create(directory.path()).unwrap();

        for broken in ["", "not hex", "abcd", &"zz".repeat(32)] {
            std::fs::write(&path, broken).unwrap();
            let replaced = IdentifierKey::load_or_create(directory.path()).unwrap();
            assert_ne!(replaced.fingerprint(), made.fingerprint());
            // And it is now a proper file again.
            let again = IdentifierKey::load_or_create(directory.path()).unwrap();
            assert_eq!(again.fingerprint(), replaced.fingerprint());
        }
    }

    #[test]
    fn an_unreadable_location_is_an_error_not_a_new_key() {
        // A directory where the file should be: it cannot be read as a file and must not be
        // overwritten.
        let directory = tempfile::tempdir().unwrap();
        std::fs::create_dir(directory.path().join(KEY_FILE_NAME)).unwrap();
        let result = IdentifierKey::load_or_create(directory.path());
        assert_eq!(result.unwrap_err().code(), "knowledge_unavailable");
        assert!(directory.path().join(KEY_FILE_NAME).is_dir());
    }
}

use std::{collections::HashMap, fmt};

use openssl::{
    hash::MessageDigest,
    pkey::PKey,
    sign::Signer,
    symm::{Cipher, decrypt_aead, encrypt_aead},
};
use rand::RngExt;
use thiserror::Error;

use super::{ContactCandidate, ContactType};

const NONCE_LEN: usize = 12;
const MIN_CONTACT_PEPPER_BYTES: usize = 32;
const TAG_LEN: usize = 16;

/// AES-256-GCM envelope bound to a key version.
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct EncryptedValue {
    pub(crate) key_version: String,
    pub(crate) bytes: Vec<u8>,
}

impl fmt::Debug for EncryptedValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("EncryptedValue")
            .field("key_version", &self.key_version)
            .field("bytes", &"[REDACTED]")
            .finish()
    }
}

/// Errors raised while protecting talent source data.
#[derive(Debug, Error, PartialEq, Eq)]
pub(crate) enum CryptoError {
    #[error("invalid AES-256 key configuration")]
    InvalidKeyConfiguration,
    #[error("active encryption key version is not configured")]
    MissingActiveKey,
    #[error("contact hash pepper is empty")]
    EmptyContactPepper,
    #[error("contact hash pepper does not meet the minimum strength policy")]
    WeakContactPepper,
    #[error("encrypted value uses an unknown key version")]
    UnknownKeyVersion,
    #[error("encrypted value has an invalid envelope")]
    InvalidEnvelope,
    #[error("unable to protect talent source data")]
    EncryptionFailed,
    #[error("unable to decrypt protected talent source data")]
    DecryptionFailed,
    #[error("unable to hash contact data")]
    ContactHashFailed,
}

/// Encrypts source values and hashes normalized contacts with separate key material.
pub(crate) struct TalentCrypto {
    active_version: String,
    keys: HashMap<String, [u8; 32]>,
    contact_pepper: Vec<u8>,
}

impl TalentCrypto {
    pub(crate) fn new(
        active_version: String,
        keys: HashMap<String, String>,
        contact_pepper: String,
    ) -> Result<Self, CryptoError> {
        let keys = keys
            .into_iter()
            .map(|(version, key)| parse_aes_key(&key).map(|key| (version, key)))
            .collect::<Result<HashMap<_, _>, _>>()?;

        if !keys.contains_key(&active_version) {
            return Err(CryptoError::MissingActiveKey);
        }
        if contact_pepper.is_empty() {
            return Err(CryptoError::EmptyContactPepper);
        }

        if contact_pepper.len() < MIN_CONTACT_PEPPER_BYTES || contact_pepper_is_placeholder(&contact_pepper) {
            return Err(CryptoError::WeakContactPepper);
        }
        Ok(Self {
            active_version,
            keys,
            contact_pepper: contact_pepper.into_bytes(),
        })
    }

    pub(crate) fn encrypt(&self, plaintext: &[u8], aad: &[u8]) -> Result<EncryptedValue, CryptoError> {
        self.encrypt_with_version(plaintext, aad, &self.active_version)
    }

    pub(crate) fn encrypt_with_version(
        &self,
        plaintext: &[u8],
        aad: &[u8],
        key_version: &str,
    ) -> Result<EncryptedValue, CryptoError> {
        let key = self.keys.get(key_version).ok_or(CryptoError::UnknownKeyVersion)?;
        let mut nonce = [0_u8; NONCE_LEN];
        rand::rng().fill(&mut nonce);
        let mut tag = [0_u8; TAG_LEN];
        let ciphertext = encrypt_aead(Cipher::aes_256_gcm(), key, Some(&nonce), aad, plaintext, &mut tag)
            .map_err(|_| CryptoError::EncryptionFailed)?;
        let mut bytes = Vec::with_capacity(NONCE_LEN + ciphertext.len() + TAG_LEN);
        bytes.extend_from_slice(&nonce);
        bytes.extend_from_slice(&ciphertext);
        bytes.extend_from_slice(&tag);

        Ok(EncryptedValue {
            key_version: key_version.to_owned(),
            bytes,
        })
    }

    pub(crate) fn decrypt(&self, value: &EncryptedValue, aad: &[u8]) -> Result<Vec<u8>, CryptoError> {
        let key = self
            .keys
            .get(&value.key_version)
            .ok_or(CryptoError::UnknownKeyVersion)?;
        if value.bytes.len() < NONCE_LEN + TAG_LEN {
            return Err(CryptoError::InvalidEnvelope);
        }

        let nonce = &value.bytes[..NONCE_LEN];
        let (ciphertext, tag) = value.bytes[NONCE_LEN..].split_at(value.bytes.len() - NONCE_LEN - TAG_LEN);
        decrypt_aead(Cipher::aes_256_gcm(), key, Some(nonce), aad, ciphertext, tag)
            .map_err(|_| CryptoError::DecryptionFailed)
    }
}
fn contact_pepper_is_placeholder(value: &str) -> bool {
    let normalized = value.trim().to_ascii_lowercase();
    [
        "change-me",
        "changeme",
        "replace-me",
        "placeholder",
        "example-secret",
        "invalid-independent-secret",
    ]
    .iter()
    .any(|marker| normalized.contains(marker))
}

/// Normalizes a contact value before private deduplication.
pub(crate) fn normalize_contact(contact_type: ContactType, value: &str) -> String {
    match contact_type {
        ContactType::Phone => normalize_phone(value),
        ContactType::Wechat => normalize_wechat(value),
        ContactType::Qq => normalize_qq(value),
        ContactType::Other => value.replace('\u{3000}', " ").trim().to_owned(),
    }
}

/// Creates a stable, type-separated HMAC-SHA256 contact hash.
pub(crate) fn contact_hash(crypto: &TalentCrypto, contact: &ContactCandidate) -> Result<String, CryptoError> {
    let key = PKey::hmac(&crypto.contact_pepper).map_err(|_| CryptoError::ContactHashFailed)?;
    let mut signer =
        Signer::new(MessageDigest::sha256(), &key).map_err(|_| CryptoError::ContactHashFailed)?;
    signer
        .update(contact_type_discriminator(contact.contact_type).as_bytes())
        .map_err(|_| CryptoError::ContactHashFailed)?;
    signer.update(&[0]).map_err(|_| CryptoError::ContactHashFailed)?;
    signer
        .update(normalize_contact(contact.contact_type, &contact.value).as_bytes())
        .map_err(|_| CryptoError::ContactHashFailed)?;
    let digest = signer.sign_to_vec().map_err(|_| CryptoError::ContactHashFailed)?;

    Ok(digest.iter().map(|byte| format!("{byte:02x}")).collect())
}

/// Masks a contact value before public display or logging.
pub(crate) fn mask_contact(contact_type: ContactType, value: &str) -> String {
    let normalized = normalize_contact(contact_type, value);
    match contact_type {
        ContactType::Phone => mask_with_edges(&normalized, 3, 4, "****"),
        ContactType::Wechat | ContactType::Qq => mask_with_edges(&normalized, 3, 3, "***"),
        ContactType::Other => "***".to_owned(),
    }
}

fn parse_aes_key(value: &str) -> Result<[u8; 32], CryptoError> {
    if value.len() != 64 || !value.is_ascii() {
        return Err(CryptoError::InvalidKeyConfiguration);
    }

    let mut key = [0_u8; 32];
    for (index, byte) in key.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16)
            .map_err(|_| CryptoError::InvalidKeyConfiguration)?;
    }
    Ok(key)
}

fn normalize_phone(value: &str) -> String {
    let mut normalized = String::new();
    for character in value.trim().chars().map(normalize_full_width_digit_or_plus) {
        if character.is_ascii_digit() || (character == '+' && normalized.is_empty()) {
            normalized.push(character);
        }
    }
    normalized
}

fn normalize_wechat(value: &str) -> String {
    value
        .trim()
        .replace('\u{3000}', " ")
        .chars()
        .filter(|character| !character.is_whitespace())
        .map(|character| {
            if character.is_ascii_uppercase() {
                character.to_ascii_lowercase()
            } else {
                character
            }
        })
        .collect()
}

fn normalize_qq(value: &str) -> String {
    value
        .trim()
        .chars()
        .map(normalize_full_width_digit)
        .filter(|character| !character.is_whitespace() && *character != '-')
        .collect()
}

fn normalize_full_width_digit_or_plus(character: char) -> char {
    match character {
        '\u{ff0b}' => '+',
        _ => normalize_full_width_digit(character),
    }
}

fn normalize_full_width_digit(character: char) -> char {
    match character {
        '\u{ff10}'..='\u{ff19}' => char::from_u32(character as u32 - '\u{ff10}' as u32 + '0' as u32)
            .expect("full-width digit conversion is valid"),
        _ => character,
    }
}

fn contact_type_discriminator(contact_type: ContactType) -> &'static str {
    match contact_type {
        ContactType::Phone => "phone",
        ContactType::Wechat => "wechat",
        ContactType::Qq => "qq",
        ContactType::Other => "other",
    }
}

fn mask_with_edges(value: &str, prefix_length: usize, suffix_length: usize, mask: &str) -> String {
    let characters = value.chars().collect::<Vec<_>>();
    if characters.len() <= prefix_length + suffix_length {
        return "***".to_owned();
    }

    let prefix = characters[..prefix_length].iter().collect::<String>();
    let suffix = characters[characters.len() - suffix_length..]
        .iter()
        .collect::<String>();
    format!("{prefix}{mask}{suffix}")
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::{EncryptedValue, TalentCrypto, contact_hash, mask_contact, normalize_contact};
    use crate::talent::{ContactCandidate, ContactType};

    const CURRENT_KEY: &str = "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f";
    const PREVIOUS_KEY: &str = "1f1e1d1c1b1a191817161514131211100f0e0d0c0b0a09080706050403020100";
    const TEST_PEPPER: &str = "synthetic-contact-pepper-material-v1";

    fn crypto(active_version: &str, keys: &[(&str, &str)]) -> TalentCrypto {
        TalentCrypto::new(
            active_version.to_owned(),
            keys.iter()
                .map(|(version, key)| ((*version).to_owned(), (*key).to_owned()))
                .collect::<HashMap<_, _>>(),
            TEST_PEPPER.to_owned(),
        )
        .unwrap()
    }

    #[test]
    fn rejects_contact_peppers_shorter_than_thirty_two_bytes() {
        let error = TalentCrypto::new(
            "current".to_owned(),
            HashMap::from([("current".to_owned(), CURRENT_KEY.to_owned())]),
            "short-contact-pepper".to_owned(),
        )
        .unwrap_err();

        assert_eq!(error, super::CryptoError::WeakContactPepper);
    }

    #[test]
    fn rejects_known_placeholder_contact_peppers_even_when_long_enough() {
        let error = TalentCrypto::new(
            "current".to_owned(),
            HashMap::from([("current".to_owned(), CURRENT_KEY.to_owned())]),
            "replace-me-contact-pepper-material-123456".to_owned(),
        )
        .unwrap_err();

        assert_eq!(error, super::CryptoError::WeakContactPepper);
    }

    #[test]
    fn accepts_a_thirty_two_byte_contact_pepper() {
        let pepper = "0123456789abcdef0123456789abcdef";

        assert_eq!(pepper.len(), 32);
        assert!(
            TalentCrypto::new(
                "current".to_owned(),
                HashMap::from([("current".to_owned(), CURRENT_KEY.to_owned())]),
                pepper.to_owned(),
            )
            .is_ok()
        );
    }
    #[test]
    fn encrypts_and_decrypts_with_aad() {
        let crypto = crypto("current", &[("current", CURRENT_KEY)]);
        let encrypted = crypto.encrypt(b"fictional source", b"batch:synthetic").unwrap();

        assert_eq!(
            crypto.decrypt(&encrypted, b"batch:synthetic").unwrap(),
            b"fictional source"
        );
    }

    #[test]
    fn encrypting_the_same_value_twice_uses_distinct_ciphertexts() {
        let crypto = crypto("current", &[("current", CURRENT_KEY)]);

        assert_ne!(
            crypto.encrypt(b"same", b"batch").unwrap().bytes,
            crypto.encrypt(b"same", b"batch").unwrap().bytes
        );
    }

    #[test]
    fn rejects_tampered_ciphertext() {
        let crypto = crypto("current", &[("current", CURRENT_KEY)]);
        let mut encrypted = crypto.encrypt(b"fictional source", b"item:synthetic").unwrap();
        encrypted.bytes[12] ^= 0x01;

        assert!(crypto.decrypt(&encrypted, b"item:synthetic").is_err());
    }

    #[test]
    fn rejects_aad_for_a_different_source_record() {
        let crypto = crypto("current", &[("current", CURRENT_KEY)]);
        let encrypted = crypto.encrypt(b"fictional source", b"item:first").unwrap();

        assert!(crypto.decrypt(&encrypted, b"item:second").is_err());
    }

    #[test]
    fn decrypts_values_with_an_older_configured_key() {
        let old_crypto = crypto("old", &[("old", PREVIOUS_KEY)]);
        let encrypted = old_crypto.encrypt(b"fictional source", b"contact:synthetic").unwrap();
        let crypto = crypto("current", &[("current", CURRENT_KEY), ("old", PREVIOUS_KEY)]);

        assert_eq!(
            crypto.decrypt(&encrypted, b"contact:synthetic").unwrap(),
            b"fictional source"
        );
    }

    #[test]
    fn encrypts_new_values_with_a_requested_older_configured_key() {
        let crypto = crypto("current", &[("current", CURRENT_KEY), ("old", PREVIOUS_KEY)]);

        let encrypted = crypto
            .encrypt_with_version(b"fictional source", b"contact:synthetic", "old")
            .unwrap();

        assert_eq!(encrypted.key_version, "old");
        assert_eq!(
            crypto.decrypt(&encrypted, b"contact:synthetic").unwrap(),
            b"fictional source"
        );
    }

    #[test]
    fn rejects_values_for_an_unknown_key_version() {
        let crypto = crypto("current", &[("current", CURRENT_KEY)]);
        let value = EncryptedValue {
            key_version: "retired".to_owned(),
            bytes: vec![0; 28],
        };

        assert!(crypto.decrypt(&value, b"batch:synthetic").is_err());
    }

    #[test]
    fn normalizes_contacts_with_the_conservative_v1_rules() {
        assert_eq!(
            normalize_contact(ContactType::Phone, "  ＋86 (138) 0000-0001  "),
            "+8613800000001"
        );
        assert_eq!(
            normalize_contact(ContactType::Wechat, " Test　 Candidate_01 "),
            "testcandidate_01"
        );
        assert_eq!(normalize_contact(ContactType::Qq, " ００1-23  "), "00123");
        assert_eq!(
            normalize_contact(ContactType::Other, "　Mixed Case Value　"),
            "Mixed Case Value"
        );
    }

    #[test]
    fn hashes_normalized_contacts_stably_without_cross_type_collisions() {
        let crypto = crypto("current", &[("current", CURRENT_KEY)]);
        let formatted_phone = ContactCandidate {
            contact_type: ContactType::Phone,
            value: "138-0000 0001".to_owned(),
        };
        let plain_phone = ContactCandidate {
            contact_type: ContactType::Phone,
            value: "13800000001".to_owned(),
        };
        let wechat = ContactCandidate {
            contact_type: ContactType::Wechat,
            value: "13800000001".to_owned(),
        };

        let formatted_hash = contact_hash(&crypto, &formatted_phone).unwrap();
        assert_eq!(formatted_hash, contact_hash(&crypto, &plain_phone).unwrap());
        assert_ne!(formatted_hash, contact_hash(&crypto, &wechat).unwrap());
        assert!(formatted_hash.chars().all(|character| character.is_ascii_hexdigit()));
        assert_eq!(formatted_hash, formatted_hash.to_ascii_lowercase());
    }

    #[test]
    fn masks_contacts_for_safe_display() {
        assert_eq!(mask_contact(ContactType::Phone, "13800000001"), "138****0001");
        assert_eq!(
            mask_contact(ContactType::Wechat, "test_candidate_01"),
            "tes***_01"
        );
    }
}

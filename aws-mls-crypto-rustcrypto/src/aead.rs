extern crate aead as rc_aead;

use std::fmt::Debug;

use aes_gcm::{Aes128Gcm, Aes256Gcm};
use aws_mls_core::crypto::CipherSuite;
use aws_mls_crypto_traits::AeadType;
use chacha20poly1305::ChaCha20Poly1305;
use rc_aead::{generic_array::GenericArray, NewAead, Payload};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AeadError {
    #[error(transparent)]
    RcAeadError(#[from] rc_aead::Error),
    #[error("AEAD ciphertext of length {0} is too short to fit the tag")]
    InvalidCipherLen(usize),
    #[error("encrypted message cannot be empty")]
    EmptyPlaintext,
    #[error("AEAD key of invalid length {0}. Expected length {1}")]
    InvalidKeyLen(usize, usize),
}

pub const TAG_LEN: usize = 16;
pub const NONCE_LEN: usize = 12;

/// Aead ID as specified in RFC 9180, Table 5.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u16)]
pub enum Aead {
    /// AES-128-GCM: 16 byte key, 12 byte nonce, 16 byte tag
    Aes128Gcm = 0x0001,
    /// AES-256-GCM: 32 byte key, 12 byte nonce, 16 byte tag
    Aes256Gcm = 0x0002,
    /// ChaCha20-Poly1305: 32 byte key, 12 byte nonce, 16 byte tag
    Chacha20Poly1305 = 0x0003,
}

impl Aead {
    pub fn new(cipher_suite: CipherSuite) -> Self {
        match cipher_suite {
            CipherSuite::P256Aes128 | CipherSuite::Curve25519Aes128 => Aead::Aes128Gcm,
            CipherSuite::Curve448Aes256 | CipherSuite::P384Aes256 | CipherSuite::P521Aes256 => {
                Aead::Aes256Gcm
            }
            CipherSuite::Curve25519ChaCha20 | CipherSuite::Curve448ChaCha20 => {
                Aead::Chacha20Poly1305
            }
        }
    }
}

impl AeadType for Aead {
    type Error = AeadError;

    fn seal(
        &self,
        key: &[u8],
        data: &[u8],
        aad: Option<&[u8]>,
        nonce: &[u8],
    ) -> Result<Vec<u8>, AeadError> {
        (!data.is_empty())
            .then_some(())
            .ok_or(AeadError::EmptyPlaintext)?;

        (key.len() == self.key_size())
            .then_some(())
            .ok_or_else(|| AeadError::InvalidKeyLen(key.len(), self.key_size()))?;

        match self {
            Aead::Aes128Gcm => {
                let cipher = Aes128Gcm::new(GenericArray::from_slice(key));
                encrypt_aead_trait(cipher, data, aad, nonce)
            }
            Aead::Aes256Gcm => {
                let cipher = Aes256Gcm::new(GenericArray::from_slice(key));
                encrypt_aead_trait(cipher, data, aad, nonce)
            }
            Aead::Chacha20Poly1305 => {
                let cipher = ChaCha20Poly1305::new(GenericArray::from_slice(key));
                encrypt_aead_trait(cipher, data, aad, nonce)
            }
        }
    }

    fn open(
        &self,
        key: &[u8],
        ciphertext: &[u8],
        aad: Option<&[u8]>,
        nonce: &[u8],
    ) -> Result<Vec<u8>, AeadError> {
        (ciphertext.len() > TAG_LEN)
            .then_some(())
            .ok_or(AeadError::InvalidCipherLen(ciphertext.len()))?;

        (key.len() == self.key_size())
            .then_some(())
            .ok_or_else(|| AeadError::InvalidKeyLen(key.len(), self.key_size()))?;

        match self {
            Aead::Aes128Gcm => {
                let cipher = Aes128Gcm::new(GenericArray::from_slice(key));
                decrypt_aead_trait(cipher, ciphertext, aad, nonce)
            }
            Aead::Aes256Gcm => {
                let cipher = Aes256Gcm::new(GenericArray::from_slice(key));
                decrypt_aead_trait(cipher, ciphertext, aad, nonce)
            }
            Aead::Chacha20Poly1305 => {
                let cipher = ChaCha20Poly1305::new(GenericArray::from_slice(key));
                decrypt_aead_trait(cipher, ciphertext, aad, nonce)
            }
        }
    }

    #[inline(always)]
    fn key_size(&self) -> usize {
        match self {
            Aead::Aes128Gcm => 16,
            Aead::Aes256Gcm => 32,
            Aead::Chacha20Poly1305 => 32,
        }
    }

    fn nonce_size(&self) -> usize {
        NONCE_LEN
    }

    fn aead_id(&self) -> u16 {
        *self as u16
    }
}

fn encrypt_aead_trait(
    cipher: impl rc_aead::Aead,
    data: &[u8],
    aad: Option<&[u8]>,
    nonce: &[u8],
) -> Result<Vec<u8>, AeadError> {
    let payload = Payload {
        msg: data,
        aad: aad.unwrap_or_default(),
    };

    Ok(cipher.encrypt(GenericArray::from_slice(nonce), payload)?)
}

fn decrypt_aead_trait(
    cipher: impl rc_aead::Aead,
    ciphertext: &[u8],
    aad: Option<&[u8]>,
    nonce: &[u8],
) -> Result<Vec<u8>, AeadError> {
    let payload = Payload {
        msg: ciphertext,
        aad: aad.unwrap_or_default(),
    };

    Ok(cipher.decrypt(GenericArray::from_slice(nonce), payload)?)
}

#[cfg(test)]
mod test {
    use aws_mls_core::crypto::CipherSuite;
    use aws_mls_crypto_traits::AeadType;

    use crate::aead::TAG_LEN;

    use super::{Aead, AeadError};

    use assert_matches::assert_matches;

    fn get_aeads() -> Vec<Aead> {
        [
            CipherSuite::Curve25519Aes128,
            CipherSuite::Curve25519ChaCha20,
            CipherSuite::Curve448Aes256,
        ]
        .into_iter()
        .map(Aead::new)
        .collect()
    }

    #[derive(serde::Deserialize)]
    struct TestCase {
        pub ciphersuite: CipherSuite,
        #[serde(with = "hex::serde")]
        pub key: Vec<u8>,
        #[serde(with = "hex::serde")]
        pub iv: Vec<u8>,
        #[serde(with = "hex::serde")]
        pub ct: Vec<u8>,
        #[serde(with = "hex::serde")]
        pub aad: Vec<u8>,
        #[serde(with = "hex::serde")]
        pub pt: Vec<u8>,
    }

    #[test]
    fn test_vectors() {
        let test_case_file = include_str!("../test_data/test_aead.json");
        let test_cases: Vec<TestCase> = serde_json::from_str(test_case_file).unwrap();

        for case in test_cases {
            let aead = Aead::new(case.ciphersuite);

            let ciphertext = aead
                .seal(&case.key, &case.pt, Some(&case.aad), &case.iv)
                .unwrap();

            assert_eq!(ciphertext, case.ct);

            let plaintext = aead
                .open(&case.key, &ciphertext, Some(&case.aad), &case.iv)
                .unwrap();

            assert_eq!(plaintext, case.pt);
        }
    }

    #[test]
    fn invalid_key() {
        for aead in get_aeads() {
            let nonce = vec![42u8; aead.nonce_size()];
            let data = b"top secret";

            let too_short = vec![42u8; aead.key_size() - 1];

            assert_matches!(
                aead.seal(&too_short, data, None, &nonce),
                Err(AeadError::InvalidKeyLen(_, _))
            );

            let too_long = vec![42u8; aead.key_size() + 1];

            assert_matches!(
                aead.seal(&too_long, data, None, &nonce),
                Err(AeadError::InvalidKeyLen(_, _))
            );
        }
    }

    #[test]
    fn invalid_ciphertext() {
        for aead in get_aeads() {
            let key = vec![42u8; aead.key_size()];
            let nonce = vec![42u8; aead.nonce_size()];

            let too_short = [0u8; TAG_LEN];

            assert_matches!(
                aead.open(&key, &too_short, None, &nonce),
                Err(AeadError::InvalidCipherLen(_))
            );
        }
    }

    #[test]
    fn aad_mismatch() {
        for aead in get_aeads() {
            let key = vec![42u8; aead.key_size()];
            let nonce = vec![42u8; aead.nonce_size()];

            let ciphertext = aead.seal(&key, b"message", Some(b"foo"), &nonce).unwrap();

            assert_matches!(
                aead.open(&key, &ciphertext, Some(b"bar"), &nonce),
                Err(AeadError::RcAeadError(_))
            );

            assert_matches!(
                aead.open(&key, &ciphertext, None, &nonce),
                Err(AeadError::RcAeadError(_))
            );
        }
    }
}
// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The encrypted private-key forms `dds.sec.auth.password` opens.
//!
//! DDS-Security 1.2 §10.3.1 Table 50, `password`: "A password used to
//! decrypt the private_key. The value of the password property shall be
//! interpreted as the Base64 encoding of the AES-128 key that shall be
//! used to decrypt the private_key using AES128-CBC."
//!
//! Cyclone DDS (`load_private_key_from_file` and `_from_data` in
//! `auth_utils.c`) and Fast DDS (`PKIDH.cpp`, the `password` property
//! handed to its file provider) give the same property to OpenSSL's
//! `PEM_read_bio_PrivateKey` as a **passphrase**, which is a different
//! reading: OpenSSL derives the key from it. A key file a vendor's
//! tooling wrote opens with the passphrase and not with the clause's
//! key, and the clause's key opens a file no vendor writes. Both are
//! read here, and which one a file takes is decided by the file:
//!
//! - `-----BEGIN ENCRYPTED PRIVATE KEY-----` — PKCS#8
//!   `EncryptedPrivateKeyInfo` (RFC 5958 §3, PBES2 of RFC 8018). Its
//!   key derivation is in the file, so the password is a passphrase and
//!   has no other reading.
//! - The OpenSSL form, `Proc-Type: 4,ENCRYPTED` and `DEK-Info:
//!   <cipher>,<IV>` above a PKCS#1 or SEC1 body. When the cipher is
//!   AES-128-CBC and the password is the base64 of sixteen bytes, those
//!   bytes are tried as the key first — the clause's reading. Otherwise,
//!   and when that key does not open the file, the password is the
//!   passphrase OpenSSL's `EVP_BytesToKey` derives the key from (MD5,
//!   salt = the IV's first eight octets, one round), which is what the
//!   two vendors' files hold.
//!
//! A wrong key is caught twice: the CBC padding has to check out and the
//! plaintext has to parse as the key the PEM label names. Whatever
//! opens, the result is the PKCS#8 DER the signing backends take.
//!
//! The primitives are the pure-Rust `aes`, `cbc`, `md-5` and `pkcs5`
//! crates in every backend, the `aws-lc` one included: opening the file
//! happens outside the validated module. A deployment that has to stay
//! inside it gives an unencrypted key.

use crate::identity::{PkiError, sec1_or_pkcs1_as_pkcs8};
use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// How the `dds.sec.auth.password` value is read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PasswordReading {
    /// DDS-Security 1.2 §10.3.1 Table 50: the Base64 encoding of the
    /// AES-128 key that decrypts the private key with AES128-CBC. A key file
    /// that is not an OpenSSL traditional AES-128-CBC one has no such key to
    /// open, and a PKCS#8 `ENCRYPTED PRIVATE KEY` neither.
    #[default]
    Clause,
    /// What Cyclone DDS and Fast DDS do besides: the value is also an
    /// OpenSSL passphrase the key is derived from (traditional PEM with any
    /// cipher, PKCS#8 PBES2). Tried after the clause's reading.
    ClauseThenPassphrase,
}

/// The private key in `pem`, decrypted with `password` under both readings
/// ([`PasswordReading::ClauseThenPassphrase`]), as PKCS#8 DER.
///
/// # Errors
/// As [`decrypt_private_key_pem_as`].
pub fn decrypt_private_key_pem(pem: &[u8], password: &str) -> Result<Vec<u8>, PkiError> {
    decrypt_private_key_pem_as(pem, password, PasswordReading::ClauseThenPassphrase)
}

/// The private key in `pem`, decrypted with `password` read as `reading`,
/// as PKCS#8 DER.
///
/// An unencrypted key is returned as it is: Cyclone's and Fast DDS's
/// OpenSSL call ignores the passphrase for one, and §10.3.1 Table 50
/// makes the password optional rather than a claim about the file.
///
/// # Errors
/// [`PkiError::InvalidPem`] naming what did not open: a malformed PEM, a
/// cipher this crate does not read, or a password that opens nothing.
pub fn decrypt_private_key_pem_as(
    pem: &[u8],
    password: &str,
    reading: PasswordReading,
) -> Result<Vec<u8>, PkiError> {
    let text =
        core::str::from_utf8(pem).map_err(|_| invalid("the private key file is not text"))?;
    let block = first_key_block(text)?;
    if block.label == "ENCRYPTED PRIVATE KEY" {
        if reading == PasswordReading::Clause {
            return Err(invalid(
                "an encrypted PKCS#8 key has no AES-128 key in the clause's reading of the password \
                 (§10.3.1 Table 50); set org.zerodds.sec.auth.password_reading to openssl_passphrase",
            ));
        }
        return decrypt_pkcs8(&block.body, password);
    }
    let Some(dek_info) = block.header("DEK-Info") else {
        // Not encrypted: the passphrase has nothing to open.
        return crate::identity::private_key_pkcs8_der(pem);
    };
    if block.header("Proc-Type").map(str::trim) != Some("4,ENCRYPTED") {
        return Err(invalid(
            "a DEK-Info header without `Proc-Type: 4,ENCRYPTED`",
        ));
    }
    let (cipher, iv_hex) = dek_info
        .split_once(',')
        .ok_or_else(|| invalid("a DEK-Info header with no IV"))?;
    let cipher = Cipher::named(cipher.trim())?;
    let iv = hex_bytes(iv_hex.trim()).ok_or_else(|| invalid("a DEK-Info IV that is not hex"))?;
    let iv: [u8; 16] = iv
        .try_into()
        .map_err(|_| invalid("a DEK-Info IV that is not sixteen octets"))?;

    let mut tried = Vec::new();
    if cipher == Cipher::Aes128 {
        if let Some(key) = clause_key(password) {
            match open_traditional(cipher, &key, &iv, &block) {
                Ok(der) => return Ok(der),
                Err(_) => tried.push("the password as the base64 AES-128 key (§10.3.1 Table 50)"),
            }
        }
    }
    if reading == PasswordReading::Clause {
        return Err(PkiError::InvalidPem(alloc::format!(
            "the private key does not open with {} (set org.zerodds.sec.auth.password_reading to \
             openssl_passphrase for a key written with an OpenSSL passphrase)",
            if tried.is_empty() {
                "a password that is not the base64 of an AES-128 key, or a cipher other than AES-128-CBC"
            } else {
                "the password as the base64 AES-128 key (§10.3.1 Table 50)"
            }
        )));
    }
    let key = evp_bytes_to_key(password.as_bytes(), &iv, cipher.key_len());
    match open_traditional(cipher, &key, &iv, &block) {
        Ok(der) => Ok(der),
        Err(_) => {
            tried.push("the password as an OpenSSL passphrase");
            Err(PkiError::InvalidPem(alloc::format!(
                "the private key does not open with {}",
                tried.join(" nor with ")
            )))
        }
    }
}

/// One PEM block: its label, its RFC 1421 headers and its body.
struct Block {
    label: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl Block {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }
}

/// The first block whose label names a private key.
fn first_key_block(text: &str) -> Result<Block, PkiError> {
    let mut lines = text.lines().map(str::trim_end);
    loop {
        let line = lines.next().ok_or(PkiError::NoPrivateKeyInPem)?;
        let Some(label) = line
            .strip_prefix("-----BEGIN ")
            .and_then(|rest| rest.strip_suffix("-----"))
        else {
            continue;
        };
        if !label.ends_with("PRIVATE KEY") {
            continue;
        }
        let end = alloc::format!("-----END {label}-----");
        let mut headers = Vec::new();
        let mut body = String::new();
        let mut in_headers = true;
        for line in lines.by_ref() {
            if line == end {
                use base64::Engine as _;
                let body = base64::engine::general_purpose::STANDARD
                    .decode(body.as_bytes())
                    .map_err(|_| invalid("a PEM body that is not base64"))?;
                return Ok(Block {
                    label: label.to_string(),
                    headers,
                    body,
                });
            }
            if in_headers {
                if line.is_empty() {
                    in_headers = false;
                    continue;
                }
                if let Some((key, value)) = line.split_once(':') {
                    headers.push((key.trim().to_string(), value.trim().to_string()));
                    continue;
                }
                in_headers = false;
            }
            body.push_str(line.trim());
        }
        return Err(invalid("a PEM block with no END line"));
    }
}

/// The ciphers the OpenSSL form names that this crate opens.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Cipher {
    Aes128,
    Aes192,
    Aes256,
}

impl Cipher {
    fn named(name: &str) -> Result<Self, PkiError> {
        match name.to_ascii_uppercase().as_str() {
            "AES-128-CBC" => Ok(Self::Aes128),
            "AES-192-CBC" => Ok(Self::Aes192),
            "AES-256-CBC" => Ok(Self::Aes256),
            other => Err(PkiError::InvalidPem(alloc::format!(
                "the private key is encrypted with {other}; this crate opens AES-128-CBC, \
                 AES-192-CBC and AES-256-CBC"
            ))),
        }
    }

    fn key_len(self) -> usize {
        match self {
            Self::Aes128 => 16,
            Self::Aes192 => 24,
            Self::Aes256 => 32,
        }
    }
}

/// §10.3.1 Table 50's reading: the password is the base64 of the
/// sixteen-octet AES-128 key.
fn clause_key(password: &str) -> Option<Vec<u8>> {
    use base64::Engine as _;
    let key = base64::engine::general_purpose::STANDARD
        .decode(password.trim())
        .ok()?;
    (key.len() == 16).then_some(key)
}

/// OpenSSL's `EVP_BytesToKey` with MD5 and one round, which is what a
/// traditional encrypted PEM was written with: `D_i = MD5(D_{i-1} ||
/// passphrase || salt)`, concatenated until there are `len` octets, the
/// salt being the IV's first eight.
fn evp_bytes_to_key(passphrase: &[u8], iv: &[u8; 16], len: usize) -> Vec<u8> {
    use md5::Digest as _;
    let salt = &iv[..8];
    let mut key = Vec::with_capacity(len + 16);
    let mut previous: Vec<u8> = Vec::new();
    while key.len() < len {
        let mut hash = md5::Md5::new();
        hash.update(&previous);
        hash.update(passphrase);
        hash.update(salt);
        previous = hash.finalize().to_vec();
        key.extend_from_slice(&previous);
    }
    key.truncate(len);
    key
}

/// Decrypts the OpenSSL form with `key` and wraps the result as PKCS#8.
fn open_traditional(
    cipher: Cipher,
    key: &[u8],
    iv: &[u8; 16],
    block: &Block,
) -> Result<Vec<u8>, PkiError> {
    use cbc::cipher::{BlockDecryptMut as _, KeyIvInit as _, block_padding::Pkcs7};
    let wrong = |_| invalid("the key does not decrypt the body");
    let plain = match cipher {
        Cipher::Aes128 => cbc::Decryptor::<aes::Aes128>::new_from_slices(key, iv)
            .map_err(|_| invalid("an AES-128 key that is not sixteen octets"))?
            .decrypt_padded_vec_mut::<Pkcs7>(&block.body)
            .map_err(wrong)?,
        Cipher::Aes192 => cbc::Decryptor::<aes::Aes192>::new_from_slices(key, iv)
            .map_err(|_| invalid("an AES-192 key that is not twenty-four octets"))?
            .decrypt_padded_vec_mut::<Pkcs7>(&block.body)
            .map_err(wrong)?,
        Cipher::Aes256 => cbc::Decryptor::<aes::Aes256>::new_from_slices(key, iv)
            .map_err(|_| invalid("an AES-256 key that is not thirty-two octets"))?
            .decrypt_padded_vec_mut::<Pkcs7>(&block.body)
            .map_err(wrong)?,
    };
    sec1_or_pkcs1_as_pkcs8(&block.label, &plain)
}

/// PKCS#8 `EncryptedPrivateKeyInfo`, decrypted with `password` as the
/// PBES2 passphrase the file's own parameters derive the key from.
fn decrypt_pkcs8(der: &[u8], password: &str) -> Result<Vec<u8>, PkiError> {
    use pkcs8::der::Decode as _;
    let info = pkcs8::EncryptedPrivateKeyInfo::from_der(der)
        .map_err(|e| PkiError::InvalidPem(alloc::format!("encrypted PKCS#8: {e}")))?;
    let document = info.decrypt(password.as_bytes()).map_err(|_| {
        invalid("the private key does not open with the password as a PKCS#8 passphrase")
    })?;
    Ok(document.as_bytes().to_vec())
}

fn hex_bytes(text: &str) -> Option<Vec<u8>> {
    if text.len() % 2 != 0 {
        return None;
    }
    (0..text.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(text.get(at..at + 2)?, 16).ok())
        .collect()
}

fn invalid(why: &str) -> PkiError {
    PkiError::InvalidPem(why.to_string())
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    /// Keys generated with OpenSSL 3.5.6 for these tests and for nothing
    /// else. `ec.pem` and `rsa.pem` are the plaintext keys and
    /// `*_pkcs8_plain.pem` OpenSSL's unencrypted PKCS#8 of them; the rest
    /// are those keys encrypted under `OpenSesame`, except
    /// `ec_trad_spec_key.pem`, whose AES-128 key is 000102…0f itself —
    /// `openssl enc -aes-128-cbc -K … -iv …` over the SEC1 DER, with the
    /// headers written around it.
    macro_rules! fixture {
        ($name:literal) => {
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/fixtures/encrypted_keys/",
                $name
            ))
        };
    }

    fn plain(pem: &[u8]) -> Vec<u8> {
        crate::identity::private_key_pkcs8_der(pem).expect("the plaintext key")
    }

    /// §10.3.1 Table 50: the password is the Base64 of the AES-128 key.
    /// By default only that reading opens a key; a passphrase of the
    /// OpenSSL form and an encrypted PKCS#8 key open only when the vendors'
    /// reading is asked for.
    #[test]
    fn the_clause_reading_is_the_default_and_the_passphrase_is_named() {
        let spec_key = fixture!("ec_trad_spec_key.pem");
        let password = "AAECAwQFBgcICQoLDA0ODw==";
        assert_eq!(
            decrypt_private_key_pem_as(spec_key, password, PasswordReading::default()).unwrap(),
            plain(fixture!("ec.pem"))
        );
        assert_eq!(PasswordReading::default(), PasswordReading::Clause);
        for encrypted in [
            &fixture!("ec_trad_aes128.pem")[..],
            &fixture!("rsa_trad_aes256.pem")[..],
            &fixture!("ec_pkcs8_pbes2.pem")[..],
        ] {
            assert!(
                decrypt_private_key_pem_as(encrypted, "OpenSesame", PasswordReading::Clause)
                    .is_err()
            );
            assert!(
                decrypt_private_key_pem_as(
                    encrypted,
                    "OpenSesame",
                    PasswordReading::ClauseThenPassphrase
                )
                .is_ok()
            );
        }
    }

    #[test]
    fn the_openssl_form_opens_with_the_passphrase_the_vendors_use() {
        for (encrypted, original) in [
            (&fixture!("ec_trad_aes128.pem")[..], &fixture!("ec.pem")[..]),
            (
                &fixture!("rsa_trad_aes128.pem")[..],
                &fixture!("rsa.pem")[..],
            ),
            (
                &fixture!("rsa_trad_aes256.pem")[..],
                &fixture!("rsa.pem")[..],
            ),
        ] {
            assert_eq!(
                decrypt_private_key_pem(encrypted, "OpenSesame").expect("opens"),
                plain(original)
            );
        }
    }

    /// Compared with OpenSSL's own unencrypted PKCS#8 of the same key
    /// (`openssl pkcs8 -topk8 -nocrypt`): OpenSSL leaves the curve out of
    /// the inner `ECPrivateKey` where the algorithm identifier names it,
    /// and this crate's SEC1 wrapping keeps it, so the two PKCS#8 forms
    /// of one EC key differ by those twelve octets.
    #[test]
    fn encrypted_pkcs8_opens_with_the_passphrase() {
        for (encrypted, original) in [
            (
                &fixture!("ec_pkcs8_pbes2.pem")[..],
                &fixture!("ec_pkcs8_plain.pem")[..],
            ),
            (
                &fixture!("rsa_pkcs8_pbes2.pem")[..],
                &fixture!("rsa_pkcs8_plain.pem")[..],
            ),
        ] {
            assert_eq!(
                decrypt_private_key_pem(encrypted, "OpenSesame").expect("opens"),
                plain(original)
            );
        }
    }

    /// §10.3.1 Table 50 read as written: the password is the base64 of
    /// the AES-128 key itself.
    #[test]
    fn the_clause_key_opens_a_file_encrypted_with_it() {
        assert_eq!(
            decrypt_private_key_pem(fixture!("ec_trad_spec_key.pem"), "AAECAwQFBgcICQoLDA0ODw==")
                .expect("opens"),
            plain(fixture!("ec.pem"))
        );
    }

    #[test]
    fn a_wrong_password_is_refused_with_both_readings_named() {
        let err =
            decrypt_private_key_pem(fixture!("ec_trad_aes128.pem"), "AAECAwQFBgcICQoLDA0ODw==")
                .expect_err("wrong");
        let text = alloc::format!("{err}");
        assert!(text.contains("base64 AES-128 key"), "{text}");
        assert!(text.contains("OpenSSL passphrase"), "{text}");
        assert!(
            decrypt_private_key_pem(fixture!("ec_pkcs8_pbes2.pem"), "wrong").is_err(),
            "PKCS#8 with the wrong passphrase"
        );
    }

    #[test]
    fn an_unencrypted_key_ignores_the_password() {
        assert_eq!(
            decrypt_private_key_pem(fixture!("ec.pem"), "anything").expect("opens"),
            plain(fixture!("ec.pem"))
        );
    }

    #[test]
    fn evp_bytes_to_key_matches_openssl() {
        // `openssl enc -aes-128-cbc -pass pass:OpenSesame -S 0011223344556677
        // -P -md md5`, OpenSSL 3.5.6: key=428117212998449E86F7775CA5AE2219.
        let iv = [
            0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0, 0, 0, 0, 0, 0, 0, 0,
        ];
        let key = evp_bytes_to_key(b"OpenSesame", &iv, 16);
        assert_eq!(
            key,
            hex_bytes(EVP_KEY_OPENSESAME_SALT_0011223344556677).unwrap()
        );
    }

    const EVP_KEY_OPENSESAME_SALT_0011223344556677: &str = "428117212998449E86F7775CA5AE2219";
}

// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! GSSUP credential source — a salted-hash password file, Spec §24.7 /
//! §10.6.2's "the mechanism by which a target validates the client's
//! username/password is implementation defined".
//!
//! No plain-text password ever sits in a daemon's configuration: a
//! GSSUP password file holds `username:salt_hex:iterations:hash_hex`,
//! one entry per line, `hash = PBKDF2-HMAC-SHA-256(password, salt,
//! iterations)` — the same primitive `crates/amqp-endpoint`'s SASL
//! SCRAM-SHA-256 uses for the same reason ([`crate::sas`]'s own module
//! doc names it as this crate's dependency-free `hmac`+`sha2` pair).
//!
//! This module does no file I/O itself — a daemon reads the file's text
//! and calls [`GssupUserStore::parse`], keeping this crate `no_std +
//! alloc`. Generating a line for a new user is [`format_entry`]; there
//! is no interactive CLI for it (an operator's own script, or a REPL
//! one-liner against [`hash_password`], produces the line — the format
//! is exactly the four colon-separated fields below and nothing else).
//!
//! ```text
//! alice:9f86d081884c7d659a2feaa0c55ad015:100000:7e2f1a...  (64 hex chars)
//! ```

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

/// PBKDF2 iteration count floor. OWASP's 2023 guidance for
/// PBKDF2-HMAC-SHA256 is >= 600 000; this module does not refuse a
/// lower stored count (an operator may have migrated from a weaker
/// setting), but [`hash_password`]'s caller should not go below it for
/// a newly created entry.
pub const RECOMMENDED_MIN_ITERATIONS: u32 = 600_000;

/// One user's stored credential.
#[derive(Debug, Clone, PartialEq, Eq)]
struct StoredCredential {
    salt: Vec<u8>,
    iterations: u32,
    hash: [u8; 32],
}

/// A parsed GSSUP password file.
#[derive(Debug, Clone, Default)]
pub struct GssupUserStore {
    users: BTreeMap<String, StoredCredential>,
}

/// Error while parsing a GSSUP password file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GssupStoreError {
    /// 1-based line number.
    pub line: usize,
    /// What was wrong with it.
    pub reason: String,
}

impl core::fmt::Display for GssupStoreError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "line {}: {}", self.line, self.reason)
    }
}

/// `HMAC-SHA-256(key, msg)`.
// `new_from_slice` is infallible for HMAC (any key length is valid) —
// the same targeted allow `crates/amqp-endpoint/src/scram.rs` uses for
// the identical construction.
#[allow(clippy::expect_used)]
fn hmac_sha256(key: &[u8], msg: &[u8]) -> [u8; 32] {
    let mut mac = <HmacSha256 as Mac>::new_from_slice(key).expect("HMAC any key length");
    mac.update(msg);
    let out = mac.finalize().into_bytes();
    let mut r = [0u8; 32];
    r.copy_from_slice(&out);
    r
}

/// PBKDF2-HMAC-SHA-256 with `dkLen = 32` (RFC 8018 §5.2, one output
/// block because the derived length equals the hash length):
/// `T = U_1 xor U_2 xor ... xor U_iterations`.
#[must_use]
pub fn hash_password(password: &[u8], salt: &[u8], iterations: u32) -> [u8; 32] {
    let mut msg = Vec::with_capacity(salt.len() + 4);
    msg.extend_from_slice(salt);
    msg.extend_from_slice(&1u32.to_be_bytes());
    let mut u = hmac_sha256(password, &msg);
    let mut t = u;
    for _ in 1..iterations.max(1) {
        u = hmac_sha256(password, &u);
        for k in 0..32 {
            t[k] ^= u[k];
        }
    }
    t
}

fn to_hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&alloc::format!("{b:02x}"));
    }
    s
}

fn from_hex(s: &str) -> Option<Vec<u8>> {
    if s.len() % 2 != 0 {
        return None;
    }
    let mut out = Vec::with_capacity(s.len() / 2);
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let hi = (bytes[i] as char).to_digit(16)?;
        let lo = (bytes[i + 1] as char).to_digit(16)?;
        out.push(((hi << 4) | lo) as u8);
        i += 2;
    }
    Some(out)
}

/// Formats one password-file line for `username`/`password`, hashed with
/// a freshly supplied `salt` and `iterations` — the caller's own source
/// of randomness picks `salt` (this crate takes no dependency on an RNG
/// for the sake of one line of tooling text).
#[must_use]
pub fn format_entry(username: &str, password: &str, salt: &[u8], iterations: u32) -> String {
    let hash = hash_password(password.as_bytes(), salt, iterations);
    alloc::format!("{username}:{}:{iterations}:{}", to_hex(salt), to_hex(&hash))
}

impl GssupUserStore {
    /// Parses a GSSUP password file's text — one `username:salt_hex:
    /// iterations:hash_hex` entry per line; blank lines and lines
    /// starting with `#` are skipped.
    ///
    /// # Errors
    /// [`GssupStoreError`] naming the first malformed line (a wrong
    /// field count, a non-numeric iteration count, or a hex field that
    /// does not decode) — refused loudly rather than silently dropping
    /// one operator's account.
    pub fn parse(text: &str) -> Result<Self, GssupStoreError> {
        let mut users = BTreeMap::new();
        for (i, raw) in text.lines().enumerate() {
            let line_no = i + 1;
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let fields: Vec<&str> = line.split(':').collect();
            let [username, salt_hex, iterations_s, hash_hex] = fields.as_slice() else {
                return Err(GssupStoreError {
                    line: line_no,
                    reason: alloc::format!(
                        "expected username:salt_hex:iterations:hash_hex, got {} field(s)",
                        fields.len()
                    ),
                });
            };
            if username.is_empty() {
                return Err(GssupStoreError {
                    line: line_no,
                    reason: "empty username".to_string(),
                });
            }
            let salt = from_hex(salt_hex).ok_or_else(|| GssupStoreError {
                line: line_no,
                reason: "salt is not valid hex".to_string(),
            })?;
            let iterations: u32 = iterations_s.parse().map_err(|_| GssupStoreError {
                line: line_no,
                reason: "iterations is not a number".to_string(),
            })?;
            if iterations == 0 {
                return Err(GssupStoreError {
                    line: line_no,
                    reason: "iterations must be at least 1".to_string(),
                });
            }
            let hash_bytes = from_hex(hash_hex).ok_or_else(|| GssupStoreError {
                line: line_no,
                reason: "hash is not valid hex".to_string(),
            })?;
            let hash: [u8; 32] = hash_bytes
                .as_slice()
                .try_into()
                .map_err(|_| GssupStoreError {
                    line: line_no,
                    reason: alloc::format!(
                        "hash must be 32 bytes (64 hex chars), got {}",
                        hash_bytes.len()
                    ),
                })?;
            if users
                .insert(
                    username.to_string(),
                    StoredCredential {
                        salt,
                        iterations,
                        hash,
                    },
                )
                .is_some()
            {
                return Err(GssupStoreError {
                    line: line_no,
                    reason: alloc::format!("duplicate username: {username}"),
                });
            }
        }
        Ok(Self { users })
    }

    /// `true` if `username` exists and `password` hashes to its stored
    /// value — constant-time in the hash comparison (not in the
    /// username lookup, which a `BTreeMap` never was, but the failure
    /// this guards is a timing side channel on the *password*, and an
    /// unknown username already takes a different, faster path than a
    /// known one with a wrong password would, which is the same
    /// asymmetry every keyed-lookup password check has).
    #[must_use]
    pub fn verify(&self, username: &str, password: &str) -> bool {
        let Some(cred) = self.users.get(username) else {
            return false;
        };
        let computed = hash_password(password.as_bytes(), &cred.salt, cred.iterations);
        constant_time_eq(&computed, &cred.hash)
    }

    /// Number of users in the store.
    #[must_use]
    pub fn len(&self) -> usize {
        self.users.len()
    }

    /// `true` if the store has no users.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.users.is_empty()
    }
}

fn constant_time_eq(a: &[u8; 32], b: &[u8; 32]) -> bool {
    let mut diff = 0u8;
    for i in 0..32 {
        diff |= a[i] ^ b[i];
    }
    diff == 0
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn a_correct_password_verifies() {
        let salt = b"somesalt";
        let line = format_entry("alice", "secret", salt, 1000);
        let store = GssupUserStore::parse(&line).unwrap();
        assert!(store.verify("alice", "secret"));
    }

    #[test]
    fn a_wrong_password_is_refused() {
        let salt = b"somesalt";
        let line = format_entry("alice", "secret", salt, 1000);
        let store = GssupUserStore::parse(&line).unwrap();
        assert!(!store.verify("alice", "wrong"));
    }

    #[test]
    fn an_unknown_username_is_refused() {
        let store = GssupUserStore::parse(&format_entry("alice", "secret", b"s", 1000)).unwrap();
        assert!(!store.verify("bob", "secret"));
    }

    #[test]
    fn comments_and_blank_lines_are_skipped() {
        let text = alloc::format!(
            "# a comment\n\n{}\n",
            format_entry("alice", "secret", b"s", 1000)
        );
        let store = GssupUserStore::parse(&text).unwrap();
        assert_eq!(store.len(), 1);
        assert!(store.verify("alice", "secret"));
    }

    #[test]
    fn several_users_all_verify_independently() {
        let text = alloc::format!(
            "{}\n{}\n",
            format_entry("alice", "one", b"salt-a", 1000),
            format_entry("bob", "two", b"salt-b", 1000)
        );
        let store = GssupUserStore::parse(&text).unwrap();
        assert!(store.verify("alice", "one"));
        assert!(store.verify("bob", "two"));
        assert!(!store.verify("alice", "two"));
        assert!(!store.verify("bob", "one"));
    }

    #[test]
    fn a_line_with_the_wrong_field_count_is_refused() {
        let err = GssupUserStore::parse("alice:onlytwo\n").unwrap_err();
        assert_eq!(err.line, 1);
    }

    #[test]
    fn non_hex_salt_is_refused() {
        let err = GssupUserStore::parse("alice:zzzz:1000:00\n").unwrap_err();
        assert!(err.reason.contains("salt"));
    }

    #[test]
    fn a_short_hash_is_refused() {
        let err = GssupUserStore::parse("alice:aa:1000:aa\n").unwrap_err();
        assert!(err.reason.contains("32 bytes"));
    }

    #[test]
    fn zero_iterations_is_refused() {
        let err = GssupUserStore::parse("alice:aa:0:00\n").unwrap_err();
        assert!(err.reason.contains("iterations"));
    }

    #[test]
    fn a_duplicate_username_is_refused() {
        let text = alloc::format!(
            "{}\n{}\n",
            format_entry("alice", "one", b"s1", 1000),
            format_entry("alice", "two", b"s2", 1000)
        );
        let err = GssupUserStore::parse(&text).unwrap_err();
        assert!(err.reason.contains("duplicate"));
    }

    #[test]
    fn hash_password_is_deterministic_for_the_same_inputs() {
        let a = hash_password(b"pw", b"salt", 500);
        let b = hash_password(b"pw", b"salt", 500);
        assert_eq!(a, b);
        let c = hash_password(b"pw", b"other-salt", 500);
        assert_ne!(a, c);
    }

    #[test]
    fn empty_store_is_empty() {
        let store = GssupUserStore::parse("").unwrap();
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);
    }
}

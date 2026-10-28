// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! SASL on the server side of the endpoint (DDS-AMQP 1.0 §2.1 Cl. 6/7,
//! §10.2): which mechanisms a connection is offered, and the check of what a
//! client sends.
//!
//! * `PLAIN` (RFC 4616) only on a TLS connection (§10.2.1): the password
//!   travels in the clear otherwise. A `PLAIN` init on plaintext gets outcome
//!   `auth` and the connection is closed (Cl. 7).
//! * `SCRAM-SHA-256` (RFC 7677): the password never travels.
//! * `EXTERNAL` (RFC 4422 §7): the identity of the client certificate of a
//!   TLS connection; not offered without one.
//! * `ANONYMOUS` (RFC 4505): only when the configuration names it, or when
//!   there is no credential store to check anything against.
//!
//! The credential store holds no passwords: one line per user,
//! `<user> SCRAM-SHA-256$<iterations>:<salt>$<StoredKey>:<ServerKey>` (the
//! RFC 5803 form, base64), from which both `PLAIN` and `SCRAM-SHA-256` are
//! checked. `amqp-dds-endpoint --hash-password <user>` writes such a line.

use std::collections::BTreeMap;
use std::string::{String, ToString};
use std::vec::Vec;

use zerodds_amqp_endpoint::annex_a::SaslConfig;
use zerodds_amqp_endpoint::annex_a::SaslMechanism as ConfigMechanism;
use zerodds_amqp_endpoint::sasl::SaslMechanism;
use zerodds_amqp_endpoint::scram::{ScramServerSecrets, b64, b64_decode};

/// PBKDF2 iterations of a line `--hash-password` writes (RFC 7677 §4 sets
/// 4096 as the minimum).
pub const DEFAULT_ITERATIONS: u32 = 4096;

/// Users and their SCRAM secrets.
#[derive(Debug, Clone, Default)]
pub struct CredentialStore {
    users: BTreeMap<String, ScramServerSecrets>,
}

impl CredentialStore {
    /// Reads the store a `credential-store-uri` names: `file:///path`,
    /// `file:path` or a plain path.
    ///
    /// # Errors
    /// Unreadable file, or a line not of the form above (named by number).
    pub fn load(uri: &str) -> Result<Self, String> {
        let path = uri
            .strip_prefix("file://")
            .or_else(|| uri.strip_prefix("file:"))
            .unwrap_or(uri);
        let text =
            std::fs::read_to_string(path).map_err(|e| format!("credential store {path}: {e}"))?;
        Self::parse(&text).map_err(|e| format!("credential store {path}: {e}"))
    }

    /// Parses the store's text; `#` starts a comment line.
    ///
    /// # Errors
    /// A malformed line, named by number.
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut users = BTreeMap::new();
        for (n, line) in text.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let bad = |why: &str| format!("line {}: {why}", n + 1);
            let (user, secret) = line
                .split_once(char::is_whitespace)
                .ok_or_else(|| bad("expected `<user> SCRAM-SHA-256$...`"))?;
            let secret = secret.trim();
            let rest = secret
                .strip_prefix("SCRAM-SHA-256$")
                .ok_or_else(|| bad("the secret must start with SCRAM-SHA-256$"))?;
            let (params, keys) = rest
                .split_once('$')
                .ok_or_else(|| bad("expected <iterations>:<salt>$<StoredKey>:<ServerKey>"))?;
            let (iterations, salt) = params
                .split_once(':')
                .ok_or_else(|| bad("expected <iterations>:<salt>"))?;
            let (stored, server) = keys
                .split_once(':')
                .ok_or_else(|| bad("expected <StoredKey>:<ServerKey>"))?;
            let iterations: u32 = iterations
                .parse()
                .map_err(|_| bad("iterations is not a number"))?;
            let salt = b64_decode(salt).ok_or_else(|| bad("salt is not base64"))?;
            let key = |s: &str, what: &str| -> Result<[u8; 32], String> {
                let v = b64_decode(s).ok_or_else(|| bad(&format!("{what} is not base64")))?;
                <[u8; 32]>::try_from(v.as_slice())
                    .map_err(|_| bad(&format!("{what} is not 32 bytes")))
            };
            let secrets = ScramServerSecrets {
                salt,
                iterations,
                stored_key: key(stored, "StoredKey")?,
                server_key: key(server, "ServerKey")?,
            };
            if users.insert(user.to_string(), secrets).is_some() {
                return Err(bad(&format!("user {user} appears twice")));
            }
        }
        Ok(Self { users })
    }

    /// A store line for `user` with `password`.
    #[must_use]
    pub fn entry_line(user: &str, password: &str, salt: &[u8], iterations: u32) -> String {
        let s = ScramServerSecrets::from_password(password, salt, iterations);
        format!(
            "{user} SCRAM-SHA-256${}:{}${}:{}",
            s.iterations,
            b64(&s.salt),
            b64(&s.stored_key),
            b64(&s.server_key)
        )
    }

    /// The SCRAM secrets of `user`.
    #[must_use]
    pub fn secrets(&self, user: &str) -> Option<ScramServerSecrets> {
        self.users.get(user).cloned()
    }

    /// Whether `password` is `user`'s: the password's StoredKey under the
    /// user's salt and iterations against the stored one.
    #[must_use]
    pub fn verify_plain(&self, user: &str, password: &str) -> bool {
        let Some(stored) = self.users.get(user) else {
            return false;
        };
        let derived = ScramServerSecrets::from_password(password, &stored.salt, stored.iterations);
        derived
            .stored_key
            .iter()
            .zip(stored.stored_key.iter())
            .fold(0u8, |acc, (a, b)| acc | (a ^ b))
            == 0
    }

    /// Number of users.
    #[must_use]
    pub fn len(&self) -> usize {
        self.users.len()
    }

    /// Whether the store has no user.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.users.is_empty()
    }
}

/// What the endpoint accepts, from the configuration.
#[derive(Debug, Clone)]
pub struct SaslPolicy {
    /// Mechanisms the configuration enables, in offer order.
    pub enabled: Vec<SaslMechanism>,
    /// The users to check `PLAIN` and `SCRAM-SHA-256` against.
    pub store: Option<CredentialStore>,
}

impl Default for SaslPolicy {
    /// No configuration: `ANONYMOUS`, and `EXTERNAL` for a client with a
    /// certificate.
    fn default() -> Self {
        Self {
            enabled: std::vec![SaslMechanism::External, SaslMechanism::Anonymous],
            store: None,
        }
    }
}

impl SaslPolicy {
    /// The policy of an `<sasl>` block: its mechanisms, its store. An empty
    /// mechanism list is the default set, with `SCRAM-SHA-256` and `PLAIN`
    /// when a store is named (and no `ANONYMOUS` then).
    ///
    /// # Errors
    /// A mechanism that cannot work as configured — `PLAIN` without TLS or
    /// without a store, `SCRAM-SHA-256` without a store, `EXTERNAL` without
    /// a client certificate requirement — or an unreadable store. Refused at
    /// start rather than offered and failing on every connection.
    pub fn from_config(cfg: &SaslConfig, tls: bool, client_certs: bool) -> Result<Self, String> {
        let store = if cfg.credential_store_uri.is_empty() {
            None
        } else {
            Some(CredentialStore::load(&cfg.credential_store_uri)?)
        };
        let enabled: Vec<SaslMechanism> = if cfg.enabled_mechanisms.is_empty() {
            let mut v = Vec::new();
            if client_certs {
                v.push(SaslMechanism::External);
            }
            if store.is_some() {
                v.push(SaslMechanism::ScramSha256);
                if tls {
                    v.push(SaslMechanism::Plain);
                }
            } else {
                v.push(SaslMechanism::Anonymous);
            }
            v
        } else {
            cfg.enabled_mechanisms
                .iter()
                .map(|m| match m {
                    ConfigMechanism::SaslPlain => SaslMechanism::Plain,
                    ConfigMechanism::SaslAnonymous => SaslMechanism::Anonymous,
                    ConfigMechanism::SaslExternal => SaslMechanism::External,
                    ConfigMechanism::SaslScramSha256 => SaslMechanism::ScramSha256,
                })
                .collect()
        };
        for m in &enabled {
            let why = match m {
                SaslMechanism::Plain if !tls => Some(
                    "PLAIN needs TLS (DDS-AMQP 1.0 §10.2.1): the password would travel in the clear",
                ),
                SaslMechanism::Plain | SaslMechanism::ScramSha256 if store.is_none() => {
                    Some("needs a credential-store-uri to check the password against")
                }
                SaslMechanism::External if !client_certs => Some(
                    "EXTERNAL needs TLS with require-client-cert: it is the certificate's identity",
                ),
                _ => None,
            };
            if let Some(why) = why {
                return Err(format!("sasl mechanism {}: {why}", m.name()));
            }
        }
        if enabled.contains(&SaslMechanism::ScramSha256) && server_nonce().is_none() {
            return Err(
                "sasl mechanism SCRAM-SHA-256: needs the tls feature (its random source)"
                    .to_string(),
            );
        }
        if enabled.is_empty() {
            return Err("sasl: no mechanism enabled".to_string());
        }
        Ok(Self { enabled, store })
    }

    /// The mechanisms offered on one connection: `EXTERNAL` only when the
    /// client presented a certificate, `PLAIN` only on TLS.
    #[must_use]
    pub fn offered(&self, tls: bool, has_client_cert: bool) -> Vec<SaslMechanism> {
        self.enabled
            .iter()
            .copied()
            .filter(|m| match m {
                SaslMechanism::Plain => tls,
                SaslMechanism::External => has_client_cert,
                _ => true,
            })
            .collect()
    }

    /// Whether a client may skip SASL: only when `ANONYMOUS` is enabled,
    /// which is what skipping it amounts to.
    #[must_use]
    pub fn allows_no_sasl(&self) -> bool {
        self.enabled.contains(&SaslMechanism::Anonymous)
    }
}

/// Lowercase hex SHA-256 of `data`: the `EXTERNAL` subject
/// `mtls:<hash of the certificate DER>`, as the other bridges name a
/// certificate (no X.500 DN parser in the tree).
#[must_use]
pub fn sha256_hex(data: &[u8]) -> String {
    zerodds_amqp_endpoint::scram::sha256(data)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// A SCRAM server nonce from the TLS provider's CSPRNG; `None` without the
/// `tls` feature, or when the source fails.
#[must_use]
pub fn server_nonce() -> Option<String> {
    #[cfg(feature = "tls")]
    {
        let mut bytes = [0u8; 18];
        zerodds_bridge_security::tls_provider()
            .secure_random
            .fill(&mut bytes)
            .ok()?;
        Some(b64(&bytes))
    }
    #[cfg(not(feature = "tls"))]
    {
        None
    }
}

/// Splits a `PLAIN` initial response, `[authzid] \0 authcid \0 passwd`
/// (RFC 4616 §2). An authzid other than empty or the authcid is refused:
/// the endpoint has no rule for acting as another user.
#[must_use]
pub fn parse_plain(response: &[u8]) -> Option<(String, String)> {
    let mut parts = response.split(|b| *b == 0);
    let authzid = parts.next()?;
    let authcid = parts.next()?;
    let passwd = parts.next()?;
    if parts.next().is_some() || authcid.is_empty() {
        return None;
    }
    if !authzid.is_empty() && authzid != authcid {
        return None;
    }
    Some((
        String::from_utf8(authcid.to_vec()).ok()?,
        String::from_utf8(passwd.to_vec()).ok()?,
    ))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn store() -> CredentialStore {
        CredentialStore::parse(&CredentialStore::entry_line(
            "alice",
            "wonder",
            b"salt-1234",
            4096,
        ))
        .unwrap()
    }

    #[test]
    fn a_store_line_checks_the_password_it_was_made_from() {
        let s = store();
        assert_eq!(s.len(), 1);
        assert!(s.verify_plain("alice", "wonder"));
        assert!(!s.verify_plain("alice", "wrong"));
        assert!(!s.verify_plain("bob", "wonder"));
        assert!(s.secrets("alice").is_some());
    }

    /// RFC 7677 §3's example user: salt `W22ZaJ0SNY7soEsUEjb6gQ==`, 4096
    /// iterations, password `pencil`; its StoredKey and ServerKey as the RFC's
    /// exchange implies them (the server signature of that exchange checks
    /// out in `zerodds-amqp-endpoint`'s SCRAM tests).
    #[test]
    fn the_line_is_the_rfc_5803_form() {
        let salt = b64_decode("W22ZaJ0SNY7soEsUEjb6gQ==").unwrap();
        let line = CredentialStore::entry_line("user", "pencil", &salt, 4096);
        assert!(line.starts_with("user SCRAM-SHA-256$4096:W22ZaJ0SNY7soEsUEjb6gQ==$"));
        let s = CredentialStore::parse(&line).unwrap();
        assert!(s.verify_plain("user", "pencil"));
    }

    #[test]
    fn malformed_lines_are_named_by_number() {
        for (text, needle) in [
            ("# c\nalice plain:wonder\n", "line 2"),
            ("alice\n", "line 1"),
            ("alice SCRAM-SHA-256$x:AA==$AA==:AA==\n", "iterations"),
            ("alice SCRAM-SHA-256$4096:AA==$AA==:AA==\n", "32 bytes"),
        ] {
            let err = CredentialStore::parse(text).unwrap_err();
            assert!(err.contains(needle), "{text}: {err}");
        }
        let line = CredentialStore::entry_line("a", "p", b"s", 4096);
        let twice = format!("{line}\n{line}\n");
        assert!(
            CredentialStore::parse(&twice)
                .unwrap_err()
                .contains("twice")
        );
    }

    #[test]
    fn plain_responses_split_as_rfc_4616_says() {
        assert_eq!(
            parse_plain(b"\0alice\0wonder"),
            Some(("alice".into(), "wonder".into()))
        );
        assert_eq!(
            parse_plain(b"alice\0alice\0wonder"),
            Some(("alice".into(), "wonder".into()))
        );
        assert_eq!(
            parse_plain(b"bob\0alice\0wonder"),
            None,
            "acting as another user"
        );
        assert_eq!(parse_plain(b"\0\0wonder"), None);
        assert_eq!(parse_plain(b"alice"), None);
    }

    #[test]
    fn the_policy_refuses_what_cannot_work_and_offers_per_connection() {
        let dir = std::env::temp_dir().join(format!("amqp-auth-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("users");
        std::fs::write(&path, CredentialStore::entry_line("a", "p", b"s", 4096)).unwrap();
        let with = |mechs: Vec<ConfigMechanism>, uri: &str| SaslConfig {
            enabled_mechanisms: mechs,
            credential_store_uri: uri.to_string(),
        };
        let uri = format!("file://{}", path.display());
        let p = SaslPolicy::from_config(&with(Vec::new(), &uri), true, false).unwrap();
        assert_eq!(
            p.enabled,
            [SaslMechanism::ScramSha256, SaslMechanism::Plain],
            "a store: no ANONYMOUS"
        );
        assert!(!p.allows_no_sasl());
        assert_eq!(p.offered(false, false), [SaslMechanism::ScramSha256]);
        let e = SaslPolicy::from_config(
            &with(std::vec![ConfigMechanism::SaslPlain], &uri),
            false,
            false,
        )
        .unwrap_err();
        assert!(e.contains("PLAIN needs TLS"), "{e}");
        let e = SaslPolicy::from_config(
            &with(std::vec![ConfigMechanism::SaslScramSha256], ""),
            true,
            false,
        )
        .unwrap_err();
        assert!(e.contains("credential-store-uri"), "{e}");
        let e = SaslPolicy::from_config(
            &with(std::vec![ConfigMechanism::SaslExternal], ""),
            true,
            false,
        )
        .unwrap_err();
        assert!(e.contains("require-client-cert"), "{e}");
        let none = SaslPolicy::from_config(&with(Vec::new(), ""), false, false).unwrap();
        assert_eq!(none.enabled, [SaslMechanism::Anonymous]);
        assert!(none.allows_no_sasl());
        let _ = std::fs::remove_dir_all(&dir);
    }
}

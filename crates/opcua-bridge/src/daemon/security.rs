// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! SecureChannel key/certificate loading and the DDS-topic ACL.
//!
//! **Why not `zerodds-bridge-security`'s TLS loader:** it returns a
//! `rustls::ServerConfig` (the shape every other bridge's TCP/WebSocket
//! transport needs); this daemon's SecureChannel crypto
//! (`zerodds_opcua_uacp::crypto`) needs a bare RSA keypair
//! (`rsa::RsaPrivateKey`/`RsaPublicKey`), a different shape entirely. Its
//! [`crate::daemon::config::ServerConfig`] therefore asks for the peer's
//! certificate (DER, sent on the wire as-is, opaque to this daemon) and its
//! RSA public key (PEM, `SubjectPublicKeyInfo`) as two separate files
//! rather than one certificate this daemon would need an X.509 parser to
//! split — a real operational rough edge, named in
//! `docs/specs/zerodds-opcua-bridge-1.0.md` §7, not hidden.
//!
//! **Where `zerodds-bridge-security` *does* apply**: [`Acl`] mirrors the
//! subject-keyed allow-list every other bridge's `acl.rules` builds
//! (`zerodds_bridge_security::acl::{Acl as BridgeAcl, AclEntry, AclOp}`),
//! adapted to a per-DDS-topic read/write check keyed by the OPC-UA
//! `UserIdentityToken`'s subject rather than a bearer/mTLS one.

use std::path::Path;

#[cfg(feature = "crypto")]
use rsa::pkcs8::{DecodePrivateKey, DecodePublicKey};
#[cfg(feature = "crypto")]
use zerodds_opcua_uacp::crypto::{RsaPrivateKey, RsaPublicKey};

/// A key/certificate file could not be loaded.
#[derive(Debug)]
pub enum SecurityError {
    /// The file could not be read.
    Io {
        /// The path.
        path: String,
        /// The OS error, rendered.
        message: String,
    },
    /// The PEM/DER content did not parse as the expected key type.
    Parse {
        /// The path.
        path: String,
        /// The underlying error, rendered.
        message: String,
    },
}

impl core::fmt::Display for SecurityError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Io { path, message } => write!(f, "{path}: {message}"),
            Self::Parse { path, message } => write!(f, "{path}: {message}"),
        }
    }
}

impl std::error::Error for SecurityError {}

#[cfg(feature = "crypto")]
fn read(path: &Path) -> Result<String, SecurityError> {
    std::fs::read_to_string(path).map_err(|e| SecurityError::Io {
        path: path.display().to_string(),
        message: e.to_string(),
    })
}

/// Reads a PKCS#8 PEM RSA private key.
///
/// # Errors
/// [`SecurityError`].
#[cfg(feature = "crypto")]
pub fn load_private_key_pem(path: &Path) -> Result<RsaPrivateKey, SecurityError> {
    let pem = read(path)?;
    RsaPrivateKey::from_pkcs8_pem(&pem).map_err(|e| SecurityError::Parse {
        path: path.display().to_string(),
        message: e.to_string(),
    })
}

/// Reads a `SubjectPublicKeyInfo` PEM RSA public key.
///
/// # Errors
/// [`SecurityError`].
#[cfg(feature = "crypto")]
pub fn load_public_key_pem(path: &Path) -> Result<RsaPublicKey, SecurityError> {
    let pem = read(path)?;
    RsaPublicKey::from_public_key_pem(&pem).map_err(|e| SecurityError::Parse {
        path: path.display().to_string(),
        message: e.to_string(),
    })
}

/// Reads a DER certificate as raw bytes — sent on the wire as-is, never
/// parsed by this daemon (see the module doc).
///
/// # Errors
/// [`SecurityError::Io`].
pub fn load_der(path: &Path) -> Result<Vec<u8>, SecurityError> {
    std::fs::read(path).map_err(|e| SecurityError::Io {
        path: path.display().to_string(),
        message: e.to_string(),
    })
}

/// A per-DDS-topic read/write ACL, keyed by the OPC-UA client's identity
/// subject.
#[derive(Debug, Clone, Default)]
pub struct Acl {
    default_deny: bool,
    rules: Vec<(String, Vec<String>, Vec<String>)>,
}

impl Acl {
    /// Builds an ACL from the config's `(subject, allow_read, allow_write)`
    /// rules.
    #[must_use]
    pub fn new(default_deny: bool, rules: Vec<(String, Vec<String>, Vec<String>)>) -> Self {
        Self {
            default_deny,
            rules,
        }
    }

    /// An ACL with no rules — every topic open to every subject (the
    /// default a daemon with no `acl:` section keeps, same as every other
    /// bridge's `zerodds_bridge_security::build_ctx`).
    #[must_use]
    pub fn open() -> Self {
        Self::default()
    }

    /// Whether `subject` may read `topic`.
    #[must_use]
    pub fn allow_read(&self, subject: &str, topic: &str) -> bool {
        self.check(subject, topic, |allow_read, _| allow_read)
    }

    /// Whether `subject` may write `topic`.
    #[must_use]
    pub fn allow_write(&self, subject: &str, topic: &str) -> bool {
        self.check(subject, topic, |_, allow_write| allow_write)
    }

    fn check(
        &self,
        subject: &str,
        topic: &str,
        side: for<'a> fn(&'a [String], &'a [String]) -> &'a [String],
    ) -> bool {
        if self.rules.is_empty() {
            return !self.default_deny;
        }
        for (rule_subject, allow_read, allow_write) in &self.rules {
            if rule_subject == subject && side(allow_read, allow_write).iter().any(|t| t == topic) {
                return true;
            }
        }
        !self.default_deny
    }

    /// Whether `topic` is exposed at all for `op`, ignoring the subject
    /// dimension — the check the config-load-time gates
    /// (`crate::daemon::server`'s topic materialization/registration,
    /// `crate::pubsub_glue`'s publisher setup, `crate::ua_client`'s DDS
    /// writer registration) apply, since none of them run inside a
    /// per-OPC-UA-session context that could name a subject. Per-session
    /// Write enforcement (`allow_write`, called from `Server`'s Write
    /// hook — `crate::daemon::server::start_with_reload` installs it) is
    /// the finer-grained check on top of this coarser one, not a
    /// replacement for it: a topic no rule names for `op` at all never
    /// gets registered in the first place, whichever subject later asks.
    #[must_use]
    pub fn topic_exposed(&self, topic: &str, op: AclOp) -> bool {
        if self.rules.is_empty() {
            return !self.default_deny;
        }
        for (_, allow_read, allow_write) in &self.rules {
            let side = match op {
                AclOp::Read => allow_read,
                AclOp::Write => allow_write,
            };
            if side.iter().any(|t| t == topic) {
                return true;
            }
        }
        !self.default_deny
    }
}

/// Renders an OPC-UA session's identity as the `subject` string
/// `acl.rules` names — the same string an operator writes in
/// `acl.rules[].subject` for that user. `Anonymous` is the literal
/// string `"anonymous"` (matchable by an explicit rule, if an operator
/// wants to name it); a certificate/issued-token subject is its raw
/// bytes hex-encoded — opaque, but stable and comparable, with no crypto
/// dependency pulled in only to compute a thumbprint for logging/ACL
/// purposes.
#[must_use]
pub fn client_subject(user: &zerodds_opcua_server::identity::ClientUserId) -> String {
    use zerodds_opcua_server::identity::ClientUserId;
    match user {
        ClientUserId::Anonymous => "anonymous".to_string(),
        ClientUserId::UserName(name) => name.clone(),
        ClientUserId::Certificate(der) => hex_encode(der),
        ClientUserId::IssuedToken(bytes) => hex_encode(bytes),
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| std::format!("{b:02x}")).collect()
}

/// Which direction [`Acl::topic_exposed`] is asked about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AclOp {
    /// DDS → OPC-UA (exposing a topic's samples).
    Read,
    /// OPC-UA → DDS (a topic a write may reach).
    Write,
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn empty_acl_is_open() {
        let acl = Acl::open();
        assert!(acl.allow_read("anyone", "AnyTopic"));
        assert!(acl.allow_write("anyone", "AnyTopic"));
    }

    #[test]
    fn matching_rule_allows_named_topic_only() {
        let acl = Acl::new(
            true,
            vec![(
                "op1".to_string(),
                vec!["Telemetry".to_string()],
                vec!["Commands".to_string()],
            )],
        );
        assert!(acl.allow_read("op1", "Telemetry"));
        assert!(!acl.allow_read("op1", "Commands"));
        assert!(acl.allow_write("op1", "Commands"));
        assert!(!acl.allow_write("op1", "Telemetry"));
    }

    #[test]
    fn default_deny_with_rules_blocks_unnamed_subject() {
        let acl = Acl::new(
            true,
            vec![("op1".to_string(), vec!["Telemetry".to_string()], vec![])],
        );
        assert!(!acl.allow_read("someone-else", "Telemetry"));
    }

    #[test]
    fn default_deny_false_with_rules_still_opens_unmatched_topics() {
        let acl = Acl::new(
            false,
            vec![("op1".to_string(), vec!["Telemetry".to_string()], vec![])],
        );
        assert!(acl.allow_read("op1", "SomeOtherTopic"));
    }

    #[test]
    fn client_subject_renders_each_identity_kind() {
        use zerodds_opcua_server::identity::ClientUserId;
        assert_eq!(client_subject(&ClientUserId::Anonymous), "anonymous");
        assert_eq!(
            client_subject(&ClientUserId::UserName("alice".to_string())),
            "alice"
        );
        assert_eq!(
            client_subject(&ClientUserId::Certificate(vec![0xde, 0xad])),
            "dead"
        );
    }

    #[test]
    fn topic_exposed_ignores_the_subject_dimension() {
        let acl = Acl::new(
            true,
            vec![("op1".to_string(), vec!["Telemetry".to_string()], vec![])],
        );
        // Named by some rule's subject — exposed regardless of who asks.
        assert!(acl.topic_exposed("Telemetry", AclOp::Read));
        assert!(!acl.topic_exposed("Telemetry", AclOp::Write));
        assert!(!acl.topic_exposed("Unnamed", AclOp::Read));
    }
}
